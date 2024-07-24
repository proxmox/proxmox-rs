use std::collections::HashMap;
use std::future::Future;
use std::io;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};

use anyhow::{format_err, Error};
use http::{HeaderMap, Method, Uri};
use hyper::http::request::Parts;
use hyper::{Body, Response};
use tower_service::Service;

use proxmox_daemon::command_socket::CommandSocket;
use proxmox_log::{FileLogOptions, FileLogger};
use proxmox_router::{Router, RpcEnvironmentType, UserInformation};
use proxmox_sys::fs::{create_path, CreateOptions};

use crate::rest::Handler;
use crate::RestEnvironment;

/// REST server configuration
pub struct ApiConfig {
    basedir: PathBuf,
    aliases: HashMap<String, PathBuf>,
    env_type: RpcEnvironmentType,
    request_log: Option<Arc<Mutex<FileLogger>>>,
    auth_log: Option<Arc<Mutex<FileLogger>>>,
    handlers: Vec<Handler>,
    auth_handler: Option<AuthHandler>,
    index_handler: Option<IndexHandler>,
    pub(crate) privileged_addr: Option<PrivilegedAddr>,

    #[cfg(feature = "templates")]
    templates: templates::Templates,
}

impl ApiConfig {
    /// Creates a new instance
    ///
    /// `basedir` - File lookups are relative to this directory.
    ///
    /// `env_type` - The environment type.
    ///
    /// `api_auth` - The Authentication handler
    ///
    /// `get_index_fn` - callback to generate the root page
    /// (index). Please note that this functions gets a reference to
    /// the [ApiConfig], so it can use [Handlebars] templates
    /// ([render_template](Self::render_template) to generate pages.
    pub fn new<B: Into<PathBuf>>(basedir: B, env_type: RpcEnvironmentType) -> Self {
        Self {
            basedir: basedir.into(),
            aliases: HashMap::new(),
            env_type,
            request_log: None,
            auth_log: None,
            handlers: Vec::new(),
            auth_handler: None,
            index_handler: None,
            privileged_addr: None,

            #[cfg(feature = "templates")]
            templates: Default::default(),
        }
    }

    /// Set the authentication handler.
    pub fn auth_handler(mut self, auth_handler: AuthHandler) -> Self {
        self.auth_handler = Some(auth_handler);
        self
    }

    /// Set the authentication handler from a function.
    pub fn auth_handler_func<Func>(self, func: Func) -> Self
    where
        Func: for<'a> Fn(&'a HeaderMap, &'a Method) -> CheckAuthFuture<'a> + Send + Sync + 'static,
    {
        self.auth_handler(AuthHandler::from_fn(func))
    }

    /// This is used for `protected` API calls to proxy to a more privileged service.
    pub fn privileged_addr(mut self, addr: impl Into<PrivilegedAddr>) -> Self {
        self.privileged_addr = Some(addr.into());
        self
    }

    /// Set the index handler.
    pub fn index_handler(mut self, index_handler: IndexHandler) -> Self {
        self.index_handler = Some(index_handler);
        self
    }

    /// Set the index handler from a function.
    pub fn index_handler_func<Func>(self, func: Func) -> Self
    where
        Func: Fn(RestEnvironment, Parts) -> IndexFuture + Send + Sync + 'static,
    {
        self.index_handler(IndexHandler::from_fn(func))
    }

    pub(crate) async fn get_index(
        &self,
        rest_env: RestEnvironment,
        parts: Parts,
    ) -> Response<Body> {
        match self.index_handler.as_ref() {
            Some(handler) => (handler.func)(rest_env, parts).await,
            None => Response::builder().status(404).body("".into()).unwrap(),
        }
    }

    pub(crate) async fn check_auth(
        &self,
        headers: &HeaderMap,
        method: &Method,
    ) -> Result<(String, Box<dyn UserInformation + Sync + Send>), AuthError> {
        match self.auth_handler.as_ref() {
            Some(handler) => (handler.func)(headers, method).await,
            None => Err(AuthError::NoData),
        }
    }

    pub(crate) fn find_alias(&self, mut components: &[&str]) -> PathBuf {
        let mut filename = self.basedir.clone();
        if components.is_empty() {
            return filename;
        }

        if let Some(subdir) = self.aliases.get(components[0]) {
            filename.push(subdir);
            components = &components[1..];
        }

        filename.extend(components);

        filename
    }

