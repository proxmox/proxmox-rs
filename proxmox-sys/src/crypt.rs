//! Rust bindings for libcrypt
//!
//! this may fail if we ever pull in dependencies that also link with libcrypt. we may eventually
//! want to switch to pure rust re-implementations of libcrypt.

use std::ffi::{CStr, CString};

use anyhow::{bail, Error};

// from libcrypt1, 'lib/crypt.h.in'
const CRYPT_OUTPUT_SIZE: usize = 384;
const CRYPT_MAX_PASSPHRASE_SIZE: usize = 512;
const CRYPT_DATA_RESERVED_SIZE: usize = 767;
const CRYPT_DATA_INTERNAL_SIZE: usize = 30720;
const CRYPT_GENSALT_OUTPUT_SIZE: usize = 192;

// the hash prefix selects the password hashing method, currently this is yescrypt. check `man
// crypt(5)` for more info
pub const HASH_PREFIX: &str = "$y$";

// the cpu cost of the password hashing function. depends on the hashing function, see`man
// crypt_gensalt(3)` and `man crypt(5) for more info
//
// `5` selects a good medium cpu time hardness that seems to be widely used by e.g. Debian
// see `YESCRYPT_COST_FACTOR` in `/etc/login.defs`
const HASH_COST: u64 = 5;

#[repr(C)]
struct CryptData {
    output: [libc::c_char; CRYPT_OUTPUT_SIZE],
    setting: [libc::c_char; CRYPT_OUTPUT_SIZE],
    input: [libc::c_char; CRYPT_MAX_PASSPHRASE_SIZE],
    reserved: [libc::c_char; CRYPT_DATA_RESERVED_SIZE],
    initialized: libc::c_char,
    internal: [libc::c_char; CRYPT_DATA_INTERNAL_SIZE],
}

/// Encrypt a password - see man crypt(3)
pub fn crypt(password: &[u8], salt: &[u8]) -> Result<String, Error> {
    #[link(name = "crypt")]
    unsafe extern "C" {
        #[link_name = "crypt_r"]
        fn __crypt_r(
            key: *const libc::c_char,
            salt: *const libc::c_char,
            data: *mut CryptData,
        ) -> *mut libc::c_char;
    }

    let mut data: CryptData = unsafe { std::mem::zeroed() };
    for (i, c) in salt.iter().take(data.setting.len() - 1).enumerate() {
        data.setting[i] = *c as libc::c_char;
    }
    for (i, c) in password.iter().take(data.input.len() - 1).enumerate() {
        data.input[i] = *c as libc::c_char;
    }

    let res = unsafe {
        let status = __crypt_r(
            &data.input as *const _,
            &data.setting as *const _,
            &mut data as *mut _,
        );
        if status.is_null() {
            bail!("internal error: crypt_r returned null pointer");
        }

        // according to man crypt(3):
        //
        // > Upon error, crypt_r, crypt_rn, and crypt_ra write an invalid hashed passphrase to the
        // > output field of their data argument, and crypt writes an invalid hash to its static
        // > storage area.  This string will be shorter than 13 characters, will begin with a ‘*’,
        // > and will not compare equal to setting.
        if data.output[0] == '*' as libc::c_char {
            bail!("internal error: crypt_r returned invalid hash");
        }
        CStr::from_ptr(&data.output as *const _)
    };
    Ok(String::from(res.to_str()?))
}

