use std::fs::OpenOptions;
use std::os::unix::io::AsRawFd;
use std::path::PathBuf;

use nix::errno::Errno;

use proxmox_sys::fs::xattr::{fgetxattr, fsetxattr};

#[test]
fn test_fsetxattr_fgetxattr() {
    let mut path = PathBuf::from(env!("CARGO_TARGET_TMPDIR").to_string());
    path.push("test-xattrs.txt");

    let file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(&path)
        .unwrap();

    let fd = file.as_raw_fd();

    if let Err(Errno::EOPNOTSUPP) = fsetxattr(fd, c"user.attribute0", b"value0") {
        return;
    }

    assert!(fsetxattr(fd, c"user.attribute0", b"value0").is_ok());
    assert!(fsetxattr(fd, c"user.empty", b"").is_ok());

    if nix::unistd::Uid::current() != nix::unistd::ROOT {
        assert_eq!(
            fsetxattr(fd, c"trusted.attribute0", b"value0"),
            Err(Errno::EPERM)
        );
    }

    let v0 = fgetxattr(fd, c"user.attribute0").unwrap();
    let v1 = fgetxattr(fd, c"user.empty").unwrap();

    assert_eq!(v0, b"value0".as_ref());
    assert_eq!(v1, b"".as_ref());
    assert_eq!(fgetxattr(fd, c"user.attribute1"), Err(Errno::ENODATA));

    std::fs::remove_file(&path).unwrap();
}
