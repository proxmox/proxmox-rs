use std::fmt::Display;
use std::net::Ipv6Addr;

use serde_with::{DeserializeFromStr, SerializeDisplay};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MacAddressError {
    #[error("the hostname must be from 1 to 63 characters long")]
    InvalidLength,
    #[error("the hostname contains invalid symbols")]
    InvalidSymbols,
}

/// EUI-48 MAC Address
#[derive(
    Clone, Copy, Debug, DeserializeFromStr, SerializeDisplay, PartialEq, Eq, Hash, PartialOrd, Ord,
)]
pub struct MacAddress([u8; 6]);

static LOCAL_PART: [u8; 8] = [0xFE, 0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
static EUI64_MIDDLE_PART: [u8; 2] = [0xFF, 0xFE];

impl MacAddress {
    pub fn new(address: [u8; 6]) -> Self {
        Self(address)
    }

    /// generates a link local IPv6-address according to RFC 4291 (Appendix A)
    pub fn eui64_link_local_address(&self) -> Ipv6Addr {
        let head = &self.0[..3];
        let tail = &self.0[3..];

        let mut eui64_address: Vec<u8> = LOCAL_PART
            .iter()
            .chain(head.iter())
            .chain(EUI64_MIDDLE_PART.iter())
            .chain(tail.iter())
            .copied()
            .collect();

        // we need to flip the 7th bit of the first eui64 byte
        eui64_address[8] ^= 0x02;

        Ipv6Addr::from(
            TryInto::<[u8; 16]>::try_into(eui64_address).expect("is an u8 array with 16 entries"),
        )
    }
}

impl std::str::FromStr for MacAddress {
    type Err = MacAddressError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split = s.split(':');

        let parsed = split
            .into_iter()
            .map(|elem| u8::from_str_radix(elem, 16))
            .collect::<Result<Vec<u8>, _>>()
            .map_err(|_| MacAddressError::InvalidSymbols)?;

        if parsed.len() != 6 {
            return Err(MacAddressError::InvalidLength);
        }

        // SAFETY: ok because of length check
        Ok(Self(parsed.try_into().unwrap()))
    }
}

impl Display for MacAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:<02X}:{:<02X}:{:<02X}:{:<02X}:{:<02X}:{:<02X}",
            self.0[0], self.0[1], self.0[2], self.0[3], self.0[4], self.0[5]
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_parse_mac_address() {
        for input in [
            "aa:aa:aa:11:22:33",
            "AA:BB:FF:11:22:33",
            "bc:24:11:AA:bb:Ef",
        ] {
            let mac_address = input.parse::<MacAddress>().expect("valid mac address");

            assert_eq!(input.to_uppercase(), mac_address.to_string());
        }

        for input in [
            "aa:aa:aa:11:22:33:aa",
            "AA:BB:FF:11:22",
            "AA:BB:GG:11:22:33",
            "AABBGG112233",
            "",
        ] {
            input
                .parse::<MacAddress>()
                .expect_err("invalid mac address");
        }
    }

    #[test]
    fn test_eui64_link_local_address() {
        let mac_address: MacAddress = "BC:24:11:49:8D:75".parse().expect("valid MAC address");

        let link_local_address =
            Ipv6Addr::from_str("fe80::be24:11ff:fe49:8d75").expect("valid IPv6 address");

        assert_eq!(link_local_address, mac_address.eui64_link_local_address());
    }
}
