use std::io;

use anyhow::{format_err, Error};
use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::sign::Signer;
use serde::{Deserialize, Serialize};

fn getrandom(mut buffer: &mut [u8]) -> Result<(), io::Error> {
    while !buffer.is_empty() {
        let res = unsafe {
            libc::getrandom(
                buffer.as_mut_ptr() as *mut libc::c_void,
                buffer.len() as libc::size_t,
                0 as libc::c_uint,
            )
        };

        if res < 0 {
            return Err(io::Error::last_os_error());
        }

        buffer = &mut buffer[(res as usize)..];
    }

    Ok(())
}

/// Recovery entries. We use HMAC-SHA256 with a random secret as a salted hash replacement.
#[derive(Clone, Deserialize, Serialize)]
pub struct Recovery {
    /// "Salt" used for the key HMAC.
    secret: String,

    /// Recovery key entries are HMACs of the original data. When used up they will become `None`
    /// since the user is presented an enumerated list of codes, so we know the indices of used and
    /// unused codes.
    entries: Vec<Option<String>>,

    /// Creation timestamp as a unix epoch.
    pub created: i64,
}

impl Recovery {
    /// Generate recovery keys and return the recovery entry along with the original string
    /// entries.
    pub(super) fn generate() -> Result<(Self, Vec<String>), Error> {
        let mut secret = [0u8; 8];
        getrandom(&mut secret)?;

        let mut this = Self {
            secret: hex::encode(&secret).to_string(),
            entries: Vec::with_capacity(10),
            created: proxmox_time::epoch_i64(),
        };

        let mut original = Vec::new();

        let mut key_data = [0u8; 80]; // 10 keys of 12 bytes
        getrandom(&mut key_data)?;
        for b in key_data.chunks(8) {
            // unwrap: encoding hex bytes to fixed sized arrays
            let entry = format!(
                "{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}",
                b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
            );
            this.entries.push(Some(this.hash(entry.as_bytes())?));
            original.push(entry);
        }

        Ok((this, original))
    }

    /// Perform HMAC-SHA256 on the data and return the result as a hex string.
    fn hash(&self, data: &[u8]) -> Result<String, Error> {
        let secret = PKey::hmac(self.secret.as_bytes())
            .map_err(|err| format_err!("error instantiating hmac key: {}", err))?;

        let mut signer = Signer::new(MessageDigest::sha256(), &secret)
            .map_err(|err| format_err!("error instantiating hmac signer: {}", err))?;

        let hmac = signer
            .sign_oneshot_to_vec(data)
            .map_err(|err| format_err!("error calculating hmac: {}", err))?;

        Ok(hex::encode(&hmac))
    }

    /// Iterator over available keys.
    fn available(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().filter_map(Option::as_deref)
    }

    /// Count the available keys.
    pub fn count_available(&self) -> usize {
        self.available().count()
    }

    /// Convenience serde method to check if either the option is `None` or the content `is_empty`.
    pub(super) fn option_is_empty(this: &Option<Self>) -> bool {
        this.as_ref()
            .map_or(true, |this| this.count_available() == 0)
    }

    /// Verify a key and remove it. Returns whether the key was valid. Errors on openssl errors.
    pub(super) fn verify(&mut self, key: &str) -> Result<bool, Error> {
        let hash = self.hash(key.as_bytes())?;
        for entry in &mut self.entries {
            if entry.as_ref() == Some(&hash) {
                *entry = None;
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// Used to inform the user about the recovery code status.
///
/// This contains the available key indices.
#[derive(Clone, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct RecoveryState(Vec<usize>);

impl RecoveryState {
    pub fn is_available(&self) -> bool {
        !self.is_unavailable()
    }

    pub fn is_unavailable(&self) -> bool {
        self.0.is_empty()
    }
}

impl From<&Option<Recovery>> for RecoveryState {
    fn from(r: &Option<Recovery>) -> Self {
        match r {
            Some(r) => Self::from(r),
            None => Self::default(),
        }
    }
}

impl From<&Recovery> for RecoveryState {
    fn from(r: &Recovery) -> Self {
        Self(
            r.entries
                .iter()
                .enumerate()
                .filter_map(|(idx, key)| if key.is_some() { Some(idx) } else { None })
                .collect(),
        )
    }
}
