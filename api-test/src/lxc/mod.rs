//! PVE LXC module

use std::collections::HashSet;

use proxmox::api::api;

use crate::schema::{
    types::{Memory, VolumeId},
    Architecture,
};

pub mod schema;

#[api({
    description: "The PVE side of an lxc container configuration.",
    cli: false,
    fields: {
        lock: "The current long-term lock held on this container by another operation.",
        onboot: {
            description: "Specifies whether a VM will be started during system bootup.",
            default: false,
        },
        startup: "The container's startup order.",
        template: {
            description: "Whether this is a template.",
            default: false,
        },
        arch: {
            description: "The container's architecture type.",
            default: Architecture::Amd64,
        },
        ostype: {
            description:
                 "OS type. This is used to setup configuration inside the container, \
                  and corresponds to lxc setup scripts in \
                  /usr/share/lxc/config/<ostype>.common.conf. \
                  Value 'unmanaged' can be used to skip and OS specific setup.",
        },
        console: {
            description: "Attach a console device (/dev/console) to the container.",
            default: true,
        },
        tty: {
            description: "Number of ttys available to the container",
            minimum: 0,
            maximum: 6,
            default: 2,
        },
        cores: {
            description:
                "The number of cores assigned to the container. \
                 A container can use all available cores by default.",
            minimum: 1,
            maximum: 128,
        },
        cpulimit: {
            description:
                "Limit of CPU usage.\
                 \n\n\
                 NOTE: If the computer has 2 CPUs, it has a total of '2' CPU time. \
                 Value '0' indicates no CPU limit.",
            minimum: 0,
            maximum: 128,
            default: 0,
        },
        cpuunits: {
            description:
                "CPU weight for a VM. Argument is used in the kernel fair scheduler. \
                 The larger the number is, the more CPU time this VM gets. \
                 Number is relative to the weights of all the other running VMs.\
                 \n\n\
                 NOTE: You can disable fair-scheduler configuration by setting this to 0.",
            minimum: 0,
            maximum: 500000,
            default: 1024,
        },
        memory: {
            description: "Amount of RAM for the VM.",
            minimum: Memory::from_mebibytes(16),
            default: Memory::from_mebibytes(512),
            serialization: crate::schema::memory::optional::Parser::<crate::schema::memory::Mb>
        },
        swap: {
            description: "Amount of SWAP for the VM.",
            minimum: Memory::from_bytes(0),
            default: Memory::from_mebibytes(512),
        },
        hostname: {
            description: "Set a host name for the container.",
            maximum_length: 255,
            minimum_length: 3,
            format: crate::schema::dns_name,
        },
        description: "Container description. Only used on the configuration web interface.",
        searchdomain: {
            description:
                "Sets DNS search domains for a container. Create will automatically use the \
                 setting from the host if you neither set searchdomain nor nameserver.",
            format: crate::schema::dns_name,
            serialization: crate::schema::string_list::optional,
        },
        nameserver: {
            description:
                "Sets DNS server IP address for a container. Create will automatically use the \
                 setting from the host if you neither set searchdomain nor nameserver.",
            format: crate::schema::ip_address,
            serialization: crate::schema::string_list::optional,
        },
        rootfs: "Container root volume",
        cmode: {
            description:
                "Console mode. By default, the console command tries to open a connection to one \
                 of the available tty devices. By setting cmode to 'console' it tries to attach \
                 to /dev/console instead. \
                 If you set cmode to 'shell', it simply invokes a shell inside the container \
                 (no login).",
            default: schema::ConsoleMode::Tty,
        },
        protection: {
            description:
                "Sets the protection flag of the container. \
                 This will prevent the CT or CT's disk remove/update operation.",
            default: false,
        },
        unprivileged: {
            description:
                "Makes the container run as unprivileged user. (Should not be modified manually.)",
            default: false,
        },
        hookscript: {
            description:
                "Script that will be exectued during various steps in the containers lifetime.",
        },
    },
})]
#[derive(Default)]
pub struct Config {
    // FIXME: short form? Since all the type info is literally factored out into the ConfigLock
    // type already...
    //#[api("The current long-term lock held on this container by another operation.")]
    pub lock: Option<schema::ConfigLock>,
    pub onboot: Option<bool>,
    pub startup: Option<crate::schema::StartupOrder>,
    pub template: Option<bool>,
    pub arch: Option<Architecture>,
    pub ostype: Option<schema::OsType>,
    pub console: Option<bool>,
    pub tty: Option<usize>,
    pub cores: Option<usize>,
    pub cpulimit: Option<usize>,
    pub cpuunits: Option<usize>,
    pub memory: Option<Memory>,
    pub swap: Option<Memory>,
    pub hostname: Option<String>,
    pub description: Option<String>,
    pub searchdomain: Option<Vec<String>>,
    pub nameserver: Option<Vec<String>>,
    pub rootfs: Option<Rootfs>,
    // pub parent: Option<String>,
    // pub snaptime: Option<usize>,
    pub cmode: Option<schema::ConsoleMode>,
    pub protection: Option<bool>,
    pub unprivileged: Option<bool>,
    // pub features: Option<schema::Features>,
    pub hookscript: Option<VolumeId>,
}

#[api({
    description: "Container's rootfs definition",
    cli: false,
    fields: {
        volume: {
            description: "Volume, device or directory to mount into the container.",
            format: crate::schema::safe_path,
            // format_description: 'volume',
            // default_key: 1,
        },
        size: {
            description: "Volume size (read only value).",
            // format_description: 'DiskSize',
        },
        acl: {
            description: "Explicitly enable or disable ACL support.",
            default: false,
        },
        ro: {
            description: "Read-only mount point.",
            default: false,
        },
        mountoptions: {
            description: "Extra mount options for rootfs/mps.",
            //format_description: "opt[;opt...]",
            format: schema::mount_options,
            serialization: crate::schema::string_set::optional,
        },
        quota: {
            description:
                "Enable user quotas inside the container (not supported with zfs subvolumes)",
            default: false,
        },
        replicate: {
            description: "Will include this volume to a storage replica job.",
            default: true,
        },
        shared: {
            description:
                "Mark this non-volume mount point as available on multiple nodes (see 'nodes')",
            //verbose_description:
            //    "Mark this non-volume mount point as available on all nodes.\n\
            //    \n\
            //    WARNING: This option does not share the mount point automatically, it assumes \
            //    it is shared already!",
            default: false,
        },
    },
})]
pub struct Rootfs {
    pub volume: String,
    pub size: Option<Memory>,
    pub acl: Option<bool>,
    pub ro: Option<bool>,
    pub mountoptions: Option<HashSet<String>>,
    pub quota: Option<bool>,
    pub replicate: Option<bool>,
    pub shared: Option<bool>,
}
