#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{bail, format_err, Context, Error};
use ldap3::adapters::{Adapter, EntriesOnly, PagedResults};
use ldap3::{Ldap, LdapConnAsync, LdapConnSettings, LdapResult, Scope, SearchEntry};
use native_tls::{Certificate, TlsConnector, TlsConnectorBuilder};
use serde::{Deserialize, Serialize};

#[cfg(feature = "sync")]
pub mod sync;

#[cfg(feature = "types")]
pub mod types;

#[derive(PartialEq, Eq, Clone, Copy, Serialize, Deserialize, Debug)]
/// LDAP connection security
pub enum ConnectionMode {
    /// unencrypted connection
    Ldap,
    /// upgrade to TLS via STARTTLS
    StartTls,
    /// TLS via LDAPS
    Ldaps,
}

#[derive(Clone, Serialize, Deserialize)]
/// Configuration for LDAP connections
pub struct Config {
    /// Array of servers that will be tried in order
    pub servers: Vec<String>,
    /// Port
    pub port: Option<u16>,
    /// LDAP attribute containing the user id. Will be used to look up the user's domain
    pub user_attr: String,
    /// LDAP base domain
    pub base_dn: String,
    /// LDAP bind domain, will be used for user lookup/sync if set
    pub bind_dn: Option<String>,
    /// LDAP bind password, will be used for user lookup/sync if set
    pub bind_password: Option<String>,
    /// Connection security
    pub tls_mode: ConnectionMode,
    /// Verify the server's TLS certificate
    pub verify_certificate: bool,
    /// Root certificates that should be trusted, in addition to
    /// the ones from the certificate store.
    /// Expects X.509 certs in PEM format.
    pub additional_trusted_certificates: Option<Vec<PathBuf>>,
    /// Override the path to the system's default certificate store
    /// in /etc/ssl/certs (added for PVE compatibility)
    pub certificate_store_path: Option<PathBuf>,
}

#[derive(Serialize, Deserialize)]
/// Parameters for LDAP user searches
pub struct SearchParameters {
    /// Attributes that should be retrieved
    pub attributes: Vec<String>,
    /// `objectclass`es of interest
    pub user_classes: Vec<String>,
    /// Custom user filter
    pub user_filter: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
/// Single LDAP user search result
pub struct SearchResult {
    /// The full user's domain
    pub dn: String,
    /// Queried user attributes
    pub attributes: HashMap<String, Vec<String>>,
}

/// Connection to an LDAP server, can be used to authenticate users.
pub struct Connection {
    /// Configuration for this connection
    config: Config,
}

impl Connection {
    /// Default port for LDAP/StartTls connections
    const LDAP_DEFAULT_PORT: u16 = 389;
    /// Default port for LDAPS connections
    const LDAPS_DEFAULT_PORT: u16 = 636;
    /// Connection timeout
    const LDAP_CONNECTION_TIMEOUT: Duration = Duration::from_secs(5);

    /// Create a new LDAP connection.
    pub fn new(config: Config) -> Self {
        Self { config }
    }

    /// Authenticate a user with username/password.
    ///
    /// The user's domain is queried is by performing an LDAP search with the configured bind_dn
    /// and bind_password. If no bind_dn is provided, an anonymous search is attempted.
    pub async fn authenticate_user(&self, username: &str, password: &str) -> Result<(), Error> {
        let user_dn = self.search_user_dn(username).await?;

        let mut ldap = self.create_connection().await?;

        // Perform actual user authentication by binding.
        let _: LdapResult = ldap.simple_bind(&user_dn, password).await?.success()?;

        // We are already authenticated, so don't fail if terminating the connection
        // does not work for some reason.
        let _: Result<(), _> = ldap.unbind().await;

        Ok(())
    }

    /// Query entities matching given search parameters
    pub async fn search_entities(
        &self,
        parameters: &SearchParameters,
    ) -> Result<Vec<SearchResult>, Error> {
        let search_filter = Self::assemble_search_filter(parameters);

        let mut ldap = self.create_connection().await?;

        if let Some(bind_dn) = self.config.bind_dn.as_deref() {
            let password = self
                .config
                .bind_password
                .as_deref()
                .ok_or_else(|| format_err!("Missing bind password for {bind_dn}"))?;
            let _: LdapResult = ldap.simple_bind(bind_dn, password).await?.success()?;
        }

        let adapters: Vec<Box<dyn Adapter<_, _>>> = vec![
            Box::new(EntriesOnly::new()),
            Box::new(PagedResults::new(500)),
        ];
        let mut search = ldap
            .streaming_search_with(
                adapters,
                &self.config.base_dn,
                Scope::Subtree,
                &search_filter,
                parameters.attributes.clone(),
            )
            .await?;

        let mut results = Vec::new();

        while let Some(entry) = search.next().await? {
            let entry = SearchEntry::construct(entry);

            results.push(SearchResult {
                dn: entry.dn,
                attributes: entry.attrs,
            })
        }
        let _res = search.finish().await.success()?;

        let _ = ldap.unbind().await;

        Ok(results)
    }