    /// Register a path alias
    ///
    /// This can be used to redirect file lookups to a specific
    /// directory, e.g.:
    ///
    /// ```
    /// use proxmox_rest_server::ApiConfig;
    /// // let mut config = ApiConfig::new(...);
    /// # fn fake(config: ApiConfig) {
    /// config.alias("extjs", "/usr/share/javascript/extjs");
    /// # }
    /// ```
    pub fn alias<S, P>(mut self, alias: S, path: P) -> Self
    where
        S: Into<String>,
        P: Into<PathBuf>,
    {
        self.aliases.insert(alias.into(), path.into());
        self
    }

    /// Register multiple path aliases. See `[ApiConfig::alias()]`.
    pub fn aliases<I, S, P>(mut self, aliases: I) -> Self
    where
        I: IntoIterator<Item = (S, P)>,
        S: Into<String>,
        P: Into<PathBuf>,
    {
        self.aliases
            .extend(aliases.into_iter().map(|(s, p)| (s.into(), p.into())));
        self
    }

    pub(crate) fn env_type(&self) -> RpcEnvironmentType {
        self.env_type
    }

    /// Register a [Handlebars] template file
    ///
    /// Those templates cane be use with [render_template](Self::render_template) to generate pages.
    #[cfg(feature = "templates")]
    pub fn register_template<P>(self, name: &str, path: P) -> Result<Self, Error>
    where
        P: Into<PathBuf>,
    {
        self.templates.register(name, path)?;
        Ok(self)
    }

    /// Checks if the template was modified since the last rendering
    /// if yes, it loads a the new version of the template
    #[cfg(feature = "templates")]
    pub fn render_template<T>(&self, name: &str, data: &T) -> Result<String, Error>
    where
        T: serde::Serialize,
    {
        self.templates.render(name, data)
    }

    /// Enable the access log feature
    ///
    /// When enabled, all requests are logged to the specified file.
    /// This function also registers a `api-access-log-reopen`
    /// command one the [CommandSocket].
    pub fn enable_access_log<P>(
        mut self,
        path: P,
        dir_opts: Option<CreateOptions>,
        file_opts: Option<CreateOptions>,
        commando_sock: &mut CommandSocket,
    ) -> Result<Self, Error>
    where
        P: Into<PathBuf>,
    {
        let path: PathBuf = path.into();
        if let Some(base) = path.parent() {
            if !base.exists() {
                create_path(base, None, dir_opts).map_err(|err| format_err!("{}", err))?;
            }
        }

        let logger_options = FileLogOptions {
            append: true,
            file_opts: file_opts.unwrap_or_default(),
            ..Default::default()
        };
        let request_log = Arc::new(Mutex::new(FileLogger::new(&path, logger_options)?));
        self.request_log = Some(Arc::clone(&request_log));

        commando_sock.register_command("api-access-log-reopen".into(), move |_args| {
            log::info!("re-opening access-log file");
            request_log.lock().unwrap().reopen()?;
            Ok(serde_json::Value::Null)
        })?;

        Ok(self)
    }

    /// Enable the authentication log feature
    ///
    /// When enabled, all authentication requests are logged to the
    /// specified file. This function also registers a
    /// `api-auth-log-reopen` command one the [CommandSocket].
    pub fn enable_auth_log<P>(
        mut self,
        path: P,
        dir_opts: Option<CreateOptions>,
        file_opts: Option<CreateOptions>,
        commando_sock: &mut CommandSocket,
    ) -> Result<Self, Error>
    where
        P: Into<PathBuf>,
    {
        let path: PathBuf = path.into();
        if let Some(base) = path.parent() {
            if !base.exists() {
                create_path(base, None, dir_opts).map_err(|err| format_err!("{}", err))?;
            }
        }

        let logger_options = FileLogOptions {
            append: true,
            prefix_time: true,
            file_opts: file_opts.unwrap_or_default(),
            ..Default::default()
        };
        let auth_log = Arc::new(Mutex::new(FileLogger::new(&path, logger_options)?));
        self.auth_log = Some(Arc::clone(&auth_log));

        commando_sock.register_command("api-auth-log-reopen".into(), move |_args| {
            log::info!("re-opening auth-log file");
            auth_log.lock().unwrap().reopen()?;
            Ok(serde_json::Value::Null)
        })?;

        Ok(self)
    }

    pub(crate) fn get_access_log(&self) -> Option<&Arc<Mutex<FileLogger>>> {
        self.request_log.as_ref()
    }

    pub(crate) fn get_auth_log(&self) -> Option<&Arc<Mutex<FileLogger>>> {
        self.auth_log.as_ref()
    }

