#![allow(clippy::manual_range_contains)]

use std::ffi::{CStr, CString};

use anyhow::{bail, format_err, Error};

/// Safe bindings to libc timelocal
///
/// We set tm_isdst to -1.
/// This also normalizes the parameter
pub fn timelocal(t: &mut libc::tm) -> Result<i64, Error> {
    t.tm_isdst = -1;

    let epoch = unsafe { libc::mktime(t) };
    if epoch == -1 {
        bail!("libc::mktime failed for {t:?}");
    }
    Ok(epoch)
}

/// Safe bindings to libc timegm
///
/// We set tm_isdst to 0.
/// This also normalizes the parameter
pub fn timegm(t: &mut libc::tm) -> Result<i64, Error> {
    t.tm_isdst = 0;

    let epoch = unsafe { libc::timegm(t) };
    if epoch == -1 {
        bail!("libc::timegm failed for {t:?}");
    }
    Ok(epoch)
}

fn new_libc_tm() -> libc::tm {
    libc::tm {
        tm_sec: 0,
        tm_min: 0,
        tm_hour: 0,
        tm_mday: 0,
        tm_mon: 0,
        tm_year: 0,
        tm_wday: 0,
        tm_yday: 0,
        tm_isdst: 0,
        tm_gmtoff: 0,
        tm_zone: std::ptr::null(),
    }
}

/// Safe bindings to libc localtime
pub fn localtime(epoch: i64) -> Result<libc::tm, Error> {
    let mut result = new_libc_tm();

    unsafe {
        if libc::localtime_r(&epoch, &mut result).is_null() {
            bail!("libc::localtime failed for '{epoch}'");
        }
    }

    Ok(result)
}

/// Safe bindings to libc gmtime
pub fn gmtime(epoch: i64) -> Result<libc::tm, Error> {
    let mut result = new_libc_tm();

    unsafe {
        if libc::gmtime_r(&epoch, &mut result).is_null() {
            bail!("libc::gmtime failed for '{epoch}'");
        }
    }

    Ok(result)
}

/// Returns Unix Epoch (now)
///
/// Note: This panics if the SystemTime::now() returns values not
/// repesentable as i64 (should never happen).
pub fn epoch_i64() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now();

    if now > UNIX_EPOCH {
        i64::try_from(now.duration_since(UNIX_EPOCH).unwrap().as_secs())
            .expect("epoch_i64: now is too large")
    } else {
        -i64::try_from(UNIX_EPOCH.duration_since(now).unwrap().as_secs())
            .expect("epoch_i64: now is too small")
    }
}

/// Returns Unix Epoch (now) as f64 with subseconds resolution
///
/// Note: This can be inaccurate for values greater the 2^53. But this
/// should never happen.
pub fn epoch_f64() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now();

    if now > UNIX_EPOCH {
        now.duration_since(UNIX_EPOCH).unwrap().as_secs_f64()
    } else {
        -UNIX_EPOCH.duration_since(now).unwrap().as_secs_f64()
    }
}

/// Safe bindings to libc strftime
pub fn strftime(format: &str, t: &libc::tm) -> Result<String, Error> {
    let format = CString::new(format).map_err(|err| format_err!("{err}"))?;
    let mut buf = vec![0u8; 8192];

    let res = unsafe {
        libc::strftime(
            buf.as_mut_ptr() as *mut libc::c_char,
            buf.len() as libc::size_t,
            format.as_ptr(),
            t as *const libc::tm,
        )
    };
    if res == !0 {
        // -1,, it's unsigned
        bail!("strftime failed");
    }

    // `res` is a `libc::size_t`, which on a different target architecture might not be directly
    // assignable to a `usize`. Thus, we actually want a cast here.
    #[allow(clippy::unnecessary_cast)]
    let len = res as usize;

    if len == 0 {
        bail!("strftime: result len is 0 (string too large)");
    };

    let c_str = CStr::from_bytes_with_nul(&buf[..len + 1]).map_err(|err| format_err!("{err}"))?;
    let str_slice: &str = c_str.to_str().unwrap();
    Ok(str_slice.to_owned())
}

