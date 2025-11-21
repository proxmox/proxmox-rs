//! Incoming connection handling for the Rest Server.
//!
//! Hyper building block.

use std::io;
use std::mem::ManuallyDrop;
use std::net::SocketAddr;
use std::os::unix::io::AsFd;
use std::path::PathBuf;
use std::pin::{pin, Pin};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{format_err, Context, Error};
use futures::FutureExt;
use openssl::ec::{EcGroup, EcKey};
use openssl::nid::Nid;
use openssl::pkey::{PKey, Private};
use openssl::ssl::{SslAcceptor, SslMethod};
use openssl::x509::X509;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;
use tokio_openssl::SslStream;
use tokio_stream::wrappers::ReceiverStream;

#[cfg(feature = "rate-limited-stream")]
use proxmox_http::{RateLimitedStream, RateLimiterTag};

#[cfg(feature = "rate-limited-stream")]
use proxmox_rate_limiter::ShareableRateLimit;

#[cfg(feature = "rate-limited-stream")]
pub type SharedRateLimit = Arc<dyn ShareableRateLimit>;

enum Tls {
    KeyCert(PKey<Private>, X509),
    FilesPem(PathBuf, PathBuf),
}

/// A builder for an `SslAcceptor` which can be configured either with certificates (or path to PEM
/// files), or otherwise builds a self-signed certificate on the fly (mostly useful during
/// development).
#[derive(Default)]
pub struct TlsAcceptorBuilder {
    tls: Option<Tls>,
    cipher_suites: Option<String>,
    cipher_list: Option<String>,
}

impl TlsAcceptorBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn certificate(mut self, key: PKey<Private>, cert: X509) -> Self {
        self.tls = Some(Tls::KeyCert(key, cert));
        self
    }

    pub fn certificate_paths_pem(
        mut self,
        key: impl Into<PathBuf>,
        cert: impl Into<PathBuf>,
    ) -> Self {
        self.tls = Some(Tls::FilesPem(key.into(), cert.into()));
        self
    }

    pub fn cipher_suites(mut self, suites: String) -> Self {
        self.cipher_suites = Some(suites);
        self
    }

    pub fn cipher_list(mut self, list: String) -> Self {
        self.cipher_list = Some(list);
        self
    }

    pub fn build(self) -> Result<SslAcceptor, Error> {
        let mut acceptor = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls()).unwrap();

        if let Some(cipher_suites) = self.cipher_suites.as_deref() {
            acceptor
                .set_ciphersuites(cipher_suites)
                .context("failed to set tls acceptor cipher suites")?;
        }
        if let Some(cipher_list) = self.cipher_list.as_deref() {
            acceptor
                .set_cipher_list(cipher_list)
                .context("failed to set tls acceptor cipher list")?;
        }

        match self.tls {
            Some(Tls::KeyCert(key, cert)) => {
                acceptor
                    .set_private_key(&key)
                    .context("failed to set tls acceptor private key")?;
                acceptor
                    .set_certificate(&cert)
                    .context("failed to set tls acceptor certificate")?;
            }
            Some(Tls::FilesPem(key, cert)) => {
                let key_content = std::fs::read(&key)
                    .with_context(|| format!("Failed to read from private key file {key:?}"))?;
                acceptor
                    .set_private_key(PKey::private_key_from_pem(&key_content)?.as_ref())
                    .context("failed to set tls acceptor private key file")?;

                {
                    // Check the permissions by opening the file
                    let _cert_fd = std::fs::File::open(&cert)
                        .with_context(|| format!("Failed to open certificate at {cert:?}"))?;
                }
                acceptor
                    .set_certificate_chain_file(cert)
                    .context("failed to set tls acceptor certificate chain file")?;
            }
            None => {
                let key = EcKey::generate(
                    EcGroup::from_curve_name(Nid::X9_62_PRIME256V1)
                        .context("failed to get NIST-P256 curve from openssl")?
                        .as_ref(),
                )
                .and_then(PKey::from_ec_key)
                .context("generating temporary ec key")?;
                //let key = openssl::rsa::Rsa::generate(4096)
                //    .and_then(PKey::from_rsa)
                //    .context("generating temporary rsa key")?;

                let mut cert =
                    X509::builder().context("generating building self signed certificate")?;
                cert.set_version(2)?;
                cert.set_pubkey(&key)?;
                cert.sign(&key, openssl::hash::MessageDigest::sha256())?;
                cert.set_not_before(openssl::asn1::Asn1Time::days_from_now(0)?.as_ref())?;
                cert.set_not_after(openssl::asn1::Asn1Time::days_from_now(365)?.as_ref())?;

                let mut name = openssl::x509::X509Name::builder()?;
                name.append_entry_by_text("C", "CA")?;
                name.append_entry_by_text("O", "Self")?;
                name.append_entry_by_text("CN", "localhost")?;
                cert.set_issuer_name(name.build().as_ref())?;

                let cert = cert.build();

                acceptor
                    .set_private_key(&key)
                    .context("failed to set tls acceptor private key")?;
                acceptor
                    .set_certificate(&cert)
                    .context("failed to set tls acceptor certificate")?;
            }
        }
        acceptor.set_options(openssl::ssl::SslOptions::NO_RENEGOTIATION);
        acceptor.check_private_key().unwrap();

        Ok(acceptor.build())
    }
}

