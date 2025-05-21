use anyhow::{bail, format_err, Error};

use std::collections::HashMap;
use std::future::Future;
use std::os::unix::io::AsFd;
use std::path::{Path, PathBuf};
use std::pin::pin;
use std::sync::Arc;

use nix::sys::socket;
use nix::unistd::Gid;
use serde::Serialize;
use serde_json::Value;
use tokio::net::UnixListener;
use tokio::sync::watch;

/// Returns the control socket path for a specific process ID.
///
/// Note: The control socket always uses @/run/proxmox-backup/ as
/// prefix for historic reason. This does not matter because the
/// generated path is unique for each ``pid`` anyways.
pub fn path_from_pid(pid: i32) -> String {
    // Note: The control socket always uses @/run/proxmox-backup/ as prefix
    // for historc reason.
    format!("\0{}/control-{}.sock", "/run/proxmox-backup", pid)
}

/// Returns the control socket path for this server.
pub fn this_path() -> String {
    path_from_pid(unsafe { libc::getpid() })
}

// Listens on a Unix Socket to handle simple command asynchronously
fn create_control_socket<P, F, W>(
    path: P,
    gid: Gid,
    abort_future: W,
    func: F,
) -> Result<impl Future<Output = ()>, Error>
where
    P: Into<PathBuf>,
    F: Fn(Value) -> Result<Value, Error> + Send + Sync + 'static,
    W: Future<Output = ()> + Send + 'static,
{
    let path: PathBuf = path.into();

    let gid = gid.as_raw();

    let socket = UnixListener::bind(&path)?;

    let func = Arc::new(func);

    let (abort_sender, abort_receiver) = watch::channel(false);

    tokio::spawn(async move {
        abort_future.await;
        let _ = abort_sender.send(true);
    });

    let abort_future = {
        let abort_receiver = abort_receiver.clone();
        async move {
            let _ = { abort_receiver }.wait_for(|&v| v).await;
        }
    };

    let control_future = Box::pin(async move {
        loop {
            use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

            let (conn, _addr) = match socket.accept().await {
                Ok(data) => data,
                Err(err) => {
                    log::error!("failed to accept on control socket {:?}: {}", path, err);
                    continue;
                }
            };

            let opt = socket::sockopt::PeerCredentials;
            let cred = match socket::getsockopt(&conn.as_fd(), opt) {
                Ok(cred) => cred,
                Err(err) => {
                    log::error!("no permissions - unable to read peer credential - {}", err);
                    continue;
                }
            };

            // check permissions (same gid, root user, or backup group)
            let mygid = Gid::current();
            if !(cred.uid() == 0 || cred.gid() == mygid.as_raw() || cred.gid() == gid) {
                log::error!("no permissions for {:?}", cred);
                continue;
            }

            let (rx, mut tx) = tokio::io::split(conn);

            let abort_future = {
                let abort_receiver = abort_receiver.clone();
                Box::pin(async move {
                    let _ = { abort_receiver }.wait_for(|&v| v).await;
                })
            };

            let func = Arc::clone(&func);
            let path = path.clone();
            tokio::spawn(futures::future::select(
                Box::pin(async move {
                    let mut rx = tokio::io::BufReader::new(rx);
                    let mut line = String::new();
                    loop {
                        line.clear();
                        match rx
                            .read_line({
                                line.clear();
                                &mut line
                            })
                            .await
                        {
                            Ok(0) => break,
                            Ok(_) => (),
                            Err(err) => {
                                log::error!("control socket {:?} read error: {}", path, err);
                                return;
                            }
                        }

                        let response = match line.parse::<Value>() {
                            Ok(param) => match func(param) {
                                Ok(res) => format!("OK: {}\n", res),
                                Err(err) => format!("ERROR: {}\n", err),
                            },
                            Err(err) => format!("ERROR: {}\n", err),
                        };

                        if let Err(err) = tx.write_all(response.as_bytes()).await {
                            log::error!("control socket {:?} write response error: {}", path, err);
                            return;
                        }
                    }
                }),
                abort_future,
            ));
        }
    });

    Ok(async move {
        let abort_future = pin!(abort_future);
        futures::future::select(control_future, abort_future).await;
    })
}

