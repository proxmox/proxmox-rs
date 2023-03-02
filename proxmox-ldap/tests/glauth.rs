use std::{
    process::{Child, Command, Stdio},
    thread::sleep,
    time::Duration,
};

use anyhow::{Context, Error};
use proxmox_ldap::*;

struct GlauthServer {
    handle: Child,
}

impl GlauthServer {
    fn new(path: &str) -> Result<Self, Error> {
        let glauth_bin = std::env::var("GLAUTH_BIN").context("GLAUTH_BIN is not set")?;
        let handle = Command::new(&glauth_bin)
            .arg("-c")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .context("Could not start glauth process")?;

        // Make 'sure' that glauth is up
        sleep(Duration::from_secs(1));

        Ok(Self { handle })
    }
}

impl Drop for GlauthServer {
    fn drop(&mut self) {
        self.handle.kill().ok();
    }
}

fn authenticate(con: &Connection, user: &str, pass: &str) -> Result<(), Error> {
    proxmox_async::runtime::block_on(con.authenticate_user(user, pass))
}

fn default_config() -> Config {
    Config {
        servers: vec!["localhost".into()],
        port: Some(3893),
        user_attr: "cn".into(),
        base_dn: "dc=example,dc=com".into(),
        bind_dn: Some("cn=serviceuser,ou=svcaccts,dc=example,dc=com".into()),
        bind_password: Some("password".into()),
        tls_mode: ConnectionMode::Ldap,
        verify_certificate: false,
        additional_trusted_certificates: None,
        certificate_store_path: Some("/etc/ssl/certs".into()),
    }
}

#[test]
#[ignore]
fn test_authentication() -> Result<(), Error> {
    let _glauth = GlauthServer::new("tests/assets/glauth.cfg")?;

    let connection = Connection::new(default_config());

    assert!(authenticate(&connection, "test1", "password").is_ok());
    assert!(authenticate(&connection, "test2", "password").is_ok());
    assert!(authenticate(&connection, "test3", "password").is_ok());
    assert!(authenticate(&connection, "test1", "invalid").is_err());
    assert!(authenticate(&connection, "invalid", "password").is_err());

    Ok(())
}

#[test]
#[ignore]
fn test_authentication_via_ipv6() -> Result<(), Error> {
    let _glauth = GlauthServer::new("tests/assets/glauth_v6.cfg")?;

    let settings = Config {
        servers: vec!["[::1]".into()],
        ..default_config()
    };

    let connection = Connection::new(settings);

    assert!(authenticate(&connection, "test1", "password").is_ok());

    Ok(())
}

#[test]
#[ignore]
fn test_authentication_via_ldaps() -> Result<(), Error> {
    let settings = Config {
        port: Some(3894),
        tls_mode: ConnectionMode::Ldaps,
        verify_certificate: true,
        additional_trusted_certificates: Some(vec!["tests/assets/glauth.crt".into()]),
        ..default_config()
    };

    let _glauth = GlauthServer::new("tests/assets/glauth.cfg")?;

    let connection = Connection::new(settings);

    assert!(authenticate(&connection, "test1", "password").is_ok());
    assert!(authenticate(&connection, "test1", "invalid").is_err());

    Ok(())
}

#[test]
#[ignore]
fn test_fallback() -> Result<(), Error> {
    let settings = Config {
        servers: vec!["invalid.host".into(), "localhost".into()],
        ..default_config()
    };

    let _glauth = GlauthServer::new("tests/assets/glauth.cfg")?;

    let connection = Connection::new(settings);
    assert!(authenticate(&connection, "test1", "password").is_ok());

    Ok(())
}

#[test]
#[ignore]
fn test_search() -> Result<(), Error> {
    let _glauth = GlauthServer::new("tests/assets/glauth.cfg")?;

    let connection = Connection::new(default_config());

    let params = SearchParameters {
        attributes: vec!["cn".into(), "mail".into(), "sn".into()],
        user_classes: vec!["posixAccount".into()],
        user_filter: Some("(cn=test*)".into()),
    };

    let search_results = proxmox_async::runtime::block_on(connection.search_entities(&params))?;

    assert_eq!(search_results.len(), 3);

    for a in search_results {
        assert!(a.dn.starts_with("cn=test"));
        assert!(a.dn.ends_with("ou=testgroup,ou=users,dc=example,dc=com"));

        assert!(a
            .attributes
            .get("mail")
            .unwrap()
            .get(0)
            .unwrap()
            .ends_with("@example.com"));
        assert!(a
            .attributes
            .get("sn")
            .unwrap()
            .get(0)
            .unwrap()
            .eq("User".into()));
    }

    Ok(())
}