#[cfg(not(feature = "rate-limited-stream"))]
type InsecureClientStream = TcpStream;
#[cfg(feature = "rate-limited-stream")]
type InsecureClientStream = RateLimitedStream<TcpStream>;

type InsecureClientStreamResult = Pin<Box<InsecureClientStream>>;

type ClientStreamResult = Pin<Box<SslStream<InsecureClientStream>>>;

#[cfg(feature = "rate-limited-stream")]
type LookupRateLimiter = dyn Fn(
        std::net::SocketAddr,
        &[RateLimiterTag],
    ) -> (Option<SharedRateLimit>, Option<SharedRateLimit>)
    + Send
    + Sync
    + 'static;

pub struct AcceptBuilder {
    debug: bool,
    tcp_keepalive_time: u32,
    max_pending_accepts: usize,

    #[cfg(feature = "rate-limited-stream")]
    lookup_rate_limiter: Option<Arc<LookupRateLimiter>>,
}

impl Default for AcceptBuilder {
    fn default() -> Self {
        Self {
            debug: false,
            tcp_keepalive_time: 120,
            max_pending_accepts: 1024,

            #[cfg(feature = "rate-limited-stream")]
            lookup_rate_limiter: None,
        }
    }
}

impl AcceptBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn debug(mut self, debug: bool) -> Self {
        self.debug = debug;
        self
    }

    pub fn tcp_keepalive_time(mut self, time: u32) -> Self {
        self.tcp_keepalive_time = time;
        self
    }

    pub fn max_pending_accepts(mut self, count: usize) -> Self {
        self.max_pending_accepts = count;
        self
    }

    #[cfg(feature = "rate-limited-stream")]
    pub fn rate_limiter_lookup(mut self, lookup_rate_limiter: Arc<LookupRateLimiter>) -> Self {
        self.lookup_rate_limiter = Some(lookup_rate_limiter);
        self
    }
}

impl AcceptBuilder {
    pub fn accept_tls(
        self,
        listener: TcpListener,
        acceptor: Arc<Mutex<SslAcceptor>>,
        // FIXME: replace return value with own trait? see now removed UnixAcceptor
    ) -> ReceiverStream<Result<ClientStreamResult, Error>> {
        let (secure_sender, secure_receiver) = mpsc::channel(self.max_pending_accepts);

        tokio::spawn(self.accept_connections(listener, acceptor, secure_sender.into()));

        ReceiverStream::new(secure_receiver)
    }

    pub fn accept_tls_optional(
        self,
        listener: TcpListener,
        acceptor: Arc<Mutex<SslAcceptor>>,
    ) -> (
        ReceiverStream<Result<ClientStreamResult, Error>>,
        ReceiverStream<Result<InsecureClientStreamResult, Error>>,
    ) {
        let (secure_sender, secure_receiver) = mpsc::channel(self.max_pending_accepts);
        let (insecure_sender, insecure_receiver) = mpsc::channel(self.max_pending_accepts);

        tokio::spawn(self.accept_connections(
            listener,
            acceptor,
            (secure_sender, insecure_sender).into(),
        ));

        (
            ReceiverStream::new(secure_receiver),
            ReceiverStream::new(insecure_receiver),
        )
    }
}

type ClientSender = mpsc::Sender<Result<ClientStreamResult, Error>>;
type InsecureClientSender = mpsc::Sender<Result<InsecureClientStreamResult, Error>>;

enum Sender {
    Secure(ClientSender),
    SecureAndInsecure(ClientSender, InsecureClientSender),
}

impl From<ClientSender> for Sender {
    fn from(sender: ClientSender) -> Self {
        Sender::Secure(sender)
    }
}

impl From<(ClientSender, InsecureClientSender)> for Sender {
    fn from(senders: (ClientSender, InsecureClientSender)) -> Self {
        Sender::SecureAndInsecure(senders.0, senders.1)
    }
}

