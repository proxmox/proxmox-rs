use anyhow::{bail, Error};

/// Safe bindings to libc timelocal
///
/// We set tm_isdst to -1.
pub fn timelocal(mut t: libc::tm) -> Result<i64, Error> {

    t.tm_isdst = -1;

    let epoch = unsafe { libc::mktime(&mut t) };
    if epoch == -1 {
        bail!("libc::mktime failed for {:?}", t);
    }
    Ok(epoch)
}

/// Safe bindings to libc timegm
///
/// We set tm_isdst to 0.
pub fn timegm(mut t: libc::tm) -> Result<i64, Error> {

    t.tm_isdst = 0;

    let epoch = unsafe { libc::timegm(&mut t) };
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
