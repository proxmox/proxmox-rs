//! Auth key handling.

use anyhow::{bail, format_err, Error};
use openssl::hash::MessageDigest;
use openssl::pkey::{HasPublic, Id, PKey, PKeyRef, Private, Public};
use openssl::rsa::Rsa;
use openssl::sign::{Signer, Verifier};

/// A private auth key used for API ticket signing and verification.
#[derive(Clone)]
pub struct PrivateKey {
    pub(crate) key: PKey<Private>,
}

/// A private auth key used for API ticket verification.
#[derive(Clone)]
pub struct PublicKey {
    pub(crate) key: PKey<Public>,
}

impl PrivateKey {
    /// Generate a new RSA auth key.
    pub fn generate_rsa() -> Result<Self, Error> {
        let rsa =
            Rsa::generate(4096).map_err(|err| format_err!("failed to generate rsa key - {err}"))?;
        Ok(Self {
            key: PKey::from_rsa(rsa)
                .map_err(|err| format_err!("failed to get PKey for rsa key - {err}"))?,
        })
    }

    /// Generate a new EC auth key.
    pub fn generate_ec() -> Result<Self, Error> {
        Ok(Self {
            key: PKey::generate_ed25519()
                .map_err(|err| format_err!("failed to generate EC PKey - {err}"))?,
        })
    }

    pub fn from_pem(data: &[u8]) -> Result<Self, Error> {
        let key = PKey::private_key_from_pem(data)
            .map_err(|err| format_err!("failed to decode private key from PEM - {err}"))?;
        Ok(Self { key })
    }

    /// Get the PEM formatted private key *unencrypted*.
    pub fn private_key_to_pem(&self) -> Result<Vec<u8>, Error> {
        // No PKCS#8 for legacy reasons:
        if let Ok(rsa) = self.key.rsa() {
            return rsa
                .private_key_to_pem()
                .map_err(|err| format_err!("failed to encode rsa private key as PEM - {err}"));
        }

        if self.key.id() == Id::ED25519 {
            return self
                .key
                .private_key_to_pem_pkcs8()
                .map_err(|err| format_err!("failed to encode ec private key as PEM - {err}"));
        }

        bail!("unexpected key data")
    }

    /// Get the PEM formatted public key.
    pub fn public_key_to_pem(&self) -> Result<Vec<u8>, Error> {
        // No PKCS#8 for legacy reasons:
        if let Ok(rsa) = self.key.rsa() {
            return rsa
                .public_key_to_pem()
                .map_err(|err| format_err!("failed to encode rsa public key as PEM - {err}"));
        }

        if self.key.id() == Id::ED25519 {
            return self
                .key
                .public_key_to_pem()
                .map_err(|err| format_err!("failed to encode ec public key as PEM - {err}"));
        }

        bail!("unexpected key data")
    }

    /// Get the public key.
    pub fn public_key(&self) -> Result<PublicKey, Error> {
        PublicKey::from_pem(&self.public_key_to_pem()?)
    }

    fn sign(&self, digest: MessageDigest, data: &[u8]) -> Result<Vec<u8>, Error> {
        let mut signer = if self.key.id() == Id::ED25519 {
            // ed25519 does not support signing with digest
            Signer::new_without_digest(&self.key)
        } else {
            Signer::new(digest, &self.key)
        }
        .map_err(|e| format_err!("could not create private key signer - {e}"))?;

        signer
            .sign_oneshot_to_vec(data)
            .map_err(|e| format_err!("could not sign with private key - {e}"))
    }
}

impl From<PKey<Private>> for PrivateKey {
    fn from(key: PKey<Private>) -> Self {
        Self { key }
    }
}

impl PublicKey {
    pub fn from_pem(data: &[u8]) -> Result<Self, Error> {
        let key = PKey::public_key_from_pem(data)
            .map_err(|err| format_err!("failed to decode public key from PEM - {err}"))?;
        Ok(Self { key })
    }

    /// Get the PEM formatted public key.
    pub fn public_key_to_pem(&self) -> Result<Vec<u8>, Error> {
        // No PKCS#8 for legacy reasons:
        if let Ok(rsa) = self.key.rsa() {
            return rsa
                .public_key_to_pem()
                .map_err(|err| format_err!("failed to encode rsa public key as PEM - {err}"));
        }

        if self.key.id() == Id::ED25519 {
            return self
                .key
                .public_key_to_pem()
                .map_err(|err| format_err!("failed to encode ec public key as PEM - {err}"));
        }

        bail!("unexpected key data")
    }
}

impl From<PKey<Public>> for PublicKey {
    fn from(key: PKey<Public>) -> Self {
        Self { key }
    }
}

#[derive(Clone)]
pub struct HMACKey {
    key: PKey<Private>,
}

