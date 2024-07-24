//! Helpers to implement restartable server listening for incoming connections.

use std::ffi::CString;
use std::future::Future;
use std::io::{self, Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::io::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::panic::UnwindSafe;
use std::path::PathBuf;
use std::pin::{pin, Pin};

use anyhow::{bail, format_err, Error};
use futures::future::{self, Either};
use nix::unistd::{fork, ForkResult};

use proxmox_sys::fd::fd_change_cloexec;
use proxmox_sys::fs::CreateOptions;

type BoxedStoreFunc = Box<dyn FnOnce() -> Result<String, Error> + UnwindSafe + Send>;

// Helper trait to "store" something in the environment to be re-used after re-executing the
// service on a reload.
#[doc(hidden)] // not public api
pub trait Reloadable: Sized {
    fn restore(var: &str) -> Result<Self, Error>;
    fn get_store_func(&self) -> Result<BoxedStoreFunc, Error>;
}

// Manages things to be stored and reloaded upon reexec.
// Anything which should be restorable should be instantiated via this struct's `restore` method,
#[derive(Default)]
struct Reloader {
    pre_exec: Vec<PreExecEntry>,
    self_exe: PathBuf,
}

// Currently we only need environment variables for storage, but in theory we could also add
// variants which need temporary files or pipes...
struct PreExecEntry {
    name: &'static str,
    store_fn: BoxedStoreFunc,
}

impl Reloader {
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            pre_exec: Vec::new(),

            // Get the path to our executable as PathBuf
            self_exe: std::fs::read_link("/proc/self/exe")?,
        })
    }

    /// Restore an object from an environment variable of the given name, or, if none exists, uses
    /// the function provided in the `or_create` parameter to instantiate the new "first" instance.
    ///
    /// Values created via this method will be remembered for later re-execution.
    pub async fn restore<T, F, U>(&mut self, name: &'static str, or_create: F) -> Result<T, Error>
    where
        T: Reloadable,
        F: FnOnce() -> U,
        U: Future<Output = Result<T, Error>>,
    {
        let res = match std::env::var(name) {
            Ok(varstr) => T::restore(&varstr)?,
            Err(std::env::VarError::NotPresent) => or_create().await?,
            Err(_) => bail!("variable {} has invalid value", name),
        };

        self.pre_exec.push(PreExecEntry {
            name,
            store_fn: res.get_store_func()?,
        });
        Ok(res)
    }

    fn pre_exec(self) -> Result<(), Error> {
        for item in self.pre_exec {
            std::env::set_var(item.name, (item.store_fn)()?);
        }
        Ok(())
    }

    fn redirect_journal_fd(&self, priority: libc::c_int, target: RawFd) -> Result<(), Error> {
        match proxmox_systemd::journal::stream_fd(
            self.self_exe.file_name().unwrap(),
            priority,
            true,
        ) {
            Ok(fd) => {
                if fd.as_raw_fd() == target {
                    std::mem::forget(fd);
                } else {
                    nix::unistd::dup2(fd.as_raw_fd(), target)?;
                }
            }
            Err(err) => {
                log::error!("failed to update STDOUT journal redirection ({})", err);
            }
        }

        Ok(())
    }

    pub fn fork_restart(self, pid_fn: Option<&str>) -> Result<(), Error> {
        // Get our parameters as Vec<CString>
        let args = std::env::args_os();
        let mut new_args = Vec::with_capacity(args.len());
        for arg in args {
            new_args.push(CString::new(arg.as_bytes())?);
        }

        // Synchronisation pipe:
        let (pold, pnew) = socketpair()?;

        // Start ourselves in the background:
        match unsafe { fork() } {
            Ok(ForkResult::Child) => {
                // Double fork so systemd can supervise us without nagging...
                match unsafe { fork() } {
                    Ok(ForkResult::Child) => {
                        std::mem::drop(pold);
                        // At this point we call pre-exec helpers. We must be certain that if they fail for
                        // whatever reason we can still call `_exit()`, so use catch_unwind.
                        let perform_reexec = move || {
                            let mut pnew = std::fs::File::from(pnew);
                            let pid = nix::unistd::Pid::this();
                            if let Err(e) = pnew.write_all(&pid.as_raw().to_ne_bytes()) {
                                log::error!("failed to send new server PID to parent: {}", e);
                                unsafe {
                                    libc::_exit(-1);
                                }
                            }

                            let mut ok = [0u8];
                            if let Err(e) = pnew.read_exact(&mut ok) {
                                log::error!("parent vanished before notifying systemd: {}", e);
                                unsafe {
                                    libc::_exit(-1);
                                }
                            }
                            assert_eq!(ok[0], 1, "reload handshake should have sent a 1 byte");

                            std::mem::drop(pnew);

                            // Try to reopen STDOUT/STDERR journald streams to get correct PID in logs
                            self.redirect_journal_fd(libc::LOG_INFO, 1)?;
                            self.redirect_journal_fd(libc::LOG_ERR, 2)?;

                            self.do_reexec(new_args)
                        };

                        match std::panic::catch_unwind(perform_reexec) {
                            Ok(Ok(())) => log::error!("do_reexec returned!"),
                            Ok(Err(err)) => log::error!("do_reexec failed: {}", err),
                            Err(_) => log::error!("panic in re-exec"),
                        }
                    }
                    Ok(ForkResult::Parent { child }) => {
                        std::mem::drop((pold, pnew));
                        log::debug!("forked off a new server (second pid: {})", child);
                    }
                    Err(e) => log::error!("fork() failed, restart delayed: {}", e),
                }
                // No matter how we managed to get here, this is the time where we bail out quickly:
                unsafe { libc::_exit(-1) }
            }
            Ok(ForkResult::Parent { child }) => {
                log::debug!(
                    "forked off a new server (first pid: {}), waiting for 2nd pid",
                    child
                );
                std::mem::drop(pnew);
                let mut pold = std::fs::File::from(pold);
                let mut child_pid = (0 as libc::pid_t).to_ne_bytes();
                let child = nix::unistd::Pid::from_raw(match pold.read_exact(&mut child_pid) {
                    Ok(()) => libc::pid_t::from_ne_bytes(child_pid),
                    Err(e) => {
                        log::error!(
                            "failed to receive pid of double-forked child process: {}",
                            e
                        );
                        // systemd will complain but won't kill the service...
                        return Ok(());
                    }
                });

                if let Some(pid_fn) = pid_fn {
                    let pid_str = format!("{}\n", child);
                    proxmox_sys::fs::replace_file(
                        pid_fn,
                        pid_str.as_bytes(),
                        CreateOptions::new(),
                        false,
                    )?;
                }

                if let Err(e) =
                    proxmox_systemd::notify::SystemdNotify::MainPid(child.into()).notify()
                {
                    log::error!("failed to notify systemd about the new main pid: {}", e);
                }
                // ensure systemd got the message about the new main PID before continuing, else it
                // will get confused if the new main process sends its READY signal before that
                if let Err(e) = proxmox_systemd::notify::barrier(u64::MAX) {
                    log::error!("failed to wait on systemd-processing: {}", e);
                }

                // notify child that it is now the new main process:
                if let Err(e) = pold.write_all(&[1u8]) {
                    log::error!("child vanished during reload: {}", e);
                }

                Ok(())
            }
            Err(e) => {
                log::error!("fork() failed, restart delayed: {}", e);
                Ok(())
            }
        }
    }

    fn do_reexec(self, args: Vec<CString>) -> Result<(), Error> {
        let exe = CString::new(self.self_exe.as_os_str().as_bytes())?;
        self.pre_exec()?;
        nix::unistd::setsid()?;
        let args: Vec<&std::ffi::CStr> = args.iter().map(|s| s.as_ref()).collect();
        nix::unistd::execvp(&exe, &args)?;
        panic!("exec misbehaved");
    }
}

