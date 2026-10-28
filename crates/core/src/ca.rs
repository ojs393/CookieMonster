
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use rcgen::{
    BasicConstraints, Certificate, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair,
    KeyUsagePurpose, SanType,
};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use rustls::ServerConfig;
use thiserror::Error;

pub const CA_DIR_NAME: &str = "local-ca";
pub const CA_CERT_FILE: &str = "cookie-monster-ca.cer";
pub const CA_KEY_FILE: &str = "cookie-monster-ca.key.pem";

#[derive(Debug, Error)]
pub enum CaError {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),
    #[error("certificate error: {0}")]
    Cert(String),
    #[error("TLS config error: {0}")]
    Tls(String),
}

pub struct LocalCa {
    pub dir: PathBuf,
    pub cert_pem: String,
    pub cert_der: Vec<u8>,
    ca_cert: Certificate,
    key_pair: KeyPair,
}

impl LocalCa {
    pub fn load_or_create(data_dir: &Path) -> Result<Self, CaError> {
        let dir = data_dir.join(CA_DIR_NAME);
        fs::create_dir_all(&dir)?;
        let cert_path = dir.join(CA_CERT_FILE);
        let key_path = dir.join(CA_KEY_FILE);

        if cert_path.exists() && key_path.exists() {
            let cert_pem = fs::read_to_string(&cert_path)?;
            let key_pem = fs::read_to_string(&key_path)?;
            let key_pair = KeyPair::from_pem(&key_pem)
                .map_err(|e| CaError::Cert(format!("load key: {e}")))?;
            let params = CertificateParams::from_ca_cert_pem(&cert_pem)
                .map_err(|e| CaError::Cert(format!("load ca params: {e}")))?;
            let ca_cert = params
                .self_signed(&key_pair)
                .map_err(|e| CaError::Cert(format!("rehydrate ca: {e}")))?;
            let cert_der = pem_to_der(&cert_pem)?;
            return Ok(Self {
                dir,
                cert_pem,
                cert_der,
                ca_cert,
                key_pair,
            });
        }

        let mut params = CertificateParams::new(Vec::<String>::new())
            .map_err(|e| CaError::Cert(e.to_string()))?;
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![
            KeyUsagePurpose::DigitalSignature,
            KeyUsagePurpose::KeyCertSign,
            KeyUsagePurpose::CrlSign,
        ];
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, "Cookie Monster Local CA");
        dn.push(DnType::OrganizationName, "Cookie Monster");
        params.distinguished_name = dn;

        let key_pair = KeyPair::generate().map_err(|e| CaError::Cert(e.to_string()))?;
        let ca_cert = params
            .self_signed(&key_pair)
            .map_err(|e| CaError::Cert(e.to_string()))?;
        let cert_pem = ca_cert.pem();
        let key_pem = key_pair.serialize_pem();
        fs::write(&cert_path, &cert_pem)?;
        fs::write(&key_path, &key_pem)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600));
        }
        let cert_der = ca_cert.der().to_vec();
        Ok(Self {
            dir,
            cert_pem,
            cert_der,
            ca_cert,
            key_pair,
        })
    }

    pub fn cert_path(&self) -> PathBuf {
        self.dir.join(CA_CERT_FILE)
    }

    pub fn server_config_for_host(&self, host: &str) -> Result<Arc<ServerConfig>, CaError> {
        let mut params = CertificateParams::new(vec![host.to_string()])
            .map_err(|e| CaError::Cert(e.to_string()))?;
        params.subject_alt_names.push(SanType::DnsName(
            host.try_into()
                .map_err(|e| CaError::Cert(format!("SAN: {e}")))?,
        ));
        let mut dn = DistinguishedName::new();
        dn.push(DnType::CommonName, host);
        params.distinguished_name = dn;
        params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
        params.extended_key_usages = vec![rcgen::ExtendedKeyUsagePurpose::ServerAuth];

        let leaf_key = KeyPair::generate().map_err(|e| CaError::Cert(e.to_string()))?;
        let leaf = params
            .signed_by(&leaf_key, &self.ca_cert, &self.key_pair)
            .map_err(|e| CaError::Cert(format!("sign leaf: {e}")))?;

        let cert_der = CertificateDer::from(leaf.der().to_vec());
        let key_der =
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(leaf_key.serialize_der()));

        let mut cfg = ServerConfig::builder()
            .with_no_client_auth()
            .with_single_cert(vec![cert_der], key_der)
            .map_err(|e| CaError::Tls(e.to_string()))?;
        cfg.alpn_protocols = vec![b"http/1.1".to_vec()];
        Ok(Arc::new(cfg))
    }
}

fn pem_to_der(pem: &str) -> Result<Vec<u8>, CaError> {
    let mut cursor = io::Cursor::new(pem.as_bytes());
    let items = rustls_pemfile::certs(&mut cursor)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| CaError::Cert(format!("pem: {e}")))?;
    items
        .into_iter()
        .next()
        .map(|c| c.to_vec())
        .ok_or_else(|| CaError::Cert("no cert in pem".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn create_and_reload_issues_leaf() {
        let dir = tempdir().unwrap();
        let ca = LocalCa::load_or_create(dir.path()).unwrap();
        assert!(ca.cert_path().exists());
        let cfg = ca.server_config_for_host("example.com").unwrap();
        assert!(!cfg.alpn_protocols.is_empty());

        let ca2 = LocalCa::load_or_create(dir.path()).unwrap();
        let _ = ca2.server_config_for_host("api.example.com").unwrap();
        assert_eq!(ca.cert_pem, ca2.cert_pem);
    }
}
