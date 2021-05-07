//! Certificate utility methods for convenience (such as CSR generation).

use std::collections::HashMap;

use openssl::hash::MessageDigest;
use openssl::nid::Nid;
use openssl::pkey::PKey;
use openssl::rsa::Rsa;
use openssl::x509::{X509Extension, X509Name, X509Req};

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
        ext.push(X509Extension::new_nid(
            None,
            None,
            Nid::BASIC_CONSTRAINTS,
            "CA:FALSE",
        )?)?;
        ext.push(X509Extension::new_nid(
            None,
            None,
            Nid::KEY_USAGE,
            "digitalSignature,keyEncipherment",
        )?)?;
        ext.push(X509Extension::new_nid(
            None,
            None,
            Nid::EXT_KEY_USAGE,
            "serverAuth,clientAuth",
        )?)?;
        ext.push(X509Extension::new_nid(
            None,
            Some(&context),
            Nid::SUBJECT_ALT_NAME,
            &identifiers
                .iter()
                .try_fold(String::new(), |mut acc, dns| {
                    if !acc.is_empty() {
                        acc.push(',');
                    }
                    use std::fmt::Write;
                    write!(acc, "DNS:{}", dns.as_ref())?;
                    Ok::<_, std::fmt::Error>(acc)
                })
                .map_err(|err| Error::Csr(err.to_string()))?,
        )?)?;
        csr.add_extensions(&ext)?;

        csr.sign(&private_key, MessageDigest::sha256())?;

        Ok(Self {
            data: csr.build().to_der()?,
            private_key_pem,
        })
    }
}
