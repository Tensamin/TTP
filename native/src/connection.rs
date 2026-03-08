use crate::{CommunicationError, ConnectionHandle};
use epsilon_core::CommunicationValue;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use wtransport::Connection;

pub const MAX_MESSAGE_SIZE: u64 = 1_000_000_000;
const CHANNEL_SIZE: usize = 100;

pub struct Sender {
    tx: mpsc::Sender<CommunicationValue>,
    _task: tokio::task::JoinHandle<()>,
    handle: Arc<ConnectionHandle>,
}

impl Sender {
    pub fn new(connection: Connection, handle: Arc<ConnectionHandle>) -> Self {
        let (tx, mut rx) = mpsc::channel::<CommunicationValue>(CHANNEL_SIZE);
        let conn_handle = handle.clone();

        let task = tokio::spawn(async move {
            let mut close_rx = conn_handle.subscribe_close();

            loop {
                tokio::select! {
                    msg = rx.recv() => {
                        match msg {
                            Some(msg) => {
                                if connection.quic_connection().close_reason().is_some() {
                                    conn_handle.close(Some(CommunicationError::StreamClosed));
                                    break;
                                }

                                if let Err(e) = Self::send_internal(&connection, &msg).await {
                                    eprintln!("Send error: {:?}", e);
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
        }
    }

    async fn send_internal(
        conn: &Connection,
        data: &CommunicationValue,
    ) -> Result<(), CommunicationError> {
        let opening = conn
            .open_uni()
            .await
            .map_err(|e| CommunicationError::ConnectionError(e))?;
        let mut stream = opening.await.map_err(|_| CommunicationError::StreamError)?;

        let bytes = data.to_bytes();
        if bytes.len() as u64 > MAX_MESSAGE_SIZE {
            return Err(CommunicationError::MessageTooLarge);
        }

        use tokio::io::AsyncWriteExt;
        stream
            .write_u32(bytes.len() as u32)
            .await
            .map_err(|_| CommunicationError::StreamError)?;
        stream.write_all(&bytes).await?;

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
                .unwrap_or(CommunicationError::StreamClosed));
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
        self.handle.close(None);
    }

    pub fn is_closed(&self) -> bool {
        self.handle.is_closed()
    }

    pub fn close_reason(&self) -> Option<CommunicationError> {
        self.handle.close_reason()
    }
}

pub struct Receiver {
    rx: Mutex<mpsc::Receiver<CommunicationValue>>,
    _task: tokio::task::JoinHandle<()>,
    handle: Arc<ConnectionHandle>,
}

impl Receiver {
    pub fn new(connection: Connection, handle: Arc<ConnectionHandle>) -> Self {
        let (tx, rx) = mpsc::channel::<CommunicationValue>(CHANNEL_SIZE);
        let conn_handle = handle.clone();

        let task = tokio::spawn(async move {
            let mut close_rx = conn_handle.subscribe_close();

            loop {
                tokio::select! {
                    result = Self::receive_internal(&connection) => {
                        match result {
                            Ok(msg) => {
                                if tx.send(msg).await.is_err() {
                                    conn_handle.close(None);
                                    break;
                                }
                            }
                            Err(e) => {
                                eprintln!("Receive error: {:?}", e);
                                conn_handle.close(Some(e));
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

    async fn receive_internal(conn: &Connection) -> Result<CommunicationValue, CommunicationError> {
        let mut stream = conn
            .accept_uni()
            .await
            .map_err(|e| CommunicationError::ConnectionError(e))?;

        use tokio::io::AsyncReadExt;
        let len = stream
            .read_u32()
            .await
            .map_err(|_| CommunicationError::StreamError)? as usize;

        if len as u64 > MAX_MESSAGE_SIZE {
            return Err(CommunicationError::MessageTooLarge);
        }

        let mut buf = vec![0u8; len];
        stream.read_exact(&mut buf).await?;

        CommunicationValue::from_bytes(&buf).ok_or(CommunicationError::ParseCommunicationValue)
    }

    pub async fn receive(&self) -> Result<CommunicationValue, CommunicationError> {
        if self.handle.is_closed() {
            return Err(self
                .handle
                .close_reason()
                .unwrap_or(CommunicationError::StreamClosed));
        }

        let result = self.rx.lock().await.recv().await;

        if self.handle.is_closed() {
            return Err(self
                .handle
                .close_reason()
                .unwrap_or(CommunicationError::StreamClosed));
        }

        result.ok_or(CommunicationError::StreamClosed)
    }

    /// Get the connection handle
    pub fn handle(&self) -> &Arc<ConnectionHandle> {
        &self.handle
    }

    /// Close this connection (also signals Sender to stop)
    pub fn close(&self) {
        self.handle.close(None);
    }

    /// Check if closed
    pub fn is_closed(&self) -> bool {
        self.handle.is_closed()
    }

    /// Get close reason
    pub fn close_reason(&self) -> Option<CommunicationError> {
        self.handle.close_reason()
    }
}
