use crate::{CommunicationError, ConnectionHandle};
use epsilon_core::CommunicationValue;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};
use wtransport::Connection as WTConnection;

pub const MAX_MESSAGE_SIZE: u64 = 1_000_000_000;
const CHANNEL_SIZE: usize = 100;

/// Unified connection that can be closed from either Sender or Receiver
#[derive(Debug)]
pub struct Connection {
    handle: Arc<ConnectionHandle>,
}

impl Connection {
    pub fn new(handle: Arc<ConnectionHandle>) -> Self {
        Self { handle }
    }

    /// Close the connection from this side
    pub fn close(&self) {
        self.handle.close();
    }

    /// Check if connection is closed
    pub fn is_open(&self) -> bool {
        self.handle.is_open()
    }

    /// Wait for connection to close
    pub async fn wait_closed(&self) {
        let mut rx = self.handle.subscribe_close();
        if *rx.borrow() {
            return;
        }
        let _ = rx.changed().await;
    }
}

pub struct Sender {
    tx: mpsc::Sender<CommunicationValue>,
    _task: tokio::task::JoinHandle<()>,
    handle: Arc<ConnectionHandle>,
    connection: Connection,
}

impl Sender {
    pub fn new(connection: WTConnection, handle: Arc<ConnectionHandle>) -> Self {
        let (tx, mut rx) = mpsc::channel::<CommunicationValue>(CHANNEL_SIZE);
        let conn_handle = handle.clone();
        let wtx_conn = connection.clone();

        let task = tokio::spawn(async move {
            // Monitor both message channel and close signal
            let mut close_rx = conn_handle.subscribe_close();

            loop {
                tokio::select! {
                    msg = rx.recv() => {
                        match msg {
                            Some(msg) => {
                                if let Err(e) = Self::send_internal(&wtx_conn, &msg).await {
                                    eprintln!("Send error: {:?}", e);
                                    conn_handle.close();
                                    break;
                                }
                            }
                            None => {
                                conn_handle.close();
                                break;
                            }
                        }
                    }
                    _ = close_rx.changed() => {
                        if *close_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        });

        Self {
            tx,
            _task: task,
            handle: handle.clone(),
            connection: Connection::new(handle),
        }
    }

    async fn send_internal(
        conn: &WTConnection,
        data: &CommunicationValue,
    ) -> Result<(), CommunicationError> {
        if conn.quic_connection().close_reason().is_some() {
            return Err(CommunicationError::StreamClosed);
        }

        let opening = conn.open_uni().await?;
        let mut stream = opening.await.map_err(|_| CommunicationError::StreamError)?;

        let bytes = data.to_bytes();
        if bytes.len() as u64 > MAX_MESSAGE_SIZE {
            return Err(CommunicationError::MessageTooLarge);
        }

        use tokio::io::AsyncWriteExt;
        stream.write_u32(bytes.len() as u32).await?;
        stream.write_all(&bytes).await?;

        tokio::spawn(async move {
            let _ = stream.finish().await;
        });
        Ok(())
    }

    pub async fn send(&self, data: &CommunicationValue) -> Result<(), CommunicationError> {
        if !self.handle.is_open() {
            return Err(CommunicationError::StreamClosed);
        }

        self.tx
            .send(data.clone())
            .await
            .map_err(|_| CommunicationError::StreamClosed)
    }

    /// Get the unified connection handle for close operations
    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    /// Close this connection (also signals Receiver to stop)
    pub fn close(&self) {
        self.handle.close();
    }

    /// Check if closed
    pub fn is_open(&self) -> bool {
        self.handle.is_open()
    }
}

pub struct Receiver {
    rx: Mutex<mpsc::Receiver<CommunicationValue>>,
    _task: tokio::task::JoinHandle<()>,
    handle: Arc<ConnectionHandle>,
    connection: Connection,
}

impl Receiver {
    pub fn new(connection: wtransport::Connection, handle: Arc<ConnectionHandle>) -> Self {
        let (tx, rx) = mpsc::channel::<CommunicationValue>(CHANNEL_SIZE);
        let conn_handle = handle.clone();
        let wrx_conn = connection.clone();

        let task = tokio::spawn(async move {
            let mut close_rx = conn_handle.subscribe_close();

            loop {
                tokio::select! {
                    result = Self::receive_internal(&wrx_conn) => {
                        match result {
                            Ok(msg) => {
                                if tx.send(msg).await.is_err() {
                                    conn_handle.close();
                                    break;
                                }
                            }
                            Err(e) => {
                                eprintln!("Receive error: {:?}", e);
                                conn_handle.close();
                                break;
                            }
                        }
                    }
                    _ = close_rx.changed() => {
                        if *close_rx.borrow() {
                            break;
                        }
                    }
                }
            }
        });

        Self {
            rx: Mutex::new(rx),
            _task: task,
            handle: handle.clone(),
            connection: Connection::new(handle),
        }
    }

    async fn receive_internal(
        conn: &WTConnection,
    ) -> Result<CommunicationValue, CommunicationError> {
        if let Some(reason) = conn.quic_connection().close_reason() {
            return Err(CommunicationError::Quinn(reason));
        }

        let mut stream = conn
            .accept_uni()
            .await
            .map_err(|e| CommunicationError::ConnectionError(e))?;

        use tokio::io::AsyncReadExt;
        let len = stream.read_u32().await? as usize;

        if len as u64 > MAX_MESSAGE_SIZE {
            return Err(CommunicationError::MessageTooLarge);
        }

        let mut buf = vec![0u8; len];
        stream.read_exact(&mut buf).await?;

        CommunicationValue::from_bytes(&buf).ok_or(CommunicationError::ParseCommunicationValue)
    }

    pub async fn receive(&self) -> Result<CommunicationValue, CommunicationError> {
        if !self.handle.is_open() {
            return Err(CommunicationError::StreamClosed);
        }

        let result = self.rx.lock().await.recv().await;

        if !self.handle.is_open() {
            return Err(CommunicationError::StreamClosed);
        }

        result.ok_or(CommunicationError::StreamClosed)
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    pub fn close(&self) {
        self.handle.close();
    }

    pub fn is_open(&self) -> bool {
        self.handle.is_open()
    }
}