impl HMACKey {
    fn from_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Ok(Self {
            key: PKey::hmac(bytes)
                .map_err(|err| format_err!("failed to create hmac key from bytes - {err}"))?,
        })
    }

    pub fn from_base64(string: &str) -> Result<Self, Error> {
        let bytes = base64::decode_config(string, base64::STANDARD_NO_PAD)
            .map_err(|e| format_err!("could not decode base64 hmac key - {e}"))?;

        Self::from_bytes(&bytes)
    }

    pub fn generate() -> Result<Self, Error> {
        // 8*64 = 512 bit security
        let mut bytes = [0u8; 64];
        openssl::rand::rand_bytes(&mut bytes)
            .map_err(|err| format_err!("failed to generate random bytes for hmac key - {err}"))?;

        Self::from_bytes(&bytes)
    }

    pub fn sign(&self, digest: MessageDigest, data: &[u8]) -> Result<Vec<u8>, Error> {
        let mut signer = Signer::new(digest, &self.key)
            .map_err(|e| format_err!("failed to create hmac signer - {e}"))?;

        signer
            .sign_oneshot_to_vec(data)
            .map_err(|e| format_err!("failed to sign to vec using hmac - {e}"))
    }

    pub fn verify(
        &self,
        digest: MessageDigest,
        signature: &[u8],
        data: &[u8],
    ) -> Result<bool, Error> {
        let digest = self.sign(digest, data).map_err(|e| {
            format_err!("failed to verify, could not create comparison signature - {e}")
        })?;

        if signature.len() == digest.len() && openssl::memcmp::eq(signature, &digest) {
            return Ok(true);
        }

        Ok(false)
    }

    /// This outputs the hmac key *without* any encryption just encoded as base64.
    pub fn to_base64(&self) -> Result<String, Error> {
        let bytes = self
            .key
            .raw_private_key()
            .map_err(|e| format_err!("could not get key as raw bytes - {e}"))?;

        Ok(base64::encode_config(bytes, base64::STANDARD_NO_PAD))
    }

    // This is needed for legacy CSRF token verifyication.
    //
    // TODO: remove once all dependent products had a major version release (PBS)
    #[cfg(feature = "api")]
    pub(crate) fn as_bytes(&self) -> Result<Vec<u8>, Error> {
        // workaround to get access to the the bytes behind the key.
        self.key
            .raw_private_key()
            .map_err(|e| format_err!("could not get raw bytes of HMAC key - {e}"))
    }
}

enum SigningKey {
    Private(PrivateKey),
    Hmac(HMACKey),
}

enum VerificationKey {
    Public(PublicKey),
    Hmac(HMACKey),
}

/// A key ring for authentication.
///
/// This can hold one active signing key for new tickets (either an HMAC secret or an asymmetric
/// key), and optionally multiple public keys and HMAC secrets for verifying them in order to
/// support key rollover.
pub struct Keyring {
    signing_key: Option<SigningKey>,
    public_keys: Vec<VerificationKey>,
}

impl Default for Keyring {
    fn default() -> Self {
        Self::new()
    }
}

impl Keyring {
    pub fn generate_new_rsa() -> Result<Self, Error> {
        PrivateKey::generate_rsa().map(Self::with_private_key)
    }

    pub fn generate_new_ec() -> Result<Self, Error> {
        PrivateKey::generate_ec().map(Self::with_private_key)
    }

    pub fn generate_new_hmac() -> Result<Self, Error> {
        HMACKey::generate().map(Self::with_hmac_key)
    }

    pub fn new() -> Self {
        Self {
            signing_key: None,
            public_keys: Vec::new(),
        }
    }

    pub fn with_public_key(key: PublicKey) -> Self {
        Self {
            signing_key: None,
            public_keys: vec![VerificationKey::Public(key)],
        }
    }

    pub fn with_private_key(key: PrivateKey) -> Self {
        Self {
            signing_key: Some(SigningKey::Private(key)),
            public_keys: Vec::new(),
        }
    }

    pub fn with_hmac_key(key: HMACKey) -> Self {
        Self {
            signing_key: Some(SigningKey::Hmac(key)),
            public_keys: Vec::new(),
        }
    }

    pub fn add_public_key(&mut self, key: PublicKey) {
        self.public_keys.push(VerificationKey::Public(key));
    }

    pub fn add_hmac_key(&mut self, key: HMACKey) {
        self.public_keys.push(VerificationKey::Hmac(key));
    }

    pub fn verify(
        &self,
        digest: MessageDigest,
        signature: &[u8],
        data: &[u8],
    ) -> Result<bool, Error> {
        fn verify_with<P: HasPublic>(
            key: &PKeyRef<P>,
            digest: MessageDigest,
            signature: &[u8],
            data: &[u8],
        ) -> Result<bool, Error> {
            let mut verifier = if key.id() == Id::ED25519 {
                // ed25519 does not support digests
                Verifier::new_without_digest(key)
            } else {
                Verifier::new(digest, key)
            }
            .map_err(|err| format_err!("failed to create openssl verifier - {err}"))?;

            verifier
                .verify_oneshot(signature, data)
                .map_err(|err| format_err!("openssl error verifying data - {err}"))
        }

        if let Some(key) = &self.signing_key {
            match key {
                SigningKey::Private(key) if verify_with(&key.key, digest, signature, data)? => {
                    return Ok(true)
                }
                SigningKey::Hmac(key) if key.verify(digest, signature, data)? => return Ok(true),
                _ => (),
            }
        }

        for key in &self.public_keys {
            match key {
                VerificationKey::Public(key) if verify_with(&key.key, digest, signature, data)? => {
                    return Ok(true)
                }
                VerificationKey::Hmac(key) if key.verify(digest, signature, data)? => {
                    return Ok(true)
                }
                _ => (),
            }
        }

        Ok(false)
    }

    pub(crate) fn sign(&self, digest: MessageDigest, data: &[u8]) -> Result<Vec<u8>, Error> {
        let signing_key = self
            .signing_key
            .as_ref()
            .ok_or_else(|| format_err!("no private key available for signing"))?;

        match signing_key {
            SigningKey::Private(key) => key.sign(digest, data),
            SigningKey::Hmac(key) => key.sign(digest, data),
        }
    }
}
