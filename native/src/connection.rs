use crate::CommunicationError;
use epsilon_core::CommunicationValue;
use wtransport::Connection;

pub const MAX_MESSAGE_SIZE: u64 = 1_000_000_000;

pub struct Sender {
    connection: Connection,
    _phantom: std::marker::PhantomData<CommunicationValue>,
}

impl Sender {
    pub fn new(connection: Connection) -> Self {
        Self {
            connection,
            _phantom: std::marker::PhantomData,
        }
    }

    pub async fn send(&self, data: &CommunicationValue) -> Result<(), CommunicationError> {
        // open_uni() returns OpeningUniStream, await it to get SendStream
        let opening = self.connection.open_uni().await?;
        let mut stream = opening.await.map_err(|_| CommunicationError::StreamError)?;

        let bytes = data.to_bytes();
        let len = bytes.len() as u64;

        if len > MAX_MESSAGE_SIZE {
            return Err(CommunicationError::MessageTooLarge);
        }

        use tokio::io::AsyncWriteExt;
        stream.write_u32(len as u32).await?;
        let _ = stream.write_all(&bytes).await;

        // SendStream uses finish() without awaiting
        stream
            .finish()
            .await
            .map_err(|_| CommunicationError::StreamError)?;

        Ok(())
    }

    pub fn close(&self) {
        self.connection
            .close(wtransport::VarInt::from_u32(0), b"sender closed");
    }
}

pub struct Receiver {
    connection: Connection,
    _phantom: std::marker::PhantomData<CommunicationValue>,
}

impl Receiver {
    pub fn new(connection: Connection) -> Self {
        Self {
            connection,
            _phantom: std::marker::PhantomData,
        }
    }

    pub async fn receive(&self) -> Result<CommunicationValue, CommunicationError> {
        // accept_uni() returns RecvStream directly (not Result)
        let mut stream = self
            .connection
            .accept_uni()
            .await
            .map_err(|e| CommunicationError::ConnectionError(e))?;

        use tokio::io::AsyncReadExt;
        let len = stream.read_u32().await? as u64;

        if len > MAX_MESSAGE_SIZE {
            return Err(CommunicationError::MessageTooLarge);
        }

        let mut buf = vec![0u8; len as usize];
        let _ = stream.read_exact(&mut buf).await;

        let value = CommunicationValue::from_bytes(&buf)
            .ok_or(CommunicationError::ParseCommunicationValue)?;

        Ok(value)
    }

    pub fn close(&self) {
        self.connection
            .close(wtransport::VarInt::from_u32(0), b"receiver closed");
    }
}