    /// Helper to check if a connection with the current configuration is possible.
    ///
    /// This performs a search with the current configuration. If the search succeeds `Ok(()) is
    /// returned, otherwise an `Error` is returned.
    pub async fn check_connection(&self) -> Result<(), Error> {
        let mut ldap = self.create_connection().await?;

        if let Some(bind_dn) = self.config.bind_dn.as_deref() {
            let password = self
                .config
                .bind_password
                .as_deref()
                .ok_or_else(|| format_err!("Missing bind password for {bind_dn}"))?;

            let _: LdapResult = ldap
                .simple_bind(bind_dn, password)
                .await?
                .success()
                .context("LDAP bind failed, bind_dn or password could be incorrect")?;
        }

        // only search base to make sure the base_dn exists while avoiding most common size limits
        let (_, _) = ldap
            .search(&self.config.base_dn, Scope::Base, "(objectClass=*)", &["*"])
            .await?
            .success()
            .context("Could not search LDAP realm, base_dn could be incorrect")?;

        if self.config.bind_dn.is_some() {
            let _: Result<(), _> = ldap.unbind().await; // ignore errors, search succeeded already
        }

        Ok(())
    }

    /// Retrieves an attribute from the root DSE according to [RFC 4512], Section 5.1
    ///
    /// [RFC 4512]: https://www.rfc-editor.org/rfc/rfc4512#section-5.1
    pub async fn retrieve_root_dse_attr(&self, attr: &str) -> Result<Vec<String>, Error> {
        let mut ldap = self.create_connection().await?;

        let (entries, _res) = ldap
            .search("", Scope::Base, "(objectClass=*)", &[attr])
            .await?
            .success()?;

        if entries.len() > 1 {
            bail!("found multiple root DSEs with attribute '{attr}'");
        }

        entries
            .into_iter()
            .next()
            .map(SearchEntry::construct)
            .and_then(|e| e.attrs.get(attr).cloned())
            .ok_or_else(|| format_err!("failed to retrieve root DSE attribute '{attr}'"))
    }

    /// Retrieve port from LDAP configuration, otherwise use the correct default
    fn port_from_config(&self) -> u16 {
        self.config.port.unwrap_or_else(|| {
            if self.config.tls_mode == ConnectionMode::Ldaps {
                Self::LDAPS_DEFAULT_PORT
            } else {
                Self::LDAP_DEFAULT_PORT
            }
        })
    }

    /// Determine correct URL scheme from LDAP config
    fn scheme_from_config(&self) -> &'static str {
        if self.config.tls_mode == ConnectionMode::Ldaps {
            "ldaps"
        } else {
            "ldap"
        }
    }

    /// Construct URL from LDAP config
    fn ldap_url_from_config(&self, server: &str) -> String {
        let port = self.port_from_config();
        let scheme = self.scheme_from_config();
        format!("{scheme}://{server}:{port}")
    }

    fn add_cert_to_builder<P: AsRef<Path>>(
        path: P,
        builder: &mut TlsConnectorBuilder,
    ) -> Result<(), Error> {
        let bytes = fs::read(path)?;
        let cert = Certificate::from_pem(&bytes)?;
        builder.add_root_certificate(cert);

        Ok(())
    }

    async fn try_connect(&self, url: &str) -> Result<(LdapConnAsync, Ldap), Error> {
        let starttls = self.config.tls_mode == ConnectionMode::StartTls;

        let mut builder = TlsConnector::builder();
        builder.danger_accept_invalid_certs(!self.config.verify_certificate);

        if let Some(certificate_paths) = self.config.additional_trusted_certificates.as_deref() {
            for path in certificate_paths {
                Self::add_cert_to_builder(path, &mut builder)?;
            }
        }

        if let Some(certificate_store_path) = self.config.certificate_store_path.as_deref() {
            builder.disable_built_in_roots(true);

            for dir_entry in fs::read_dir(certificate_store_path)? {
                let dir_entry = dir_entry?;

                if !dir_entry.metadata()?.is_dir() {
                    Self::add_cert_to_builder(dir_entry.path(), &mut builder)?;
                }
            }
        }

        LdapConnAsync::with_settings(
            LdapConnSettings::new()
                .set_starttls(starttls)
                .set_conn_timeout(Self::LDAP_CONNECTION_TIMEOUT)
                .set_connector(builder.build()?),
            url,
        )
        .await
        .map_err(|e| e.into())
    }

