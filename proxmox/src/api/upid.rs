use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{bail, Error};

use crate::api::schema::{ApiStringFormat, ApiType, Schema, StringSchema};
use crate::const_regex;
use crate::sys::linux::procfs;

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
    pub pid: libc::pid_t,
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

crate::forward_serialize_to_display!(UPID);
crate::forward_deserialize_to_from_str!(UPID);

const_regex! {
    pub PROXMOX_UPID_REGEX = concat!(
        r"^UPID:(?P<node>[a-zA-Z0-9]([a-zA-Z0-9\-]*[a-zA-Z0-9])?):(?P<pid>[0-9A-Fa-f]{8}):",
        r"(?P<pstart>[0-9A-Fa-f]{8,9}):(?P<task_id>[0-9A-Fa-f]{8,16}):(?P<starttime>[0-9A-Fa-f]{8}):",
        r"(?P<wtype>[^:\s]+):(?P<wid>[^:\s]*):(?P<authid>[^:\s]+):$"
    );
}

pub const PROXMOX_UPID_FORMAT: ApiStringFormat =
    ApiStringFormat::Pattern(&PROXMOX_UPID_REGEX);

pub const UPID_SCHEMA: Schema = StringSchema::new("Unique Process/Task Identifier")
    .min_length("UPID:N:12345678:12345678:12345678:::".len())
    .max_length(128) // arbitrary
    .format(&PROXMOX_UPID_FORMAT)
    .schema();

impl ApiType for UPID {
    const API_SCHEMA: Schema = UPID_SCHEMA;
}

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
            pstart: procfs::PidStat::read_from_pid(nix::unistd::Pid::from_raw(pid))?.starttime,
            starttime: crate::tools::time::epoch_i64(),
            task_id,
            worker_type: worker_type.to_owned(),
            worker_id,
            auth_id,
            node: crate::tools::nodename().to_owned(),
        })
    }
}


impl std::str::FromStr for UPID {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if let Some(cap) = PROXMOX_UPID_REGEX.captures(s) {

            let worker_id = if cap["wid"].is_empty() {
                None
            } else {
                let wid = crate::tools::systemd::unescape_unit(&cap["wid"])?;
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
            crate::tools::systemd::escape_unit(id, false)
        } else {
            String::new()
        };

        // Note: pstart can be > 32bit if uptime > 497 days, so this can result in
        // more that 8 characters for pstart

        write!(f, "UPID:{}:{:08X}:{:08X}:{:08X}:{:08X}:{}:{}:{}:",
               self.node, self.pid, self.pstart, self.task_id, self.starttime, self.worker_type, wid, self.auth_id)
    }
}
