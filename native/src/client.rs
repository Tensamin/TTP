use rustls::pki_types::pem::PemObject;
use wtransport::tls::Sha256Digest;
use wtransport::{ClientConfig, Connection, Endpoint};

use crate::{CommunicationError, Receiver, Sender};

pub async fn connect(
    url: &str,
    server_cert: Vec<u8>,
) -> Result<(Sender, Receiver), CommunicationError> {
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
    // Parse the server certificate to extract its SHA-256 hash for pinning
    let cert_der = rustls::pki_types::CertificateDer::pem_slice_iter(&server_cert)
        .next()
        .ok_or(CommunicationError::CertificateParseFailed)?
        .map_err(|_| CommunicationError::CertificateParseFailed)?;

    // Compute SHA-256 hash of the certificate
    let hash = aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, cert_der.as_ref());
    let hash_bytes: [u8; 32] = hash
        .as_ref()
        .try_into()
        .map_err(|_| CommunicationError::CertificateParseFailed)?;

    let digest = Sha256Digest::new(hash_bytes);

    // Build client config with certificate hash pinning
    // This only accepts certificates matching the specific hash
    let client_config = ClientConfig::builder()
        .with_bind_default()
        .with_server_certificate_hashes([digest])
        .build();

    Ok(client_config)
}
