//! U2F implementation.

use std::mem::MaybeUninit;
use std::io;

use anyhow::{bail, format_err, Error};
use openssl::ec::{EcGroup, EcKey, EcPoint};
use openssl::ecdsa::EcdsaSig;
use openssl::pkey::Public;
use openssl::sha;
use openssl::x509::X509;
use serde::{Deserialize, Serialize};

const CHALLENGE_LEN: usize = 32;
const U2F_VERSION: &str = "U2F_V2";

/// The "key" part of a registration, passed to `u2f.sign` in the registered keys list.
///
/// Part of the U2F API, therefore `camelCase` and base64url without padding.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisteredKey {
    /// Identifies the key handle on the client side. Used to create authentication challenges, so
    /// the client knows which key to use. Must be remembered.
    #[serde(with = "bytes_as_base64url_nopad")]
    pub key_handle: Vec<u8>,

    pub version: String,
}

/// Data we get when a u2f token responds to a registration challenge.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub struct Registration {
    /// The key consisting of key handle and version, which can be passed to the registered-keys
    /// list in `u2f.sign` in the browser.
    pub key: RegisteredKey,

    /// Public part of the client key identified via the `key_handle`. Required to verify future
    /// authentication responses. Must be remembered.
    #[serde(with = "bytes_as_base64")]
    pub public_key: Vec<u8>,

    /// Attestation certificate (in DER format) from which we originally copied the `key_handle`.
    /// Not necessary for authentication, unless the hardware tokens should be restricted to
    /// specific provider identities. Optional.
    #[serde(
        with = "bytes_as_base64",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub certificate: Vec<u8>,
}

/// Result from a successful authentication. The client's hardware token will inform us about the
/// user-presence (it may have been configured to respond automatically instead of requiring user
/// interaction), and the number of authentications the key has performed.
/// We probably won't make much use of this.
#[derive(Clone, Debug)]
pub struct Authentication {
    /// `true` if the user had to be present.
    pub user_present: bool,

    /// authentication count
    pub counter: usize,
}

/// The hardware replies with a client data json object containing some information - this is the
/// subset we actually make use of.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientData {
    /// The challenge the device responded to. This should be compared against the server side
    /// cached challenge!
    challenge: String,

    /// The origin the the browser told the device the challenge was coming from.
    origin: String,
}

/// A registration challenge to be sent to the `u2f.register` function in the browser.
///
/// Part of the U2F API, therefore `camelCase`.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationChallenge {
    pub challenge: String,
    pub version: String,
    pub app_id: String,
}

/// The response we get from a successful call to the `u2f.register` function in the browser.
///
/// Part of the U2F API, therefore `camelCase`.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationResponse {
    registration_data: String,
    client_data: String,
    version: String,
}

/// Authentication challenge data to be sent to the `u2f.sign` function in the browser. Does not
/// include the registered keys.
///
/// Part of the U2F API, therefore `camelCase`.
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthChallenge {
    pub challenge: String,
    pub app_id: String,
}

/// The response we get from a successful call to the `u2f.sign` function in the browser.
///
/// Part of the U2F API, therefore `camelCase` and base64url without padding.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponse {
    #[serde(with = "bytes_as_base64url_nopad")]
    pub key_handle: Vec<u8>,
    pub client_data: String,
    pub signature_data: String,
}

impl AuthResponse {
    pub fn key_handle(&self) -> &[u8] {
        &self.key_handle
    }
}

/// A u2f context to create or verify challenges with.
#[derive(Deserialize, Serialize)]
pub struct U2f {
    app_id: String,
    origin: String,
}

impl U2f {
    /// Create a new U2F context consisting of an appid and origin.
    pub fn new(app_id: String, origin: String) -> Self {
        Self { app_id, origin }
    }

    /// Get a challenge object which can be directly passed to `u2f.register` on the browser side.
    pub fn registration_challenge(&self) -> Result<RegistrationChallenge, Error> {
        Ok(RegistrationChallenge {
            challenge: challenge()?,
            version: U2F_VERSION.to_owned(),
            app_id: self.app_id.clone(),
        })
    }

