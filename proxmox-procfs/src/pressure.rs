//! Utilities for reading [Pressure Stall Information][psi] for the system or cgroups.
//!
//! To read pressure data, refer to [`PressureData::read_system`] and [`PressureData::read_cgroup`].
//! [`PressureData::read_file`] can be use for lower-level access, proving the path to the
//! pressure file directly.
//!
//! # Examples
//!
//! Read system-wide CPU pressure:
//!
//! ```no_run
//! use proxmox_procfs::pressure::{PressureData, Resource};
//!
//! let cpu = PressureData::read_system(Resource::Cpu).unwrap();
//! println!("CPU some avg10: {:.2}%", cpu.some.average_10);
//! ```
//!
//! Read cgroup-level memory pressure:
//!
//! ```no_run
//! use proxmox_procfs::pressure::{PressureData, Resource};
//!
//! let mem = PressureData::read_cgroup("system.slice", Resource::Memory).unwrap();
//! println!("mem some avg10: {:.2}%", mem.some.average_10);
//! ```
//!
//! [psi]: https://docs.kernel.org/accounting/psi.html
//!

use std::ffi::OsStr;
use std::fs::File;
use std::io::{BufRead, BufReader, ErrorKind};
use std::path::{Path, PathBuf};
use std::str::FromStr;

#[derive(thiserror::Error, Debug)]
/// Error type for pressure-related errors.
pub enum Error {
    /// General IO error when reading the pressure stall information file.
    #[error("could not read pressure stall info file: {0}")]
    Io(#[from] std::io::Error),

    /// Pressure stall info file does not exist.
    /// This is a distinct error variant so that the caller can differentiate between a
    /// disappeared cgroup (e.g. if the guest was stopped) and other kinds of IO errors
    #[error("pressure stall info file does not exist: {0}")]
    NotFound(PathBuf),

    /// The contents of the pressure stall file are unexpected. Should not really happen,
    /// hopefully.
    #[error("unexpected pressure stall file format: {0}")]
    InvalidFormat(String),
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Debug)]
/// Pressure stall information data.
pub struct PressureData {
    /// At least some tasks were stalled on a given resource.
    pub some: PressureRecord,
    /// All non-idle tasks were stalled on a given resource.
    ///
    /// Note: When querying CPU pressure stall information on a system level,
    /// all members in `full` contain 0 (see [here]).
    ///
    /// [here]: https://docs.kernel.org/accounting/psi.html#pressure-interface
    pub full: PressureRecord,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "kebab-case"))]
#[derive(Clone, Debug)]
/// Individual record corresponding to one line from a pressure stall information file.
pub struct PressureRecord {
    /// Average pressure stall ratio over the last 10 seconds.
    pub average_10: f64,
    /// Average pressure stall ratio over the last 60 seconds.
    pub average_60: f64,
    /// Average pressure stall ratio over the last 300 seconds.
    pub average_300: f64,
    /// Total stall time in microseconds.
    pub total: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PressureRecordKind {
    Full,
    Some,
}

impl FromStr for PressureRecordKind {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "some" => Ok(Self::Some),
            "full" => Ok(Self::Full),
            _ => Err(Error::InvalidFormat(format!("invalid pressure kind '{s}'"))),
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Debug, PartialEq)]
/// Which pressure stall information to query.
pub enum Resource {
    /// Query CPU pressure stall information.
    Cpu,
    /// Query memory pressure stall information.
    Memory,
    /// Query IO pressure stall information.
    Io,
}

impl Resource {
    fn into_proc_path(self) -> &'static Path {
        match self {
            Resource::Cpu => Path::new("/proc/pressure/cpu"),
            Resource::Memory => Path::new("/proc/pressure/memory"),
            Resource::Io => Path::new("/proc/pressure/io"),
        }
    }

    fn into_cgroup_path_component(self) -> &'static OsStr {
        match self {
            Resource::Cpu => OsStr::new("cpu.pressure"),
            Resource::Memory => OsStr::new("memory.pressure"),
            Resource::Io => OsStr::new("io.pressure"),
        }
    }
}

impl PressureData {
    /// Read pressure stall information for the entire host from `/proc/pressure/*`.
    ///
    /// ```no_run
    /// use proxmox_procfs::pressure::*;
    ///
    /// let pressure = PressureData::read_system(Resource::Cpu).unwrap();
    /// println!("{}", pressure.some.average_10);
    ///
    ///```
    pub fn read_system(what: Resource) -> Result<PressureData, Error> {
        Self::read_file(what.into_proc_path())
    }

    /// Read pressure stall information for a cgroup.
    ///
    /// The `cgroup` parameter will be directly used to assemble the path for the PSI file. For
    /// instance, if set to `lxc/101`, then `/sys/fs/cgroup/lxc/101/cpu.pressure` will be read.
    ///
    /// Note: This functions will return [`Error::NotFound`] in case the pressure file does not exist,
    /// usually meaning that the cgroup does not exist (any more). This distinct error variant allows
    /// the caller to differentiate this case from other kinds of IO errors.
    ///
    /// ```no_run
    /// use proxmox_procfs::pressure::{PressureData, Resource};
    ///
    /// let pressure = PressureData::read_cgroup("qemu.slice/100.scope", Resource::Cpu).unwrap();
    /// println!("{}", pressure.some.average_10);
    ///
    /// let pressure = PressureData::read_cgroup("lxc/101", Resource::Io).unwrap();
    /// println!("{}", pressure.some.average_10);
    ///
    /// ```
    pub fn read_cgroup(cgroup: &str, resource: Resource) -> Result<PressureData, Error> {
        let path = Path::new("/sys/fs/cgroup/")
            .join(cgroup)
            .join(resource.into_cgroup_path_component());

        Self::read_file(&path)
    }

