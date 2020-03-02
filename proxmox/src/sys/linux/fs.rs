//! File system syscall wrappers

// /usr/include/linux/fs.h: #define FS_IOC_GETFLAGS _IOR('f', 1, long)
/// Read Linux file system attributes (see man chattr)
nix::ioctl_read!(read_attr_fd, b'f', 1, usize);

// /usr/include/linux/msdos_fs.h: #define FAT_IOCTL_GET_ATTRIBUTES _IOR('r', 0x10, __u32)
/// Read FAT file system attributes
nix::ioctl_read!(read_fat_attr_fd, b'r', 0x10, u32);

// From /usr/include/linux/fs.h
// #define FS_IOC_FSGETXATTR _IOR('X', 31, struct fsxattr)
nix::ioctl_read!(fs_ioc_fsgetxattr, b'X', 31, FSXAttr);
// #define FS_IOC_FSSETXATTR _IOW('X', 32, struct fsxattr)
nix::ioctl_write_ptr!(fs_ioc_fssetxattr, b'X', 32, FSXAttr);

/// Data structure for fsgetxattr and fssetxattr
#[repr(C)]
#[derive(Debug)]
pub struct FSXAttr {
    pub fsx_xflags: u32,
    pub fsx_extsize: u32,
    pub fsx_nextents: u32,
    pub fsx_projid: u32,
    pub fsx_cowextsize: u32,
    pub fsx_pad: [u8; 8],
}

impl Default for FSXAttr {
    fn default() -> Self {
        FSXAttr {
            fsx_xflags: 0u32,
            fsx_extsize: 0u32,
            fsx_nextents: 0u32,
            fsx_projid: 0u32,
            fsx_cowextsize: 0u32,
            fsx_pad: [0u8; 8],
        }
    }
}