    /// Convenience method to verify the json formatted response object string.
    pub fn registration_verify(
        &self,
        challenge: &str,
        response: &str,
    ) -> Result<Option<Registration>, Error> {
        let response: RegistrationResponse = serde_json::from_str(response)
            .map_err(|err| format_err!("error parsing response: {}", err))?;
        self.registration_verify_obj(challenge, response)
    }

    /// Verifies the registration response object.
    pub fn registration_verify_obj(
        &self,
        challenge: &str,
        response: RegistrationResponse,
    ) -> Result<Option<Registration>, Error> {
        let client_data_decoded = decode(&response.client_data)
            .map_err(|err| format_err!("error decoding client data in response: {}", err))?;

        let client_data: ClientData = serde_json::from_reader(&mut &client_data_decoded[..])
            .map_err(|err| format_err!("error parsing client data: {}", err))?;

        if client_data.challenge != challenge {
            bail!("registration challenge did not match");
        }

        if client_data.origin != self.origin {
            bail!(
                "origin in client registration did not match: {:?} != {:?}",
                client_data.origin,
                self.origin,
            );
        }

        let registration_data = decode(&response.registration_data)
            .map_err(|err| format_err!("error decoding registration data in response: {}", err))?;

        let registration_data = RegistrationResponseData::from_raw(&registration_data)?;

        let mut digest = sha::Sha256::new();
        digest.update(&[0u8]);
        digest.update(&sha::sha256(self.app_id.as_bytes()));
        digest.update(&sha::sha256(&client_data_decoded));
        digest.update(registration_data.key_handle);
        digest.update(registration_data.public_key);
        let digest = digest.finish();

        let signature = EcdsaSig::from_der(registration_data.signature)
            .map_err(|err| format_err!("error decoding signature in response: {}", err))?;

        // can we decode the public key?
        drop(decode_public_key(registration_data.public_key)?);

        match signature.verify(&digest, &registration_data.cert_key) {
            Ok(true) => Ok(Some(Registration {
                key: RegisteredKey {
                    key_handle: registration_data.key_handle.to_vec(),
                    version: response.version,
                },
                public_key: registration_data.public_key.to_vec(),
                certificate: registration_data.certificate.to_vec(),
            })),
            Ok(false) => Ok(None),
            Err(err) => bail!("openssl error while verifying signature: {}", err),
        }
    }

    /// Get a challenge object which can be directly passwd to `u2f.sign` on the browser side.
    pub fn auth_challenge(&self) -> Result<AuthChallenge, Error> {
        Ok(AuthChallenge {
            challenge: challenge()?,
            app_id: self.app_id.clone(),
        })
    }

    /// Convenience method to verify the json formatted response object string.
    pub fn auth_verify(
        &self,
        public_key: &[u8],
        challenge: &str,
        response: &str,
    ) -> Result<Option<Authentication>, Error> {
        let response: AuthResponse = serde_json::from_str(response)
            .map_err(|err| format_err!("error parsing response: {}", err))?;
        self.auth_verify_obj(public_key, challenge, response)
    }