//  The `libc` crate does not yet contain bindings for `strftime_l`
#[link(name = "c")]
extern "C" {
    #[link_name = "strftime_l"]
    fn libc_strftime_l(
        s: *mut libc::c_char,
        max: libc::size_t,
        format: *const libc::c_char,
        time: *const libc::tm,
        locale: libc::locale_t,
    ) -> libc::size_t;
}

/// Safe bindings to libc's `strftime_l`
///
/// The compared to `strftime`, `strftime_l` allows the caller to provide a
/// locale object.
pub fn strftime_l(format: &str, t: &libc::tm, locale: &Locale) -> Result<String, Error> {
    let format = CString::new(format).map_err(|err| format_err!("{err}"))?;
    let mut buf = vec![0u8; 8192];

    let res = unsafe {
        libc_strftime_l(
            buf.as_mut_ptr() as *mut libc::c_char,
            buf.len() as libc::size_t,
            format.as_ptr(),
            t as *const libc::tm,
            locale.locale,
        )
    };
    if res == !0 {
        // -1,, it's unsigned
        // man strftime: POSIX.1-2001  does  not  specify  any  errno  settings   for strftime()
        bail!("strftime failed");
    }

    // `res` is a `libc::size_t`, which on a different target architecture might not be directly
    // assignable to a `usize`. Thus, we actually want a cast here.
    #[allow(clippy::unnecessary_cast)]
    let len = res as usize;

    if len == 0 {
        bail!("strftime: result len is 0 (string too large)");
    };

    let c_str = CStr::from_bytes_with_nul(&buf[..len + 1]).map_err(|err| format_err!("{err}"))?;
    let str_slice: &str = c_str.to_str()?;
    Ok(str_slice.to_owned())
}

/// A safe wrapper for `libc::locale_t`.
///
/// The enclosed locale object will be automatically freed by `Drop::drop`
pub struct Locale {
    locale: libc::locale_t,
}

impl Locale {
    /// Portable, minimal locale environment
    const C: &'static str = "C";

    /// Create a new locale object.
    /// `category_mask` is expected to be a bitmask based on the
    /// LC_*_MASK constants from libc. The locale will be set to `locale`
    /// for every entry in the bitmask which is set.
    pub fn new(category_mask: i32, locale: &str) -> Result<Self, Error> {
        let format = CString::new(locale).map_err(|err| format_err!("{err}"))?;

        let locale =
            unsafe { libc::newlocale(category_mask, format.as_ptr(), std::ptr::null_mut()) };

        if locale.is_null() {
            return Err(std::io::Error::last_os_error().into());
        }

        Ok(Self { locale })
    }
}

impl Drop for Locale {
    fn drop(&mut self) {
        unsafe {
            libc::freelocale(self.locale);
        }
    }
}

/// Format epoch as local time
pub fn strftime_local(format: &str, epoch: i64) -> Result<String, Error> {
    let localtime = localtime(epoch)?;
    strftime(format, &localtime)
}

/// Format epoch as utc time
pub fn strftime_utc(format: &str, epoch: i64) -> Result<String, Error> {
    let gmtime = gmtime(epoch)?;
    strftime(format, &gmtime)
}

/// Convert Unix epoch into RFC3339 UTC string
pub fn epoch_to_rfc3339_utc(epoch: i64) -> Result<String, Error> {
    let gmtime = gmtime(epoch)?;

    let year = gmtime.tm_year + 1900;
    if year < 0 || year > 9999 {
        bail!("epoch_to_rfc3339_utc: wrong year '{year}'");
    }

    strftime("%010FT%TZ", &gmtime)
}

/// Convert Unix epoch into RFC3339 local time with TZ
pub fn epoch_to_rfc3339(epoch: i64) -> Result<String, Error> {
    use std::fmt::Write as _;

    let localtime = localtime(epoch)?;

    let year = localtime.tm_year + 1900;
    if year < 0 || year > 9999 {
        bail!("epoch_to_rfc3339: wrong year '{year}'");
    }

    // Note: We cannot use strftime %z because of missing collon

    let mut offset = localtime.tm_gmtoff;

    let prefix = if offset < 0 {
        offset = -offset;
        '-'
    } else {
        '+'
    };

    let mins = offset / 60;
    let hours = mins / 60;
    let mins = mins % 60;

    let mut s = strftime("%10FT%T", &localtime)?;
    s.push(prefix);
    let _ = write!(s, "{hours:02}:{mins:02}");

    Ok(s)
}