    /// Read pressure stall information from a provided path.
    ///
    /// ```no_run
    /// use proxmox_procfs::pressure::{PressureData, Resource};
    ///
    /// let pressure = PressureData::read_file("/proc/pressure/io").unwrap();
    /// println!("{}", pressure.some.average_10);
    ///
    /// ```
    pub fn read_file<P: AsRef<Path>>(path: P) -> Result<PressureData, Error> {
        let file = match File::open(path.as_ref()) {
            Ok(file) => file,
            Err(err) if err.kind() == ErrorKind::NotFound => {
                return Err(Error::NotFound(path.as_ref().into()))
            }
            Err(err) => return Err(Error::Io(err)),
        };

        let reader = BufReader::new(file);

        PressureData::read(reader)
    }

    fn read<R: BufRead>(mut reader: R) -> Result<Self, Error> {
        // Depending on the length of the 'total' field, one line in the pressure output is around
        // 60 characters long. Pre-alloc roughly double the size to pretty much eliminate the need
        // for ever having to resize the vec.
        let mut buf = String::with_capacity(128);

        let (some_kind, some) = Self::read_pressure_line(&mut reader, &mut buf)?;
        buf.clear();

        let (full_kind, full) = Self::read_pressure_line(&mut reader, &mut buf)?;

        if some_kind != PressureRecordKind::Some || full_kind != PressureRecordKind::Full {
            return Err(Error::InvalidFormat(
                "unexpected pressure record structure".into(),
            ));
        }

        Ok(PressureData { some, full })
    }

    fn read_pressure_line<R: BufRead>(
        reader: &mut R,
        buf: &mut String,
    ) -> Result<(PressureRecordKind, PressureRecord), Error> {
        // The buffer should be empty. It is only passed by the caller as a performance
        // optimization
        debug_assert!(buf.is_empty());

        reader.read_line(buf)?;

        Self::read_record(buf)
    }

    fn read_record(line: &str) -> Result<(PressureRecordKind, PressureRecord), Error> {
        let mut iter = line.split_ascii_whitespace();

        let kind = iter
            .next()
            .ok_or_else(|| Error::InvalidFormat("missing pressure kind field".into()))
            .and_then(PressureRecordKind::from_str)?;

        let average_10 = Self::parse_field(iter.next(), "avg10=")?;
        let average_60 = Self::parse_field(iter.next(), "avg60=")?;
        let average_300 = Self::parse_field(iter.next(), "avg300=")?;
        let total = Self::parse_field(iter.next(), "total=")?;

        Ok((
            kind,
            PressureRecord {
                average_10,
                average_60,
                average_300,
                total,
            },
        ))
    }

    fn parse_field<T: FromStr>(s: Option<&str>, prefix: &str) -> Result<T, Error>
    where
        <T as FromStr>::Err: std::fmt::Display,
    {
        s.and_then(|s| s.strip_prefix(prefix))
            .ok_or_else(|| {
                Error::InvalidFormat(format!("expected '{prefix}' prefix for next field"))
            })?
            .parse()
            .map_err(|err| Error::InvalidFormat(format!("failed to parse '{prefix}': {err}")))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_read_psi() {
        let s = "some avg10=1.42 avg60=2.09 avg300=1.42 total=40979658
full avg10=0.08 avg60=0.18 avg300=0.13 total=22865313
";

        let mut reader = std::io::Cursor::new(s);
        let stats = PressureData::read(&mut reader).unwrap();

        assert_eq!(stats.some.total, 40979658);
        assert!((stats.some.average_10 - 1.42).abs() < f64::EPSILON);
        assert!((stats.some.average_60 - 2.09).abs() < f64::EPSILON);
        assert!((stats.some.average_300 - 1.42).abs() < f64::EPSILON);

        assert_eq!(stats.full.total, 22865313);
        assert!((stats.full.average_10 - 0.08).abs() < f64::EPSILON);
        assert!((stats.full.average_60 - 0.18).abs() < f64::EPSILON);
        assert!((stats.full.average_300 - 0.13).abs() < f64::EPSILON);
    }

    #[test]
    fn test_read_error() {
        let s = "invalid avg10=1.42 avg60=2.09 avg300=1.42 total=40979658
full avg10=0.08 avg60=0.18 avg300=0.13 total=22865313
";

        let mut reader = std::io::Cursor::new(s);
        assert!(PressureData::read(&mut reader).is_err());
    }

    #[test]
    fn test_invalid_field() {
        let s = "some foo=1.42 avg60=2.09 avg300=1.42 total=40979658
full avg10=0.08 avg60=0.18 avg300=0.13 total=22865313
";

        let mut reader = std::io::Cursor::new(s);
        assert!(PressureData::read(&mut reader).is_err());
    }

    #[test]
    fn test_read_system_pressure() {
        for resource in [Resource::Io, Resource::Memory, Resource::Cpu] {
            PressureData::read_system(resource).unwrap();
        }
    }

    #[test]
    fn test_read_cgroup_pressure() {
        for resource in [Resource::Io, Resource::Memory, Resource::Cpu] {
            PressureData::read_cgroup("system.slice", resource).unwrap();
        }
    }

    #[test]
    fn test_read_file_notfound() {
        assert!(matches!(
            PressureData::read_file("/invalid"),
            Err(Error::NotFound(_))
        ))
    }
}
