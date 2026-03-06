use crate::{CommunicationError, Receiver, Sender};
use rustls::pki_types::pem::PemObject;
use rustls::{ClientConfig as RustlsClientConfig, RootCertStore};
use wtransport::{ClientConfig, Connection, Endpoint};

pub async fn connect(
    url: &str,
    server_cert: Vec<u8>,
) -> Result<(Sender, Receiver), CommunicationError> {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let client_config = configure_client(server_cert)?;
    let endpoint = Endpoint::client(client_config)?;

    let connecting = endpoint.connect(url);

    let connection: Connection = connecting
        .await
        .map_err(|e| CommunicationError::ConnectingError(e))?;

    let sender = Sender::new(connection.clone());
    let receiver = Receiver::new(connection);

    Ok((sender, receiver))
}

fn configure_client(server_cert: Vec<u8>) -> Result<ClientConfig, CommunicationError> {
    let mut root_store = RootCertStore::empty();
    let cert = rustls::pki_types::CertificateDer::pem_slice_iter(&server_cert)
        .next()
        .ok_or(CommunicationError::CertificateParseFailed)?
        .map_err(|_| CommunicationError::CertificateParseFailed)?;

    root_store
        .add(cert)
        .map_err(|_| CommunicationError::CertificateParseFailed)?;

    let mut tls_config = RustlsClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    tls_config.alpn_protocols = vec![
        b"h3".to_vec(),
        b"h3-29".to_vec(),
        b"h3-28".to_vec(),
        b"h3-27".to_vec(),
    ];

    let client_config = ClientConfig::builder()
        .with_bind_default()
        .with_custom_tls(tls_config)
        .build();

    Ok(client_config)
}