/// Parse RFC3339 into Unix epoch
pub fn parse_rfc3339(input_str: &str) -> Result<i64, Error> {
    parse_rfc3339_do(input_str)
        .map_err(|err| format_err!("failed to parse rfc3339 timestamp ({input_str:?}) - {err}",))
}

fn parse_rfc3339_do(input_str: &str) -> Result<i64, Error> {
    let input = input_str.as_bytes();

    let expect = |pos: usize, c: u8| {
        if input[pos] != c {
            bail!("unexpected char at pos {pos}");
        }
        Ok(())
    };

    let digit = |pos: usize| -> Result<i32, Error> {
        let digit = input[pos] as i32;
        if digit < 48 || digit > 57 {
            bail!("unexpected char at pos {pos}");
        }
        Ok(digit - 48)
    };

    fn check_max(i: i32, max: i32) -> Result<i32, Error> {
        if i > max {
            bail!("value too large ({i} > {max})");
        }
        Ok(i)
    }

    if input.len() < 20 || input.len() > 25 {
        bail!("timestamp of unexpected length");
    }

    let tz = input[19];

    match tz {
        b'Z' => {
            if input.len() != 20 {
                bail!("unexpected length in UTC timestamp");
            }
        }
        b'+' | b'-' => {
            if input.len() != 25 {
                bail!("unexpected length in timestamp");
            }
        }
        _ => bail!("unexpected timezone indicator"),
    }

    let mut tm = crate::TmEditor::new(true);

    tm.set_year(digit(0)? * 1000 + digit(1)? * 100 + digit(2)? * 10 + digit(3)?)?;
    expect(4, b'-')?;
    tm.set_mon(check_max(digit(5)? * 10 + digit(6)?, 12)?)?;
    expect(7, b'-')?;
    tm.set_mday(check_max(digit(8)? * 10 + digit(9)?, 31)?)?;

    expect(10, b'T')?;

    tm.set_hour(check_max(digit(11)? * 10 + digit(12)?, 23)?)?;
    expect(13, b':')?;
    tm.set_min(check_max(digit(14)? * 10 + digit(15)?, 59)?)?;
    expect(16, b':')?;
    tm.set_sec(check_max(digit(17)? * 10 + digit(18)?, 60)?)?;

    let epoch = tm.into_epoch()?;
    if tz == b'Z' {
        return Ok(epoch);
    }

    let hours = check_max(digit(20)? * 10 + digit(21)?, 23)?;
    expect(22, b':')?;
    let mins = check_max(digit(23)? * 10 + digit(24)?, 59)?;

    let offset = (hours * 3600 + mins * 60) as i64;

    let epoch = match tz {
        b'+' => epoch - offset,
        b'-' => epoch + offset,
        _ => unreachable!(), // already checked above
    };

    Ok(epoch)
}

/// Convert Unix epoch into RFC2822 local time with TZ
pub fn epoch_to_rfc2822(epoch: i64) -> Result<String, Error> {
    let localtime = localtime(epoch)?;
    let locale = Locale::new(libc::LC_ALL, Locale::C)?;
    let rfc2822_date = strftime_l("%a, %d %b %Y %T %z", &localtime, &locale)?;

    Ok(rfc2822_date)
}

#[test]
fn test_leap_seconds() {
    let convert_reconvert = |epoch| {
        let rfc3339 =
            epoch_to_rfc3339_utc(epoch).expect("leap second epoch to rfc3339 should work");

        let parsed =
            parse_rfc3339(&rfc3339).expect("parsing converted leap second epoch should work");

        assert_eq!(epoch, parsed);
    };

    // 2005-12-31T23:59:59Z was followed by a leap second
    let epoch = 1136073599;
    convert_reconvert(epoch);
    convert_reconvert(epoch + 1);
    convert_reconvert(epoch + 2);

    let parsed = parse_rfc3339("2005-12-31T23:59:60Z").expect("parsing leap second should work");
    assert_eq!(parsed, epoch + 1);
}

