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
    Idle,
}

pub struct Sender {
    send_guard: Mutex<()>,
    stream_guard: Mutex<Option<wtransport::SendStream>>,
    handle: Arc<ConnectionHandle>,
    connection: Connection,
}

impl Sender {
    pub fn new(connection: Connection, handle: Arc<ConnectionHandle>) -> Self {
        Self {
            send_guard: Mutex::new(()),
            stream_guard: Mutex::new(None),
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

    fn normalize_send_error(error: CommunicationError) -> CommunicationError {
        match error {
            CommunicationError::ConnectionError(_)
            | CommunicationError::ReadExactError(_)
            | CommunicationError::ClosedError(_)
            | CommunicationError::StreamReadExactError(_)
            | CommunicationError::StreamError => CommunicationError::StreamClosed,
            other => other,
        }
    }

    async fn open_uni_stream(
        conn: &Connection,
    ) -> Result<wtransport::SendStream, CommunicationError> {
        let opening = timeout(
            Duration::from_millis(OPEN_STREAM_TIMEOUT_MS),
            conn.open_uni(),
        )
        .await
        .map_err(|_| CommunicationError::StreamError)?
        .map_err(CommunicationError::ConnectionError)?;

        let stream = timeout(Duration::from_millis(OPEN_STREAM_TIMEOUT_MS), opening)
            .await
            .map_err(|_| CommunicationError::StreamError)?
            .map_err(|_| CommunicationError::StreamError)?;

        Ok(stream)
    }

    async fn ensure_stream<'a>(
        conn: &Connection,
        stream_opt: &'a mut Option<wtransport::SendStream>,
    ) -> Result<&'a mut wtransport::SendStream, CommunicationError> {
        if stream_opt.is_none() {
            *stream_opt = Some(Self::open_uni_stream(conn).await?);
        }

        match stream_opt.as_mut() {
            Some(stream) => Ok(stream),
            None => Err(CommunicationError::StreamError),
        }
    }

    async fn send_on_persistent_stream(
        conn: &Connection,
        stream_opt: &mut Option<wtransport::SendStream>,
        data: &CommunicationValue,
    ) -> Result<(), CommunicationError> {
        let first_result = {
            let stream = Self::ensure_stream(conn, stream_opt).await?;
            Self::write_frame(stream, data).await
        };

        if first_result.is_ok() {
            return Ok(());
        }

        *stream_opt = None;
        let second_result = {
            let stream = Self::ensure_stream(conn, stream_opt).await?;
            Self::write_frame(stream, data).await
        };

        if second_result.is_ok() {
            return Ok(());
        }

        *stream_opt = None;
        second_result
    }

    async fn send_close_frame(conn: &Connection) -> Result<(), CommunicationError> {
        let mut stream = Self::open_uni_stream(conn).await?;

        use tokio::io::AsyncWriteExt;
        timeout(
            Duration::from_millis(WRITE_TIMEOUT_MS),
            stream.write_u32(CLOSE_FRAME_LEN),
        )
        .await
        .map_err(|_| CommunicationError::StreamError)?
        .map_err(|_| CommunicationError::StreamError)?;

        if let Err(e) = timeout(Duration::from_millis(WRITE_TIMEOUT_MS), stream.finish())
            .await
            .map_err(|_| CommunicationError::StreamError)?
        {
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

        let _send_lock = self.send_guard.lock().await;

        if self.connection.quic_connection().close_reason().is_some() {
            let reason = self
                .handle
                .close_reason()
                .unwrap_or(CommunicationError::StreamClosed);
            self.handle.close(Some(reason.clone()));
            return Err(reason);
        }

        let mut stream_opt = self.stream_guard.lock().await;

        match Self::send_on_persistent_stream(&self.connection, &mut stream_opt, data).await {
            Ok(()) => Ok(()),
            Err(e) => {
                let normalized = Self::normalize_send_error(e);
                *stream_opt = None;

                if self.connection.quic_connection().close_reason().is_some()
                    || matches!(normalized, CommunicationError::StreamClosed)
                {
                    self.handle.close(Some(normalized.clone()));
                }

                Err(normalized)
            }
        }
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
                            Ok(ReceivedFrame::Idle) => {
                                transient_errors = 0;
                                if connection.quic_connection().close_reason().is_some() {
                                    let close_error = CommunicationError::StreamClosed;
                                    let _ = tx.send(Err(close_error.clone())).await;
                                    conn_handle.close(Some(close_error));
                                    break;
                                }
                                continue;
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
                return Ok(ReceivedFrame::Idle);
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
