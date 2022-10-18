mod release_file;
use anyhow::{bail, Error};
pub use release_file::{CompressionType, FileReference, FileReferenceType, ReleaseFile};

mod packages_file;
pub use packages_file::PackagesFile;

mod sources_file;
pub use sources_file::SourcesFile;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct CheckSums {
    pub md5: Option<[u8; 16]>,
    pub sha1: Option<[u8; 20]>,
    pub sha256: Option<[u8; 32]>,
    pub sha512: Option<[u8; 64]>,
}

macro_rules! assert_hash_equality {
    ($l:expr, $r:expr) => {{
        if $l != $r {
            bail!("hash mismatch: {} != {}", hex::encode($l), hex::encode($r));
        }
    }};
}

impl CheckSums {
    pub fn is_secure(&self) -> bool {
        self.sha256.is_some() || self.sha512.is_some()
    }

    pub fn verify(&self, input: &[u8]) -> Result<(), Error> {
        if !self.is_secure() {
            bail!("No SHA256/SHA512 checksum specified.");
        }

        if let Some(expected) = self.sha512 {
            let digest = openssl::sha::sha512(input);
            assert_hash_equality!(digest, expected);
            Ok(())
        } else if let Some(expected) = self.sha256 {
            let digest = openssl::sha::sha256(input);
            assert_hash_equality!(digest, expected);
            Ok(())
        } else {
            bail!("No trusted checksum found - verification not possible.");
        }
    }

    /// Merge two instances of `CheckSums`.
    pub fn merge(&mut self, rhs: &CheckSums) -> Result<(), Error> {
        match (self.sha512, rhs.sha512) {
            (_, None) => {}
            (None, Some(sha512)) => self.sha512 = Some(sha512),
            (Some(left), Some(right)) => assert_hash_equality!(left, right),
        };
        match (self.sha256, rhs.sha256) {
            (_, None) => {}
            (None, Some(sha256)) => self.sha256 = Some(sha256),
            (Some(left), Some(right)) => assert_hash_equality!(left, right),
        };
        match (self.sha1, rhs.sha1) {
            (_, None) => {}
            (None, Some(sha1)) => self.sha1 = Some(sha1),
            (Some(left), Some(right)) => assert_hash_equality!(left, right),
        };
        match (self.md5, rhs.md5) {
            (_, None) => {}
            (None, Some(md5)) => self.md5 = Some(md5),
            (Some(left), Some(right)) => assert_hash_equality!(left, right),
        };

        Ok(())
    }
}
