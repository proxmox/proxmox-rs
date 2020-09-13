use anyhow::{bail, Error};
use std::ffi::{CStr, CString};

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

/// Safe bindings to libc time
pub fn time() -> Result<i64, Error> {
    let now = unsafe { libc::time(std::ptr::null_mut()) };
    if now < 0 {
        bail!("libc::time failed with {}", now);
    }
    Ok(now)
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

/// Convert Unix epoch into RFC3339 UTC string
pub fn epoch_to_rfc_3339_utc(epoch: i64) -> Result<String, Error> {
    let gmtime = gmtime(epoch)?;
    strftime("%FT%TZ", &gmtime)
}

/// Convert Unix epoch into RFC3339 local time with TZ
pub fn epoch_to_rfc_3339(epoch: i64) -> Result<String, Error> {

    let localtime = localtime(epoch)?;

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
