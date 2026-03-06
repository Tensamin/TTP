use crate::{CommunicationError, Receiver, Sender};
use rustls::pki_types::{PrivateKeyDer, pem::PemObject};
use wtransport::{Connection, Endpoint, ServerConfig};

pub struct Host {
    incoming: tokio::sync::mpsc::Receiver<(Sender, Receiver)>,
    _task: tokio::task::JoinHandle<()>,
}

impl Host {
    pub async fn next(&mut self) -> Option<(Sender, Receiver)> {
        self.incoming.recv().await
    }
}

pub async fn host(
    port: u16,
    cert_pem: Vec<u8>,
    key_pem: Vec<u8>,
) -> Result<Host, CommunicationError> {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let server_config = configure_server(port, cert_pem, key_pem).await?;
    let endpoint = Endpoint::server(server_config)
        .map_err(|e| CommunicationError::Other(format!("Endpoint creation failed: {}", e)))?;

    let (incoming_tx, incoming_rx) = tokio::sync::mpsc::channel(16);

    let task = tokio::spawn(async move {
        loop {
            let incoming_session = endpoint.accept().await;

            let request = match incoming_session.await {
                Ok(req) => req,
                Err(e) => {
                    eprintln!("Session accept error: {:?}", e);
                    continue;
                }
            };

            println!("New WebTransport request from: {:?}", request.authority());
            println!("Path: {:?}", request.path());

            let connection = match request.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    eprintln!("WebTransport accept error: {:?}", e);
                    continue;
                }
            };

            println!("WebTransport connection established!");

            let incoming_tx = incoming_tx.clone();
            tokio::spawn(handle_connection(connection, incoming_tx));
        }
    });

    Ok(Host {
        incoming: incoming_rx,
        _task: task,
    })
}

async fn handle_connection(
    connection: Connection,
    tx: tokio::sync::mpsc::Sender<(Sender, Receiver)>,
) {
    let sender = Sender::new(connection.clone());
    let receiver = Receiver::new(connection);
    let _ = tx.send((sender, receiver)).await;
}

async fn configure_server(
    port: u16,
    cert_pem: Vec<u8>,
    key_pem: Vec<u8>,
) -> Result<ServerConfig, CommunicationError> {
    let cert_chain = rustls::pki_types::CertificateDer::pem_slice_iter(&cert_pem)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| CommunicationError::CertificateLoadFailed)?;

    let key = PrivateKeyDer::from_pem_slice(&key_pem)
        .map_err(|_| CommunicationError::CertificateParseFailed)?;

    let mut tls_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, key)
        .map_err(|_| CommunicationError::CertificateLoadFailed)?;

    tls_config.alpn_protocols = vec![b"h3".to_vec()];

    let server_config = ServerConfig::builder()
        .with_bind_default(port)
        .with_custom_tls(tls_config)
        .build();

    Ok(server_config)
}
