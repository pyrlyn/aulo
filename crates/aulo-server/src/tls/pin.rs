//! The client half of pinning, for `aulo connect --pin`: trust exactly one
//! server certificate, named by its SHA-256, instead of a chain to a CA.

use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::crypto::{WebPkiSupportedAlgorithms, verify_tls12_signature, verify_tls13_signature};
use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
use rustls::{CertificateError, ClientConfig, DigitallySignedStruct, SignatureScheme};

use super::{CertFingerprint, provider};
use crate::error::ServerError;

/// gRPC runs over HTTP/2 only; offering anything else would let a server
/// pick HTTP/1.1 and fail later with a less useful error.
const ALPN_H2: &[u8] = b"h2";

/// Accepts a server only if its leaf certificate has the pinned fingerprint.
///
/// Chain, host name and validity dates are deliberately not checked: the pin
/// already names the one certificate this client accepts, and a self-signed
/// certificate has no chain or meaningful name to check. The handshake
/// signature still is, which proves the server holds that certificate's key.
#[derive(Debug)]
pub struct PinnedServerVerifier {
    pin: CertFingerprint,
    algorithms: WebPkiSupportedAlgorithms,
}

impl PinnedServerVerifier {
    pub fn new(pin: CertFingerprint) -> Self {
        Self {
            pin,
            algorithms: provider().signature_verification_algorithms,
        }
    }
}

impl ServerCertVerifier for PinnedServerVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp_response: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if CertFingerprint::of(end_entity) == self.pin {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::InvalidCertificate(
                CertificateError::ApplicationVerificationFailure,
            ))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls12_signature(message, cert, dss, &self.algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        verify_tls13_signature(message, cert, dss, &self.algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.algorithms.supported_schemes()
    }
}

/// A rustls client config that trusts only the pinned certificate and
/// speaks HTTP/2, ready for a TLS connector under a tonic channel.
pub fn pinned_client_config(pin: CertFingerprint) -> Result<ClientConfig, ServerError> {
    let mut config = ClientConfig::builder_with_provider(Arc::new(provider()))
        .with_safe_default_protocol_versions()
        .map_err(ServerError::TlsClient)?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(PinnedServerVerifier::new(pin)))
        .with_no_client_auth();
    config.alpn_protocols = vec![ALPN_H2.to_vec()];
    Ok(config)
}
