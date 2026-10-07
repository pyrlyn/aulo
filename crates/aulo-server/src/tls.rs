//! TLS for the TCP listener (spec §11, §13).
//!
//! The server presents either the user's certificate or a self-signed one
//! made on first start. Clients of a self-signed server cannot chain it to a
//! CA, so they pin its SHA-256 fingerprint instead ([`pinned_client_config`]).

mod pin;

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Write};
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use rustls::crypto::CryptoProvider;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::sign::CertifiedKey;
use sha2::{Digest, Sha256};
use tonic::transport::{Identity, ServerTlsConfig};

pub use pin::{PinnedServerVerifier, pinned_client_config};

use crate::error::ServerError;

const CERT_FILE: &str = "cert.pem";
const KEY_FILE: &str = "key.pem";
/// Bytes in a SHA-256 digest.
const DIGEST_LEN: usize = 32;
// The pin, not the name, is what clients check, so one stable name is enough
// for tools that insist on a SAN.
const SELF_SIGNED_NAME: &str = "localhost";

/// The SHA-256 of a certificate's DER encoding: what `aulo connect --pin`
/// takes and what the server prints for its user to copy. Shown as
/// colon-separated uppercase hex, the form `openssl x509 -fingerprint
/// -sha256` prints, so the two can be compared by eye.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CertFingerprint([u8; DIGEST_LEN]);

impl CertFingerprint {
    pub fn of(cert_der: &[u8]) -> Self {
        Self(Sha256::digest(cert_der).into())
    }

    pub fn as_bytes(&self) -> &[u8; DIGEST_LEN] {
        &self.0
    }
}

impl fmt::Display for CertFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, byte) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(":")?;
            }
            write!(f, "{byte:02X}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for CertFingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CertFingerprint({self})")
    }
}

/// Accepts the displayed form and bare hex, in either case, so a pin pasted
/// from `openssl` or from aulo's own output both work.
impl FromStr for CertFingerprint {
    type Err = ServerError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let digits: Vec<u8> = s.bytes().filter(|b| *b != b':').collect();
        if digits.len() != DIGEST_LEN * 2 {
            return Err(ServerError::InvalidFingerprint);
        }
        let mut bytes = [0; DIGEST_LEN];
        for (byte, [hi, lo]) in bytes.iter_mut().zip(digits.as_chunks::<2>().0) {
            let value = (hex_digit(*hi)? << 4) | hex_digit(*lo)?;
            *byte = u8::try_from(value).map_err(|_| ServerError::InvalidFingerprint)?;
        }
        Ok(Self(bytes))
    }
}

// Not `u8::from_str_radix`: it accepts a leading `+`, which is not hex.
fn hex_digit(b: u8) -> Result<u32, ServerError> {
    char::from(b)
        .to_digit(16)
        .ok_or(ServerError::InvalidFingerprint)
}

/// A certificate chain and its private key, checked to belong together.
#[derive(Clone, PartialEq, Eq)]
pub struct TlsIdentity {
    cert_pem: Vec<u8>,
    key_pem: Vec<u8>,
    fingerprint: CertFingerprint,
}

// The key must never reach a log line.
impl fmt::Debug for TlsIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TlsIdentity")
            .field("fingerprint", &self.fingerprint)
            .finish_non_exhaustive()
    }
}

impl TlsIdentity {
    /// The user's PEM certificate chain (leaf first) and private key.
    pub fn from_pem_files(cert: &Path, key: &Path) -> Result<Self, ServerError> {
        let cert_pem = fs::read(cert).map_err(ServerError::io("read", cert.display()))?;
        let key_pem = fs::read(key).map_err(ServerError::io("read", key.display()))?;
        Self::from_pem(cert_pem, key_pem, cert, key)
    }

    /// The self-signed identity kept in `dir`, generated there on first start.
    ///
    /// On Unix `dir` is created owner-only and both files are written 0600;
    /// an existing key that other users can read is refused, since anyone who
    /// read it could impersonate the server to every pinned client.
    pub fn self_signed(dir: &Path) -> Result<Self, ServerError> {
        private_dir(dir)?;
        let cert = dir.join(CERT_FILE);
        let key = dir.join(KEY_FILE);
        match fs::symlink_metadata(&cert) {
            Ok(_) => {
                check_private(&key)?;
                return Self::from_pem_files(&cert, &key);
            }
            Err(e) if e.kind() == ErrorKind::NotFound => {}
            Err(e) => return Err(ServerError::io("inspect", cert.display())(e)),
        }
        let generated = rcgen::generate_simple_self_signed(vec![SELF_SIGNED_NAME.to_owned()])?;
        let cert_pem = generated.cert.pem().into_bytes();
        let key_pem = generated.signing_key.serialize_pem().into_bytes();
        // The certificate goes last: its presence marks a finished
        // generation, so a start cut short after the key just generates again.
        write_private(&key, &key_pem)?;
        write_private(&cert, &cert_pem)?;
        let identity = Self::from_pem(cert_pem, key_pem, &cert, &key)?;
        tracing::info!(
            fingerprint = %identity.fingerprint,
            path = %cert.display(),
            "generated a self-signed TLS certificate"
        );
        Ok(identity)
    }

