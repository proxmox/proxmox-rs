use std::collections::HashSet;
use std::convert::TryFrom;
use std::fmt;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;
use std::sync::{LazyLock, RwLock};
use std::time::Instant;

use anyhow::{bail, format_err, Error};
use nix::unistd::Pid;
use serde::Serialize;

use crate::fs::{file_read_firstline, read_firstline};

pub mod mountinfo;
#[doc(inline)]
pub use mountinfo::MountInfo;

/// POSIX sysconf call
pub fn sysconf(name: i32) -> i64 {
    unsafe extern "C" {
        fn sysconf(name: i32) -> i64;
    }
    unsafe { sysconf(name) }
}

pub static CLOCK_TICKS: LazyLock<f64> = LazyLock::new(|| sysconf(libc::_SC_CLK_TCK) as f64);

/// Selected contents of the `/proc/PID/stat` file.
pub struct PidStat {
    pub pid: Pid,
    pub ppid: Pid,
    pub status: u8,
    pub utime: u64,
    pub stime: u64,
    pub num_threads: u64,
    pub starttime: u64,
    pub vsize: u64,
    pub rss: i64,
}

impl PidStat {
    /// Retrieve the `stat` file contents of a process.
    pub fn read_from_pid(pid: Pid) -> Result<Self, Error> {
        let stat = Self::parse(std::str::from_utf8(&std::fs::read(format!(
            "/proc/{pid}/stat"
        ))?)?)?;
        if stat.pid != pid {
            bail!(
                "unexpected pid for process: found pid {} in /proc/{}/stat",
                stat.pid.as_raw(),
                pid
            );
        }
        Ok(stat)
    }

    /// Parse the contents of a `/proc/PID/stat` file.
    pub fn parse(statstr: &str) -> Result<PidStat, Error> {
        // It starts with the pid followed by a '('.
        let cmdbeg = statstr
            .find('(')
            .ok_or_else(|| format_err!("missing '(' in /proc/PID/stat"))?;

        if !statstr[..=cmdbeg].ends_with(" (") {
            bail!("bad /proc/PID/stat line before the '('");
        }

        let pid: u32 = statstr[..(cmdbeg - 1)]
            .parse()
            .map_err(|e| format_err!("bad pid in /proc/PID/stat: {}", e))?;
        let pid = Pid::from_raw(pid as i32);

        // After the '(' we have an arbitrary command name, then ')' and the remaining values
        let cmdend = statstr
            .rfind(')')
            .ok_or_else(|| format_err!("missing ')' in /proc/PID/stat"))?;
        let mut parts = statstr[cmdend + 1..].trim_start().split_ascii_whitespace();

        // helpers:
        fn required<'a>(value: Option<&'a str>, what: &'static str) -> Result<&'a str, Error> {
            value.ok_or_else(|| format_err!("missing '{}' in /proc/PID/stat", what))
        }

        fn req_num<T>(value: Option<&str>, what: &'static str) -> Result<T, Error>
        where
            T: FromStr,
            <T as FromStr>::Err: Into<Error>,
        {
            required(value, what)?.parse::<T>().map_err(|e| e.into())
        }

        fn req_byte(value: Option<&str>, what: &'static str) -> Result<u8, Error> {
            let value = required(value, what)?;
            if value.len() != 1 {
                bail!("invalid '{}' in /proc/PID/stat", what);
            }
            Ok(value.as_bytes()[0])
        }

        let out = PidStat {
            pid,
            status: req_byte(parts.next(), "status")?,
            ppid: Pid::from_raw(req_num::<u32>(parts.next(), "ppid")? as i32),
            utime: req_num::<u64>(parts.nth(9), "utime")?,
            stime: req_num::<u64>(parts.next(), "stime")?,
            num_threads: req_num::<u64>(parts.nth(4), "num_threads")?,
            starttime: req_num::<u64>(parts.nth(1), "start_time")?,
            vsize: req_num::<u64>(parts.next(), "vsize")?,
            rss: req_num::<i64>(parts.next(), "rss")? * 4096,
        };

        let _ = req_num::<u64>(parts.next(), "it_real_value")?;
        // and more...

        Ok(out)
    }
}

impl TryFrom<Pid> for PidStat {
    type Error = Error;

