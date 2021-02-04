use std::convert::{TryFrom, TryInto};

use openssl::hash::{Hasher, MessageDigest};
use openssl::pkey::{HasPublic, Id, PKeyRef};
use serde::Serialize;

use crate::b64u;
use crate::Error;

/// An RSA public key.
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RsaPublicKey {
    #[serde(with = "b64u::bytes")]
    e: Vec<u8>,
    #[serde(with = "b64u::bytes")]
    n: Vec<u8>,
}

/// An EC public key.
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EcPublicKey {
    crv: &'static str,
    #[serde(with = "b64u::bytes")]
    x: Vec<u8>,
    #[serde(with = "b64u::bytes")]
    y: Vec<u8>,
}

/// A public key.
///
/// Internally tagged, so this already contains the 'kty' member.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kty")]
pub enum PublicKey {
    #[serde(rename = "RSA")]
    Rsa(RsaPublicKey),
    #[serde(rename = "EC")]
    Ec(EcPublicKey),
}

impl PublicKey {
    /// The thumbprint is the b64u encoded sha256sum of the *canonical* json representation.
    pub fn thumbprint(&self) -> Result<String, Error> {
        let mut hasher = Hasher::new(MessageDigest::sha256())?;
        crate::json::to_hash_canonical(&serde_json::to_value(self)?, &mut hasher)?;
        Ok(b64u::encode(hasher.finish()?.as_ref()))
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Jwk {
    #[serde(rename = "use", skip_serializing_if = "Option::is_none")]
    pub usage: Option<String>,

    /// The key data is internally tagged, we can just flatten it.
    #[serde(flatten)]
    pub key: PublicKey,
}

impl<P: HasPublic> TryFrom<&PKeyRef<P>> for Jwk {
    type Error = Error;

    fn try_from(key: &PKeyRef<P>) -> Result<Self, Self::Error> {
        Ok(Self {
            key: key.try_into()?,
            usage: None,
        })
    }
}

impl<P: HasPublic> TryFrom<&PKeyRef<P>> for PublicKey {
    type Error = Error;

    fn try_from(key: &PKeyRef<P>) -> Result<Self, Self::Error> {
        match key.id() {
            Id::RSA => Ok(PublicKey::Rsa(RsaPublicKey::try_from(&key.rsa()?)?)),
            Id::EC => Ok(PublicKey::Ec(EcPublicKey::try_from(&key.ec_key()?)?)),
            _ => Err(Error::UnsupportedKeyType),
        }
    }
}

impl<P: HasPublic> TryFrom<&openssl::rsa::Rsa<P>> for RsaPublicKey {
    type Error = Error;

    fn try_from(key: &openssl::rsa::Rsa<P>) -> Result<Self, Self::Error> {
        Ok(RsaPublicKey {
            e: key.e().to_vec(),
            n: key.n().to_vec(),
        })
    }
}

impl<P: HasPublic> TryFrom<&openssl::ec::EcKey<P>> for EcPublicKey {
    type Error = Error;

    fn try_from(key: &openssl::ec::EcKey<P>) -> Result<Self, Self::Error> {
        let group = key.group();

        if group.curve_name() != Some(openssl::nid::Nid::X9_62_PRIME256V1) {
            return Err(Error::UnsupportedGroup);
        }

        let mut ctx = openssl::bn::BigNumContext::new()?;
        let mut x = openssl::bn::BigNum::new()?;
        let mut y = openssl::bn::BigNum::new()?;
        let _: () = key
            .public_key()
            .affine_coordinates_gfp(group, &mut x, &mut y, &mut ctx)?;

        Ok(EcPublicKey {
            crv: "P-256",
            x: x.to_vec(),
            y: y.to_vec(),
        })
    }
}
