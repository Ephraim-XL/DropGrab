//! Device identity (section 4.1) and certificate pinning (section 4.4, 3.3).
//!
//! Each install has a UUID device id and a self-signed ECDSA P-256 certificate. The wire
//! fingerprint is the lowercase hex SHA-256 of the certificate DER. Clients verify the server
//! certificate by fingerprint, never by certificate authority.

use std::path::Path;
use std::sync::Arc;

use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, ServerName, UnixTime};
use rustls::server::danger::{ClientCertVerified, ClientCertVerifier};
use rustls::{DigitallySignedStruct, DistinguishedName, SignatureScheme};
use sha2::{Digest, Sha256};

use crate::db::Db;

/// The client certificate a peer presented on the TLS handshake, as its wire fingerprint.
/// `None` means the peer sent no certificate. Set by the peer listener for every request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PeerCert(pub Option<String>);

/// Verifies that the presented fingerprint matches the claimed one. Absent or different
/// certificates are refused (section 4.4, mutual TLS). Returns the failure message.
pub fn check_claimed_fingerprint(peer: &PeerCert, claimed: &str) -> Result<(), &'static str> {
    match peer.0.as_deref() {
        None => Err("client certificate required"),
        Some(fp) if fp == claimed => Ok(()),
        Some(_) => Err("client certificate does not match the claimed fingerprint"),
    }
}

#[derive(Clone)]
pub struct Identity {
    pub device_id: String,
    pub cert_der: Vec<u8>,
    pub cert_pem: String,
    pub key_pem: String,
    /// Lowercase hex SHA-256 of `cert_der`.
    pub fingerprint: String,
}

pub fn install_crypto_provider() {
    // Installing twice returns Err; that is fine.
    let _ = rustls::crypto::ring::default_provider().install_default();
}

pub fn fingerprint_of(der: &[u8]) -> String {
    hex::encode(Sha256::digest(der))
}

/// Four-digit verification code from both fingerprints (section 4.3): hash of the sorted pair,
/// reduced to four digits.
pub fn verify_code(fp_a: &str, fp_b: &str) -> String {
    let (lo, hi) = if fp_a <= fp_b { (fp_a, fp_b) } else { (fp_b, fp_a) };
    let digest = Sha256::digest(format!("{lo}|{hi}").as_bytes());
    let n = u32::from_be_bytes([digest[0], digest[1], digest[2], digest[3]]);
    format!("{:04}", n % 10_000)
}

impl Identity {
    /// Loads the identity from the database and `dir`, or creates it on first launch.
    pub fn load_or_create(dir: &Path, db: &Db) -> Result<Identity, String> {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let cert_path = dir.join("cert.pem");
        let key_path = dir.join("key.pem");
        let der_path = dir.join("cert.der");
        let device_id = match db.meta_get("device_id").map_err(|e| e.to_string())? {
            Some(id) => id,
            None => {
                let id = uuid::Uuid::new_v4().to_string();
                db.meta_set("device_id", &id).map_err(|e| e.to_string())?;
                id
            }
        };
        if cert_path.exists() && key_path.exists() && der_path.exists() {
            let cert_pem = std::fs::read_to_string(&cert_path).map_err(|e| e.to_string())?;
            let key_pem = std::fs::read_to_string(&key_path).map_err(|e| e.to_string())?;
            let cert_der = std::fs::read(&der_path).map_err(|e| e.to_string())?;
            let fingerprint = fingerprint_of(&cert_der);
            return Ok(Identity { device_id, cert_der, cert_pem, key_pem, fingerprint });
        }
        let ck = rcgen::generate_simple_self_signed(vec!["dropgrab.local".to_string()])
            .map_err(|e| e.to_string())?;
        let cert_pem = ck.cert.pem();
        let key_pem = ck.key_pair.serialize_pem();
        let cert_der = ck.cert.der().to_vec();
        std::fs::write(&cert_path, &cert_pem).map_err(|e| e.to_string())?;
        std::fs::write(&der_path, &cert_der).map_err(|e| e.to_string())?;
        std::fs::write(&key_path, &key_pem).map_err(|e| e.to_string())?;
        restrict_permissions(&key_path);
        let fingerprint = fingerprint_of(&cert_der);
        Ok(Identity { device_id, cert_der, cert_pem, key_pem, fingerprint })
    }
}

#[cfg(unix)]
fn restrict_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_permissions(_path: &Path) {}