    fn try_from(pid: Pid) -> Result<Self, Error> {
        Self::read_from_pid(pid)
    }
}

#[test]
fn test_read_proc_pid_stat() {
    let stat = PidStat::parse(
        "28900 (zsh) S 22489 28900 28900 34826 10252 4194304 6851 5946551 0 2344 6 3 25205 1413 \
         20 0 1 0 287592 12496896 1910 18446744073709551615 93999319244800 93999319938061 \
         140722897984224 0 0 0 2 3686404 134295555 1 0 0 17 10 0 0 0 0 0 93999320079088 \
         93999320108360 93999343271936 140722897992565 140722897992570 140722897992570 \
         140722897993707 0",
    )
    .expect("successful parsing of a sample /proc/PID/stat entry");
    assert_eq!(stat.pid, Pid::from_raw(28900));
    assert_eq!(stat.ppid, Pid::from_raw(22489));
    assert_eq!(stat.status, b'S');
    assert_eq!(stat.utime, 6);
    assert_eq!(stat.stime, 3);
    assert_eq!(stat.num_threads, 1);
    assert_eq!(stat.starttime, 287592);
    assert_eq!(stat.vsize, 12496896);
    assert_eq!(stat.rss, 1910 * 4096);
}

pub fn check_process_running(pid: libc::pid_t) -> Option<PidStat> {
    PidStat::read_from_pid(Pid::from_raw(pid))
        .ok()
        .filter(|stat| stat.status != b'Z')
}

pub fn check_process_running_pstart(pid: libc::pid_t, pstart: u64) -> Option<PidStat> {
    if let Some(info) = check_process_running(pid) {
        if info.starttime == pstart {
            return Some(info);
        }
    }
    None
}

pub fn read_proc_uptime() -> Result<(f64, f64), Error> {
    let path = "/proc/uptime";
    let line = file_read_firstline(path)?;
    let mut values = line.split_whitespace().map(|v| v.parse::<f64>());

    match (values.next(), values.next()) {
        (Some(Ok(up)), Some(Ok(idle))) => Ok((up, idle)),
        _ => bail!("Error while parsing '{}'", path),
    }
}

pub fn read_proc_uptime_ticks() -> Result<(u64, u64), Error> {
    let (mut up, mut idle) = read_proc_uptime()?;
    up *= *CLOCK_TICKS;
    idle *= *CLOCK_TICKS;
    Ok((up as u64, idle as u64))
}

#[derive(Debug, Default, Serialize)]
/// The CPU fields from `/proc/stat` with their native time value. Multiply
/// with CLOCK_TICKS to get the real value.
pub struct ProcFsStat {
    /// Time spent in user mode.
    pub user: u64,
    /// Time spent in user mode with low priority (nice).
    pub nice: u64,
    /// Time spent in system mode.
    pub system: u64,
    /// Time spent in the  idle  task.
    pub idle: u64,
    /// Time waiting for I/O to complete.  This value is not reiable, see `man 5 proc`
    pub iowait: u64,
    /// Time servicing interrupts.
    pub irq: u64,
    /// Time servicing softirqs.
    pub softirq: u64,
    /// Stolen time, which is the time spent in other operating systems when running
    /// in a virtualized environment.
    pub steal: u64,
    /// Time spent running a virtual  CPU  for  guest  operating systems under the control of the
    /// Linux kernel.
    pub guest: u64,
    /// Time spent running a niced guest (virtual CPU for guest operating systems under the control
    /// of the Linux kernel).
    pub guest_nice: u64,
    /// The sum of all other u64  fields
    pub total: u64,
    /// The percentage (0 - 1.0) of cpu utilization from the whole system, basica underlying calculation
    /// `1 - (idle / total)` but with delta values between now and the last call to `read_proc_stat` (min. 1s interval)
    pub cpu: f64,
    /// The number of cpus listed in `/proc/stat`.
    pub cpu_count: u32,
    /// The percentage (0 - 1.0) of system wide iowait.
    pub iowait_percent: f64,
}

static PROC_LAST_STAT: LazyLock<RwLock<(ProcFsStat, Instant, bool)>> =
    LazyLock::new(|| RwLock::new((ProcFsStat::default(), Instant::now(), true)));

/// reads `/proc/stat`. For now only total host CPU usage is handled as the
/// other metrics are not really interesting
pub fn read_proc_stat() -> Result<ProcFsStat, Error> {
    let sample_time = Instant::now();
    let update_duration;
    let mut stat = {
        let bytes = std::fs::read("/proc/stat")?;
        parse_proc_stat(unsafe { std::str::from_utf8_unchecked(&bytes) }).unwrap()
    };

    {
        // read lock scope
        let prev_read_guarded = PROC_LAST_STAT.read().unwrap();
        let (prev_stat, prev_time, first_time) = &*prev_read_guarded;
        update_duration = sample_time.saturating_duration_since(*prev_time);
        // only update if data is old
        if update_duration.as_millis() < 1000 && !first_time {
            stat.cpu = prev_stat.cpu;
            stat.iowait_percent = prev_stat.iowait_percent;
            return Ok(stat);
        }
    }

    {
        let delta_seconds =
            (update_duration.as_secs() as f64) * *CLOCK_TICKS * (stat.cpu_count as f64);

        // write lock scope
        let mut prev_write_guarded = PROC_LAST_STAT.write().unwrap();
        // we do not expect much lock contention, so sample_time should be
        // recent. Else, we'd need to reread & parse here to get current data
        let (prev_stat, prev_time, first_time) = &mut *prev_write_guarded;

        let delta_total = stat.total - prev_stat.total;
        let delta_idle = stat.idle - prev_stat.idle;

        stat.cpu = 1. - (delta_idle as f64) / (delta_total as f64);

        if !*first_time {
            let delta_iowait = ((stat.iowait - prev_stat.iowait) as f64).min(delta_seconds);
            stat.iowait_percent = delta_iowait / delta_seconds;
        }

        *prev_stat = ProcFsStat { ..stat };
        *prev_time = sample_time;
        *first_time = false;
    }

    Ok(stat)
}

fn parse_proc_stat(statstr: &str) -> Result<ProcFsStat, Error> {
    let mut cpu_count = 0u32;
    let mut data = None;
    for line in statstr.lines() {
        let mut parts = line.trim_start().split_ascii_whitespace();
        match parts.next() {
            None => continue,
            Some("cpu") => data = Some(parse_proc_stat_cpu_line(parts)?),
            Some(key) if key.starts_with("cpu") => cpu_count += 1,
            _ => (),
        }
    }

    match data {
        None => bail!("failed to find 'cpu' line in /proc/stat"),
        Some(mut data) => {
            data.cpu_count = cpu_count;
            Ok(data)
        }
    }
}

fn parse_proc_stat_cpu_line<'a>(
    mut parts: impl Iterator<Item = &'a str>,
) -> Result<ProcFsStat, Error> {
    // helpers:
    fn required<'a>(value: Option<&'a str>, what: &'static str) -> Result<&'a str, Error> {
        value.ok_or_else(|| format_err!("missing '{}' in /proc/stat", what))
    }

    fn req_num<T>(value: Option<&str>, what: &'static str) -> Result<T, Error>
    where
        T: FromStr,
        <T as FromStr>::Err: std::fmt::Display,
    {
        required(value, what)?
            .parse::<T>()
            .map_err(|e| format_err!("error parsing {}: {}", what, e))
    }

    let mut stat = ProcFsStat {
        user: req_num::<u64>(parts.next(), "user")?,
        nice: req_num::<u64>(parts.next(), "nice")?,
        system: req_num::<u64>(parts.next(), "system")?,
        idle: req_num::<u64>(parts.next(), "idle")?,
        iowait: req_num::<u64>(parts.next(), "iowait")?,
        irq: req_num::<u64>(parts.next(), "irq")?,
        softirq: req_num::<u64>(parts.next(), "softirq")?,
        steal: req_num::<u64>(parts.next(), "steal")?,
        guest: req_num::<u64>(parts.next(), "guest")?,
        guest_nice: req_num::<u64>(parts.next(), "guest_nice")?,
        total: 0,
        cpu: 0.0,
        cpu_count: 0,
        iowait_percent: 0.0,
    };
    stat.total = stat.user
        + stat.nice
        + stat.system
        + stat.iowait
        + stat.irq
        + stat.softirq
        + stat.steal
        + stat.idle;

    // returns avg. heuristic for the first request
    stat.cpu = 1. - (stat.idle as f64) / (stat.total as f64);

    Ok(stat)
}

#[test]
fn test_read_proc_stat() {
    let stat = parse_proc_stat(
        "cpu  2845612 241 173179 264715515 93366 0 7925 141017 0 0\n\
         cpu0 174375 9 11367 16548335 5741 0 2394 8500 0 0\n\
         cpu1 183367 11 11423 16540656 4644 0 1235 8888 0 0\n\
         cpu2 179636 21 20463 16534540 4802 0 456 9270 0 0\n\
         cpu3 184560 9 11379 16532136 7113 0 225 8967 0 0\n\
         cpu4 182341 17 10277 16542865 3274 0 181 8461 0 0\n\
         cpu5 179771 22 9910 16548859 2259 0 112 8328 0 0\n\
         cpu6 181185 14 8933 16548550 2057 0 78 8313 0 0\n\
         cpu7 176326 12 8514 16553428 2246 0 76 8812 0 0\n\
         cpu8 177341 13 7942 16553880 1576 0 56 8565 0 0\n\
         cpu9 176883 10 8648 16547886 3067 0 103 8788 0 0\n\
         cpu10 169446 4 7993 16561700 1584 0 39 8797 0 0\n\
         cpu11 170878 7 7783 16560870 1526 0 23 8444 0 0\n\
         cpu12 164062 12 7839 16567155 1686 0 43 8794 0 0\n\
         cpu13 164303 4 7661 16567497 1528 0 41 8525 0 0\n\
         cpu14 173709 2 11478 16546352 3965 0 571 9414 0 0\n\
         cpu15 207422 67 21561 16460798 46292 0 2283 10142 0 0\n\
         intr 29200426 1 9 0 0 0 0 3 0 1 0 275744 40 16 0 0 166292 0 0 0 0 0 0 0 0 0 0 \
         0 1463843 0 1048751 328317 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 \
         0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 \
         0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 \
         0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 \
         0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 \
         0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 \
         0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 \
         0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 \
         0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 \
         0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 \
         0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 \
         0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 \
         0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 \
         0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0\n\
         ctxt 27543372\n\
         btime 1576502436\n\
         processes 701089\n\
         procs_running 2\n\
         procs_blocked 0\n\
         softirq 36227960 0 16653965 39 1305264 1500573 0 38330 5024204 356 11705229",
    )
    .expect("successful parsed a sample /proc/stat entry");
    assert_eq!(stat.user, 2845612);
    assert_eq!(stat.nice, 241);
    assert_eq!(stat.system, 173179);
    assert_eq!(stat.idle, 264715515);
    assert_eq!(stat.iowait, 93366);
    assert_eq!(stat.irq, 0);
    assert_eq!(stat.softirq, 7925);
    assert_eq!(stat.steal, 141017);
    assert_eq!(stat.guest, 0);
    assert_eq!(stat.guest_nice, 0);
    assert_eq!(stat.total, 267976855);
    assert_eq!(stat.cpu, 0.012170230149167183);
    assert_eq!(stat.cpu_count, 16);
    assert_eq!(stat.iowait_percent, 0.0);
}

#[derive(Debug, Serialize)]
/// Memory stats relevant for Proxmox projects.
///
/// NOTE: Unlike the name would suggest this is not a 1:1 representation of /proc/meminfo and also
/// includes some other metrics like KSM shared pages gathered from sysfs.
/// Ensure this is what you want if you return this type from your code for others to consume.
pub struct ProcFsMemInfo {
    /// Total usable RAM, i.e. installed memory minus bad regions and kernel binary code.
    pub memtotal: u64,
    /// Memory that's guaranteed completely unused, almost always quite a bit smaller than the
    /// amount of memory actually available for programs.
    pub memfree: u64,
    /// The amount of memory that is available for a new workload, without pushing the system into
    /// swap.
    pub memavailable: u64,
    /// Calculated from subtracting MemAvailable from MemTotal.
    pub memused: u64,
    /// Memory shared through the Kernel Same-Page Merging mechanism. Metric gathered from sysfs.
    pub memshared: u64,
    /// Total amount of swap space available.
    pub swaptotal: u64,
    //// Amount of swap space that is currently unused.
    pub swapfree: u64,
    /// Used swap, i.e., swaptotal - swapfree.
    pub swapused: u64,
}

pub fn read_meminfo() -> Result<ProcFsMemInfo, Error> {
    let path = "/proc/meminfo";

    let meminfo_str = std::fs::read_to_string(path)?;
    parse_proc_meminfo(&meminfo_str)
}

fn parse_proc_meminfo(text: &str) -> Result<ProcFsMemInfo, Error> {
    let mut meminfo = ProcFsMemInfo {
        memtotal: 0,
        memfree: 0,
        memavailable: 0,
        memused: 0,
        memshared: 0,
        swaptotal: 0,
        swapfree: 0,
        swapused: 0,
    };

    for line in text.lines() {
        let mut content_iter = line.split_whitespace();
        if let (Some(key), Some(value)) = (content_iter.next(), content_iter.next()) {
            match key {
                "MemTotal:" => meminfo.memtotal = value.parse::<u64>()? * 1024,
                "MemFree:" => meminfo.memfree = value.parse::<u64>()? * 1024,
                "MemAvailable:" => meminfo.memavailable = value.parse::<u64>()? * 1024,
                "SwapTotal:" => meminfo.swaptotal = value.parse::<u64>()? * 1024,
                "SwapFree:" => meminfo.swapfree = value.parse::<u64>()? * 1024,
                _ => continue,
            }
        }
    }

    // NOTE: MemAvailable is the only metric that will actually represent how much memory is
    // available for a new workload, without pushing the system into swap, no amount of calculating
    // with BUFFER, CACHE, .. will get you there, only the kernel can know this.
    // For details see https://git.kernel.org/torvalds/c/34e431b0ae398fc54ea69ff85ec700722c9da773
    meminfo.memused = meminfo.memtotal - meminfo.memavailable;

    meminfo.swapused = meminfo.swaptotal - meminfo.swapfree;

    meminfo.memshared = match read_firstline("/sys/kernel/mm/ksm/pages_sharing") {
        Ok(spages_line) => spages_line.trim_end().parse::<u64>()? * 4096,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => 0,
        Err(err) => bail!("unable to get KSM pages_sharing - {err}"),
    };

    Ok(meminfo)
}

#[test]
fn test_read_proc_meminfo() {
    let meminfo = parse_proc_meminfo(
        "MemTotal:       32752584 kB
MemFree:         2106048 kB
MemAvailable:   13301592 kB
Buffers:               0 kB
Cached:           490072 kB
SwapCached:            0 kB
Active:           658700 kB
Inactive:          59528 kB
Active(anon):     191996 kB
Inactive(anon):    49880 kB
Active(file):     466704 kB
Inactive(file):     9648 kB
Unevictable:       16008 kB
Mlocked:           12936 kB
SwapTotal:             3 kB
SwapFree:              2 kB
Zswap:                 0 kB
Zswapped:              0 kB
Dirty:                 0 kB
Writeback:             0 kB
AnonPages:        244204 kB
Mapped:            66032 kB
Shmem:              9960 kB
KReclaimable:   11525744 kB
Slab:           21002876 kB
SReclaimable:   11525744 kB
SUnreclaim:      9477132 kB
KernelStack:        6816 kB
PageTables:         4812 kB
SecPageTables:         0 kB
NFS_Unstable:          0 kB
Bounce:                0 kB
WritebackTmp:          0 kB
CommitLimit:    16376292 kB
Committed_AS:     316368 kB
VmallocTotal:   34359738367 kB
VmallocUsed:      983836 kB
VmallocChunk:          0 kB
Percpu:            12096 kB
HardwareCorrupted:     0 kB
AnonHugePages:         0 kB
ShmemHugePages:        0 kB
ShmemPmdMapped:        0 kB
FileHugePages:         0 kB
FilePmdMapped:         0 kB
Unaccepted:            0 kB
HugePages_Total:       0
HugePages_Free:        0
HugePages_Rsvd:        0
HugePages_Surp:        0
Hugepagesize:       2048 kB
Hugetlb:               0 kB
DirectMap4k:      237284 kB
DirectMap2M:    13281280 kB
DirectMap1G:    22020096 kB
",
    )
    .expect("successful parsed a sample /proc/meminfo entry");

    assert_eq!(meminfo.memtotal, 33538646016);
    assert_eq!(meminfo.memused, 19917815808);
    assert_eq!(meminfo.memfree, 2156593152);
    assert_eq!(meminfo.memavailable, 13620830208);
    assert_eq!(meminfo.swapfree, 2048);
    assert_eq!(meminfo.swaptotal, 3072);
    assert_eq!(meminfo.swapused, 1024);
}

#[derive(Clone, Debug)]
pub struct ProcFsCPUInfo {
    pub user_hz: f64,
    pub mhz: f64,
    pub model: String,
    pub hvm: bool,
    pub sockets: usize,
    pub cpus: usize,
}

static CPU_INFO: Option<ProcFsCPUInfo> = None;

pub fn read_cpuinfo() -> Result<ProcFsCPUInfo, Error> {
    if let Some(cpu_info) = &CPU_INFO {
        return Ok(cpu_info.clone());
    }

    let path = "/proc/cpuinfo";
    let file = OpenOptions::new().read(true).open(path)?;

    let mut cpuinfo = ProcFsCPUInfo {
        user_hz: *CLOCK_TICKS,
        mhz: 0.0,
        model: String::new(),
        hvm: false,
        sockets: 0,
        cpus: 0,
    };

    let mut socket_ids = HashSet::new();
    for line in BufReader::new(&file).lines() {
        let content = line?;
        if content.is_empty() {
            continue;
        }
        let mut content_iter = content.split(':');
        match (content_iter.next(), content_iter.next()) {
            (Some(key), Some(value)) => match key.trim_end() {
                "processor" => cpuinfo.cpus += 1,
                "model name" => cpuinfo.model = value.trim().to_string(),
                "cpu MHz" => cpuinfo.mhz = value.trim().parse::<f64>()?,
                "flags" => cpuinfo.hvm = value.contains(" vmx ") || value.contains(" svm "),
                "physical id" => {
                    let id = value.trim().parse::<u8>()?;
                    socket_ids.insert(id);
                }
                _ => continue,
            },
            _ => bail!("Error while parsing '{}'", path),
        }
    }
    cpuinfo.sockets = socket_ids.len();

    Ok(cpuinfo)
}

#[derive(Debug)]
pub struct ProcFsMemUsage {
    pub size: u64,
    pub resident: u64,
    pub shared: u64,
}

pub fn read_memory_usage() -> Result<ProcFsMemUsage, Error> {
    let path = format!("/proc/{}/statm", std::process::id());
    let line = file_read_firstline(&path)?;
    let mut values = line.split_whitespace().map(|v| v.parse::<u64>());

    let ps = 4096;
    match (values.next(), values.next(), values.next()) {
        (Some(Ok(size)), Some(Ok(resident)), Some(Ok(shared))) => Ok(ProcFsMemUsage {
            size: size * ps,
            resident: resident * ps,
            shared: shared * ps,
        }),
        _ => bail!("Error while parsing '{}'", path),
    }
}

#[derive(Debug, Serialize)]
pub struct ProcFsNetDev {
    pub device: String,
    pub receive: u64,
    pub send: u64,
}

pub fn read_proc_net_dev() -> Result<Vec<ProcFsNetDev>, Error> {
    let path = "/proc/net/dev";
    let file = OpenOptions::new().read(true).open(path)?;

    let mut result = Vec::new();
    for line in BufReader::new(&file).lines().skip(2) {
        let content = line?;
        let mut iter = content.split_whitespace();
        match (iter.next(), iter.next(), iter.nth(7)) {
            (Some(device), Some(receive), Some(send)) => {
                result.push(ProcFsNetDev {
                    device: device[..device.len() - 1].to_string(),
                    receive: receive.parse::<u64>()?,
                    send: send.parse::<u64>()?,
                });
            }
            _ => bail!("Error while parsing '{}'", path),
        }
    }

    Ok(result)
}

// Parse a hexadecimal digit into a byte.
#[inline]
fn hex_nibble(c: u8) -> Result<u8, Error> {
    Ok(match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'f' => c - b'a' + 0xa,
        b'A'..=b'F' => c - b'A' + 0xa,
        _ => bail!("not a hex digit: {}", c as char),
    })
}

