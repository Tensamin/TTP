use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommunicationError {
    #[error("QUIC error: {0}")]
    Quinn(#[from] quinn::ConnectionError),

    #[error("Tokio IO error: {0}")]
    TokioIo(#[from] tokio::io::Error),

    #[error("ParseCommunicationValue error")]
    ParseCommunicationValue,

    #[error("ParseBool error: {0}")]
    ParseBool(#[from] std::str::ParseBoolError),

    #[error("ParseInt error: {0}")]
    ParseInt(#[from] std::num::ParseIntError),

    #[error("ParseFloat error: {0}")]
    ParseFloat(#[from] std::num::ParseFloatError),

    #[error("ParseAddr error: {0}")]
    ParseAddr(#[from] std::net::AddrParseError),

    #[error("Connect error")]
    ConnectError(#[from] quinn::ConnectError),

    #[error("ReadToEnd error")]
    ReadToEndError(#[from] quinn::ReadToEndError),

    #[error("Write error")]
    WriteError(#[from] quinn::WriteError),

    #[error("Closed error")]
    ClosedError(#[from] quinn::ClosedStream),

    #[error("Message too large")]
    MessageTooLarge,

    #[error("ReadExactError: {0}")]
    ReadExactError(#[from] quinn::ReadExactError),

    #[error("StreamClosedError")]
    StreamClosed,

    #[error("Other: {0}")]
    Other(String),
}
