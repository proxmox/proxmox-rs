use std::io;
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr};

use tokio::net::{ToSocketAddrs, UdpSocket};

/// Helper to connect to UDP addresses without having to manually bind to the correct ip address
pub async fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<UdpSocket> {
    let mut last_err = None;
    for address in tokio::net::lookup_host(&addr).await? {
        let bind_address = match address {
            SocketAddr::V4(_) => SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0),
            SocketAddr::V6(_) => SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), 0),
        };
        let socket = match UdpSocket::bind(bind_address).await {
            Ok(sock) => sock,
            Err(err) => {
                last_err = Some(err);
                continue;
            }
        };
        match socket.connect(address).await {
            Ok(()) => return Ok(socket),
            Err(err) => {
                last_err = Some(err);
                continue;
            }
        }
    }

    Err(last_err.unwrap_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "could not resolve to any addresses",
        )
    }))
}