fn fd_store_func(fd: RawFd) -> Result<BoxedStoreFunc, Error> {
    let fd = unsafe {
        OwnedFd::from_raw_fd(nix::fcntl::fcntl(
            fd,
            nix::fcntl::FcntlArg::F_DUPFD_CLOEXEC(0),
        )?)
    };
    Ok(Box::new(move || {
        fd_change_cloexec(fd.as_raw_fd(), false)?;
        Ok(fd.into_raw_fd().to_string())
    }))
}

/// NOTE: This must only be used for *async* I/O objects!
unsafe fn fd_restore_func<T>(var: &str) -> Result<T, Error>
where
    T: FromRawFd,
{
    let fd = var
        .parse::<u32>()
        .map_err(|e| format_err!("invalid file descriptor: {}", e))? as RawFd;
    fd_change_cloexec(fd, true)?;
    Ok(unsafe { T::from_raw_fd(fd) })
}

// For now all we need to do is store and reuse a tcp listening socket:
impl Reloadable for tokio::net::TcpListener {
    // NOTE: The socket must not be closed when the store-function is called:
    fn get_store_func(&self) -> Result<BoxedStoreFunc, Error> {
        fd_store_func(self.as_raw_fd())
    }

    fn restore(var: &str) -> Result<Self, Error> {
        Ok(Self::from_std(unsafe { fd_restore_func(var) }?)?)
    }
}

