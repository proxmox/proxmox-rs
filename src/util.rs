//! Certificate utility methods for convenience (such as CSR generation).

use std::collections::HashMap;

use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkey::PKey;
use openssl::rsa::Rsa;
use openssl::x509::{self, X509Name, X509Req};

use crate::Error;

/// A certificate signing request.
pub struct Csr {
    /// DER encoded certificate request.
    pub data: Vec<u8>,

    /// PEM formatted PKCS#8 private key.
    pub private_key_pem: Vec<u8>,
}

impl Csr {
    /// Generate a CSR in DER format with a PEM formatted PKCS8 private key.
    ///
    /// The `identifiers` should be a list of domains. The `attributes` should have standard names
    /// recognized by openssl.
    pub fn generate(
        identifiers: &[impl AsRef<str>],
        attributes: &HashMap<String, &str>,
    ) -> Result<Self, Error> {
        if identifiers.is_empty() {
            return Err(Error::Csr("cannot generate empty CSR".to_string()));
        }

        let private_key = Rsa::generate(4096)
            .and_then(PKey::from_rsa)
            .map_err(|err| Error::Ssl("failed to generate RSA key: {}", err))?;

        let private_key_pem = private_key
            .private_key_to_pem_pkcs8()
            .map_err(|err| Error::Ssl("failed to format private key as PEM pkcs8: {}", err))?;

        let mut name = X509Name::builder()?;
        if !attributes.contains_key("CN") {
            name.append_entry_by_nid(Nid::COMMONNAME, identifiers[0].as_ref())?;
        }
        for (key, value) in attributes {
            name.append_entry_by_text(key, value)?;
        }
        let name = name.build();

        let mut csr = X509Req::builder()?;
        csr.set_subject_name(&name)?;
        csr.set_pubkey(&private_key)?;

        let context = csr.x509v3_context(None);
        let mut ext = openssl::stack::Stack::new()?;
        ext.push(x509::extension::BasicConstraints::new().build()?)?;
        ext.push(
            x509::extension::KeyUsage::new()
                .digital_signature()
                .key_encipherment()
                .build()?,
        )?;
        ext.push(
            x509::extension::ExtendedKeyUsage::new()
                .server_auth()
                .client_auth()
                .build()?,
        )?;
        let mut san = x509::extension::SubjectAlternativeName::new();
        for dns in identifiers {
            san.dns(dns.as_ref());
        }
        ext.push({ san }.build(&context)?)?;
        csr.add_extensions(&ext)?;

        csr.sign(&private_key, MessageDigest::sha256())?;

        Ok(Self {
            data: csr.build().to_der()?,
            private_key_pem,
        })
    }
}