    /// Create LDAP connection
    ///
    /// If a connection to the server cannot be established, the fallbacks
    /// are tried.
    async fn create_connection(&self) -> Result<Ldap, Error> {
        let mut last_error = None;

        for server in &self.config.servers {
            match self.try_connect(&self.ldap_url_from_config(server)).await {
                Ok((connection, ldap)) => {
                    ldap3::drive!(connection);
                    return Ok(ldap);
                }
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        Err(last_error.unwrap())
    }

    /// Search a user's domain.
    async fn search_user_dn(&self, username: &str) -> Result<String, Error> {
        let mut ldap = self.create_connection().await?;

        if let Some(bind_dn) = self.config.bind_dn.as_deref() {
            let password = self
                .config
                .bind_password
                .as_deref()
                .ok_or_else(|| format_err!("Missing bind password for {bind_dn}"))?;
            let _: LdapResult = ldap.simple_bind(bind_dn, password).await?.success()?;

            let user_dn = self.do_search_user_dn(username, &mut ldap).await;

            ldap.unbind().await?;

            user_dn
        } else {
            self.do_search_user_dn(username, &mut ldap).await
        }
    }

    async fn do_search_user_dn(&self, username: &str, ldap: &mut Ldap) -> Result<String, Error> {
        let query = format!("(&({}={}))", self.config.user_attr, username);

        let (entries, _res) = ldap
            .search(&self.config.base_dn, Scope::Subtree, &query, &["dn"])
            .await?
            .success()?;

        if entries.len() > 1 {
            bail!(
                "found multiple users with attribute `{}={}`",
                self.config.user_attr,
                username
            )
        }

        if let Some(entry) = entries.into_iter().next() {
            let entry = SearchEntry::construct(entry);

            return Ok(entry.dn);
        }

        bail!("user not found")
    }

    fn assemble_search_filter(parameters: &SearchParameters) -> String {
        use FilterElement::*;

        let attr_wildcards = Or(parameters
            .attributes
            .iter()
            .map(|attr| Condition(attr, "*"))
            .collect());
        let user_classes = Or(parameters
            .user_classes
            .iter()
            .map(|class| Condition("objectclass", class))
            .collect());

        if let Some(user_filter) = &parameters.user_filter {
            And(vec![Verbatim(user_filter), attr_wildcards, user_classes])
        } else {
            And(vec![attr_wildcards, user_classes])
        }
        .to_string()
    }
}

#[allow(dead_code)]
enum FilterElement<'a> {
    And(Vec<FilterElement<'a>>),
    Or(Vec<FilterElement<'a>>),
    Condition(&'a str, &'a str),
    Not(Box<FilterElement<'a>>),
    Verbatim(&'a str),
}

impl Display for FilterElement<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        fn write_children(f: &mut Formatter<'_>, children: &[FilterElement]) -> std::fmt::Result {
            for child in children {
                write!(f, "{child}")?;
            }

            Ok(())
        }

        match self {
            FilterElement::And(children) => {
                write!(f, "(&")?;
                write_children(f, children)?;
                write!(f, ")")?;
            }
            FilterElement::Or(children) => {
                write!(f, "(|")?;
                write_children(f, children)?;
                write!(f, ")")?;
            }
            FilterElement::Not(element) => {
                write!(f, "(!{element})")?;
            }
            FilterElement::Condition(attr, value) => {
                write!(f, "({attr}={value})")?;
            }
            FilterElement::Verbatim(verbatim) => {
                if !verbatim.starts_with('(') && !verbatim.ends_with(')') {
                    write!(f, "({verbatim})")?
                } else {
                    write!(f, "{verbatim}")?
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::FilterElement::*;

    #[test]
    fn test_filter_elements_to_string() {
        assert_eq!("(uid=john)", Condition("uid", "john").to_string());
        assert_eq!(
            "(!(uid=john))",
            Not(Box::new(Condition("uid", "john"))).to_string()
        );

        assert_eq!("(foo=bar)", &Verbatim("(foo=bar)").to_string());
        assert_eq!("(foo=bar)", &Verbatim("foo=bar").to_string());

        let filter_string = And(vec![
            Condition("givenname", "john"),
            Condition("sn", "doe"),
            Or(vec![
                Condition("email", "john@foo"),
                Condition("email", "john@bar"),
            ]),
        ])
        .to_string();

        assert_eq!(
            "(&(givenname=john)(sn=doe)(|(email=john@foo)(email=john@bar)))",
            &filter_string
        );
    }
}
