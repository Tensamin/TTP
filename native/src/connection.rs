use crate::CommunicationError;
use epsilon_core::CommunicationValue;
use quinn::{Connection, VarInt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
        let mut stream = self.connection.open_uni().await?;

        let bytes = data.to_bytes();
        let len = bytes.len() as u64;

        if len > MAX_MESSAGE_SIZE {
            return Err(CommunicationError::MessageTooLarge);
        }

        stream.write_u32(len as u32).await?;
        stream.write_all(&bytes).await?;
        stream.finish()?;

        Ok(())
    }

    pub fn close(&self) {
        self.connection.close(VarInt::from_u32(0), b"sender closed");
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
        let mut stream = self.connection.accept_uni().await?;

        // Read length (u32 = 4 bytes)
        let len = stream.read_u32().await? as u64;

        if len > MAX_MESSAGE_SIZE {
            return Err(CommunicationError::MessageTooLarge);
        }

        let mut buf = vec![0u8; len as usize];
        stream.read_exact(&mut buf).await?;

        let value = CommunicationValue::from_bytes(&buf)
            .ok_or(CommunicationError::ParseCommunicationValue)?;

        Ok(value)
    }

    pub fn close(&self) {
        self.connection
            .close(VarInt::from_u32(0), b"receiver closed");
    }
}
