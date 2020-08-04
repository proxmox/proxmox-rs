//! Implementation of TOTP, U2F and other mechanisms.

use std::convert::TryFrom;
use std::fmt;
use std::time::{Duration, SystemTime};

use anyhow::{anyhow, bail, Error};
use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::sign::Signer;
use percent_encoding::{percent_decode, percent_encode};
use serde::{Serialize, Serializer};

/// Algorithms supported by the TOTP. This is simply an enum limited to the most common
/// available implementations.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Algorithm {
    Sha1,
    Sha256,
    Sha512,
}

impl Into<MessageDigest> for Algorithm {
    fn into(self) -> MessageDigest {
        match self {
            Algorithm::Sha1 => MessageDigest::sha1(),
            Algorithm::Sha256 => MessageDigest::sha256(),
            Algorithm::Sha512 => MessageDigest::sha512(),
        }
    }
}

/// Displayed in a way compatible with the `otpauth` URI specification.
impl fmt::Display for Algorithm {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Algorithm::Sha1 => write!(f, "SHA1"),
            Algorithm::Sha256 => write!(f, "SHA256"),
            Algorithm::Sha512 => write!(f, "SHA512"),
        }
    }
}

/// Parsed in a way compatible with the `otpauth` URI specification.
impl std::str::FromStr for Algorithm {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Error> {
        Ok(match s {
            "SHA1" => Algorithm::Sha1,
            "SHA256" => Algorithm::Sha256,
            "SHA512" => Algorithm::Sha512,
            _ => bail!("unsupported algorithm: {}", s),
        })
    }
}

/// OTP secret builder.
#[derive(Clone, Debug, Eq, PartialEq)]
#[repr(transparent)]
pub struct TotpBuilder {
    inner: Totp,
}

impl From<Totp> for TotpBuilder {
    #[inline]
    fn from(inner: Totp) -> Self {
        Self { inner }
    }
}

impl TotpBuilder {
    pub fn secret(mut self, secret: Vec<u8>) -> Self {
        self.inner.secret = secret;
        self
    }

    /// Set the requested number of decimal digits.
    pub fn digits(mut self, digits: u8) -> Self {
        self.inner.digits = digits;
        self
    }

    /// Set the algorithm.
    pub fn algorithm(mut self, algorithm: Algorithm) -> Self {
        self.inner.algorithm = algorithm;
        self
    }

    /// Set the issuer.
    pub fn issuer(mut self, issuer: String) -> Self {
        self.inner.issuer = Some(issuer);
        self
    }

    /// Set the account name. This is required to create an URI.
    pub fn account_name(mut self, account_name: String) -> Self {
        self.inner.account_name = Some(account_name);
        self
    }

    /// Set the duration, in seconds, for which a value is valid.
    ///
    /// Panics if `seconds` is 0.
    pub fn step(mut self, seconds: usize) -> Self {
        if seconds == 0 {
            panic!("zero as 'step' value is invalid");
        }

        self.inner.step = seconds;
        self
    }

    /// Finalize the OTP instance.
    pub fn build(self) -> Totp {
        self.inner
    }
}

/// OTP secret key to produce OTP values with and the desired default number of decimal digits to
/// use for its values (defaults to 6).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Totp {
    /// The secret shared with the client.
    secret: Vec<u8>,

    /// The requested number decimal digits.
    digits: u8,

    /// The algorithm, defaults to sha1.
    algorithm: Algorithm,

    /// The duration, in seconds, for which a value is valid. Defaults to 30 seconds.
    step: usize,

    /// An optional issuer. To help users identify their TOTP settings.
    issuer: Option<String>,

    /// An optional account name, possibly chosen by the user, to identify their TOTP settings.
    account_name: Option<String>,
}

impl Totp {
    /// Allow modifying parameters by turning this into a builder.
    pub fn into_builder(self) -> TotpBuilder {
        self.into()
    }

    /// Duplicate the value into a new builder to modify parameters.
    pub fn to_builder(&self) -> TotpBuilder {
        self.clone().into()
    }

