use crate::{CommunicationError, ConnectionHandle};
use epsilon_core::CommunicationValue;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio::time::{Duration, sleep};
use wtransport::Connection;

pub const MAX_MESSAGE_SIZE: u64 = 1_000_000_000;
const CHANNEL_SIZE: usize = 100;
const CLOSE_FRAME_LEN: u32 = u32::MAX;
const APPLICATION_CLOSE_CODE: u32 = 0;
const APPLICATION_CLOSE_REASON: &str = "epsilon-close";
const MAX_SEND_RETRIES: usize = 3;
const SEND_RETRY_BASE_DELAY_MS: u64 = 20;

enum ReceivedFrame {
    Message(CommunicationValue),
    ClosedByPeer,
}

pub struct Sender {
    tx: mpsc::Sender<CommunicationValue>,
    _task: tokio::task::JoinHandle<()>,
    handle: Arc<ConnectionHandle>,
    connection: Connection,
}

impl Sender {
    pub fn new(connection: Connection, handle: Arc<ConnectionHandle>) -> Self {
        let (tx, mut rx) = mpsc::channel::<CommunicationValue>(CHANNEL_SIZE);
        let conn_handle = handle.clone();
        let task_connection = connection.clone();

        let task = tokio::spawn(async move {
            let mut close_rx = conn_handle.subscribe_close();

            loop {
                tokio::select! {
                    msg = rx.recv() => {
                        match msg {
                            Some(msg) => {
                                if task_connection.quic_connection().close_reason().is_some() {
                                    println!("[connection::Sender] connection already closed before send");
                                    conn_handle.close(Some(CommunicationError::StreamClosed));
                                    break;
                                }

                                if let Err(e) = Self::send_internal(&task_connection, &msg).await {
                                    println!("[connection::Sender] send failed permanently: {:?}", e);
                                    conn_handle.close(Some(e));
                                    break;
                                }
                            }
                            None => {
                                conn_handle.close(None);
                                break;
                            }
                        }
                    }
                    _ = close_rx.changed() => {
                        if close_rx.borrow().is_some() {
                            break;
                        }
                    }
                }
            }
        });

        Self {
            tx,
            _task: task,
            handle,
            connection,
        }
    }

    async fn send_internal(
        conn: &Connection,
        data: &CommunicationValue,
    ) -> Result<(), CommunicationError> {
        let bytes = data.to_bytes();
        if bytes.len() as u64 > MAX_MESSAGE_SIZE || bytes.len() as u64 >= CLOSE_FRAME_LEN as u64 {
            return Err(CommunicationError::MessageTooLarge);
        }

        let mut attempt: usize = 0;
        loop {
            attempt += 1;
            println!(
                "[Sender] send attempt {attempt}/{MAX_SEND_RETRIES}, payload_bytes={}",
                bytes.len()
            );

            let opening = match conn.open_uni().await {
                Ok(opening) => opening,
                Err(e) => {
                    println!("[Sender] open_uni failed on attempt {attempt}: {e}");
                    if attempt >= MAX_SEND_RETRIES {
                        return Err(CommunicationError::ConnectionError(e));
                    }
                    let delay_ms = SEND_RETRY_BASE_DELAY_MS * (1u64 << (attempt - 1));
                    sleep(Duration::from_millis(delay_ms)).await;
                    continue;
                }
            };

            let mut stream = match opening.await {
                Ok(stream) => stream,
                Err(_) => {
                    println!("[Sender] stream open failed on attempt {attempt}");
                    if attempt >= MAX_SEND_RETRIES {
                        return Err(CommunicationError::StreamError);
                    }
                    let delay_ms = SEND_RETRY_BASE_DELAY_MS * (1u64 << (attempt - 1));
                    sleep(Duration::from_millis(delay_ms)).await;
                    continue;
                }
            };

            use tokio::io::AsyncWriteExt;
            if let Err(_) = stream.write_u32(bytes.len() as u32).await {
                println!("[Sender] write length failed on attempt {attempt}");
                if attempt >= MAX_SEND_RETRIES {
                    return Err(CommunicationError::StreamError);
                }
                let delay_ms = SEND_RETRY_BASE_DELAY_MS * (1u64 << (attempt - 1));
                sleep(Duration::from_millis(delay_ms)).await;
                continue;
            }

            if let Err(e) = stream.write_all(&bytes).await {
                println!("[Sender] write payload failed on attempt {attempt}: {e}");
                if attempt >= MAX_SEND_RETRIES {
                    return Err(e.into());
                }
                let delay_ms = SEND_RETRY_BASE_DELAY_MS * (1u64 << (attempt - 1));
                sleep(Duration::from_millis(delay_ms)).await;
                continue;
            }

            tokio::spawn(async move {
                if let Err(e) = stream.finish().await {
                    println!("[Sender] stream finish failed: {e}");
                }
            });
            return Ok(());
        }
    }

    async fn send_close_frame(conn: &Connection) -> Result<(), CommunicationError> {
        let opening = conn
            .open_uni()
            .await
            .map_err(|e| CommunicationError::ConnectionError(e))?;
        let mut stream = opening.await.map_err(|_| CommunicationError::StreamError)?;

        use tokio::io::AsyncWriteExt;
        stream
            .write_u32(CLOSE_FRAME_LEN)
            .await
            .map_err(|_| CommunicationError::StreamError)?;

        tokio::spawn(async move {
            let _ = stream.finish().await;
        });
        Ok(())
    }

