use quinn::Endpoint;

use crate::{CommunicationError, Receiver, Sender};

pub async fn connect(addr: &str) -> Result<(Sender, Receiver), CommunicationError> {
    let endpoint = Endpoint::client("[::]:0".parse()?)?;

    let connection = endpoint.connect(addr.parse()?, "localhost")?.await?;

    Ok((Sender::new(connection.clone()), Receiver::new(connection)))
}