fn hexstr_to_ipv4addr<T: AsRef<[u8]>>(hex: T) -> Result<Ipv4Addr, Error> {
    let hex = hex.as_ref();
    if hex.len() != 8 {
        bail!("Error while converting hex string to IPv4 address: unexpected string length");
    }

    let mut addr = [0u8; 4];
    for i in 0..4 {
        addr[3 - i] = (hex_nibble(hex[i * 2])? << 4) + hex_nibble(hex[i * 2 + 1])?;
    }

    Ok(Ipv4Addr::from(addr))
}

#[derive(Debug)]
pub struct ProcFsNetRoute {
    pub dest: Ipv4Addr,
    pub gateway: Ipv4Addr,
    pub mask: Ipv4Addr,
    pub metric: u32,
    pub mtu: u32,
    pub iface: String,
}

pub fn read_proc_net_route() -> Result<Vec<ProcFsNetRoute>, Error> {
    let path = "/proc/net/route";
    let file = OpenOptions::new().read(true).open(path)?;

    let mut result = Vec::new();
    for line in BufReader::new(&file).lines().skip(1) {
        let content = line?;
        if content.is_empty() {
            continue;
        }
        let mut iter = content.split_whitespace();

        let mut next = || {
            iter.next()
                .ok_or_else(|| format_err!("Error while parsing '{}'", path))
        };

        let (iface, dest, gateway) = (next()?, next()?, next()?);
        for _ in 0..3 {
            next()?;
        }
        let (metric, mask, mtu) = (next()?, next()?, next()?);

        result.push(ProcFsNetRoute {
            dest: hexstr_to_ipv4addr(dest)?,
            gateway: hexstr_to_ipv4addr(gateway)?,
            mask: hexstr_to_ipv4addr(mask)?,
            metric: metric.parse()?,
            mtu: mtu.parse()?,
            iface: iface.to_string(),
        });
    }

    Ok(result)
}

