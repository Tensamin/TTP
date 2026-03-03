use crate::CommunicationError;
pub use crate::Receiver;
pub use crate::Sender;
use quinn::rustls::pki_types::pem::PemObject;
use quinn::{Connection, Endpoint, ServerConfig, rustls};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct Host {
    incoming: mpsc::Receiver<(Sender, Receiver)>,
    _task: tokio::task::JoinHandle<()>,
}

impl Host {
    pub async fn next(&mut self) -> Option<(Sender, Receiver)> {
        self.incoming.recv().await
    }
}

pub async fn host(port: u16, server_config: ServerConfig) -> Result<Host, CommunicationError> {
    let addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
    let endpoint = Endpoint::server(server_config, addr)?;

    let (incoming_tx, incoming_rx) = mpsc::channel(16);

    let task = tokio::spawn(async move {
        while let Some(incoming) = endpoint.accept().await {
            let conn = match incoming.await {
                Ok(c) => c,
                Err(_) => continue,
            };

            let incoming_tx = incoming_tx.clone();
            tokio::spawn(handle_connection(conn, incoming_tx));
        }
    });

    Ok(Host {
        incoming: incoming_rx,
        _task: task,
    })
}

async fn handle_connection(connection: Connection, tx: mpsc::Sender<(Sender, Receiver)>) {
    let sender = Sender::new(connection.clone());
    let receiver = Receiver::new(connection);

    // Non-blocking send - if channel full, connection is dropped
    let _ = tx.try_send((sender, receiver));
}

pub fn generate_self_signed_cert() -> (Vec<CertificateDer<'static>>, PrivateKeyDer<'static>) {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let cert_der = CertificateDer::from(cert.cert.der().clone());
    let key_der = PrivateKeyDer::from_pem(
        rustls::pki_types::pem::SectionKind::Certificate,
        cert.signing_key.serialize_der(),
    )
    .unwrap();
    (vec![cert_der], key_der)
}

pub fn configure_server() -> ServerConfig {
    let (certs, key) = generate_self_signed_cert();

    let mut crypto = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .unwrap();

    // ALPN for both native QUIC and WebTransport
    crypto.alpn_protocols = vec![
        b"h3".to_vec(),    // HTTP/3 (WebTransport)
        b"hq-29".to_vec(), // QUIC (legacy)
    ];

    ServerConfig::with_crypto(Arc::new(
        quinn::crypto::rustls::QuicServerConfig::try_from(crypto).unwrap(),
    ))
}