    pub(crate) fn find_handler<'a>(&'a self, path_components: &[&str]) -> Option<&'a Handler> {
        self.handlers
            .iter()
            .find(|handler| path_components.strip_prefix(handler.prefix).is_some())
    }

    pub fn default_api2_handler(mut self, router: &'static Router) -> Self {
        self.handlers.push(Handler::default_api2_handler(router));
        self
    }

    pub fn formatted_router(
        mut self,
        prefix: &'static [&'static str],
        router: &'static Router,
    ) -> Self {
        self.handlers
            .push(Handler::formatted_router(prefix, router));
        self
    }

    pub fn unformatted_router(
        mut self,
        prefix: &'static [&'static str],
        router: &'static Router,
    ) -> Self {
        self.handlers
            .push(Handler::unformatted_router(prefix, router));
        self
    }
}

#[cfg(feature = "templates")]
mod templates {
    use std::collections::HashMap;
    use std::fs::metadata;
    use std::path::PathBuf;
    use std::sync::RwLock;
    use std::time::SystemTime;

    use anyhow::{bail, format_err, Error};
    use handlebars::Handlebars;
    use serde::Serialize;

    #[derive(Default)]
    pub struct Templates {
        templates: RwLock<Handlebars<'static>>,
        template_files: RwLock<HashMap<String, (SystemTime, PathBuf)>>,
    }

    impl Templates {
        pub fn register<P>(&self, name: &str, path: P) -> Result<(), Error>
        where
            P: Into<PathBuf>,
        {
            if self.template_files.read().unwrap().contains_key(name) {
                bail!("template already registered");
            }

            let path: PathBuf = path.into();
            let metadata = metadata(&path)?;
            let mtime = metadata.modified()?;

            self.templates
                .write()
                .unwrap()
                .register_template_file(name, &path)?;
            self.template_files
                .write()
                .unwrap()
                .insert(name.to_string(), (mtime, path));

            Ok(())
        }

        pub fn render<T>(&self, name: &str, data: &T) -> Result<String, Error>
        where
            T: Serialize,
        {
            let path;
            let mtime;
            {
                let template_files = self.template_files.read().unwrap();
                let (old_mtime, old_path) = template_files
                    .get(name)
                    .ok_or_else(|| format_err!("template not found"))?;

                mtime = metadata(old_path)?.modified()?;
                if mtime <= *old_mtime {
                    return self
                        .templates
                        .read()
                        .unwrap()
                        .render(name, data)
                        .map_err(|err| format_err!("{}", err));
                }
                path = old_path.to_path_buf();
            }

            {
                let mut template_files = self.template_files.write().unwrap();
                let mut templates = self.templates.write().unwrap();

                templates.register_template_file(name, &path)?;
                template_files.insert(name.to_string(), (mtime, path));

                templates
                    .render(name, data)
                    .map_err(|err| format_err!("{}", err))
            }
        }
    }
}

pub type IndexFuture = Pin<Box<dyn Future<Output = Response<Body>> + Send>>;
pub type IndexFunc = Box<dyn Fn(RestEnvironment, Parts) -> IndexFuture + Send + Sync>;

pub struct IndexHandler {
    func: IndexFunc,
}

impl From<IndexFunc> for IndexHandler {
    fn from(func: IndexFunc) -> Self {
        Self { func }
    }
}

impl IndexHandler {
    pub fn new_static_body<B>(body: B) -> Self
    where
        B: Clone + Send + Sync + Into<Body> + 'static,
    {
        Self::from_fn(move |_, _| {
            let body = body.clone().into();
            Box::pin(async move { Response::builder().status(200).body(body).unwrap() })
        })
    }

    pub fn from_fn<Func>(func: Func) -> Self
    where
        Func: Fn(RestEnvironment, Parts) -> IndexFuture + Send + Sync + 'static,
    {
        Self::from(Box::new(func) as IndexFunc)
    }
}

pub type CheckAuthOutput = Result<(String, Box<dyn UserInformation + Send + Sync>), AuthError>;
pub type CheckAuthFuture<'a> = Pin<Box<dyn Future<Output = CheckAuthOutput> + Send + 'a>>;
pub type CheckAuthFunc =
    Box<dyn for<'a> Fn(&'a HeaderMap, &'a Method) -> CheckAuthFuture<'a> + Send + Sync>;

pub struct AuthHandler {
    func: CheckAuthFunc,
}

