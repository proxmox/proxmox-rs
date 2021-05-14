use std::os::unix::io::RawFd;

use nix::sys::socket::sockopt::{KeepAlive, TcpKeepIdle};
use nix::sys::socket::setsockopt;

/// Set TCP keepalive time on a socket
///
/// See "man 7 tcp" for details.
///
/// The default on Linux is 7200 (2 hours) which is far too long for
/// many of our use cases.
pub fn set_tcp_keepalive(
    socket_fd: RawFd,
    tcp_keepalive_time: u32,
) -> nix::Result<()> {

    setsockopt(socket_fd, KeepAlive, &true)?;
    setsockopt(socket_fd, TcpKeepIdle, &tcp_keepalive_time)?;

    Ok(())
}