/// Accepts a server certificate only when its SHA-256 equals the expected fingerprint.
#[derive(Debug)]
struct PinVerifier {
    expected: String,
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl ServerCertVerifier for PinVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _server_name: &ServerName<'_>,
        _ocsp: &[u8],
        _now: UnixTime,
    ) -> Result<ServerCertVerified, rustls::Error> {
        if fingerprint_of(end_entity.as_ref()) == self.expected {
            Ok(ServerCertVerified::assertion())
        } else {
            Err(rustls::Error::General("TLS_MISMATCH: certificate fingerprint differs".into()))
        }
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

/// Accepts any client certificate that the client can prove it holds the key for. The TLS
/// handshake verifies the CertificateVerify signature, so the presenter owns the certificate.
/// Whether that certificate belongs to the claimed sender is checked by the handlers
/// (`check_claimed_fingerprint`), not here, because a fingerprint is only known after the request.
/// Client auth is requested but optional: `info` and pairing must work before any trust exists.
#[derive(Debug)]
pub struct PeerCertVerifier {
    provider: Arc<rustls::crypto::CryptoProvider>,
}

impl PeerCertVerifier {
    pub fn new() -> PeerCertVerifier {
        PeerCertVerifier { provider: Arc::new(rustls::crypto::ring::default_provider()) }
    }
}

impl ClientCertVerifier for PeerCertVerifier {
    fn offer_client_auth(&self) -> bool {
        true
    }

    fn client_auth_mandatory(&self) -> bool {
        false
    }

    fn root_hint_subjects(&self) -> &[DistinguishedName] {
        &[]
    }

    fn verify_client_cert(
        &self,
        _end_entity: &CertificateDer<'_>,
        _intermediates: &[CertificateDer<'_>],
        _now: UnixTime,
    ) -> Result<ClientCertVerified, rustls::Error> {
        Ok(ClientCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &CertificateDer<'_>,
        dss: &DigitallySignedStruct,
    ) -> Result<HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(message, cert, dss, &self.provider.signature_verification_algorithms)
    }

    fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
        self.provider.signature_verification_algorithms.supported_schemes()
    }
}

/// Server TLS config for the peer listener: this device's certificate, and client certificates
/// requested (see `PeerCertVerifier`).
pub fn peer_server_config(identity: &Identity) -> Result<Arc<rustls::ServerConfig>, String> {
    install_crypto_provider();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let certs = vec![CertificateDer::from(identity.cert_der.clone())];
    let key = PrivateKeyDer::from_pem_slice(identity.key_pem.as_bytes()).map_err(|e| e.to_string())?;
    let mut config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| e.to_string())?
        .with_client_cert_verifier(Arc::new(PeerCertVerifier::new()))
        .with_single_cert(certs, key)
        .map_err(|e| e.to_string())?;
    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(Arc::new(config))
}

/// Builds a `reqwest` client that only talks to a peer presenting `fingerprint`.
/// It presents no client certificate, so it is only for endpoints that do not identify the
/// caller (web share, `info`). Peer protocol calls use `mtls_client`.
pub fn pinned_client(fingerprint: &str, timeout: std::time::Duration) -> Result<reqwest::Client, String> {
    build_pinned(fingerprint, None, timeout)
}

/// Like `pinned_client`, and also presents this install's certificate (mutual TLS). The peer
/// checks that this certificate matches the sender fingerprint it is given in the request.
pub fn mtls_client(identity: &Identity, fingerprint: &str, timeout: std::time::Duration) -> Result<reqwest::Client, String> {
    build_pinned(fingerprint, Some(identity), timeout)
}

fn build_pinned(fingerprint: &str, identity: Option<&Identity>, timeout: std::time::Duration) -> Result<reqwest::Client, String> {
    install_crypto_provider();
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let verifier = PinVerifier { expected: fingerprint.to_string(), provider: provider.clone() };
    let builder = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13])
        .map_err(|e| e.to_string())?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier));
    let config = match identity {
        None => builder.with_no_client_auth(),
        Some(id) => {
            let certs = vec![CertificateDer::from(id.cert_der.clone())];
            let key = PrivateKeyDer::from_pem_slice(id.key_pem.as_bytes()).map_err(|e| e.to_string())?;
            builder.with_client_auth_cert(certs, key).map_err(|e| e.to_string())?
        }
    };
    reqwest::Client::builder()
        .use_preconfigured_tls(config)
        .timeout(timeout)
        .connect_timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_code_is_symmetric_and_four_digits() {
        let a = "aa".repeat(32);
        let b = "bb".repeat(32);
        let c1 = verify_code(&a, &b);
        let c2 = verify_code(&b, &a);
        assert_eq!(c1, c2);
        assert_eq!(c1.len(), 4);
        assert!(c1.bytes().all(|x| x.is_ascii_digit()));
    }

    #[test]
    fn identity_is_stable_across_loads() {
        let dir = tempfile::tempdir().unwrap();
        let db = Db::open_memory().unwrap();
        let first = Identity::load_or_create(dir.path(), &db).unwrap();
        let second = Identity::load_or_create(dir.path(), &db).unwrap();
        assert_eq!(first.device_id, second.device_id);
        assert_eq!(first.fingerprint, second.fingerprint);
        assert_eq!(first.fingerprint.len(), 64);
    }
}