struct AcceptState {
    socket: InsecureClientStream,
    peer: SocketAddr,
    acceptor: Arc<Mutex<SslAcceptor>>,
    accept_counter: Arc<()>,
}

struct AcceptFlags {
    is_debug: bool,
}

impl AcceptBuilder {
    async fn accept_connections(
        self,
        listener: TcpListener,
        acceptor: Arc<Mutex<SslAcceptor>>,
        sender: Sender,
    ) {
        let accept_counter = Arc::new(());
        let mut shutdown_future = pin!(proxmox_daemon::shutdown_future().fuse());

        loop {
            let (socket, peer) = futures::select! {
                res = self.try_setup_socket(&listener).fuse() => match res {
                    Ok(socket_peer) => socket_peer,
                    Err(err) => {
                        log::error!("couldn't set up TCP socket: {err}");
                        continue;
                    }
                },
                _ = shutdown_future => break,
            };

            let acceptor = Arc::clone(&acceptor);
            let accept_counter = Arc::clone(&accept_counter);

            if Arc::strong_count(&accept_counter) > self.max_pending_accepts {
                log::error!("[{peer}] connection rejected - too many open connections");
                continue;
            }

            let state = AcceptState {
                socket,
                peer,
                acceptor,
                accept_counter,
            };

            let flags = AcceptFlags {
                is_debug: self.debug,
            };

            match sender {
                Sender::Secure(ref secure_sender) => {
                    let accept_future = Self::do_accept_tls(state, flags, secure_sender.clone());

                    tokio::spawn(accept_future);
                }
                Sender::SecureAndInsecure(ref secure_sender, ref insecure_sender) => {
                    let accept_future = Self::do_accept_tls_optional(
                        state,
                        flags,
                        secure_sender.clone(),
                        insecure_sender.clone(),
                    );

                    tokio::spawn(accept_future);
                }
            };
        }
    }

    async fn try_setup_socket(
        &self,
        listener: &TcpListener,
    ) -> Result<(InsecureClientStream, SocketAddr), Error> {
        let (socket, peer) = match listener.accept().await {
            Ok(connection) => connection,
            Err(error) => {
                return Err(format_err!(error)).context("error while accepting tcp stream")
            }
        };

        socket
            .set_nodelay(true)
            .with_context(|| format!("[{peer}] error while setting TCP_NODELAY on socket"))?;

        proxmox_sys::linux::socket::set_tcp_keepalive(&socket.as_fd(), self.tcp_keepalive_time)
            .with_context(|| format!("[{peer}] error while setting SO_KEEPALIVE on socket"))?;

        #[cfg(feature = "rate-limited-stream")]
        let socket = match self.lookup_rate_limiter.clone() {
            Some(lookup) => {
                RateLimitedStream::with_limiter_update_cb(socket, move |tags| lookup(peer, tags))
            }
            None => RateLimitedStream::with_limiter(socket, None, None),
        };

        Ok((socket, peer))
    }

    async fn do_accept_tls(state: AcceptState, flags: AcceptFlags, secure_sender: ClientSender) {
        let peer = state.peer;

        let ssl = {
            // limit acceptor_guard scope
            // Acceptor can be reloaded using the command socket "reload-certificate" command
            let acceptor_guard = state.acceptor.lock().unwrap();

            match openssl::ssl::Ssl::new(acceptor_guard.context()) {
                Ok(ssl) => ssl,
                Err(err) => {
                    log::error!(
                        "[{peer}] failed to create Ssl object from Acceptor context - {err}"
                    );
                    return;
                }
            }
        };

        let secure_stream = match tokio_openssl::SslStream::new(ssl, state.socket) {
            Ok(stream) => stream,
            Err(err) => {
                log::error!(
                    "[{peer}] failed to create SslStream using ssl and connection socket - {err}"
                );
                return;
            }
        };

        let mut secure_stream = Box::pin(secure_stream);

        let accept_future =
            tokio::time::timeout(Duration::new(10, 0), secure_stream.as_mut().accept());

        let result = accept_future.await;

        match result {
            Ok(Ok(())) => {
                if secure_sender.send(Ok(secure_stream)).await.is_err() && flags.is_debug {
                    log::error!("[{peer}] detected closed connection channel");
                }
            }
            Ok(Err(err)) => {
                if flags.is_debug {
                    log::error!("[{peer}] https handshake failed - {err}");
                }
            }
            Err(_) => {
                if flags.is_debug {
                    log::error!("[{peer}] https handshake timeout");
                }
            }
        }

        drop(state.accept_counter); // decrease reference count
    }

