use crate::{CommunicationError, ConnectionHandle};
use epsilon_core::CommunicationValue;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use tokio::time::{Duration, sleep, timeout};
use wtransport::Connection;

pub const MAX_MESSAGE_SIZE: u64 = 1_000_000_000;
const CHANNEL_SIZE: usize = 100;
const CLOSE_FRAME_LEN: u32 = u32::MAX;
const APPLICATION_CLOSE_CODE: u32 = 0;
const APPLICATION_CLOSE_REASON: &str = "epsilon-close";

const OPEN_STREAM_TIMEOUT_MS: u64 = 2_000;
const WRITE_TIMEOUT_MS: u64 = 2_000;
const ACCEPT_STREAM_TIMEOUT_MS: u64 = 10_000;
const READ_TIMEOUT_MS: u64 = 5_000;
const FORCE_CLOSE_DELAY_MS: u64 = 300;
const MAX_TRANSIENT_RECV_ERRORS: usize = 5;
const TRANSIENT_RECV_BACKOFF_MS: u64 = 25;

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

            let opening = match timeout(
                Duration::from_millis(OPEN_STREAM_TIMEOUT_MS),
                task_connection.open_uni(),
            )
            .await
            {
                Ok(Ok(opening)) => opening,
                Ok(Err(e)) => {
                    println!("[Sender] open_uni failed during setup: {e}");
                    conn_handle.close(Some(CommunicationError::ConnectionError(e)));
                    return;
                }
                Err(_) => {
                    println!("[Sender] open_uni timed out during setup");
                    conn_handle.close(Some(CommunicationError::StreamError));
                    return;
                }
            };

            let mut stream =
                match timeout(Duration::from_millis(OPEN_STREAM_TIMEOUT_MS), opening).await {
                    Ok(Ok(stream)) => stream,
                    Ok(Err(_)) => {
                        println!("[Sender] uni stream open failed during setup");
                        conn_handle.close(Some(CommunicationError::StreamError));
                        return;
                    }
                    Err(_) => {
                        println!("[Sender] uni stream open timed out during setup");
                        conn_handle.close(Some(CommunicationError::StreamError));
                        return;
                    }
                };

            loop {
                tokio::select! {
                    msg = rx.recv() => {
                        match msg {
                            Some(msg) => {
                                if task_connection.quic_connection().close_reason().is_some() || conn_handle.is_closed() {
                                    conn_handle.close(Some(CommunicationError::StreamClosed));
                                    break;
                                }

                                if let Err(e) = Self::write_frame(&mut stream, &msg).await {
                                    println!("[Sender] write_frame failed permanently: {:?}", e);
                                    conn_handle.close(Some(CommunicationError::StreamClosed));
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

            if let Err(e) = stream.finish().await {
                println!("[Sender] stream finish failed at sender task end: {e}");
            }
        });

        Self {
            tx,
            _task: task,
            handle,
            connection,
        }
    }

    async fn write_frame(
        stream: &mut wtransport::SendStream,
        data: &CommunicationValue,
    ) -> Result<(), CommunicationError> {
        let bytes = data.to_bytes();
        if bytes.len() as u64 > MAX_MESSAGE_SIZE || bytes.len() as u64 >= CLOSE_FRAME_LEN as u64 {
            return Err(CommunicationError::MessageTooLarge);
        }

        use tokio::io::AsyncWriteExt;

        timeout(
            Duration::from_millis(WRITE_TIMEOUT_MS),
            stream.write_u32(bytes.len() as u32),
        )
        .await
        .map_err(|_| CommunicationError::StreamError)?
        .map_err(|_| CommunicationError::StreamError)?;

        timeout(
            Duration::from_millis(WRITE_TIMEOUT_MS),
            stream.write_all(&bytes),
        )
        .await
        .map_err(|_| CommunicationError::StreamError)?
        .map_err(CommunicationError::from)?;

        Ok(())
    }

    async fn send_close_frame(conn: &Connection) -> Result<(), CommunicationError> {
        let opening = timeout(
            Duration::from_millis(OPEN_STREAM_TIMEOUT_MS),
            conn.open_uni(),
        )
        .await
        .map_err(|_| CommunicationError::StreamError)?
        .map_err(CommunicationError::ConnectionError)?;

        let mut stream = timeout(Duration::from_millis(OPEN_STREAM_TIMEOUT_MS), opening)
            .await
            .map_err(|_| CommunicationError::StreamError)?
            .map_err(|_| CommunicationError::StreamError)?;

        use tokio::io::AsyncWriteExt;
        timeout(
            Duration::from_millis(WRITE_TIMEOUT_MS),
            stream.write_u32(CLOSE_FRAME_LEN),
        )
        .await
        .map_err(|_| CommunicationError::StreamError)?
        .map_err(|_| CommunicationError::StreamError)?;

        if let Err(e) = stream.finish().await {
            println!("[Sender] close frame finish failed: {e}");
        }

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
            if let Err(e) = Self::send_close_frame(&connection).await {
                println!("[Sender] failed to send close frame: {:?}", e);
            }

            handle.close(Some(CommunicationError::StreamClosed));

            sleep(Duration::from_millis(FORCE_CLOSE_DELAY_MS)).await;
            if connection.quic_connection().close_reason().is_none() {
                connection.quic_connection().close(
                    APPLICATION_CLOSE_CODE.into(),
                    APPLICATION_CLOSE_REASON.as_bytes(),
                );
            }
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
            let mut transient_errors: usize = 0;

            loop {
                tokio::select! {
                    result = Self::receive_internal(&connection) => {
                        match result {
                            Ok(ReceivedFrame::Message(msg)) => {
                                transient_errors = 0;
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
                                println!("[Receiver] receive_internal failed: {:?}", e);

                                let is_transient = matches!(
                                    e,
                                    CommunicationError::ConnectionError(_)
                                        | CommunicationError::ReadExactError(_)
                                        | CommunicationError::ClosedError(_)
                                        | CommunicationError::StreamReadExactError(_)
                                        | CommunicationError::StreamError
                                );

                                if is_transient && transient_errors < MAX_TRANSIENT_RECV_ERRORS {
                                    transient_errors += 1;
                                    println!(
                                        "[Receiver] transient receive error {}/{}; continuing",
                                        transient_errors,
                                        MAX_TRANSIENT_RECV_ERRORS
                                    );
                                    sleep(Duration::from_millis(TRANSIENT_RECV_BACKOFF_MS)).await;
                                    continue;
                                }

                                let close_error = match e {
                                    CommunicationError::ConnectionError(_) => CommunicationError::StreamClosed,
                                    CommunicationError::ReadExactError(_) => CommunicationError::StreamClosed,
                                    CommunicationError::ClosedError(_) => CommunicationError::StreamClosed,
                                    CommunicationError::StreamReadExactError(_) => CommunicationError::StreamClosed,
                                    CommunicationError::StreamError => CommunicationError::StreamClosed,
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
        let mut stream = match timeout(
            Duration::from_millis(ACCEPT_STREAM_TIMEOUT_MS),
            conn.accept_uni(),
        )
        .await
        {
            Ok(Ok(stream)) => stream,
            Ok(Err(e)) => {
                println!("[Receiver] accept_uni failed: {e}");
                return Err(CommunicationError::ConnectionError(e));
            }
            Err(_) => {
                println!("[Receiver] accept_uni timed out");
                return Err(CommunicationError::StreamError);
            }
        };

        use tokio::io::AsyncReadExt;
        let len = match timeout(Duration::from_millis(READ_TIMEOUT_MS), stream.read_u32()).await {
            Ok(Ok(len)) => len,
            Ok(Err(e)) => {
                println!("[Receiver] read_u32 failed: {e}");
                return Err(CommunicationError::StreamError);
            }
            Err(_) => {
                println!("[Receiver] read_u32 timed out");
                return Err(CommunicationError::StreamError);
            }
        };

        if len == CLOSE_FRAME_LEN {
            return Ok(ReceivedFrame::ClosedByPeer);
        }

        let len = len as usize;

        if len as u64 > MAX_MESSAGE_SIZE {
            return Err(CommunicationError::MessageTooLarge);
        }

        let mut buf = vec![0u8; len];
        match timeout(
            Duration::from_millis(READ_TIMEOUT_MS),
            stream.read_exact(&mut buf),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                println!("[Receiver] read_exact failed (len={}): {:?}", len, e);
                return Err(e.into());
            }
            Err(_) => {
                println!("[Receiver] read_exact timed out (len={})", len);
                return Err(CommunicationError::StreamError);
            }
        }

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