    /// Create a new empty OTP instance with default values and a predefined secret key.
    pub fn empty() -> Self {
        Self {
            secret: Vec::new(),
            digits: 6,
            algorithm: Algorithm::Sha1,
            step: 30,
            issuer: None,
            account_name: None,
        }
    }

    /// Create an OTP builder prefilled with default values.
    pub fn builder() -> TotpBuilder {
        TotpBuilder {
            inner: Self::empty(),
        }
    }

    /// Create a new OTP secret key builder using a secret specified in hexadecimal bytes.
    pub fn builder_from_hex(secret: &str) -> Result<TotpBuilder, Error> {
        crate::tools::hex_to_bin(secret).map(|secret| Self::builder().secret(secret))
    }

    /// Get the secret key in binary form.
    pub fn secret(&self) -> &[u8] {
        &self.secret
    }

    /// Get the used algorithm.
    pub fn algorithm(&self) -> Algorithm {
        self.algorithm
    }

    /// Get the step duration.
    pub fn step(&self) -> Duration {
        Duration::from_secs(self.step as u64)
    }

    /// Get the issuer, if any.
    pub fn issuer(&self) -> Option<&str> {
        self.issuer.as_ref().map(|s| s.as_str())
    }

    /// Get the account name, if any.
    pub fn account_name(&self) -> Option<&str> {
        self.account_name.as_ref().map(|s| s.as_str())
    }

    /// Raw signing function.
    fn sign(&self, input_data: &[u8]) -> Result<TotpValue, Error> {
        let secret = PKey::hmac(&self.secret)
            .map_err(|err| anyhow!("error instantiating hmac key: {}", err))?;

        let mut signer = Signer::new(self.algorithm.into(), &secret)
            .map_err(|err| anyhow!("error instantiating hmac signer: {}", err))?;

        signer
            .update(input_data)
            .map_err(|err| anyhow!("error calculating hmac (error in update): {}", err))?;

        let hmac = signer
            .sign_to_vec()
            .map_err(|err| anyhow!("error calculating hmac (error in sign): {}", err))?;

        let byte_offset = usize::from(
            hmac.last()
                .ok_or_else(|| anyhow!("error calculating hmac (too short)"))?
                & 0xF,
        );

        let value = u32::from_be_bytes(
            TryFrom::try_from(
                hmac.get(byte_offset..(byte_offset + 4))
                    .ok_or_else(|| anyhow!("error calculating hmac (too short)"))?,
            )
            .unwrap(),
        ) & 0x7fffffff;

        Ok(TotpValue {
            value,
            digits: u32::from(self.digits),
        })
    }

    /// Create a HOTP value for a counter.
    ///
    /// This is currently private as for actual counter mode we should have a validate helper
    /// which forces handling of too-low-but-within-range values explicitly!
    fn counter(&self, count: u64) -> Result<TotpValue, Error> {
        self.sign(&count.to_be_bytes())
    }

    /// Convert a time stamp into a counter value. This makes it easier and cheaper to check a
    /// range of values.
    fn time_to_counter(&self, time: SystemTime) -> Result<u64, Error> {
        match time.duration_since(SystemTime::UNIX_EPOCH) {
            Ok(epoch) => Ok(epoch.as_secs() / (self.step as u64)),
            Err(_) => bail!("refusing to create otp value for negative time"),
        }
    }

    /// Create a TOTP value for a time stamp.
    pub fn time(&self, time: SystemTime) -> Result<TotpValue, Error> {
        self.counter(self.time_to_counter(time)?)
    }

    /// Verify a time value within a range.
    ///
    /// This will iterate through `steps` and check if the provided `time + step * step_size`
    /// matches. If a match is found, the matching step will be returned.
    pub fn verify(
        &self,
        digits: &str,
        time: SystemTime,
        steps: std::ops::RangeInclusive<isize>,
    ) -> Result<Option<isize>, Error> {
        let count = self.time_to_counter(time)? as i64;
        for step in steps {
            if self.counter((count + step as i64) as u64)? == digits {
                return Ok(Some(step));
            }
        }
        Ok(None)
    }