    async fn do_accept_tls_optional(
        state: AcceptState,
        flags: AcceptFlags,
        secure_sender: ClientSender,
        insecure_sender: InsecureClientSender,
    ) {
        const CLIENT_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

        let peer = state.peer;

        #[cfg(feature = "rate-limited-stream")]
        let socket_ref = state.socket.inner();

        #[cfg(not(feature = "rate-limited-stream"))]
        let socket_ref = &state.socket;

        let handshake_res =
            Self::wait_for_client_tls_handshake(socket_ref, CLIENT_HANDSHAKE_TIMEOUT).await;

        match handshake_res {
            Ok(true) => {
                Self::do_accept_tls(state, flags, secure_sender).await;
            }
            Ok(false) => {
                let insecure_stream = Box::pin(state.socket);

                if let Err(send_err) = insecure_sender.send(Ok(insecure_stream)).await {
                    log::error!("[{peer}] failed to accept connection - connection channel closed: {send_err}");
                }
            }
            Err(err) => {
                log::error!("[{peer}] failed to check for TLS handshake: {err}");
            }
        }
    }

    async fn wait_for_client_tls_handshake(
        incoming_stream: &TcpStream,
        timeout: Duration,
    ) -> Result<bool, Error> {
        const HANDSHAKE_BYTES_LEN: usize = 5;

        let future = async {
            let mut previous_peek_len = 0;
            incoming_stream
                .async_io(tokio::io::Interest::READABLE, || {
                    let mut buf = [0; HANDSHAKE_BYTES_LEN];

                    use std::os::fd::{AsRawFd, FromRawFd};

                    // Convert to standard lib TcpStream so we can peek without interfering
                    // with tokio's internals. Wrap the stream in ManuallyDrop in order to prevent
                    // the destructor from being called, closing the connection and messing up
                    // invariants.
                    let raw_fd = incoming_stream.as_raw_fd();
                    let std_stream =
                        unsafe { ManuallyDrop::new(std::net::TcpStream::from_raw_fd(raw_fd)) };

                    let peek_res = std_stream.peek(&mut buf);

                    match peek_res {
                        // If we didn't get enough bytes, raise an EAGAIN / EWOULDBLOCK which tells
                        // tokio to await the readiness of the socket again. This should normally
                        // only be used if the socket isn't actually ready, but is fine to do here
                        // in our case.
                        //
                        // This means we will peek into the stream's queue until we got
                        // HANDSHAKE_BYTE_LEN bytes or an error.
                        Ok(peek_len) if peek_len < HANDSHAKE_BYTES_LEN => {
                            // if we detect the same peek len again but still got a readable stream,
                            // the connection was probably closed, so abort here
                            if peek_len == previous_peek_len {
                                Err(io::ErrorKind::ConnectionAborted.into())
                            } else {
                                previous_peek_len = peek_len;
                                Err(io::ErrorKind::WouldBlock.into())
                            }
                        }
                        // Either we got Ok(HANDSHAKE_BYTES_LEN) or some error.
                        res => res.map(|_| contains_tls_handshake_fragment(&buf)),
                    }
                })
                .await
                .context("couldn't peek into incoming TCP stream")
        };

        tokio::time::timeout(timeout, future)
            .await
            .context("timed out while waiting for client to initiate TLS handshake")?
    }
}

/// Checks whether an [SSL 3.0 / TLS plaintext fragment][0] being part of a
/// SSL / TLS handshake is contained in the given buffer.
///
/// Such a fragment might look as follows:
/// ```ignore
/// [0x16, 0x3, 0x1, 0x02, 0x00, ...]
/// //  |    |    |     |_____|
/// //  |    |    |            \__ content length interpreted as u16
/// //  |    |    |                must not exceed 0x4000 (2^14) bytes
/// //  |    |    |
/// //  |    |     \__ any minor version
/// //  |    |
/// //  |     \__ major version 3
/// //  |
/// //   \__ content type is handshake(22)
/// ```
///
/// If a slice like this is detected at the beginning of the given buffer,
/// a TLS handshake is most definitely being made.
///
/// [0]: https://datatracker.ietf.org/doc/html/rfc6101#section-5.2
#[inline]
fn contains_tls_handshake_fragment(buf: &[u8]) -> bool {
    const SLICE_LENGTH: usize = 5;
    const CONTENT_SIZE: u16 = 1 << 14; // max length of a TLS plaintext fragment

    if buf.len() < SLICE_LENGTH {
        return false;
    }

    buf[0] == 0x16 && buf[1] == 0x3 && (((buf[3] as u16) << 8) + buf[4] as u16) <= CONTENT_SIZE
}