impl From<CheckAuthFunc> for AuthHandler {
    fn from(func: CheckAuthFunc) -> Self {
        Self { func }
    }
}

impl AuthHandler {
    pub fn from_fn<Func>(func: Func) -> Self
    where
        Func: for<'a> Fn(&'a HeaderMap, &'a Method) -> CheckAuthFuture<'a> + Send + Sync + 'static,
    {
        Self::from(Box::new(func) as CheckAuthFunc)
    }
}

/// Authentication Error
pub enum AuthError {
    Generic(Error),
    NoData,
}

impl From<Error> for AuthError {
    fn from(err: Error) -> Self {
        AuthError::Generic(err)
    }
}

#[derive(Clone, Debug)]
/// For `protected` requests we support TCP or Unix connections.
pub enum PrivilegedAddr {
    Tcp(std::net::SocketAddr),
    Unix(std::os::unix::net::SocketAddr),
}

impl From<std::net::SocketAddr> for PrivilegedAddr {
    fn from(addr: std::net::SocketAddr) -> Self {
        Self::Tcp(addr)
    }
}

impl From<std::os::unix::net::SocketAddr> for PrivilegedAddr {
    fn from(addr: std::os::unix::net::SocketAddr) -> Self {
        Self::Unix(addr)
    }
}

impl Service<Uri> for PrivilegedAddr {
    type Response = PrivilegedSocket;
    type Error = io::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, _cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn call(&mut self, _req: Uri) -> Self::Future {
        match self {
            PrivilegedAddr::Tcp(addr) => {
                let addr = *addr;
                Box::pin(async move {
                    tokio::net::TcpStream::connect(addr)
                        .await
                        .map(PrivilegedSocket::Tcp)
                })
            }
            PrivilegedAddr::Unix(addr) => {
                let addr = addr.clone();
                Box::pin(async move {
                    tokio::net::UnixStream::connect(addr.as_pathname().ok_or_else(|| {
                        io::Error::new(io::ErrorKind::Other, "empty path for unix socket")
                    })?)
                    .await
                    .map(PrivilegedSocket::Unix)
                })
            }
        }
    }
}

/// A socket which is either a TCP stream or a UNIX stream.
pub enum PrivilegedSocket {
    Tcp(tokio::net::TcpStream),
    Unix(tokio::net::UnixStream),
}

impl tokio::io::AsyncRead for PrivilegedSocket {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Tcp(s) => Pin::new(s).poll_read(cx, buf),
            Self::Unix(s) => Pin::new(s).poll_read(cx, buf),
        }
    }
}

impl tokio::io::AsyncWrite for PrivilegedSocket {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Self::Tcp(s) => Pin::new(s).poll_write(cx, buf),
            Self::Unix(s) => Pin::new(s).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Tcp(s) => Pin::new(s).poll_flush(cx),
            Self::Unix(s) => Pin::new(s).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.get_mut() {
            Self::Tcp(s) => Pin::new(s).poll_shutdown(cx),
            Self::Unix(s) => Pin::new(s).poll_shutdown(cx),
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        match self.get_mut() {
            Self::Tcp(s) => Pin::new(s).poll_write_vectored(cx, bufs),
            Self::Unix(s) => Pin::new(s).poll_write_vectored(cx, bufs),
        }
    }

    fn is_write_vectored(&self) -> bool {
        match self {
            Self::Tcp(s) => s.is_write_vectored(),
            Self::Unix(s) => s.is_write_vectored(),
        }
    }
}

impl hyper::client::connect::Connection for PrivilegedSocket {
    fn connected(&self) -> hyper::client::connect::Connected {
        match self {
            Self::Tcp(s) => s.connected(),
            Self::Unix(_) => hyper::client::connect::Connected::new(),
        }
    }
}

/// Implements hyper's `Accept` for `UnixListener`s.
pub struct UnixAcceptor {
    listener: tokio::net::UnixListener,
}

impl From<tokio::net::UnixListener> for UnixAcceptor {
    fn from(listener: tokio::net::UnixListener) -> Self {
        Self { listener }
    }
}

impl hyper::server::accept::Accept for UnixAcceptor {
    type Conn = tokio::net::UnixStream;
    type Error = io::Error;

    fn poll_accept(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<io::Result<Self::Conn>>> {
        Pin::new(&mut self.get_mut().listener)
            .poll_accept(cx)
            .map(|res| match res {
                Ok((stream, _addr)) => Some(Ok(stream)),
                Err(err) => Some(Err(err)),
            })
    }
}
