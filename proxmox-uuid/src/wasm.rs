pub fn uuid_generate(out: *mut [u8; 16]) {

    // TODO: implement soemthing better than this

    let time = js_sys::Date::now() as u64;
    let random1 = (js_sys::Math::random() * f64::MAX) as u64;
    let random2 = (js_sys::Math::random() * f64::MAX) as u64;
    let random3 = (js_sys::Math::random() * f64::MAX) as u64;
    let random4 = (js_sys::Math::random() * f64::MAX) as u64;

    let mut bytes1 = [0u8; 16];
    let mut bytes2 = [0u8; 16];
    let mut bytes3 = [0u8; 16];

    bytes1[0..8].copy_from_slice(&random1.to_le_bytes());
    bytes1[8..16].copy_from_slice(&random2.to_le_bytes());

    let random3 = random3.to_le_bytes();

    bytes2[0..4].copy_from_slice(&random3[0..4]);
    bytes2[4..12].copy_from_slice(&random4.to_le_bytes());
    bytes2[12..16].copy_from_slice(&random3[4..8]);

    bytes3[0..8].copy_from_slice(&time.to_le_bytes());
    bytes3[8..16].copy_from_slice(&time.to_le_bytes());

    if out.is_null() { return; }

    let out = unsafe { out.as_mut().unwrap() };

    for i in 0..16 {
        let v = bytes1[i] ^  bytes2[i]  ^ bytes3[i];
        out[i] = v;
    }
}

// Copied from uuid crate: https://github.com/uuid-rs/uuid.git
// adopted types to our needs

const UPPER: [u8; 16] = [
    b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'A', b'B',
    b'C', b'D', b'E', b'F',
];
const LOWER: [u8; 16] = [
    b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9', b'a', b'b',
    b'c', b'd', b'e', b'f',
];
/// The segments of a UUID's [u8; 16] corresponding to each group.
const BYTE_POSITIONS: [usize; 6] = [0, 4, 6, 8, 10, 16];
/// The locations that hyphens are written into the buffer, after each
/// group.
const HYPHEN_POSITIONS: [usize; 4] = [8, 13, 18, 23];

pub fn uuid_encode(
    uuid: &[u8; 16],
    upper: bool,
) -> String {
    let mut buffer = [0u8; 36];

    let hex = if upper { &UPPER } else { &LOWER };

    for group in 0..5 {
        let hyphens_before = group;
        for idx in BYTE_POSITIONS[group]..BYTE_POSITIONS[group + 1] {
            let b = uuid[idx];
            let out_idx = hyphens_before + 2 * idx;

            buffer[out_idx] = hex[(b >> 4) as usize];
            buffer[out_idx + 1] = hex[(b & 0b1111) as usize];
        }

        if group != 4 {
            buffer[HYPHEN_POSITIONS[group]] = b'-';
        }
    }

    std::str::from_utf8(&mut buffer[..])
        .expect("found non-ASCII output characters while encoding a UUID")
        .to_string()
}

impl fmt::LowerHex for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", uuid_encode(self.as_bytes(), false))
    }
}

impl fmt::UpperHex for Uuid {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", uuid_encode(self.as_bytes(), true))
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for Uuid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&uuid_encode(self.as_bytes(), false))
    }
}
