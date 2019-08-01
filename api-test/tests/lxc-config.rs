use failure::Error;

use proxmox::api::ApiType;

use api_test::lxc;

/// This just checks the string in order to avoid `T: Eq` as requirement.
/// in other words:
///    assert that serialize(value) == serialize(deserialize(serialize(value)))
/// We assume that serialize(value) has already been checked before entering this function.
fn check_ser_de<T>(value: &T) -> Result<(), Error>
where
    T: ApiType + serde::Serialize + serde::de::DeserializeOwned,
{
    assert!(value.verify().is_ok());
    let s1 = serde_json::to_string(value)?;
    let v2: T = serde_json::from_str(&s1)?;
    assert!(v2.verify().is_ok());
    let s2 = serde_json::to_string(&v2)?;
    assert_eq!(s1, s2);
    Ok(())
}

#[test]
fn lxc_config() -> Result<(), Error> {
    let mut config = lxc::Config::default();
    assert!(config.verify().is_ok());
    assert_eq!(serde_json::to_string(&config)?, "{}");
    check_ser_de(&config)?;
    assert_eq!(*config.onboot(), false);
    assert_eq!(*config.template(), false);
    assert_eq!(*config.arch(), api_test::schema::Architecture::Amd64);
    assert_eq!(*config.console(), true);
    assert_eq!(*config.tty(), 2);
    assert_eq!(*config.cmode(), api_test::lxc::schema::ConsoleMode::Tty);
    assert_eq!(config.memory().as_bytes(), 512 << 20);

    config.lock = Some(lxc::schema::ConfigLock::Backup);
    check_ser_de(&config)?;
    assert_eq!(serde_json::to_string(&config)?, r#"{"lock":"backup"}"#);

    // test the renamed one:
    config.lock = Some(lxc::schema::ConfigLock::SnapshotDelete);
    check_ser_de(&config)?;
    assert_eq!(
        serde_json::to_string(&config)?,
        r#"{"lock":"snapshot-delete"}"#
    );

    config.onboot = Some(true);
    check_ser_de(&config)?;
    assert_eq!(
        serde_json::to_string(&config)?,
        r#"{"lock":"snapshot-delete","onboot":true}"#
    );
    assert_eq!(*config.onboot(), true);

    config.lock = None;
    config.onboot = Some(false);
    check_ser_de(&config)?;
    assert_eq!(serde_json::to_string(&config)?, r#"{"onboot":false}"#);
    assert_eq!(*config.onboot(), false);

    config.onboot = None;
    check_ser_de(&config)?;
    assert_eq!(*config.onboot(), false);

    config.set_onboot(true);
    check_ser_de(&config)?;
    assert_eq!(*config.onboot(), true);
    assert_eq!(serde_json::to_string(&config)?, r#"{"onboot":true}"#);

    config.set_onboot(false);
    check_ser_de(&config)?;
    assert_eq!(*config.onboot(), false);
    assert_eq!(serde_json::to_string(&config)?, r#"{"onboot":false}"#);

    config.set_template(true);
    check_ser_de(&config)?;
    assert_eq!(*config.template(), true);
    assert_eq!(
        serde_json::to_string(&config)?,
        r#"{"onboot":false,"template":true}"#
    );

    config.onboot = None;
    config.template = None;

    config.startup = Some(api_test::schema::StartupOrder {
        order: Some(5),
        ..Default::default()
    });
    check_ser_de(&config)?;
    assert_eq!(
        serde_json::to_string(&config)?,
        r#"{"startup":{"order":5}}"#
    );

    config = serde_json::from_str(r#"{"memory":"123MiB"}"#)?;
    assert!(config.verify().is_ok());
    assert_eq!(serde_json::to_string(&config)?, r#"{"memory":123}"#);

    config = serde_json::from_str(r#"{"memory":"1024MiB"}"#)?;
    assert!(config.verify().is_ok());
    assert_eq!(serde_json::to_string(&config)?, r#"{"memory":1024}"#);

    config = serde_json::from_str(r#"{"memory":"1300001KiB"}"#)?;
    assert!(config.verify().is_ok());
    assert_eq!(
        serde_json::to_string(&config)?,
        r#"{"memory":"1300001KiB"}"#
    );

    // test numeric values
    config = serde_json::from_str(r#"{"tty":3}"#)?;
    assert!(config.verify().is_ok());
    assert_eq!(serde_json::to_string(&config)?, r#"{"tty":3}"#);
    assert!(serde_json::from_str::<lxc::Config>(r#"{"tty":"3"}"#).is_err()); // string as int

    config = serde_json::from_str(r#"{"tty":9}"#)?;
    assert_eq!(
        config.verify().map_err(|e| e.to_string()),
        Err("field tty out of range, must be <= 6".to_string())
    );

    config = serde_json::from_str(r#"{"hostname":"xx"}"#)?;
    assert_eq!(
        config.verify().map_err(|e| e.to_string()),
        Err("field hostname too short, must be >= 3 characters".to_string())
    );

    config = serde_json::from_str(r#"{"hostname":"foo.bar.com"}"#)?;
    assert_eq!(
        serde_json::to_string(&config)?,
        r#"{"hostname":"foo.bar.com"}"#
    );
    assert!(config.verify().is_ok());

    config = serde_json::from_str(r#"{"hostname":"foo"}"#)?;
    assert!(config.verify().is_ok());

    config = serde_json::from_str(r#"{"hostname":"..."}"#)?;
    assert_eq!(
        config.verify().map_err(|e| e.to_string()),
        Err("field hostname does not match format DNS name".to_string()),
    );

    config = serde_json::from_str(r#"{"searchdomain":"foo.bar"}"#)?;
    assert_eq!(
        serde_json::to_string(&config)?,
        r#"{"searchdomain":"foo.bar"}"#
    );

    config = serde_json::from_str(r#"{"searchdomain":"foo.."}"#)?;
    assert_eq!(
        config.verify().map_err(|e| e.to_string()),
        Err("field searchdomain does not match format DNS name".to_string()),
    );

    config = serde_json::from_str(r#"{"searchdomain":"foo.com, bar.com"}"#)?;
    assert!(config.verify().is_ok());
    assert_eq!(
        serde_json::to_string(&config)?,
        r#"{"searchdomain":"foo.com, bar.com"}"#
    );
    config = serde_json::from_str(r#"{"searchdomain":["foo.com", "bar.com"]}"#)?;
    assert!(config.verify().is_ok());
    assert_eq!(
        serde_json::to_string(&config)?,
        r#"{"searchdomain":"foo.com, bar.com"}"#
    );

    config = serde_json::from_str(r#"{"nameserver":["127.0.0.1", "::1"]}"#)?;
    check_ser_de(&config)?;

    config = serde_json::from_str(r#"{"nameserver":"127.0.0.1, foo"}"#)?;
    assert_eq!(
        config.verify().map_err(|e| e.to_string()),
        Err("field nameserver does not match format IP Address".to_string()),
    );

    config = serde_json::from_str(r#"{"cmode":"tty"}"#)?;
    check_ser_de(&config)?;
    config = serde_json::from_str(r#"{"cmode":"shell"}"#)?;
    check_ser_de(&config)?;
    config = serde_json::from_str(r#"{"hookscript":"local:snippets/foo.sh"}"#)?;
    check_ser_de(&config)?;

    Ok(())
}
