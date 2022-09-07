use anyhow::{bail, Error};

use openssl::{hash::MessageDigest, pkey::Public};
use serde::{Deserialize, Serialize};

use crate::SubscriptionInfo;

#[derive(PartialEq, Eq, Hash, Serialize, Deserialize)]
/// Input for offline key signing requests
pub struct ServerBlob {
    /// Server ID generated with [[crate::get_hardware_address()]].
    pub serverid: String,
    /// Subscription key
    pub key: String,
}

/// Common abstraction for signing operations returning [String]-representation of signature.
pub trait Signer {
    fn sign(&self, data: &[u8]) -> Result<String, Error>;
}

impl Signer for openssl::pkey::PKey<openssl::pkey::Private> {
    fn sign(&self, data: &[u8]) -> Result<String, Error> {
        use openssl::pkey;

        // see Signer docs - different algorithms require different constructors
        let mut signer = match self.id() {
            pkey::Id::ED25519 | pkey::Id::ED448 => {
                openssl::sign::Signer::new_without_digest(self.as_ref())?
            }
            pkey::Id::RSA | pkey::Id::EC => {
                openssl::sign::Signer::new(MessageDigest::sha512(), self.as_ref())?
            }
            id => bail!("Unsupported key type '{id:?}'"),
        };

        Ok(hex::encode(signer.sign_oneshot_to_vec(data)?))
    }
}

/// Common verifier abstraction for signatures created by [Signer]s.
pub trait Verifier {
    fn verify(&self, data: &[u8], signature: &str) -> Result<(), Error>;
}

impl Verifier for openssl::pkey::PKey<Public> {
    fn verify(&self, data: &[u8], signature: &str) -> Result<(), Error> {
        use openssl::pkey;

        // see Verifier docs - different algorithms require different constructors
        let mut verifier = match self.id() {
            pkey::Id::ED25519 | pkey::Id::ED448 => {
                openssl::sign::Verifier::new_without_digest(self.as_ref())?
            }
            pkey::Id::RSA | pkey::Id::EC => {
                openssl::sign::Verifier::new(MessageDigest::sha512(), self.as_ref())?
            }
            id => bail!("Unsupported key type '{id:?}'"),
        };
        if !verifier.verify_oneshot(&hex::decode(signature)?, data)? {
            bail!("Signature mismatch.");
        }

        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
/// A signed response to a signature request for offline keys.
pub struct SignedResponse {
    /// Signature of response
    pub signature: String,
    /// Payload (signed [SubscriptionInfo]s)
    pub blobs: Vec<SubscriptionInfo>,
}

impl SignedResponse {
    /// Verify outer signature (of server response)
    pub fn verify(self, key: &openssl::pkey::PKey<Public>) -> Result<Vec<SubscriptionInfo>, Error> {
        let canonical =
            proxmox_serde::json::to_canonical_json(&serde_json::to_value(&self.blobs)?)?;

        match key.verify(&canonical, &self.signature) {
            Ok(()) => Ok(self.blobs),
            Err(err) => bail!("Failed to verify response signature - {err}"),
        }
    }
}

#[derive(Serialize, Deserialize)]
/// A sign request for offline keys
pub struct SignRequest {
    /// Subscription key of `proxmox-offline-mirror` instance issuing this request (must be
    /// [crate::SubscriptionStatus::Active]).
    pub mirror_key: ServerBlob,
    /// Offline keys that should be signed by server.
    pub blobs: Vec<ServerBlob>,
}
