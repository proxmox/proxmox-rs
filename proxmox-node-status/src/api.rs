use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use anyhow::{bail, format_err, Error};

use proxmox_schema::api;
use proxmox_schema::api_types::NODE_SCHEMA;
use proxmox_sys::boot_mode;
use proxmox_sys::linux::procfs;

pub use crate::types::{
    BootModeInformation, KernelVersionInformation, NodeCpuInformation, NodeInformation,
    NodeMemoryCounters, NodePowerCommand, NodeStatus, NodeSwapCounters, StorageStatus,
};

static TLS_CERT_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn init_node_status_api<P: AsRef<Path>>(cert_path: P) -> Result<(), Error> {
    TLS_CERT_PATH
        .set(cert_path.as_ref().to_owned())
        .map_err(|_e| format_err!("cannot set certificate path twice!"))
}

fn procfs_to_node_cpu_info(info: procfs::ProcFsCPUInfo) -> NodeCpuInformation {
    NodeCpuInformation {
        model: info.model,
        sockets: info.sockets,
        cpus: info.cpus,
    }
}

fn boot_mode_to_info(bm: boot_mode::BootMode, sb: boot_mode::SecureBoot) -> BootModeInformation {
    use boot_mode::BootMode;
    use boot_mode::SecureBoot;

    match (bm, sb) {
        (BootMode::Efi, SecureBoot::Enabled) => BootModeInformation {
            mode: crate::types::BootMode::Efi,
            secureboot: true,
        },
        (BootMode::Efi, SecureBoot::Disabled) => BootModeInformation {
            mode: crate::types::BootMode::Efi,
            secureboot: false,
        },
        (BootMode::Bios, _) => BootModeInformation {
            mode: crate::types::BootMode::LegacyBios,
            secureboot: false,
        },
    }
}

fn certificate_fingerprint() -> Result<String, Error> {
    let cert_path = TLS_CERT_PATH.get().ok_or_else(|| {
        format_err!("certificate path needs to be set before calling node status endpoints")
    })?;
    let x509 = openssl::x509::X509::from_pem(&proxmox_sys::fs::file_get_contents(cert_path)?)?;
    let fp = x509.digest(openssl::hash::MessageDigest::sha256())?;

    Ok(hex::encode(fp)
        .as_bytes()
        .chunks(2)
        .map(|v| std::str::from_utf8(v).unwrap())
        .collect::<Vec<&str>>()
        .join(":"))
}

#[api(
    input: {
        properties: {
            node: {
                schema: NODE_SCHEMA,
            },
        },
    },
    returns: {
        type: NodeStatus,
    },
)]
/// Read node memory, CPU and (root) disk usage
pub async fn get_status() -> Result<NodeStatus, Error> {
    let meminfo: procfs::ProcFsMemInfo = procfs::read_meminfo()?;
    let memory = NodeMemoryCounters {
        total: meminfo.memtotal,
        used: meminfo.memused,
        free: meminfo.memfree,
    };

    let swap = NodeSwapCounters {
        total: meminfo.swaptotal,
        used: meminfo.swapused,
        free: meminfo.swapfree,
    };

    let kstat: procfs::ProcFsStat = procfs::read_proc_stat()?;
    let cpu = kstat.cpu;
    let wait = kstat.iowait_percent;

    let loadavg = procfs::Loadavg::read()?;
    let loadavg = [loadavg.one(), loadavg.five(), loadavg.fifteen()];

    let cpuinfo = procfs::read_cpuinfo()?;
    let cpuinfo = procfs_to_node_cpu_info(cpuinfo);

    let uname = nix::sys::utsname::uname()?;
    let kernel_version = KernelVersionInformation::from_uname_parts(
        uname.sysname(),
        uname.release(),
        uname.version(),
        uname.machine(),
    );

    let disk = tokio::task::spawn_blocking(move || proxmox_sys::fs::fs_info(c"/"))
        .await
        .map_err(|err| format_err!("error waiting for fs_info call: {err}"))??;

    let boot_info = boot_mode_to_info(boot_mode::BootMode::query(), boot_mode::SecureBoot::query());

    Ok(NodeStatus {
        memory,
        swap,
        root: StorageStatus {
            total: disk.total,
            used: disk.used,
            avail: disk.available,
        },
        uptime: procfs::read_proc_uptime()?.0 as u64,
        loadavg,
        kversion: kernel_version.get_legacy(),
        current_kernel: kernel_version,
        cpuinfo,
        cpu,
        wait,
        info: NodeInformation {
            fingerprint: certificate_fingerprint()?,
        },
        boot_info,
    })
}

#[api(
    protected: true,
    input: {
        properties: {
            node: {
                schema: NODE_SCHEMA,
            },
            command: {
                type: NodePowerCommand,
            },
        }
    },
)]
/// Reboot or shutdown the node.
pub fn reboot_or_shutdown(command: NodePowerCommand) -> Result<(), Error> {
    let systemctl_command = match command {
        NodePowerCommand::Reboot => "reboot",
        NodePowerCommand::Shutdown => "poweroff",
    };

    let output = Command::new("systemctl")
        .arg(systemctl_command)
        .output()
        .map_err(|err| format_err!("failed to execute systemctl - {err}"))?;

    if !output.status.success() {
        match output.status.code() {
            Some(code) => {
                let msg = String::from_utf8(output.stderr)
                    .map(|m| {
                        if m.is_empty() {
                            String::from("no error message")
                        } else {
                            m
                        }
                    })
                    .unwrap_or_else(|_| String::from("non utf8 error message (suppressed)"));
                bail!("command failed with status code: {code} - {msg}");
            }
            None => bail!("systemctl terminated by signal"),
        }
    }
    Ok(())
}