    pub async fn send(&self, data: &CommunicationValue) -> Result<(), CommunicationError> {
        if self.handle.is_closed() {
            return Err(self
                .handle
                .close_reason()
                .unwrap_or(CommunicationError::UseAfterClosed));
        }

        self.tx.send(data.clone()).await.map_err(|_| {
            self.handle
                .close_reason()
                .unwrap_or(CommunicationError::StreamClosed)
        })
    }

    pub fn handle(&self) -> &Arc<ConnectionHandle> {
        &self.handle
    }

    pub fn close(&self) {
        let connection = self.connection.clone();
        let handle = self.handle.clone();

        tokio::spawn(async move {
            let _ = Self::send_close_frame(&connection).await;
            connection.quic_connection().close(
                APPLICATION_CLOSE_CODE.into(),
                APPLICATION_CLOSE_REASON.as_bytes(),
            );
            handle.close(Some(CommunicationError::StreamClosed));
        });
    }
    pub fn is_open(&self) -> bool {
        self.handle.is_open()
    }
    pub fn is_closed(&self) -> bool {
        self.handle.is_closed()
    }

    pub fn close_reason(&self) -> Option<CommunicationError> {
        self.handle.close_reason()
    }
}

pub struct Receiver {
    rx: Mutex<mpsc::Receiver<Result<CommunicationValue, CommunicationError>>>,
    _task: tokio::task::JoinHandle<()>,
    handle: Arc<ConnectionHandle>,
}

impl Receiver {
    pub fn new(connection: Connection, handle: Arc<ConnectionHandle>) -> Self {
        let (tx, rx) =
            mpsc::channel::<Result<CommunicationValue, CommunicationError>>(CHANNEL_SIZE);
        let conn_handle = handle.clone();

        let task = tokio::spawn(async move {
            let mut close_rx = conn_handle.subscribe_close();

            loop {
                tokio::select! {
                    result = Self::receive_internal(&connection) => {
                        match result {
                            Ok(ReceivedFrame::Message(msg)) => {
                                if tx.send(Ok(msg)).await.is_err() {
                                    conn_handle.close(None);
                                    break;
                                }
                            }
                            Ok(ReceivedFrame::ClosedByPeer) => {
                                let close_error = CommunicationError::StreamClosed;
                                let _ = tx.send(Err(close_error.clone())).await;
                                conn_handle.close(Some(close_error));
                                break;
                            }
                            Err(e) => {
                                let close_error = match e {
                                    CommunicationError::ConnectionError(_) => CommunicationError::StreamClosed,
                                    CommunicationError::ReadExactError(_) => CommunicationError::StreamClosed,
                                    CommunicationError::ClosedError(_) => CommunicationError::StreamClosed,
                                    other => other,
                                };
                                let _ = tx.send(Err(close_error.clone())).await;
                                conn_handle.close(Some(close_error));
                                break;
                            }
                        }
                    }
                    _ = close_rx.changed() => {
                        if close_rx.borrow().is_some() {
                            break;
                        }
                    }
                }
            }
        });

        Self {
            rx: Mutex::new(rx),
            _task: task,
            handle,
        }
    }

    async fn receive_internal(conn: &Connection) -> Result<ReceivedFrame, CommunicationError> {
        let mut stream = conn
            .accept_uni()
            .await
            .map_err(|e| CommunicationError::ConnectionError(e))?;

        use tokio::io::AsyncReadExt;
        let len = stream
            .read_u32()
            .await
            .map_err(|_| CommunicationError::StreamError)?;

        if len == CLOSE_FRAME_LEN {
            return Ok(ReceivedFrame::ClosedByPeer);
        }

        let len = len as usize;

        if len as u64 > MAX_MESSAGE_SIZE {
            return Err(CommunicationError::MessageTooLarge);
        }

        let mut buf = vec![0u8; len];
        stream.read_exact(&mut buf).await?;

        let message = CommunicationValue::from_bytes(&buf)
            .ok_or(CommunicationError::ParseCommunicationValue)?;

        Ok(ReceivedFrame::Message(message))
    }

    pub async fn receive(&self) -> Result<CommunicationValue, CommunicationError> {
        if self.handle.is_closed() {
            return Err(self
                .handle
                .close_reason()
                .unwrap_or(CommunicationError::StreamClosed));
        }

        let result = self.rx.lock().await.recv().await;

        if let Some(result) = result {
            return result;
        }

        Err(self
            .handle
            .close_reason()
            .unwrap_or(CommunicationError::StreamClosed))
    }

    pub fn handle(&self) -> &Arc<ConnectionHandle> {
        &self.handle
    }

    pub fn close(&self) {
        self.handle.close(None);
    }

    pub fn is_open(&self) -> bool {
        self.handle.is_open()
    }

    pub fn is_closed(&self) -> bool {
        self.handle.is_closed()
    }

    pub fn close_reason(&self) -> Option<CommunicationError> {
        self.handle.close_reason()
    }
}
