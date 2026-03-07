use crate::CommunicationError;
use epsilon_core::CommunicationValue;
use tokio::sync::mpsc;
use wtransport::Connection;

pub const MAX_MESSAGE_SIZE: u64 = 1_000_000_000;
const CHANNEL_SIZE: usize = 100;

pub struct Sender {
    tx: mpsc::Sender<CommunicationValue>,
    _task: tokio::task::JoinHandle<()>,
}

impl Sender {
    pub fn new(connection: Connection) -> Self {
        let (tx, mut rx) = mpsc::channel::<CommunicationValue>(CHANNEL_SIZE);

        let task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                if let Err(e) = Self::send_internal(&connection, &msg).await {
                    eprintln!("Send error: {:?}", e);
                    break;
                }
            }
        });

        Self { tx, _task: task }
    }

    async fn send_internal(
        conn: &Connection,
        data: &CommunicationValue,
    ) -> Result<(), CommunicationError> {
        let opening = conn.open_uni().await?;
        let mut stream = opening.await.map_err(|_| CommunicationError::StreamError)?;

        let bytes = data.to_bytes();
        if bytes.len() as u64 > MAX_MESSAGE_SIZE {
            return Err(CommunicationError::MessageTooLarge);
        }

        use tokio::io::AsyncWriteExt;
        stream.write_u32(bytes.len() as u32).await?;
        stream.write_all(&bytes).await?;

        // Don't block on finish
        tokio::spawn(async move {
            let _ = stream.finish().await;
        });
        Ok(())
    }

    pub async fn send(&self, data: &CommunicationValue) -> Result<(), CommunicationError> {
        self.tx
            .send(data.clone())
            .await
            .map_err(|_| CommunicationError::StreamClosed)
    }

    pub fn close(&self) {
        self._task.abort();
    }
}

pub struct Receiver {
    rx: mpsc::Receiver<CommunicationValue>,
    _task: tokio::task::JoinHandle<()>,
}

impl Receiver {
    pub fn new(connection: Connection) -> Self {
        let (tx, rx) = mpsc::channel::<CommunicationValue>(CHANNEL_SIZE);

        let task = tokio::spawn(async move {
            loop {
                match Self::receive_internal(&connection).await {
                    Ok(msg) => {
                        if tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        eprintln!("Receive error: {:?}", e);
                        break;
                    }
                }
            }
        });

        Self { rx, _task: task }
    }

    async fn receive_internal(conn: &Connection) -> Result<CommunicationValue, CommunicationError> {
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

    pub async fn receive(&mut self) -> Result<CommunicationValue, CommunicationError> {
        self.rx.recv().await.ok_or(CommunicationError::StreamClosed)
    }

    pub fn close(&self) {
        self._task.abort();
    }
}