    /// Create an otpauth URI for this configuration.
    pub fn to_uri(&self) -> Result<String, Error> {
        use std::fmt::Write;

        let mut out = String::new();

        write!(out, "otpauth://totp/")?;

        let account_name = match &self.account_name {
            Some(account_name) => account_name,
            None => bail!("cannot create otpauth uri without an account name"),
        };

        let issuer = match &self.issuer {
            Some(issuer) => {
                let issuer = percent_encode(issuer.as_bytes(), percent_encoding::NON_ALPHANUMERIC)
                    .to_string();
                write!(out, "{}:", issuer)?;
                Some(issuer)
            }
            None => None,
        };

        write!(
            out,
            "{}?secret={}",
            percent_encode(account_name.as_bytes(), percent_encoding::NON_ALPHANUMERIC),
            base32::encode(base32::Alphabet::RFC4648 { padding: false }, &self.secret),
        )?;
        write!(out, "&digits={}", self.digits)?;
        write!(out, "&algorithm={}", self.algorithm)?;
        write!(out, "&step={}", self.step)?;

        if let Some(issuer) = issuer {
            write!(out, "&issuer={}", issuer)?;
        }

        Ok(out)
    }
}

impl std::str::FromStr for Totp {
    type Err = Error;

    fn from_str(uri: &str) -> Result<Self, Error> {
        if !uri.starts_with("otpauth://totp/") {
            bail!("not an otpauth uri");
        }

        let uri = &uri.as_bytes()[15..];
        let qmark = uri
            .iter()
            .position(|&b| b == b'?')
            .ok_or_else(|| anyhow!("missing '?' in otp uri"))?;

        let account = &uri[..qmark];
        let uri = &uri[(qmark + 1)..];

        // FIXME: Also split on "%3A" / "%3a"
        let mut account = account.splitn(2, |&b| b == b':');
        let first_part = percent_decode(
            &account
                .next()
                .ok_or_else(|| anyhow!("missing account in otpauth uri"))?,
        )
        .decode_utf8_lossy()
        .into_owned();

        let mut totp = Totp::empty();

        match account.next() {
            Some(account_name) => {
                totp.issuer = Some(first_part);
                totp.account_name =
                    Some(percent_decode(account_name).decode_utf8_lossy().to_string());
            }
            None => totp.account_name = Some(first_part),
        }

        for parts in uri.split(|&b| b == b'&') {
            let mut parts = parts.splitn(2, |&b| b == b'=');
            let key = percent_decode(
                &parts
                    .next()
                    .ok_or_else(|| anyhow!("bad key in otpauth uri"))?,
            )
            .decode_utf8()?;
            let value = percent_decode(
                &parts
                    .next()
                    .ok_or_else(|| anyhow!("bad value in otpauth uri"))?,
            );

            match &*key {
                "secret" => {
                    totp.secret = base32::decode(
                        base32::Alphabet::RFC4648 { padding: false },
                        &value.decode_utf8()?,
                    )
                    .ok_or_else(|| anyhow!("failed to decode otp secret in otpauth url"))?
                }
                "digits" => totp.digits = value.decode_utf8()?.parse()?,
                "algorithm" => totp.algorithm = value.decode_utf8()?.parse()?,
                "step" => totp.step = value.decode_utf8()?.parse()?,
                "issuer" => totp.issuer = Some(value.decode_utf8_lossy().into_owned()),
                _other => bail!("unrecognized otpauth uri parameter: {}", key),
            }
        }

        if totp.secret.is_empty() {
            bail!("missing secret in otpauth url");
        }

        Ok(totp)
    }
}

crate::forward_deserialize_to_from_str!(Totp);

impl Serialize for Totp {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::Error;

        serializer.serialize_str(
            &self
                .to_uri()
                .map_err(|err| Error::custom(err.to_string()))?,
        )
    }
}

/// A HOTP value with a decimal digit limit.
#[derive(Clone, Copy, Debug)]
pub struct TotpValue {
    value: u32,
    digits: u32,
}

