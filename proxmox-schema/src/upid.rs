use anyhow::{bail, Error};

use crate::{const_regex, ApiStringFormat, ApiType, Schema, StringSchema};

/// Unique Process/Task Identifier
///
/// We use this to uniquely identify worker task. UPIDs have a short
/// string repesentaion, which gives additional information about the
/// type of the task. for example:
/// ```text
/// UPID:{node}:{pid}:{pstart}:{task_id}:{starttime}:{worker_type}:{worker_id}:{userid}:
/// UPID:elsa:00004F37:0039E469:00000000:5CA78B83:garbage_collection::root@pam:
/// ```
/// Please note that we use tokio, so a single thread can run multiple
/// tasks.
// #[api] - manually implemented API type
#[derive(Debug, Clone)]
pub struct UPID {
    /// The Unix PID
    pub pid: i32, // really libc::pid_t, but we don't want this as a dependency for proxmox-schema
    /// The Unix process start time from `/proc/pid/stat`
    pub pstart: u64,
    /// The task start time (Epoch)
    pub starttime: i64,
    /// The task ID (inside the process/thread)
    pub task_id: usize,
    /// Worker type (arbitrary ASCII string)
    pub worker_type: String,
    /// Worker ID (arbitrary ASCII string)
    pub worker_id: Option<String>,
    /// The authenticated entity who started the task
    pub auth_id: String,
    /// The node name.
    pub node: String,
}

const_regex! {
    pub PROXMOX_UPID_REGEX = concat!(
        r"^UPID:(?P<node>[a-zA-Z0-9]([a-zA-Z0-9\-]*[a-zA-Z0-9])?):(?P<pid>[0-9A-Fa-f]{8}):",
        r"(?P<pstart>[0-9A-Fa-f]{8,9}):(?P<task_id>[0-9A-Fa-f]{8,16}):(?P<starttime>[0-9A-Fa-f]{8}):",
        r"(?P<wtype>[^:\s]+):(?P<wid>[^:\s]*):(?P<authid>[^:\s]+):$"
    );
}

pub const PROXMOX_UPID_FORMAT: ApiStringFormat = ApiStringFormat::Pattern(&PROXMOX_UPID_REGEX);

pub const UPID_SCHEMA: Schema = StringSchema::new("Unique Process/Task Identifier")
    .min_length("UPID:N:12345678:12345678:12345678:::".len())
    .format(&PROXMOX_UPID_FORMAT)
    .schema();

impl ApiType for UPID {
    const API_SCHEMA: Schema = UPID_SCHEMA;
}

impl std::str::FromStr for UPID {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(cap) = PROXMOX_UPID_REGEX.captures(s) {
            let worker_id = if cap["wid"].is_empty() {
                None
            } else {
                let wid = unescape_id(&cap["wid"])?;
                Some(wid)
            };

            Ok(UPID {
                pid: i32::from_str_radix(&cap["pid"], 16).unwrap(),
                pstart: u64::from_str_radix(&cap["pstart"], 16).unwrap(),
                starttime: i64::from_str_radix(&cap["starttime"], 16).unwrap(),
                task_id: usize::from_str_radix(&cap["task_id"], 16).unwrap(),
                worker_type: cap["wtype"].to_string(),
                worker_id,
                auth_id: cap["authid"].to_string(),
                node: cap["node"].to_string(),
            })
        } else {
            bail!("unable to parse UPID '{}'", s);
        }
    }
}

impl std::fmt::Display for UPID {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let wid = if let Some(ref id) = self.worker_id {
            escape_id(id)
        } else {
            String::new()
        };

        // Note: pstart can be > 32bit if uptime > 497 days, so this can result in
        // more that 8 characters for pstart

        write!(
            f,
            "UPID:{}:{:08X}:{:08X}:{:08X}:{:08X}:{}:{}:{}:",
            self.node,
            self.pid,
            self.pstart,
            self.task_id,
            self.starttime,
            self.worker_type,
            wid,
            self.auth_id
        )
    }
}

impl serde::Serialize for UPID {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::ser::Serializer,
    {
        serializer.serialize_str(&ToString::to_string(self))
    }
}

impl<'de> serde::Deserialize<'de> for UPID {
    fn deserialize<D>(deserializer: D) -> Result<UPID, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct ForwardToStrVisitor;