/// Send a command to the specified socket
pub async fn send<P, T>(path: P, params: &T) -> Result<Value, Error>
where
    P: AsRef<Path>,
    T: ?Sized + Serialize,
{
    let mut command_string = serde_json::to_string(params)?;
    command_string.push('\n');
    send_raw(path.as_ref(), &command_string).await
}

/// Send a raw command (string) to the specified socket
pub async fn send_raw<P>(path: P, command_string: &str) -> Result<Value, Error>
where
    P: AsRef<Path>,
{
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt};

    let mut conn = tokio::net::UnixStream::connect(path)
        .await
        .map_err(move |err| format_err!("control socket connect failed - {}", err))?;

    conn.write_all(command_string.as_bytes()).await?;
    if !command_string.as_bytes().ends_with(b"\n") {
        conn.write_all(b"\n").await?;
    }

    AsyncWriteExt::shutdown(&mut conn).await?;
    let mut rx = tokio::io::BufReader::new(conn);
    let mut data = String::new();
    if rx.read_line(&mut data).await? == 0 {
        bail!("no response");
    }
    if let Some(res) = data.strip_prefix("OK: ") {
        match res.parse::<Value>() {
            Ok(v) => Ok(v),
            Err(err) => bail!("unable to parse json response - {}", err),
        }
    } else if let Some(err) = data.strip_prefix("ERROR: ") {
        bail!("{}", err);
    } else {
        bail!("unable to parse response: {}", data);
    }
}

// A callback for a specific command socket.
type CommandSocketFn =
    Box<(dyn Fn(Option<&Value>) -> Result<Value, Error> + Send + Sync + 'static)>;

/// Tooling to get a single control command socket where one can
/// register multiple commands dynamically.
///
/// The socket is activated by calling [spawn](CommandSocket::spawn),
/// which spawns an async tokio task to process the commands.
pub struct CommandSocket {
    socket: PathBuf,
    gid: Gid,
    commands: HashMap<String, CommandSocketFn>,
}

impl CommandSocket {
    /// Creates a new instance.
    pub fn new(gid: Gid) -> Self {
        CommandSocket {
            socket: this_path().into(),
            gid,
            commands: HashMap::new(),
        }
    }

    /// Spawn the socket and consume self, meaning you cannot register commands anymore after
    /// calling this.
    ///
    /// The `abort_future` is typically a `last_worker_future()` and is there because this
    /// `spawn()`s a task which would otherwise never finish.
    pub fn spawn<F>(self, abort_future: F) -> Result<(), Error>
    where
        F: Future<Output = ()> + Send + 'static,
    {
        let control_future = create_control_socket(
            self.socket.to_owned(),
            self.gid,
            abort_future,
            move |param| {
                let param = param.as_object().ok_or_else(|| {
                    format_err!("unable to parse parameters (expected json object)")
                })?;

                let command = match param.get("command") {
                    Some(Value::String(command)) => command.as_str(),
                    None => bail!("no command"),
                    _ => bail!("unable to parse command"),
                };

                if !self.commands.contains_key(command) {
                    bail!("got unknown command '{}'", command);
                }

                match self.commands.get(command) {
                    None => bail!("got unknown command '{}'", command),
                    Some(handler) => {
                        let args = param.get("args"); //.unwrap_or(&Value::Null);
                        (handler)(args)
                    }
                }
            },
        )?;

        tokio::spawn(control_future);

        Ok(())
    }

    /// Register a new command with a callback.
    pub fn register_command<F>(&mut self, command: String, handler: F) -> Result<(), Error>
    where
        F: Fn(Option<&Value>) -> Result<Value, Error> + Send + Sync + 'static,
    {
        if self.commands.contains_key(&command) {
            bail!("command '{}' already exists!", command);
        }

        self.commands.insert(command, Box::new(handler));

        Ok(())
    }
}
