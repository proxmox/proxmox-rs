use std::io;

use anyhow::{bail, format_err, Error};
use sequoia_openpgp::cert::CertParser;
use sequoia_openpgp::parse::stream::{
    DetachedVerifierBuilder, MessageLayer, MessageStructure, VerificationError, VerificationHelper,
    VerifierBuilder,
};
use sequoia_openpgp::parse::{PacketParser, PacketParserResult, Parse};
use sequoia_openpgp::policy::StandardPolicy;
use sequoia_openpgp::types::HashAlgorithm;
use sequoia_openpgp::{Cert, KeyHandle};
use serde::{Deserialize, Serialize};

use proxmox_api_macro::api;
use proxmox_schema::Updater;

#[api(
    properties: {
        "allow-sha1": {
            type: bool,
            default: false,
            optional: true,
        },
        "min-dsa-key-size": {
            type: u64,
            optional: true,
        },
        "min-rsa-key-size": {
            type: u64,
            optional: true,
        },
    },
)]
#[derive(Default, Serialize, Deserialize, Updater, Clone, Debug)]
#[serde(rename_all = "kebab-case")]
/// Weak Cryptography Configuration
pub struct WeakCryptoConfig {
    /// Whether to allow SHA-1 based signatures
    #[serde(default)]
    pub allow_sha1: bool,
    /// Whether to lower the key size cutoff for DSA-based signatures
    #[serde(default)]
    pub min_dsa_key_size: Option<u64>,
    /// Whether to lower the key size cutoff for RSA-based signatures
    #[serde(default)]
    pub min_rsa_key_size: Option<u64>,
}

struct CertWrapper<'a> {
    cert: &'a Cert,
}

impl VerificationHelper for CertWrapper<'_> {
    fn get_certs(&mut self, _ids: &[KeyHandle]) -> sequoia_openpgp::Result<Vec<Cert>> {
        // Return public keys for signature verification here.
        Ok(vec![self.cert.clone()])
    }

    fn check(&mut self, structure: MessageStructure) -> sequoia_openpgp::Result<()> {
        // In this function, we implement our signature verification policy.

        // we don't want compression and/or encryption
        let layers: Vec<_> = structure.iter().collect();
        if layers.len() > 1 || layers.is_empty() {
            bail!(
                "Unexpected GPG message structure - expected plain signed data, got {} layers!",
                layers.len()
            );
        }
        let layer = &layers[0];
        let mut errors = Vec::new();

        let MessageLayer::SignatureGroup { results } = layer else {
            bail!("Unexpected message structure")
        };

        for result in results {
            if let Err(e) = result {
                errors.push(e);
            } else {
                // good signature, early return
                return Ok(());
            }
        }

        let mut context = String::new();
        let errlen = errors.len();

        if errlen > 1 {
            context.push_str(&format!("\nEncountered {errlen} errors:"));
        }

        for (n, err) in errors.iter().enumerate() {
            if errlen > 1 {
                context.push_str(&format!("\nSignature #{n}: {err}"));
            } else {
                context.push_str(&format!("\n{err}"));
            }
            match err {
                VerificationError::MalformedSignature { error, .. }
                | VerificationError::UnboundKey { error, .. }
                | VerificationError::BadKey { error, .. }
                | VerificationError::BadSignature { error, .. } => {
                    let mut cause = error.chain();
                    if cause.len() > 1 {
                        cause.next(); // already included in `err` above
                        context.push_str("Caused by:");
                        for (n, e) in cause.enumerate() {
                            context.push_str(&format!("\t{n}: {e}"));
                        }
                    }
                }
                VerificationError::MissingKey { .. }
                | VerificationError::UnknownSignature { .. } => {} // doesn't contain a cause
                _ => {} // we already print the error above in full
            };
        }

        Err(anyhow::anyhow!("No valid signature found.").context(context))
    }
}

/// Verifies GPG-signed `msg` was signed by `key`, returning the verified data without signature.
pub fn verify_signature(
    msg: &[u8],
    key: &[u8],
    detached_sig: Option<&[u8]>,
    weak_crypto: &WeakCryptoConfig,
) -> Result<Vec<u8>, Error> {
    let mut policy = StandardPolicy::new();
    if weak_crypto.allow_sha1 {
        policy.accept_hash(HashAlgorithm::SHA1);
    }
    if let Some(min_dsa) = weak_crypto.min_dsa_key_size {
        if min_dsa <= 1024 {
            policy.accept_asymmetric_algo(sequoia_openpgp::policy::AsymmetricAlgorithm::DSA1024);
        }
    }
    if let Some(min_rsa) = weak_crypto.min_rsa_key_size {
        if min_rsa <= 1024 {
            policy.accept_asymmetric_algo(sequoia_openpgp::policy::AsymmetricAlgorithm::RSA1024);
        }
    }

    let verifier = |cert| {
        let helper = CertWrapper { cert: &cert };

        if let Some(sig) = detached_sig {
            let mut verifier =
                DetachedVerifierBuilder::from_bytes(sig)?.with_policy(&policy, None, helper)?;
            verifier.verify_bytes(msg)?;
            Ok(msg.to_vec())
        } else {
            let mut verified = Vec::new();
            let mut verifier =
                VerifierBuilder::from_bytes(msg)?.with_policy(&policy, None, helper)?;
            let _bytes = io::copy(&mut verifier, &mut verified)?;
            if !verifier.message_processed() {
                bail!("Failed to verify message!");
            }
            Ok(verified)
        }
    };

    let mut packet_parser = PacketParser::from_bytes(key)?;

    // parse all packets to see whether this is a simple certificate or a keyring
    while let PacketParserResult::Some(pp) = packet_parser {
        packet_parser = pp.recurse()?.1;
    }

    if let PacketParserResult::EOF(eof) = packet_parser {
        // verify against a single certificate
        if eof.is_cert().is_ok() {
            let cert = Cert::from_bytes(key)?;
            return verifier(cert);
        // verify against a keyring
        } else if eof.is_keyring().is_ok() {
            let packet_parser = PacketParser::from_bytes(key)?;

            return CertParser::from(packet_parser)
                // flatten here as we ignore packets that aren't a certificate
                .flatten()
                // keep trying to verify the message until the first certificate that succeeds
                .find_map(|c| verifier(c).ok())
                // if no certificate verified the message, abort
                .ok_or_else(|| format_err!("No key in keyring could verify the message!"));
        }
    }

    // neither a keyring nor a certificate was detect, so we abort here
    bail!("'key-path' contains neither a keyring nor a certificate, aborting!");
}
