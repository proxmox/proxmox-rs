use std::io;

use anyhow::{Error, bail, format_err};
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
    if let Some(min_dsa) = weak_crypto.min_dsa_key_size
        && min_dsa <= 1024
    {
        policy.accept_asymmetric_algo(sequoia_openpgp::policy::AsymmetricAlgorithm::DSA1024);
    }
    if let Some(min_rsa) = weak_crypto.min_rsa_key_size
        && min_rsa <= 1024
    {
        policy.accept_asymmetric_algo(sequoia_openpgp::policy::AsymmetricAlgorithm::RSA1024);
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

#[cfg(test)]
mod tests {
    use super::{WeakCryptoConfig, verify_signature};
    use anyhow::Result;
    use sequoia_openpgp::packet::prelude::SignatureBuilder;
    use sequoia_openpgp::packet::signature::subpacket::NotationDataFlags;
    use sequoia_openpgp::serialize::MarshalInto;
    use sequoia_openpgp::types::{HashAlgorithm, SignatureType};
    use sequoia_openpgp::{cert::prelude::*, policy::StandardPolicy, serialize::stream::*};
    use std::io::Write;

    const MESSAGE: &[u8] = b"Hello, pgp!";

    fn setup(
        name: &str,
        mail: &str,
        hash: Option<HashAlgorithm>,
        detached: bool,
    ) -> Result<(Cert, Vec<u8>)> {
        let mut policy = StandardPolicy::new();

        if let Some(h) = hash {
            policy.accept_hash(h);
        }

        let (cert, _sig) =
            CertBuilder::general_purpose(Some(format!("{name} <{mail}>"))).generate()?;

        let keypair = cert
            .keys()
            .secret()
            .with_policy(&policy, None)
            .supported()
            .alive()
            .revoked(false)
            .for_signing()
            .next()
            .unwrap()
            .key()
            .clone()
            .into_keypair()?;

        let mut sink = Vec::new();

        {
            let message = Signer::with_template(
                Message::new(&mut sink),
                keypair,
                SignatureBuilder::new(SignatureType::Text)
                    .add_notation(
                        mail,
                        name,
                        NotationDataFlags::empty().set_human_readable(),
                        false,
                    )?
                    .set_hash_algo(hash.unwrap_or(HashAlgorithm::SHA256)),
            )?
            .hash_algo(hash.unwrap_or(HashAlgorithm::SHA256))?;

            if detached {
                let mut message = message.detached().build()?;
                message.write_all(MESSAGE)?;
                message.finalize()?;
            } else {
                let mut message = LiteralWriter::new(message.build()?).build()?;
                message.write_all(MESSAGE)?;
                message.finalize()?;
            }
        }

        Ok((cert, sink))
    }

    fn root_cause_no_valid_sig(err: anyhow::Error) -> bool {
        err.root_cause()
            .to_string()
            .contains("No valid signature found.")
    }

    #[test]
    fn verify_attached_signature_success() -> Result<()> {
        // using same signature will work
        {
            let (cert, sink) = setup("Nicolas Frey", "n.frey@proxmox.com", None, false)?;
            let verified =
                verify_signature(&sink, &cert.to_vec()?, None, &WeakCryptoConfig::default())?;

            assert_eq!(verified, MESSAGE);
        }

        Ok(())
    }

    #[test]
    fn verify_attached_signature_fail() -> Result<()> {
        // using different signatures will fail
        {
            let (cert1, sink1) = setup("Nicolas Frey", "n.frey@proxmox.com", None, false)?;
            let (cert2, sink2) = setup("Proxmox Support Team", "support@proxmox.com", None, false)?;

            assert!(
                verify_signature(&sink1, &cert2.to_vec()?, None, &WeakCryptoConfig::default())
                    .is_err_and(root_cause_no_valid_sig)
            );
            assert!(
                verify_signature(&sink2, &cert1.to_vec()?, None, &WeakCryptoConfig::default())
                    .is_err_and(root_cause_no_valid_sig)
            );
        }

        Ok(())
    }

    #[test]
    fn verify_detached_signature_success() -> Result<()> {
        // using same signature will work
        {
            let (cert, sink) = setup("Nicolas Frey", "n.frey@proxmox.com", None, true)?;
            let verified = verify_signature(
                MESSAGE,
                &cert.to_vec()?,
                Some(&sink),
                &WeakCryptoConfig::default(),
            )?;
            assert_eq!(verified, MESSAGE);
        }

        Ok(())
    }

    #[test]
    fn verify_detached_signature_fail() -> Result<()> {
        // using different signatures will fail
        {
            let (cert1, sink1) = setup("Nicolas Frey", "n.frey@proxmox.com", None, true)?;
            let (cert2, sink2) = setup("Proxmox Support Team", "support@proxmox.com", None, true)?;

            assert!(
                verify_signature(
                    MESSAGE,
                    &cert2.to_vec()?,
                    Some(&sink1),
                    &WeakCryptoConfig::default()
                )
                .is_err_and(root_cause_no_valid_sig)
            );

            assert!(
                verify_signature(
                    MESSAGE,
                    &cert1.to_vec()?,
                    Some(&sink2),
                    &WeakCryptoConfig::default()
                )
                .is_err_and(root_cause_no_valid_sig)
            );
        }

        Ok(())
    }

    #[test]
    fn weak_crypto_config_allow_sha1() -> Result<()> {
        let (cert, sink) = setup(
            "Nicolas Frey",
            "n.frey@proxmox.com",
            Some(HashAlgorithm::SHA1),
            false,
        )?;

        // allowing sha1 will make the policy accept this signature
        {
            let verified = verify_signature(
                &sink,
                &cert.to_vec()?,
                None,
                &WeakCryptoConfig {
                    allow_sha1: true,
                    ..Default::default()
                },
            )?;
            assert_eq!(verified, MESSAGE);
        }

        // while this will fail
        {
            assert!(
                verify_signature(&sink, &cert.to_vec()?, None, &WeakCryptoConfig::default())
                    .is_err_and(root_cause_no_valid_sig)
            );
        }

        Ok(())
    }
}