        impl<'a> serde::de::Visitor<'a> for ForwardToStrVisitor {
            type Value = UPID;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a valid UPID")
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<UPID, E> {
                v.parse::<UPID>().map_err(|_| {
                    serde::de::Error::invalid_value(serde::de::Unexpected::Str(v), &self)
                })
            }
        }

        deserializer.deserialize_str(ForwardToStrVisitor)
    }
}

// the following two are copied as they're the only `proxmox-systemd` dependencies in this crate,
// and this crate has MUCH fewer dependencies without it
/// non-path systemd-unit compatible escaping
fn escape_id(unit: &str) -> String {
    use std::fmt::Write;

    let mut escaped = String::new();

    for (i, &c) in unit.as_bytes().iter().enumerate() {
        if c == b'/' {
            escaped.push('-');
        } else if (i == 0 && c == b'.')
            || !matches!(c, b'_' | b'.' | b'0'..=b'9' | b'a'..=b'z' | b'A'..=b'Z')
        {
            // unwrap: writing to a String
            write!(escaped, "\\x{:02x}", c).unwrap();
        } else {
            escaped.push(char::from(c));
        }
    }

    escaped
}

fn hex_digit(d: u8) -> Result<u8, Error> {
    match d {
        b'0'..=b'9' => Ok(d - b'0'),
        b'A'..=b'F' => Ok(d - b'A' + 10),
        b'a'..=b'f' => Ok(d - b'a' + 10),
        _ => bail!("got invalid hex digit"),
    }
}

/// systemd-unit compatible escaping
fn unescape_id(text: &str) -> Result<String, Error> {
    let mut i = text.as_bytes();

    let mut data: Vec<u8> = Vec::new();

    loop {
        if i.is_empty() {
            break;
        }
        let next = i[0];
        if next == b'\\' {
            if i.len() < 4 || i[1] != b'x' {
                bail!("error in escape sequence");
            }
            let h1 = hex_digit(i[2])?;
            let h0 = hex_digit(i[3])?;
            data.push(h1 << 4 | h0);
            i = &i[4..]
        } else if next == b'-' {
            data.push(b'/');
            i = &i[1..]
        } else {
            data.push(next);
            i = &i[1..]
        }
    }

    let text = String::from_utf8(data)?;

    Ok(text)
}

#[cfg(feature = "upid-api-impl")]
mod upid_impl {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use anyhow::{bail, format_err, Error};

    use super::UPID;

    impl UPID {
        /// Create a new UPID
        pub fn new(
            worker_type: &str,
            worker_id: Option<String>,
            auth_id: String,
        ) -> Result<Self, Error> {
            let pid = unsafe { libc::getpid() };

            let bad: &[_] = &['/', ':', ' '];

            if worker_type.contains(bad) {
                bail!("illegal characters in worker type '{}'", worker_type);
            }

            if auth_id.contains(bad) {
                bail!("illegal characters in auth_id '{}'", auth_id);
            }

            static WORKER_TASK_NEXT_ID: AtomicUsize = AtomicUsize::new(0);

            let task_id = WORKER_TASK_NEXT_ID.fetch_add(1, Ordering::SeqCst);

            Ok(UPID {
                pid,
                pstart: get_pid_start(pid)?,
                starttime: epoch_i64(),
                task_id,
                worker_type: worker_type.to_owned(),
                worker_id,
                auth_id,
                node: nix::sys::utsname::uname()
                    .nodename()
                    .split('.')
                    .next()
                    .ok_or_else(|| format_err!("failed to get nodename from uname()"))?
                    .to_owned(),
            })
        }
    }

    fn get_pid_start(pid: libc::pid_t) -> Result<u64, Error> {
        let statstr = String::from_utf8(std::fs::read(format!("/proc/{}/stat", pid))?)?;
        let cmdend = statstr
            .rfind(')')
            .ok_or_else(|| format_err!("missing ')' in /proc/PID/stat"))?;
        let starttime = statstr[cmdend + 1..]
            .trim_start()
            .split_ascii_whitespace()
            .nth(19)
            .ok_or_else(|| format_err!("failed to find starttime in /proc/{}/stat", pid))?;
        starttime.parse().map_err(|err| {
            format_err!(
                "failed to parse starttime from /proc/{}/stat ({:?}): {}",
                pid,
                starttime,
                err,
            )
        })
    }

    // Copied as this is the only `proxmox-time` dependency in this crate
    // and this crate has MUCH fewer dependencies without it
    fn epoch_i64() -> i64 {
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
}