    /// The fingerprint of the leaf certificate, which clients pin.
    pub fn fingerprint(&self) -> CertFingerprint {
        self.fingerprint
    }

    pub(crate) fn server_config(&self, handshake_timeout: Duration) -> ServerTlsConfig {
        install_provider();
        ServerTlsConfig::new()
            .identity(Identity::from_pem(&self.cert_pem, &self.key_pem))
            .timeout(handshake_timeout)
    }

    fn from_pem(
        cert_pem: Vec<u8>,
        key_pem: Vec<u8>,
        cert_path: &Path,
        key_path: &Path,
    ) -> Result<Self, ServerError> {
        let pem_error = |path: &Path| {
            let path = path.to_owned();
            move |source| ServerError::Pem { path, source }
        };
        let chain = CertificateDer::pem_slice_iter(&cert_pem)
            .collect::<Result<Vec<_>, _>>()
            .map_err(pem_error(cert_path))?;
        let leaf = chain
            .first()
            .ok_or_else(|| ServerError::NoCertificate(cert_path.to_owned()))?;
        let fingerprint = CertFingerprint::of(leaf);
        let key = PrivateKeyDer::from_pem_slice(&key_pem).map_err(pem_error(key_path))?;
        // rustls runs the same check when tonic builds the acceptor; running
        // it here makes a wrong key fail at load, before anything is bound.
        CertifiedKey::from_der(chain, key, &provider()).map_err(ServerError::TlsIdentity)?;
        Ok(Self {
            cert_pem,
            key_pem,
            fingerprint,
        })
    }
}

/// aws-lc-rs, the provider the rest of the workspace (reqwest) already builds.
pub(crate) fn provider() -> CryptoProvider {
    rustls::crypto::aws_lc_rs::default_provider()
}

// tonic builds its acceptor from the process-wide default provider, and
// rustls panics instead of choosing when a second provider feature appears
// in the build. Installing ours first keeps that a non-event; an Err only
// means someone installed one already, which is fine.
fn install_provider() {
    let _ = provider().install_default();
}

#[cfg(unix)]
fn private_dir(dir: &Path) -> Result<(), ServerError> {
    crate::listener::owner_only_dir(dir)
}

// Windows has no mode bits: the directory keeps the ACL it inherits from
// where the caller put it, normally the user's own profile.
#[cfg(not(unix))]
fn private_dir(dir: &Path) -> Result<(), ServerError> {
    fs::create_dir_all(dir).map_err(ServerError::io("create", dir.display()))
}

#[cfg(unix)]
fn check_private(path: &Path) -> Result<(), ServerError> {
    use std::os::unix::fs::PermissionsExt;

    let meta = fs::symlink_metadata(path).map_err(ServerError::io("inspect", path.display()))?;
    if !meta.is_file() || meta.permissions().mode() & 0o077 != 0 {
        return Err(ServerError::InsecureFile(path.to_owned()));
    }
    Ok(())
}

#[cfg(not(unix))]
fn check_private(_path: &Path) -> Result<(), ServerError> {
    Ok(())
}

/// Writes through a temporary file and a rename, so a crash never leaves a
/// half-written PEM that the next start would refuse.
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), ServerError> {
    let tmp = path.with_extension("pem.tmp");
    match fs::remove_file(&tmp) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(e) => return Err(ServerError::io("remove", tmp.display())(e)),
    }
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&tmp)
        .map_err(ServerError::io("create", tmp.display()))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(ServerError::io("write", tmp.display()))?;
    fs::rename(&tmp, path).map_err(ServerError::io("rename", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_round_trips_through_its_display_form() {
        let fp = CertFingerprint::of(b"certificate");
        let shown = fp.to_string();
        assert_eq!(shown.len(), DIGEST_LEN * 3 - 1);
        assert_eq!(shown.parse::<CertFingerprint>().unwrap(), fp);
        let bare = shown.replace(':', "").to_lowercase();
        assert_eq!(bare.parse::<CertFingerprint>().unwrap(), fp);
    }

    #[test]
    fn malformed_fingerprints_are_refused() {
        let fp = CertFingerprint::of(b"certificate").to_string();
        for bad in [
            "",
            "AB",
            &fp[..fp.len() - 1],
            &format!("{fp}:00"),
            &format!("G{}", &fp[1..]),
            &format!("+{}", &fp[1..]),
        ] {
            assert!(bad.parse::<CertFingerprint>().is_err(), "{bad:?}");
        }
        // Non-ASCII must not panic on a char boundary inside a hex pair.
        let wide = "é".repeat(DIGEST_LEN);
        assert!(wide.parse::<CertFingerprint>().is_err());
    }
}
