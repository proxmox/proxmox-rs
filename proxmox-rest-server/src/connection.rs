//! Incoming connection handling for the Rest Server.
//!
//! Hyper building block.

use std::os::unix::io::AsRawFd;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Context as _;
use anyhow::Error;
use futures::FutureExt;
use openssl::ec::{EcGroup, EcKey};
use openssl::nid::Nid;
use openssl::pkey::{PKey, Private};
use openssl::ssl::{SslAcceptor, SslFiletype, SslMethod};
use openssl::x509::X509;
use tokio::net::{TcpListener, TcpStream};
use tokio_openssl::SslStream;
use tokio_stream::wrappers::ReceiverStream;

#[cfg(feature = "rate-limited-stream")]
use proxmox_http::{RateLimitedStream, ShareableRateLimit};

#[cfg(feature = "rate-limited-stream")]
pub type SharedRateLimit = Arc<dyn ShareableRateLimit>;

enum Tls {
    KeyCert(PKey<Private>, X509),
    FilesPem(PathBuf, PathBuf),
}

/// A builder for an `SslAcceptor` which can be configured either with certificates (or path to PEM
/// files), or otherwise builds a self-signed certificate on the fly (mostly useful during
/// development).
pub struct TlsAcceptorBuilder {
    tls: Option<Tls>,
}

impl TlsAcceptorBuilder {
    pub fn new() -> Self {
        Self { tls: None }
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

    pub fn build(self) -> Result<SslAcceptor, Error> {
        let mut acceptor = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls()).unwrap();

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
                acceptor
                    .set_private_key_file(key, SslFiletype::PEM)
                    .context("failed to set tls acceptor private key file")?;
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

#[cfg(feature = "rate-limited-stream")]
type ClientStreamResult = Pin<Box<SslStream<RateLimitedStream<TcpStream>>>>;
#[cfg(not(feature = "rate-limited-stream"))]
type ClientStreamResult = Pin<Box<SslStream<TcpStream>>>;

#[cfg(feature = "rate-limited-stream")]
type LookupRateLimiter = dyn Fn(std::net::SocketAddr) -> (Option<SharedRateLimit>, Option<SharedRateLimit>)
    + Send
    + Sync
    + 'static;

pub struct AcceptBuilder {
    acceptor: Arc<Mutex<SslAcceptor>>,
    debug: bool,
    tcp_keepalive_time: u32,
    max_pending_accepts: usize,

    #[cfg(feature = "rate-limited-stream")]
    lookup_rate_limiter: Option<Arc<LookupRateLimiter>>,
}

impl AcceptBuilder {
    pub fn new() -> Result<Self, Error> {
        Ok(Self::with_acceptor(Arc::new(Mutex::new(
            TlsAcceptorBuilder::new().build()?,
        ))))
    }

    pub fn with_acceptor(acceptor: Arc<Mutex<SslAcceptor>>) -> Self {
        Self {
            acceptor,
            debug: false,
            tcp_keepalive_time: 120,
            max_pending_accepts: 1024,

            #[cfg(feature = "rate-limited-stream")]
            lookup_rate_limiter: None,
        }
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

    pub fn accept(
        self,
        listener: TcpListener,
    ) -> impl hyper::server::accept::Accept<Conn = ClientStreamResult, Error = Error> {
        let (sender, receiver) = tokio::sync::mpsc::channel(self.max_pending_accepts);

        tokio::spawn(self.accept_connections(listener, sender));

        //receiver
        hyper::server::accept::from_stream(ReceiverStream::new(receiver))
    }

    async fn accept_connections(
        self,
        listener: TcpListener,
        sender: tokio::sync::mpsc::Sender<Result<ClientStreamResult, Error>>,
    ) {
        let accept_counter = Arc::new(());
        let mut shutdown_future = crate::shutdown_future().fuse();

        loop {
            let (sock, peer) = futures::select! {
                res = listener.accept().fuse() => match res {
                    Ok(conn) => conn,
                    Err(err) => {
                        eprintln!("error accepting tcp connection: {err}");
                        continue;
                    }
                },
                _ =  shutdown_future => break,
            };
            #[cfg(not(feature = "rate-limited-stream"))]
            drop(peer);

            sock.set_nodelay(true).unwrap();
            let _ = proxmox_sys::linux::socket::set_tcp_keepalive(
                sock.as_raw_fd(),
                self.tcp_keepalive_time,
            );

            #[cfg(feature = "rate-limited-stream")]
            let sock = match self.lookup_rate_limiter.clone() {
                Some(lookup) => {
                    RateLimitedStream::with_limiter_update_cb(sock, move || lookup(peer))
                }
                None => RateLimitedStream::with_limiter(sock, None, None),
            };

            let ssl = {
                // limit acceptor_guard scope
                // Acceptor can be reloaded using the command socket "reload-certificate" command
                let acceptor_guard = self.acceptor.lock().unwrap();

                match openssl::ssl::Ssl::new(acceptor_guard.context()) {
                    Ok(ssl) => ssl,
                    Err(err) => {
                        eprintln!("failed to create Ssl object from Acceptor context - {err}");
                        continue;
                    }
                }
            };

            let stream = match tokio_openssl::SslStream::new(ssl, sock) {
                Ok(stream) => stream,
                Err(err) => {
                    eprintln!("failed to create SslStream using ssl and connection socket - {err}");
                    continue;
                }
            };

            let mut stream = Box::pin(stream);
            let sender = sender.clone();

            if Arc::strong_count(&accept_counter) > self.max_pending_accepts {
                eprintln!("connection rejected - too many open connections");
                continue;
            }

            let accept_counter = Arc::clone(&accept_counter);
            tokio::spawn(async move {
                let accept_future =
                    tokio::time::timeout(Duration::new(10, 0), stream.as_mut().accept());

                let result = accept_future.await;

                match result {
                    Ok(Ok(())) => {
                        if sender.send(Ok(stream)).await.is_err() && self.debug {
                            log::error!("detect closed connection channel");
                        }
                    }
                    Ok(Err(err)) => {
                        if self.debug {
                            log::error!("https handshake failed - {err}");
                        }
                    }
                    Err(_) => {
                        if self.debug {
                            log::error!("https handshake timeout");
                        }
                    }
                }

                drop(accept_counter); // decrease reference count
            });
        }
    }
}