impl TotpValue {
    /// Change the number of decimal digits used for this HOTP value.
    pub fn digits(self, digits: u32) -> Self {
        Self { digits, ..self }
    }

    /// Get the raw integer value before truncation.
    pub fn raw(&self) -> u32 {
        self.value
    }

    /// Get the integer value truncated to the requested number of decimal digits.
    pub fn value(&self) -> u32 {
        self.value % 10u32.pow(self.digits)
    }
}

impl fmt::Display for TotpValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{0:0width$}",
            self.value(),
            width = (self.digits as usize)
        )
    }
}

impl PartialEq<u32> for TotpValue {
    fn eq(&self, other: &u32) -> bool {
        self.value() == *other
    }
}

/// For convenience we allow directly comparing with a string. This will make sure the string has
/// the exact number of digits while parsing it explicitly as a decimal string.
impl PartialEq<&str> for TotpValue {
    fn eq(&self, other: &&str) -> bool {
        // Since we use `from_str_radix` with a radix of 10 explicitly, we can check the number of
        // bytes against the number of digits.
        if other.as_bytes().len() != (self.digits as usize) {
            return false;
        }

        match u32::from_str_radix(*other, 10) {
            Ok(value) => self.value() == value,
            Err(_) => false,
        }
    }
}

#[test]
fn test_otp() {
    // Validated via:
    // ```sh
    // $ oathtool --hotp -c1 87259aa6550f059bca8c
    // 337037
    // ```
    const SECRET_1: &str = "87259aa6550f059bca8c";
    const EXPECTED_1: &str = "337037";
    const EXPECTED_2: &str = "296746";
    const EXPECTED_3: &str = "251167";
    const EXPECTED_4_D8: &str = "11899249";

    let hotp = Totp::builder_from_hex(SECRET_1)
        .expect("failed to create Totp key")
        .digits(6)
        .build();
    assert_eq!(
        hotp.counter(1).expect("failed to create hotp value"),
        EXPECTED_1,
    );
    assert_eq!(
        hotp.counter(2)
            .expect("failed to create hotp value")
            .digits(6),
        EXPECTED_2,
    );
    assert_eq!(
        hotp.counter(3)
            .expect("failed to create hotp value")
            .digits(6),
        EXPECTED_3,
    );
    assert_eq!(
        hotp.counter(4)
            .expect("failed to create hotp value")
            .digits(8),
        EXPECTED_4_D8,
    );

    let hotp = hotp
        .into_builder()
        .account_name("My Account".to_string())
        .build();
    let uri = hotp.to_uri().expect("failed to create otpauth uri");
    let parsed: Totp = uri.parse().expect("failed to parse otp uri");
    assert_eq!(parsed, hotp);
    assert_eq!(parsed.issuer, None);
    assert_eq!(
        parsed.account_name.as_ref().map(String::as_str),
        Some("My Account")
    );

    const SECRET_2: &str = "a60b1b20679b1a64e21a";
    const EXPECTED: &str = "7757717";
    // Validated via:
    // ```sh
    // $ oathtool --totp -d7 -s30 --now='2020-08-04 15:14:23 UTC' a60b1b20679b1a64e21a
    // 7757717
    // $ date -d'2020-08-04 15:14:23 UTC' +%s
    // 1596554063
    // ```
    //
    let totp = Totp::builder_from_hex(SECRET_2)
        .expect("failed to create Totp key")
        .build();
    assert_eq!(
        totp.time(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1596554063))
            .expect("failed to create totp value")
            .digits(7),
        EXPECTED,
    );

    let totp = totp
        .into_builder()
        .account_name("The Account Name".to_string())
        .issuer("An Issuer".to_string())
        .build();
    let uri = totp.to_uri().expect("failed to create otpauth uri");
    let parsed: Totp = uri.parse().expect("failed to parse otp uri");
    assert_eq!(parsed, totp);
    assert_eq!(
        parsed.issuer.as_ref().map(String::as_str),
        Some("An Issuer")
    );
    assert_eq!(
        parsed.account_name.as_ref().map(String::as_str),
        Some("The Account Name")
    );
}