    /// Verifies the authentication response object.
    pub fn auth_verify_obj(
        &self,
        public_key: &[u8],
        challenge: &str,
        response: AuthResponse,
    ) -> Result<Option<Authentication>, Error> {
        let client_data_decoded = decode(&response.client_data)
            .map_err(|err| format_err!("error decoding client data in response: {}", err))?;

        let client_data: ClientData = serde_json::from_reader(&mut &client_data_decoded[..])
            .map_err(|err| format_err!("error parsing client data: {}", err))?;

        if client_data.challenge != challenge {
            bail!("authentication challenge did not match");
        }

        if client_data.origin != self.origin {
            bail!(
                "origin in client authentication did not match: {:?} != {:?}",
                client_data.origin,
                self.origin,
            );
        }

        let signature_data = decode(&response.signature_data)
            .map_err(|err| format_err!("error decoding signature data in response: {}", err))?;

        // an ecdsa signature is much longer than 16 bytes but we only need to parse the first 5
        // anyway...
        if signature_data.len() < 1 + 4 + 16 {
            bail!("invalid signature data");
        }

        let presence_and_counter_bytes = &signature_data[0..5];
        let user_present = presence_and_counter_bytes[0] != 0;
        let counter_bytes = &presence_and_counter_bytes[1..];
        let counter: u32 =
            u32::from_be(unsafe { std::ptr::read_unaligned(counter_bytes.as_ptr() as *const u32) });
        let signature = EcdsaSig::from_der(&signature_data[5..])
            .map_err(|err| format_err!("error decoding signature in response: {}", err))?;

        let public_key = decode_public_key(public_key)?;

        let mut digest = sha::Sha256::new();
        digest.update(&sha::sha256(self.app_id.as_bytes()));
        digest.update(presence_and_counter_bytes);
        digest.update(&sha::sha256(&client_data_decoded));
        let digest = digest.finish();

        match signature.verify(&digest, &public_key) {
            Ok(true) => Ok(Some(Authentication {
                user_present,
                counter: counter as usize,
            })),
            Ok(false) => Ok(None),
            Err(err) => bail!("openssl error while verifying signature: {}", err),
        }
    }
}

/// base64url encoding
fn encode(data: &[u8]) -> String {
    let mut out = base64::encode_config(data, base64::URL_SAFE_NO_PAD);
    while out.ends_with('=') {
        out.pop();
    }
    out
}

/// base64url decoding
fn decode(data: &str) -> Result<Vec<u8>, Error> {
    Ok(base64::decode_config(data, base64::URL_SAFE_NO_PAD)?)
}

/// produce a challenge, which is just a bunch of random data
fn challenge() -> Result<String, Error> {
    let mut data = MaybeUninit::<[u8; CHALLENGE_LEN]>::uninit();
    let data = unsafe {
        let buf: &mut [u8; CHALLENGE_LEN] = &mut *data.as_mut_ptr();
        let rc = libc::getrandom(buf.as_mut_ptr() as *mut libc::c_void, buf.len(), 0);
        if rc == -1 {
            return Err(io::Error::last_os_error().into());
        }
        if rc as usize != buf.len() {
            // `CHALLENGE_LEN` is small, so short reads cannot happen (see `getrandom(2)`)
            bail!("short getrandom call");
        }
        data.assume_init()
    };
    Ok(encode(&data))
}

/// Used while parsing the binary registration response. The slices point directly into the
/// original response byte data, and the public key is extracted from the contained X509
/// certificate.
#[derive(Debug)]
pub struct RegistrationResponseData<'a> {
    public_key: &'a [u8],
    key_handle: &'a [u8],
    certificate: &'a [u8],
    signature: &'a [u8],

    /// The client's public key in decoded and parsed form.
    cert_key: EcKey<Public>,
}

impl<'a> RegistrationResponseData<'a> {
    /// Parse the binary registration data into its parts and extract the certificate's public key.
    ///
    /// See https://fidoalliance.org/specs/fido-u2f-v1.2-ps-20170411/fido-u2f-raw-message-formats-v1.2-ps-20170411.html
    pub fn from_raw(data: &'a [u8]) -> Result<Self, Error> {
        // [ 0x05 | 65b pubkey | 1b keyhandle len | keyhandle | certificate (1 DER obj) | signature ]

        if data.len() <= (1 + 65 + 1 + 71) {
            bail!("registration data too short");
        }

        if data[0] != 0x05 {
            bail!(
                "invalid registration data, reserved byte is 0x{:02x}, expected 0x05",
                data[0]
            );
        }

        let public_key = &data[1..66];

        let key_handle_len = usize::from(data[66]);
        let data = &data[67..];

        if data.len() <= key_handle_len + 71 {
            bail!("registration data invalid too short");
        }

        let key_handle = &data[..key_handle_len];
        let data = &data[key_handle_len..];
        if data[0] != 0x30 {
            bail!("error decoding X509 certificate: not a SEQUENCE tag");
        }
        let cert_len = der_length(&data[1..])? + 1; // plus the tag!
        let certificate = &data[..cert_len];
        let x509 = X509::from_der(certificate)
            .map_err(|err| format_err!("error decoding X509 certificate: {}", err))?;
        let signature = &data[cert_len..];

        Ok(Self {
            public_key,
            key_handle,
            certificate,
            signature,
            cert_key: x509.public_key()?.ec_key()?,
        })
    }
}