#[test]
fn test_rfc3339_range() {
    // also tests single-digit years/first decade values
    let lower = -62167219200;
    let lower_str = "0000-01-01T00:00:00Z";

    let upper = 253402300799;
    let upper_str = "9999-12-31T23:59:59Z";

    let converted =
        epoch_to_rfc3339_utc(lower).expect("converting lower bound of RFC3339 range should work");
    assert_eq!(converted, lower_str);

    let converted =
        epoch_to_rfc3339_utc(upper).expect("converting upper bound of RFC3339 range should work");
    assert_eq!(converted, upper_str);

    let parsed =
        parse_rfc3339(lower_str).expect("parsing lower bound of RFC3339 range should work");
    assert_eq!(parsed, lower);

    let parsed =
        parse_rfc3339(upper_str).expect("parsing upper bound of RFC3339 range should work");
    assert_eq!(parsed, upper);

    epoch_to_rfc3339_utc(lower - 1)
        .expect_err("converting below lower bound of RFC3339 range should fail");

    epoch_to_rfc3339_utc(upper + 1)
        .expect_err("converting above upper bound of RFC3339 range should fail");

    let first_century = -59011459201;
    let first_century_str = "0099-12-31T23:59:59Z";

    let converted = epoch_to_rfc3339_utc(first_century)
        .expect("converting epoch representing first century year should work");
    assert_eq!(converted, first_century_str);

    let parsed =
        parse_rfc3339(first_century_str).expect("parsing first century string should work");
    assert_eq!(parsed, first_century);

    let first_millenium = -59011459200;
    let first_millenium_str = "0100-01-01T00:00:00Z";

    let converted = epoch_to_rfc3339_utc(first_millenium)
        .expect("converting epoch representing first millenium year should work");
    assert_eq!(converted, first_millenium_str);

    let parsed =
        parse_rfc3339(first_millenium_str).expect("parsing first millenium string should work");
    assert_eq!(parsed, first_millenium);
}

#[test]
fn test_gmtime_range() {
    // year must fit into i32
    let lower = -67768040609740800;
    let upper = 67768036191676799;

    let mut lower_tm = gmtime(lower).expect("gmtime should work as long as years fit into i32");
    let res = timegm(&mut lower_tm).expect("converting back to epoch should work");
    assert_eq!(lower, res);

    gmtime(lower - 1).expect_err("gmtime should fail for years not fitting into i32");

    let mut upper_tm = gmtime(upper).expect("gmtime should work as long as years fit into i32");
    let res = timegm(&mut upper_tm).expect("converting back to epoch should work");
    assert_eq!(upper, res);

    gmtime(upper + 1).expect_err("gmtime should fail for years not fitting into i32");
}

#[test]
fn test_timezones() {
    let input = "2020-12-30T00:00:00+06:30";
    let epoch = 1609263000;
    let expected_utc = "2020-12-29T17:30:00Z";

    let parsed = parse_rfc3339(input).expect("parsing failed");
    assert_eq!(parsed, epoch);

    let res = epoch_to_rfc3339_utc(parsed).expect("converting to RFC failed");
    assert_eq!(expected_utc, res);
}

#[test]
fn test_strftime_l() {
    let epoch = 1609263000;

    let locale = Locale::new(libc::LC_ALL, Locale::C).expect("could not create locale");
    let time = gmtime(epoch).expect("gmtime failed");

    let formatted = strftime_l("%a, %d %b %Y %T %z", &time, &locale).expect("strftime_l failed");

    assert_eq!(formatted, "Tue, 29 Dec 2020 17:30:00 +0000");
}

#[test]
fn test_epoch_to_rfc2822() {
    let epoch = 1609263000;
    // Output is TZ-dependent, so only verify that it did not fail.
    // Internally, it uses strftime_l which we test already.
    assert!(epoch_to_rfc2822(epoch).is_ok());
}