/// Rust wrapper around `crypt_gensalt_rn` from libcrypt. Useful to generate salts for crypt.
///
/// - `prefix`: The prefix that selects the hashing method to use (see `man crypt(5)`)
/// - `count`: The CPU time cost parameter (e.g., for `yescrypt` between 1 and 11, see `man
/// crypt(5)`)
/// - `rbytes`: The byte slice that contains cryptographically random bytes for generating the salt
pub fn crypt_gensalt(prefix: &str, count: u64, rbytes: &[u8]) -> Result<String, Error> {
    #[link(name = "crypt")]
    unsafe extern "C" {
        #[link_name = "crypt_gensalt_rn"]
        fn __crypt_gensalt_rn(
            prefix: *const libc::c_char,
            count: libc::c_ulong,
            // `crypt_gensalt_rn`'s signature expects a char pointer here, which would be a pointer
            // to an `i8` slice in rust. however, this is interpreted as raw bytes that are used as
            // entropy, which in rust usually is a `u8` slice. so use this signature to avoid a
            // pointless transmutation (random bytes are random, whether interpreted as `i8` or
            // `u8`)
            rbytes: *const u8,
            nrbytes: libc::c_int,
            output: *mut libc::c_char,
            output_size: libc::c_int,
        ) -> *mut libc::c_char;
    }

    let prefix = CString::new(prefix)?;

    #[allow(clippy::useless_conversion)]
    let mut output = [libc::c_char::from(0); CRYPT_GENSALT_OUTPUT_SIZE];

    let status = unsafe {
        __crypt_gensalt_rn(
            prefix.as_ptr(),
            count,
            rbytes.as_ptr(),
            rbytes.len().try_into()?,
            output.as_mut_ptr(),
            output.len().try_into()?,
        )
    };

    if status.is_null() {
        bail!("internal error: crypt_gensalt_rn returned a null pointer");
    }

    // according to man crypt_gensalt_rn(3):
    //
    // > Upon error, in addition to returning a null pointer, crypt_gensalt and crypt_gensalt_rn
    // > will write an invalid setting string to their output buffer, if there is enough space;
    // > this string will begin with a ‘*’ and will not be equal to prefix.
    //
    // while it states that this is "in addition" to returning a null pointer, this isn't how
    // `crypt_r` seems to behave (sometimes only setting an invalid hash) so add this here too just
    // in case.
    if output[0] == '*' as libc::c_char {
        bail!("internal error: crypt_gensalt_rn could not create a valid salt");
    }

    let res = unsafe { CStr::from_ptr(output.as_ptr()) };

    Ok(res.to_str()?.to_string())
}

/// Encrypt a password using sha256 hashing method
pub fn encrypt_pw(password: &str) -> Result<String, Error> {
    // 8*32 = 256 bits security (128+ recommended, see `man crypt(5)`)
    let salt = crate::linux::random_data(32)?;

    let salt = crypt_gensalt(HASH_PREFIX, HASH_COST, &salt)?;

    crypt(password.as_bytes(), salt.as_bytes())
}

/// Verify if an encrypted password matches
pub fn verify_crypt_pw(password: &str, enc_password: &str) -> Result<(), Error> {
    let verify = crypt(password.as_bytes(), enc_password.as_bytes())?;

    // `openssl::memcmp::eq()`'s runtime does not depend on the content of the arrays only the
    // length, this makes it harder to exploit timing side-channels.
    if verify.len() != enc_password.len()
        || !openssl::memcmp::eq(verify.as_bytes(), enc_password.as_bytes())
    {
        bail!("invalid credentials");
    }

    Ok(())
}

#[test]
fn test_hash_and_verify_passphrase() {
    let phrase = "supersecretpassphrasenoonewillguess";

    let hash = encrypt_pw(phrase).expect("could not hash test password");
    verify_crypt_pw(phrase, &hash).expect("could not verify test password");
}

#[test]
#[should_panic]
fn test_wrong_passphrase_fails() {
    let phrase = "supersecretpassphrasenoonewillguess";

    let hash = encrypt_pw(phrase).expect("could not hash test password");
    verify_crypt_pw("nope", &hash).expect("could not verify test password");
}

#[test]
fn test_old_passphrase_hash() {
    let phrase = "supersecretpassphrasenoonewillguess";
    // `$5$` -> sha256crypt, our previous default implementation
    let hash = "$5$bx7fjhlS8yMPM3Nc$yRgB5vyoTWeRcYn31RFTg5hAGyTInUq.l0HqLKzRuRC";

    verify_crypt_pw(phrase, hash).expect("could not verify test password");
}

#[test]
#[should_panic]
fn test_old_hash_wrong_passphrase_fails() {
    let phrase = "nope";
    // `$5$` -> sha256crypt, our previous default implementation
    let hash = "$5$bx7fjhlS8yMPM3Nc$yRgB5vyoTWeRcYn31RFTg5hAGyTInUq.l0HqLKzRuRC";

    verify_crypt_pw(phrase, hash).expect("could not verify test password");
}