/// Decode the raw 65 byte ec public key into an `openssl::EcKey<Public>`.
fn decode_public_key(data: &[u8]) -> Result<EcKey<Public>, Error> {
    if data.len() != 65 {
        bail!("invalid public key length {}, expected 65", data.len());
    }

    let group = EcGroup::from_curve_name(openssl::nid::Nid::X9_62_PRIME256V1)
        .map_err(|err| format_err!("openssl error, failed to instantiate ec curve: {}", err))?;

    let mut bn = openssl::bn::BigNumContext::new().map_err(|err| {
        format_err!(
            "openssl error, failed to instantiate bignum context: {}",
            err
        )
    })?;

    let point = EcPoint::from_bytes(&group, data, &mut bn)
        .map_err(|err| format_err!("failed to decode public key point: {}", err))?;

    let key = EcKey::from_public_key(&group, &point)
        .map_err(|err| format_err!("failed to instantiate public key: {}", err))?;

    key.check_key()
        .map_err(|err| format_err!("public key failed self check: {}", err))?;

    Ok(key)
}

/// The only DER thing we need: lengths.
///
/// Returns the length *including* the size of the length itself.
fn der_length(data: &[u8]) -> Result<usize, Error> {
    if data[0] == 0 {
        bail!("error decoding X509 certificate: bad length (0)");
    }

    if data[0] < 0x80 {
        return Ok(usize::from(data[0]) + 1);
    }

    let count = usize::from(data[0] & 0x7F);
    if count == 0x7F {
        // X.609; 8.1.3.5, the value `1111111` shall not be used
        bail!("error decoding X509 certificate: illegal length value");
    }

    if count == 0 {
        // "indefinite" form not allowed in DER
        bail!("error decoding X509 certificate: illegal length form");
    }

    if count > std::mem::size_of::<usize>() {
        bail!("error decoding X509 certificate: unsupported length");
    }

    if count > (data.len() - 1) {
        bail!("error decoding X509 certificate: truncated length data");
    }

    let mut len = 0;
    for i in 0..count {
        len = (len << 8) | usize::from(data[1 + i]);
    }

    Ok(len + count + 1)
}

#[cfg(test)]
mod test {
    // The test data in here is generated with a yubi key...

    use serde::Deserialize;

    const TEST_APPID: &str = "https://u2ftest.enonet.errno.eu";