fn hexstr_to_ipv6addr<T: AsRef<[u8]>>(hex: T) -> Result<Ipv6Addr, Error> {
    let hex = hex.as_ref();
    if hex.len() != 32 {
        bail!("Error while converting hex string to IPv6 address: unexpected string length");
    }

    let mut addr = std::mem::MaybeUninit::<[u8; 16]>::uninit();
    let addr = unsafe {
        let ap = &mut *addr.as_mut_ptr();
        for i in 0..16 {
            ap[i] = (hex_nibble(hex[i * 2])? << 4) + hex_nibble(hex[i * 2 + 1])?;
        }
        addr.assume_init()
    };

    Ok(Ipv6Addr::from(addr))
}

fn hexstr_to_u8<T: AsRef<[u8]>>(hex: T) -> Result<u8, Error> {
    let hex = hex.as_ref();
    if hex.len() != 2 {
        bail!("Error while converting hex string to u8: unexpected string length");
    }

    Ok((hex_nibble(hex[0])? << 4) + hex_nibble(hex[1])?)
}

fn hexstr_to_u32<T: AsRef<[u8]>>(hex: T) -> Result<u32, Error> {
    let hex = hex.as_ref();
    if hex.len() != 8 {
        bail!("Error while converting hex string to u32: unexpected string length");
    }

    let mut bytes = [0u8; 4];
    for i in 0..4 {
        bytes[i] = (hex_nibble(hex[i * 2])? << 4) + hex_nibble(hex[i * 2 + 1])?;
    }

    Ok(u32::from_be_bytes(bytes))
}

