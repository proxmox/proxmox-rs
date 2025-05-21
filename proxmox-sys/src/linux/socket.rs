use std::os::fd::AsFd;

use nix::sys::socket::setsockopt;
use nix::sys::socket::sockopt::{KeepAlive, TcpKeepIdle};

/// Set TCP keepalive time on a socket
///
/// See "man 7 tcp" for details.
///
/// The default on Linux is 7200 seconds (2 hours) which is far too long for many of our use cases.
pub fn set_tcp_keepalive<F: AsFd>(socket_fd: &F, tcp_keepalive_time: u32) -> nix::Result<()> {
    setsockopt(socket_fd, KeepAlive, &true)?;
    setsockopt(socket_fd, TcpKeepIdle, &tcp_keepalive_time)?;

    Ok(())
}