    const TEST_REGISTRATION_JSON: &str =
        "{\"challenge\":\"mZoWLngnAh8p98nPkFOIBXecd0CbmgEx5tEd5jNswgY\",\"response\":{\"client\
        Data\":\"eyJjaGFsbGVuZ2UiOiJtWm9XTG5nbkFoOHA5OG5Qa0ZPSUJYZWNkMENibWdFeDV0RWQ1ak5zd2dZI\
        iwib3JpZ2luIjoiaHR0cHM6Ly91MmZ0ZXN0LmVub25ldC5lcnJuby5ldSIsInR5cCI6Im5hdmlnYXRvci5pZC5\
        maW5pc2hFbnJvbGxtZW50In0\",\"registrationData\":\"BQR_9TmMowVeoAHp3ABljCa90eNG87t76D4W\
        c9nsmK9ihNhhYNxYIq9tnRUPTBZ2X4kZKSB0LXMm32lOKQlNB56QQHlt81cRBfID7BvHk_XIJZc5ks5D3R1ZV1\
        1fJudp3F-ii_KSdZaFb4cGaq0rEaVDfNR2ZR0T0ApMMCeTIaDAJRQwggJEMIIBLqADAgECAgRVYr6gMAsGCSqG\
        SIb3DQEBCzAuMSwwKgYDVQQDEyNZdWJpY28gVTJGIFJvb3QgQ0EgU2VyaWFsIDQ1NzIwMDYzMTAgFw0xNDA4MD\
        EwMDAwMDBaGA8yMDUwMDkwNDAwMDAwMFowKjEoMCYGA1UEAwwfWXViaWNvIFUyRiBFRSBTZXJpYWwgMTQzMjUz\
        NDY4ODBZMBMGByqGSM49AgEGCCqGSM49AwEHA0IABEszH3c9gUS5mVy-RYVRfhdYOqR2I2lcvoWsSCyAGfLJuU\
        Z64EWw5m8TGy6jJDyR_aYC4xjz_F2NKnq65yvRQwmjOzA5MCIGCSsGAQQBgsQKAgQVMS4zLjYuMS40LjEuNDE0\
        ODIuMS41MBMGCysGAQQBguUcAgEBBAQDAgUgMAsGCSqGSIb3DQEBCwOCAQEArBbZs262s6m3bXWUs09Z9Pc-28\
        n96yk162tFHKv0HSXT5xYU10cmBMpypXjjI-23YARoXwXn0bm-BdtulED6xc_JMqbK-uhSmXcu2wJ4ICA81BQd\
        PutvaizpnjlXgDJjq6uNbsSAp98IStLLp7fW13yUw-vAsWb5YFfK9f46Yx6iakM3YqNvvs9M9EUJYl_VrxBJqn\
        yLx2iaZlnpr13o8NcsKIJRdMUOBqt_ageQg3ttsyq_3LyoNcu7CQ7x8NmeCGm_6eVnZMQjDmwFdymwEN4OxfnM\
        5MkcKCYhjqgIGruWkVHsFnJa8qjZXneVvKoiepuUQyDEJ2GcqvhU2YKY1zBFAiEA2mcfAS2XRcWy1lLJikFHGJ\
        SbtOrrwswjOKEzwp6EonkCIFBxbLAmwUnblAWOVELASi610ZfPK-7qx2VwkWfHqnll\",\"version\":\"U2F\
        _V2\"}}";