#[derive(Debug)]
pub struct ProcFsNetIPv6Route {
    pub dest: Ipv6Addr,
    pub prefix: u8,
    pub gateway: Ipv6Addr,
    pub metric: u32,
    pub iface: String,
}

pub fn read_proc_net_ipv6_route() -> Result<Vec<ProcFsNetIPv6Route>, Error> {
    let path = "/proc/net/ipv6_route";
    let file = OpenOptions::new().read(true).open(path)?;

    let mut result = Vec::new();
    for line in BufReader::new(&file).lines() {
        let content = line?;
        if content.is_empty() {
            continue;
        }
        let mut iter = content.split_whitespace();

        let mut next = || {
            iter.next()
                .ok_or_else(|| format_err!("Error while parsing '{}'", path))
        };

        let (dest, prefix) = (next()?, next()?);
        for _ in 0..2 {
            next()?;
        }
        let (nexthop, metric) = (next()?, next()?);
        for _ in 0..3 {
            next()?;
        }
        let iface = next()?;

        result.push(ProcFsNetIPv6Route {
            dest: hexstr_to_ipv6addr(dest)?,
            prefix: hexstr_to_u8(prefix)?,
            gateway: hexstr_to_ipv6addr(nexthop)?,
            metric: hexstr_to_u32(metric)?,
            iface: iface.to_string(),
        });
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_proc_net_route() {
        read_proc_net_route().unwrap();
    }

    #[test]
    fn test_read_proc_net_ipv6_route() {
        read_proc_net_ipv6_route().unwrap();
    }
}

/// Read the load avage from `/proc/loadavg`.
pub fn read_loadavg() -> Result<Loadavg, Error> {
    Loadavg::read()
}

/// Load average: floating point values for 1, 5 and 15 minutes of runtime.
#[derive(Clone, Debug)]
pub struct Loadavg(pub f64, pub f64, pub f64);

impl Loadavg {
    /// Read the load avage from `/proc/loadavg`.
    pub fn read() -> Result<Self, Error> {
        let bytes = std::fs::read("/proc/loadavg")?;
        Self::parse(unsafe { std::str::from_utf8_unchecked(&bytes) })
    }

    /// Parse the value triplet.
    fn parse(line: &str) -> Result<Self, Error> {
        let mut parts = line.trim_start().split_ascii_whitespace();
        let missing = || format_err!("missing field in /proc/loadavg");
        let one: f64 = parts.next().ok_or_else(missing)?.parse()?;
        let five: f64 = parts.next().ok_or_else(missing)?.parse()?;
        let fifteen: f64 = parts.next().ok_or_else(missing)?.parse()?;
        Ok(Self(one, five, fifteen))
    }

    /// Named method for the one minute load average.
    pub fn one(&self) -> f64 {
        self.0
    }

    /// Named method for the five minute load average.
    pub fn five(&self) -> f64 {
        self.1
    }

    /// Named method for the fifteen minute load average.
    pub fn fifteen(&self) -> f64 {
        self.2
    }
}

impl fmt::Display for Loadavg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "({}, {}, {})", self.0, self.1, self.2)
    }
}

#[test]
fn test_loadavg() {
    let avg = Loadavg::parse("0.44 0.48 0.44 2/1062 18549").expect("loadavg parser failed");
    assert_eq!((avg.one() * 1000.0) as u64, 440u64);
    assert_eq!((avg.five() * 1000.0) as u64, 480u64);
    assert_eq!((avg.fifteen() * 1000.0) as u64, 440u64);
}
