//! 'Memory' type, represents an amount of memory.

use failure::Error;

use proxmox::api::api;

// TODO: manually implement Serialize/Deserialize to support both numeric and string
// representations. Numeric always being bytes, string having suffixes.
#[api({
    description: "Represents an amount of memory and can be expressed with suffixes such as MiB.",
    serialize_as_string: true,
})]
#[derive(Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Debug)]
#[repr(transparent)]
pub struct Memory(pub u64);

impl std::str::FromStr for Memory {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.ends_with("KiB") {
            Ok(Self::from_kibibytes(s[..s.len() - 3].parse()?))
        } else if s.ends_with("MiB") {
            Ok(Self::from_mebibytes(s[..s.len() - 3].parse()?))
        } else if s.ends_with("GiB") {
            Ok(Self::from_gibibytes(s[..s.len() - 3].parse()?))
        } else if s.ends_with("TiB") {
            Ok(Self::from_tebibytes(s[..s.len() - 3].parse()?))
        } else if s.ends_with("K") {
            Ok(Self::from_kibibytes(s[..s.len() - 1].parse()?))
        } else if s.ends_with("M") {
            Ok(Self::from_mebibytes(s[..s.len() - 1].parse()?))
        } else if s.ends_with("G") {
            Ok(Self::from_gibibytes(s[..s.len() - 1].parse()?))
        } else if s.ends_with("T") {
            Ok(Self::from_tebibytes(s[..s.len() - 1].parse()?))
        } else if s.ends_with("b") || s.ends_with("B") {
            Ok(Self::from_bytes(s[..s.len() - 1].parse()?))
        } else {
            Ok(Self::from_bytes(s[..s.len() - 1].parse()?))
        }
    }
}
proxmox::api::derive_parse_cli_from_str!(Memory);

impl std::fmt::Display for Memory {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        const SUFFIXES: &'static [&'static str] = &["b", "KiB", "MiB", "GiB", "TiB"];
        let mut n = self.0;
        let mut i = 0;
        while i < SUFFIXES.len() && (n & 0x3ff) == 0 {
            n >>= 10;
            i += 1;
        }
        write!(f, "{}{}", n, SUFFIXES[i])
    }
}

impl Memory {
    pub const fn from_bytes(v: u64) -> Self {
        Self(v)
    }

    pub const fn as_bytes(self) -> u64 {
        self.0
    }

    pub const fn from_kibibytes(v: u64) -> Self {
        Self(v * 1024)
    }

    pub const fn as_kibibytes(self) -> u64 {
        self.0 / 1024
    }

    pub const fn from_si_kilobytes(v: u64) -> Self {
        Self(v * 1_000)
    }

    pub const fn as_si_kilobytes(self) -> u64 {
        self.0 / 1_000
    }

    pub const fn from_mebibytes(v: u64) -> Self {
        Self(v * 1024 * 1024)
    }

    pub const fn as_mebibytes(self) -> u64 {
        self.0 / 1024 / 1024
    }

    pub const fn from_si_megabytes(v: u64) -> Self {
        Self(v * 1_000_000)
    }

    pub const fn as_si_megabytes(self) -> u64 {
        self.0 / 1_000_000
    }

    pub const fn from_gibibytes(v: u64) -> Self {
        Self(v * 1024 * 1024 * 1024)
    }

    pub const fn as_gibibytes(self) -> u64 {
        self.0 / 1024 / 1024 / 1024
    }

    pub const fn from_si_gigabytes(v: u64) -> Self {
        Self(v * 1_000_000_000)
    }

    pub const fn as_si_gigabytes(self) -> u64 {
        self.0 / 1_000_000_000
    }

    pub const fn from_tebibytes(v: u64) -> Self {
        Self(v * 1024 * 1024 * 1024 * 1024)
    }

    pub const fn as_tebibytes(self) -> u64 {
        self.0 / 1024 / 1024 / 1024 / 1024
    }

    pub const fn from_si_terabytes(v: u64) -> Self {
        Self(v * 1_000_000_000_000)
    }

    pub const fn as_si_terabytes(self) -> u64 {
        self.0 / 1_000_000_000_000
    }
}

impl std::ops::Add<Memory> for Memory {
    type Output = Memory;

    fn add(self, rhs: Memory) -> Memory {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::AddAssign<Memory> for Memory {
    fn add_assign(&mut self, rhs: Memory) {
        self.0 += rhs.0;
    }
}

impl std::ops::Sub<Memory> for Memory {
    type Output = Memory;

    fn sub(self, rhs: Memory) -> Memory {
        Self(self.0 - rhs.0)
    }
}

impl std::ops::SubAssign<Memory> for Memory {
    fn sub_assign(&mut self, rhs: Memory) {
        self.0 -= rhs.0;
    }
}

impl std::ops::Mul<u64> for Memory {
    type Output = Memory;

    fn mul(self, rhs: u64) -> Memory {
        Self(self.0 * rhs)
    }
}

impl std::ops::MulAssign<u64> for Memory {
    fn mul_assign(&mut self, rhs: u64) {
        self.0 *= rhs;
    }
}

#[test]
fn memory() {
    assert_eq!(Memory::from_mebibytes(1).as_kibibytes(), 1024);
    assert_eq!(Memory::from_mebibytes(1).as_bytes(), 1024 * 1024);
    assert_eq!(Memory::from_si_megabytes(1).as_bytes(), 1_000_000);
    assert_eq!(Memory::from_tebibytes(1), Memory::from_gibibytes(1024));
    assert_eq!(Memory::from_gibibytes(1), Memory::from_mebibytes(1024));
    assert_eq!(Memory::from_mebibytes(1), Memory::from_kibibytes(1024));
    assert_eq!(Memory::from_kibibytes(1), Memory::from_bytes(1024));
    assert_eq!(
        Memory::from_kibibytes(1) + Memory::from_bytes(6),
        Memory::from_bytes(1030)
    );
    assert_eq!("1M".parse::<Memory>().unwrap(), Memory::from_mebibytes(1));
    assert_eq!("1MiB".parse::<Memory>().unwrap(), Memory::from_mebibytes(1));
}