    const TEST_AUTH_JSON: &str =
        "{\"challenge\":\"8LE_-7Rd1vB3Otn3vJ7GyiwRQtYPMv-BWliCejH0d4Y\",\"response\":{\"clientD\
        ata\":\"eyJjaGFsbGVuZ2UiOiI4TEVfLTdSZDF2QjNPdG4zdko3R3lpd1JRdFlQTXYtQldsaUNlakgwZDRZIiw\
        ib3JpZ2luIjoiaHR0cHM6Ly91MmZ0ZXN0LmVub25ldC5lcnJuby5ldSIsInR5cCI6Im5hdmlnYXRvci5pZC5nZX\
        RBc3NlcnRpb24ifQ\",\"keyHandle\":\"eW3zVxEF8gPsG8eT9cgllzmSzkPdHVlXXV8m52ncX6KL8pJ1loVv\
        hwZqrSsRpUN81HZlHRPQCkwwJ5MhoMAlFA\",\"signatureData\":\"AQAAAQEwRAIgKdM9cmCLZDxntY-dT_\
        OXbcVA1D5ewQunXVC-CYZ65pUCIAIOUBsu-dOmTym0ITZt6x75BFUSGlqYRuH5JKBcyO3M\"},\"user\":{\"c\
        ertificate\":\"MIICRDCCAS6gAwIBAgIEVWK+oDALBgkqhkiG9w0BAQswLjEsMCoGA1UEAxMjWXViaWNvIFUy\
        RiBSb290IENBIFNlcmlhbCA0NTcyMDA2MzEwIBcNMTQwODAxMDAwMDAwWhgPMjA1MDA5MDQwMDAwMDBaMCoxKDA\
        mBgNVBAMMH1l1YmljbyBVMkYgRUUgU2VyaWFsIDE0MzI1MzQ2ODgwWTATBgcqhkjOPQIBBggqhkjOPQMBBwNCAA\
        RLMx93PYFEuZlcvkWFUX4XWDqkdiNpXL6FrEgsgBnyyblGeuBFsOZvExsuoyQ8kf2mAuMY8/xdjSp6uucr0UMJo\
        zswOTAiBgkrBgEEAYLECgIEFTEuMy42LjEuNC4xLjQxNDgyLjEuNTATBgsrBgEEAYLlHAIBAQQEAwIFIDALBgkq\
        hkiG9w0BAQsDggEBAKwW2bNutrOpt211lLNPWfT3PtvJ/espNetrRRyr9B0l0+cWFNdHJgTKcqV44yPtt2AEaF8\
        F59G5vgXbbpRA+sXPyTKmyvroUpl3LtsCeCAgPNQUHT7rb2os6Z45V4AyY6urjW7EgKffCErSy6e31td8lMPrwL\
        Fm+WBXyvX+OmMeompDN2Kjb77PTPRFCWJf1a8QSap8i8dommZZ6a9d6PDXLCiCUXTFDgarf2oHkIN7bbMqv9y8q\
        DXLuwkO8fDZnghpv+nlZ2TEIw5sBXcpsBDeDsX5zOTJHCgmIY6oCBq7lpFR7BZyWvKo2V53lbyqInqblEMgxCdh\
        nKr4VNmCmNc=\",\"key\":{\"keyHandle\":\"eW3zVxEF8gPsG8eT9cgllzmSzkPdHVlXXV8m52ncX6KL8pJ\
        1loVvhwZqrSsRpUN81HZlHRPQCkwwJ5MhoMAlFA\",\"version\":\"U2F_V2\"},\"public-key\":\"BH/1\
        OYyjBV6gAencAGWMJr3R40bzu3voPhZz2eyYr2KE2GFg3Fgir22dFQ9MFnZfiRkpIHQtcybfaU4pCU0HnpA=\"}\
        }";

    #[test]
    fn test_registration() {
        let data = TEST_REGISTRATION_JSON;

        #[derive(Deserialize)]
        struct TestChallenge {
            challenge: String,
            response: super::RegistrationResponse,
        }

        let ts: TestChallenge =
            serde_json::from_str(&data).expect("failed to parse json test data");

        let context = super::U2f::new(TEST_APPID.to_string(), TEST_APPID.to_string());
        let res = context
            .registration_verify_obj(&ts.challenge, ts.response)
            .expect("error trying to verify registration");
        assert!(
            res.is_some(),
            "test registration signature fails verification"
        );
    }

    #[test]
    fn test_authentication() {
        let data = TEST_AUTH_JSON;

        #[derive(Deserialize)]
        struct TestChallenge {
            challenge: String,
            user: super::Registration,
            response: super::AuthResponse,
        }

        let ts: TestChallenge =
            serde_json::from_str(&data).expect("failed to parse json test data");

        let context = super::U2f::new(TEST_APPID.to_string(), TEST_APPID.to_string());
        let res = context
            .auth_verify_obj(&ts.user.public_key, &ts.challenge, ts.response)
            .expect("error trying to verify authentication");
        assert!(
            res.is_some(),
            "test authentication signature fails verification"
        );
    }
}

mod bytes_as_base64 {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&base64::encode(&data))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        use serde::de::Error;
        String::deserialize(deserializer).and_then(|string| {
            base64::decode(&string).map_err(|err| Error::custom(err.to_string()))
        })
    }
}

mod bytes_as_base64url_nopad {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(data: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&base64::encode_config(
            data.as_ref(),
            base64::URL_SAFE_NO_PAD,
        ))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        use serde::de::Error;
        String::deserialize(deserializer).and_then(|string| {
            base64::decode_config(&string, base64::URL_SAFE_NO_PAD)
                .map_err(|err| Error::custom(err.to_string()))
        })
    }
}
