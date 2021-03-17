use std::convert::TryFrom;

use openssl::hash::{Hasher, MessageDigest};
use openssl::pkey::{HasPrivate, PKeyRef};
use openssl::sign::Signer;
use serde::Serialize;

use crate::b64u;
use crate::key::{Jwk, PublicKey};
use crate::Error;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Protected {
    alg: &'static str,
    nonce: String,
    url: String,
    #[serde(flatten)]
    key: KeyId,
}

/// Acme requires to the use of *either* `jwk` *or* `kid` depending on the action taken.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum KeyId {
    /// This is the actual JWK structure.
    Jwk(Jwk),

    /// This should be the account location.
    Kid(String),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Jws {
    protected: String,
    payload: String,
    signature: String,
}

impl Jws {
    pub fn new<P, T>(
        key: &PKeyRef<P>,
        location: Option<String>,
        url: String,
        nonce: String,
        payload: &T,
    ) -> Result<Self, Error>
    where
        P: HasPrivate,
        T: Serialize,
    {
        Self::new_full(
            key,
            location,
            url,
            nonce,
            b64u::encode(serde_json::to_string(payload)?.as_bytes()),
        )
    }

    pub fn new_full<P: HasPrivate>(
        key: &PKeyRef<P>,
        location: Option<String>,
        url: String,
        nonce: String,
        payload: String,
    ) -> Result<Self, Error> {
        let jwk = Jwk::try_from(key)?;

        let pubkey = jwk.key.clone();
        let mut protected = Protected {
            alg: "",
            nonce,
            url,
            key: match location {
                Some(location) => KeyId::Kid(location),
                None => KeyId::Jwk(jwk),
            },
        };

        let (digest, ec_order_bytes): (MessageDigest, usize) = match &pubkey {
            PublicKey::Rsa(_) => (Self::prepare_rsa(key, &mut protected)?, 0),
            PublicKey::Ec(_) => Self::prepare_ec(key, &mut protected)?,
        };

        let protected_data = b64u::encode(serde_json::to_string(&protected)?.as_bytes());

        let signature = {
            let prot = protected_data.as_bytes();
            let payload = payload.as_bytes();
            match &pubkey {
                PublicKey::Rsa(_) => Self::sign_rsa(key, digest, prot, payload),
                PublicKey::Ec(_) => Self::sign_ec(key, digest, ec_order_bytes, prot, payload),
            }?
        };

        let signature = b64u::encode(&signature);

        Ok(Jws {
            protected: protected_data,
            payload,
            signature,
        })
    }

    fn prepare_rsa<P>(_key: &PKeyRef<P>, protected: &mut Protected) -> Result<MessageDigest, Error>
    where
        P: HasPrivate,
    {
        protected.alg = "RS256";
        Ok(MessageDigest::sha256())
    }

    /// Returns the digest and the size of the two signature components 'r' and 's'.
    fn prepare_ec<P>(
        _key: &PKeyRef<P>,
        protected: &mut Protected,
    ) -> Result<(MessageDigest, usize), Error>
    where
        P: HasPrivate,
    {
        // Note: if we support >256 bit keys we'll want to also support using ES512 here probably
        protected.alg = "ES256";
        //  'r' and 's' are each 256 bit numbers:
        Ok((MessageDigest::sha256(), 32))
    }

    fn sign_rsa<P>(
        key: &PKeyRef<P>,
        digest: MessageDigest,
        protected: &[u8],
        payload: &[u8],
    ) -> Result<Vec<u8>, Error>
    where
        P: HasPrivate,
    {
        let mut signer = Signer::new(digest, key)?;
        signer.set_rsa_padding(openssl::rsa::Padding::PKCS1)?;
        signer.update(protected)?;
        signer.update(b".")?;
        signer.update(payload)?;
        Ok(signer.sign_to_vec()?)
    }

    fn sign_ec<P>(
        key: &PKeyRef<P>,
        digest: MessageDigest,
        ec_order_bytes: usize,
        protected: &[u8],
        payload: &[u8],
    ) -> Result<Vec<u8>, Error>
    where
        P: HasPrivate,
    {
        let mut hasher = Hasher::new(digest)?;
        hasher.update(protected)?;
        hasher.update(b".")?;
        hasher.update(payload)?;
        let sig =
            openssl::ecdsa::EcdsaSig::sign(hasher.finish()?.as_ref(), key.ec_key()?.as_ref())?;
        let r = sig.r().to_vec();
        let s = sig.s().to_vec();
        let mut out = Vec::with_capacity(r.len() + s.len());
        out.extend(std::iter::repeat(0u8).take(ec_order_bytes - r.len()));
        out.extend(r);
        out.extend(std::iter::repeat(0u8).take(ec_order_bytes - s.len()));
        out.extend(s);
        Ok(out)
    }
}
