use quinn::{Connection, Endpoint, ServerConfig};
use tokio::sync::mpsc;

use crate::{CommunicationError, Receiver, Sender};

pub async fn host(port: u16, server_config: ServerConfig) -> Result<Host, CommunicationError> {
    let endpoint = Endpoint::server(server_config, format!("0.0.0.0:{port}").parse()?)?;

    let (tx, rx) = mpsc::channel(100);

    tokio::spawn(async move {
        while let Some(connecting) = endpoint.accept().await {
            if let Ok(connection) = connecting.await {
                let _ = tx.send(connection).await;
            }
        }
    });

    Ok(Host { incoming: rx })
}

pub struct Host {
    incoming: mpsc::Receiver<Connection>,
}

impl Host {
    pub async fn next(&mut self) -> Option<(Sender, Receiver)> {
        if let Some(connection) = self.incoming.recv().await {
            return Some((Sender::new(connection.clone()), Receiver::new(connection)));
        }

        None
    }
}
