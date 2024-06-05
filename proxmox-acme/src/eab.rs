use openssl::hash::MessageDigest;
use openssl::pkey::{HasPrivate, PKeyRef};
use openssl::sign::Signer;

use serde::Serialize;

use crate::key::Jwk;
use crate::types::ExternalAccountBinding;
use crate::{b64u, Error};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct Protected {
    alg: &'static str,
    url: String,
    kid: String,
}

impl ExternalAccountBinding {
    /// Create a new instance
    pub fn new<P>(
        eab_kid: &str,
        eab_hmac_key: &PKeyRef<P>,
        jwk: Jwk,
        url: String,
    ) -> Result<Self, Error>
    where
        P: HasPrivate,
    {
        let protected = Protected {
            alg: "HS256",
            kid: eab_kid.to_string(),
            url,
        };
        let payload = b64u::encode(serde_json::to_string(&jwk)?.as_bytes());
        let protected_data = b64u::encode(serde_json::to_string(&protected)?.as_bytes());
        let signature = {
            let protected = protected_data.as_bytes();
            let payload = payload.as_bytes();
            Self::sign_hmac(eab_hmac_key, protected, payload)?
        };

        let signature = b64u::encode(&signature);
        Ok(ExternalAccountBinding {
            protected: protected_data,
            payload,
            signature,
        })
    }

    fn sign_hmac<P>(key: &PKeyRef<P>, protected: &[u8], payload: &[u8]) -> Result<Vec<u8>, Error>
    where
        P: HasPrivate,
    {
        let mut signer = Signer::new(MessageDigest::sha256(), key)?;
        signer.update(protected)?;
        signer.update(b".")?;
        signer.update(payload)?;
        Ok(signer.sign_to_vec()?)
    }
}
