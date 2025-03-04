use std::fmt;

use crate::Uuid;

#[link(name = "uuid")]
unsafe extern "C" {
    pub fn uuid_generate(out: *mut [u8; 16]);
    fn uuid_unparse_lower(input: *const [u8; 16], out: *mut u8);
    fn uuid_unparse_upper(input: *const [u8; 16], out: *mut u8);
}

impl fmt::LowerHex for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut buf = [0u8; 37];
        unsafe {
            uuid_unparse_lower(self.as_bytes(), buf.as_mut_ptr());
        }
        write!(f, "{}", unsafe {
            std::str::from_utf8_unchecked(&buf[..36])
        })
    }
}

impl fmt::UpperHex for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut buf = [0u8; 37];
        unsafe {
            uuid_unparse_upper(self.as_bytes(), buf.as_mut_ptr());
        }
        write!(f, "{}", unsafe {
            std::str::from_utf8_unchecked(&buf[..36])
        })
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Uuid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut buf = [0u8; 37];
        unsafe {
            uuid_unparse_lower(self.as_bytes(), buf.as_mut_ptr());
        }
        serializer.serialize_str(unsafe { std::str::from_utf8_unchecked(&buf[..36]) })
    }
}
