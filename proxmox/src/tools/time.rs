use anyhow::{bail, format_err, Error};
use std::ffi::{CStr, CString};

mod tm_editor;
pub use tm_editor::*;

/// Safe bindings to libc timelocal
///
/// We set tm_isdst to -1.
/// This also normalizes the parameter
pub fn timelocal(t: &mut libc::tm) -> Result<i64, Error> {
    t.tm_isdst = -1;

    let epoch = unsafe { libc::mktime(t) };
    if epoch == -1 {
        bail!("libc::mktime failed for {:?}", t);
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
        bail!("libc::timegm failed for {:?}", t);
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
            bail!("libc::localtime failed for '{}'", epoch);
        }
    }

    Ok(result)
}

/// Safe bindings to libc gmtime
pub fn gmtime(epoch: i64) -> Result<libc::tm, Error> {
    let mut result = new_libc_tm();

    unsafe {
        if libc::gmtime_r(&epoch, &mut result).is_null() {
            bail!("libc::gmtime failed for '{}'", epoch);
        }
    }

    Ok(result)
}

/// Returns Unix Epoch (now)
///
/// Note: This panics if the SystemTime::now() returns values not
/// repesentable as i64 (should never happen).
pub fn epoch_i64() -> i64 {
    use std::convert::TryFrom;
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
/// Note: This can be inacurrate for values greater the 2^53. But this
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

//  rust libc bindings do not include strftime
#[link(name = "c")]
extern {
    #[link_name = "strftime"]
    fn libc_strftime(
        s: *mut libc::c_char,
        max: libc::size_t,
        format: *const libc::c_char,
        time: *const libc::tm,
    ) -> libc::size_t;
}

/// Safe bindings to libc strftime
pub fn strftime(format: &str, t: &libc::tm) -> Result<String, Error> {

    let format = CString::new(format)?;
    let mut buf = vec![0u8; 8192];

    let res = unsafe {
        libc_strftime(
            buf.as_mut_ptr() as *mut libc::c_char,
            buf.len() as libc::size_t,
            format.as_ptr(),
            t as *const libc::tm,
        )
    };

    let len = nix::errno::Errno::result(res).map(|r| r as usize)?;
    if len == 0 {
        bail!("strftime: result len is 0 (string too large)");
    };

    let c_str = CStr::from_bytes_with_nul(&buf[..len+1])?;
    let str_slice: &str = c_str.to_str().unwrap();
    Ok(str_slice.to_owned())
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
        bail!("epoch_to_rfc3339_utc: wrong year '{}'", year);
    }

    strftime("%FT%TZ", &gmtime)
}

/// Convert Unix epoch into RFC3339 local time with TZ
pub fn epoch_to_rfc3339(epoch: i64) -> Result<String, Error> {

    let localtime = localtime(epoch)?;

    let year = localtime.tm_year + 1900;
    if year < 0 || year > 9999 {
        bail!("epoch_to_rfc3339: wrong year '{}'", year);

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

    let mut s = strftime("%FT%T", &localtime)?;
    s.push(prefix);
    s.push_str(&format!("{:02}:{:02}", hours, mins));

    Ok(s)
}

/// Parse RFC3339 into Unix epoch
pub fn parse_rfc3339(i: &str) -> Result<i64, Error> {

    let input = i.as_bytes();

    let expect = |pos: usize, c: u8| {
        if input[pos] != c {
            bail!("unexpected char at pos {}", pos);
        }
        Ok(())
    };

    let digit = |pos: usize| -> Result<i32, Error> {
        let digit = input[pos] as i32;
        if digit < 48 || digit > 57 {
            bail!("unexpected char at pos {}", pos);
        }
        Ok(digit - 48)
    };

    let check_max = |i: i32, max: i32| {
        if i > max {
            bail!("value too large ({} > {})", i, max);
        }
        Ok(i)
    };

    crate::try_block!({

        if i.len() < 20 || i.len() > 25 { bail!("wrong length"); }

        let tz = input[19];

        match tz {
            b'Z' => if i.len() != 20 { bail!("wrong length"); },
            b'+' | b'-' =>  if i.len() != 25 { bail!("wrong length"); },
            _ => bail!("got unknown timezone indicator"),
        }

        let mut tm = TmEditor::new(true);

        tm.set_year(digit(0)?*1000 + digit(1)?*100 + digit(2)?*10+digit(3)?)?;
        expect(4, b'-')?;
        tm.set_mon(check_max(digit(5)?*10 + digit(6)?, 12)?)?;
        expect(7, b'-')?;
        tm.set_mday(check_max(digit(8)?*10 + digit(9)?, 31)?)?;

        expect(10, b'T')?;

        tm.set_hour(check_max(digit(11)?*10 + digit(12)?, 23)?)?;
        expect(13, b':')?;
        tm.set_min(check_max(digit(14)?*10 + digit(15)?, 59)?)?;
        expect(16, b':')?;
        tm.set_sec(check_max(digit(17)?*10 + digit(18)?, 59)?)?;

        let epoch = tm.into_epoch()?;
        if tz == b'Z' {
            return Ok(epoch);
        }

        let hours = check_max(digit(20)?*10 + digit(21)?, 23)?;
        expect(22, b':')?;
        let mins = check_max(digit(23)?*10 + digit(24)?, 23)?;

        let offset = (hours*3600 + mins*60) as i64;

        let epoch = match tz {
            b'+' => epoch - offset,
            b'-' => epoch + offset,
            _ => unreachable!(), // already checked above
        };

        Ok(epoch)
    }).map_err(|err| format_err!("parse_rfc_3339 failed - {}", err))
}