// For now all we need to do is store and reuse a tcp listening socket:
impl Reloadable for tokio::net::UnixListener {
    // NOTE: The socket must not be closed when the store-function is called:
    fn get_store_func(&self) -> Result<BoxedStoreFunc, Error> {
        fd_store_func(self.as_raw_fd())
    }

    fn restore(var: &str) -> Result<Self, Error> {
        Ok(Self::from_std(unsafe { fd_restore_func(var) }?)?)
    }
}

pub trait Listenable: Reloadable {
    type Address;
    fn bind(addr: &Self::Address) -> Pin<Box<dyn Future<Output = io::Result<Self>> + Send + '_>>;
}

impl Listenable for tokio::net::TcpListener {
    type Address = std::net::SocketAddr;

    fn bind(addr: &Self::Address) -> Pin<Box<dyn Future<Output = io::Result<Self>> + Send + '_>> {
        Box::pin(Self::bind(addr))
    }
}

impl Listenable for tokio::net::UnixListener {
    type Address = std::os::unix::net::SocketAddr;

    fn bind(addr: &Self::Address) -> Pin<Box<dyn Future<Output = io::Result<Self>> + Send + '_>> {
        Box::pin(async move {
            let addr = addr.as_pathname().ok_or_else(|| {
                io::Error::new(io::ErrorKind::Other, "missing path for unix socket")
            })?;
            Self::bind(addr)
        })
    }
}

/// This creates a future representing a daemon which reloads itself when receiving a SIGHUP.
/// If this is started regularly, a listening socket is created. In this case, the file descriptor
/// number will be remembered in `PROXMOX_BACKUP_LISTEN_FD`.
/// If the variable already exists, its contents will instead be used to restore the listening
/// socket.  The finished listening socket is then passed to the `create_service` function which
/// can be used to setup the TLS and the HTTP daemon. The returned future has to call
/// [systemd_notify] with [SystemdNotify::Ready](proxmox_systemd::notify::SystemdNotify) when the
/// service is ready.
pub async fn create_daemon<F, S, L>(
    address: L::Address,
    create_service: F,
    pidfn: Option<&str>,
) -> Result<(), Error>
where
    L: Listenable,
    F: FnOnce(L) -> Result<S, Error>,
    S: Future<Output = Result<(), Error>>,
{
    let mut reloader = Reloader::new()?;

    let listener: L = reloader
        .restore("PROXMOX_BACKUP_LISTEN_FD", move || async move {
            Ok(L::bind(&address).await?)
        })
        .await?;

    let service = create_service(listener)?;

    let service = async move {
        if let Err(err) = service.await {
            log::error!("server error: {}", err);
        }
    };

    let server_future = Box::pin(service);
    let shutdown_future = pin!(crate::shutdown_future());

    let finish_future = match future::select(server_future, shutdown_future).await {
        Either::Left((_, _)) => {
            crate::request_shutdown(); // make sure we are in shutdown mode
            None
        }
        Either::Right((_, server_future)) => Some(server_future),
    };

    let mut reloader = Some(reloader);

    if crate::is_reload_requested() {
        log::info!("daemon reload...");
        if let Err(e) = proxmox_systemd::notify::SystemdNotify::Reloading.notify() {
            log::error!("failed to notify systemd about the state change: {}", e);
        }
        if let Err(e) = proxmox_systemd::notify::barrier(u64::MAX) {
            log::error!("failed to wait on systemd-processing: {}", e);
        }

        if let Err(e) = reloader.take().unwrap().fork_restart(pidfn) {
            log::error!("error during reload: {}", e);
            let _ =
                proxmox_systemd::notify::SystemdNotify::Status("error during reload".to_string())
                    .notify();
        }
    } else {
        log::info!("daemon shutting down...");
    }

    if let Some(future) = finish_future {
        future.await;
    }

    log::info!("daemon shut down.");
    Ok(())
}

/// safe wrapper for `nix::sys::socket::socketpair` defaulting to `O_CLOEXEC` and guarding the file
/// descriptors.
fn socketpair() -> Result<(OwnedFd, OwnedFd), Error> {
    use nix::sys::socket;
    let (pa, pb) = socket::socketpair(
        socket::AddressFamily::Unix,
        socket::SockType::Stream,
        None,
        socket::SockFlag::SOCK_CLOEXEC,
    )?;
    Ok(unsafe { (OwnedFd::from_raw_fd(pa), OwnedFd::from_raw_fd(pb)) })
}
