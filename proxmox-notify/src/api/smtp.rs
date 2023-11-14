use proxmox_http_error::HttpError;

use crate::api::{http_bail, http_err};
use crate::endpoints::smtp::{
    DeleteableSmtpProperty, SmtpConfig, SmtpConfigUpdater, SmtpPrivateConfig,
    SmtpPrivateConfigUpdater, SMTP_TYPENAME,
};
use crate::Config;

/// Get a list of all smtp endpoints.
///
/// The caller is responsible for any needed permission checks.
/// Returns a list of all smtp endpoints or a `HttpError` if the config is
/// erroneous (`500 Internal server error`).
pub fn get_endpoints(config: &Config) -> Result<Vec<SmtpConfig>, HttpError> {
    config
        .config
        .convert_to_typed_array(SMTP_TYPENAME)
        .map_err(|e| http_err!(NOT_FOUND, "Could not fetch endpoints: {e}"))
}

/// Get smtp endpoint with given `name`.
///
/// The caller is responsible for any needed permission checks.
/// Returns the endpoint or a `HttpError` if the endpoint was not found (`404 Not found`).
pub fn get_endpoint(config: &Config, name: &str) -> Result<SmtpConfig, HttpError> {
    config
        .config
        .lookup(SMTP_TYPENAME, name)
        .map_err(|_| http_err!(NOT_FOUND, "endpoint '{name}' not found"))
}

/// Add a new smtp endpoint.
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns a `HttpError` if:
///   - an entity with the same name already exists (`400 Bad request`)
///   - the configuration could not be saved (`500 Internal server error`)
///   - mailto *and* mailto_user are both set to `None`
pub fn add_endpoint(
    config: &mut Config,
    endpoint_config: &SmtpConfig,
    private_endpoint_config: &SmtpPrivateConfig,
) -> Result<(), HttpError> {
    if endpoint_config.name != private_endpoint_config.name {
        // Programming error by the user of the crate, thus we panic
        panic!("name for endpoint config and private config must be identical");
    }

    super::ensure_unique(config, &endpoint_config.name)?;

    if endpoint_config.mailto.is_none() && endpoint_config.mailto_user.is_none() {
        http_bail!(
            BAD_REQUEST,
            "must at least provide one recipient, either in mailto or in mailto-user"
        );
    }

    super::set_private_config_entry(
        config,
        private_endpoint_config,
        SMTP_TYPENAME,
        &endpoint_config.name,
    )?;

    config
        .config
        .set_data(&endpoint_config.name, SMTP_TYPENAME, endpoint_config)
        .map_err(|e| {
            http_err!(
                INTERNAL_SERVER_ERROR,
                "could not save endpoint '{}': {e}",
                endpoint_config.name
            )
        })
}

/// Update existing smtp endpoint
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns a `HttpError` if:
///   - the configuration could not be saved (`500 Internal server error`)
///   - mailto *and* mailto_user are both set to `None`
pub fn update_endpoint(
    config: &mut Config,
    name: &str,
    updater: &SmtpConfigUpdater,
    private_endpoint_config_updater: &SmtpPrivateConfigUpdater,
    delete: Option<&[DeleteableSmtpProperty]>,
    digest: Option<&[u8]>,
) -> Result<(), HttpError> {
    super::verify_digest(config, digest)?;

    let mut endpoint = get_endpoint(config, name)?;

    if let Some(delete) = delete {
        for deleteable_property in delete {
            match deleteable_property {
                DeleteableSmtpProperty::Author => endpoint.author = None,
                DeleteableSmtpProperty::Comment => endpoint.comment = None,
                DeleteableSmtpProperty::Disable => endpoint.disable = None,
                DeleteableSmtpProperty::Mailto => endpoint.mailto = None,
                DeleteableSmtpProperty::MailtoUser => endpoint.mailto_user = None,
                DeleteableSmtpProperty::Password => super::set_private_config_entry(
                    config,
                    &SmtpPrivateConfig {
                        name: name.to_string(),
                        password: None,
                    },
                    SMTP_TYPENAME,
                    name,
                )?,
                DeleteableSmtpProperty::Port => endpoint.port = None,
                DeleteableSmtpProperty::Username => endpoint.username = None,
            }
        }
    }

    if let Some(mailto) = &updater.mailto {
        endpoint.mailto = Some(mailto.iter().map(String::from).collect());
    }
    if let Some(mailto_user) = &updater.mailto_user {
        endpoint.mailto_user = Some(mailto_user.iter().map(String::from).collect());
    }
    if let Some(from_address) = &updater.from_address {
        endpoint.from_address = from_address.into();
    }
    if let Some(server) = &updater.server {
        endpoint.server = server.into();
    }
    if let Some(port) = &updater.port {
        endpoint.port = Some(*port);
    }
    if let Some(username) = &updater.username {
        endpoint.username = Some(username.into());
    }
    if let Some(mode) = &updater.mode {
        endpoint.mode = Some(*mode);
    }
    if let Some(password) = &private_endpoint_config_updater.password {
        super::set_private_config_entry(
            config,
            &SmtpPrivateConfig {
                name: name.into(),
                password: Some(password.into()),
            },
            SMTP_TYPENAME,
            name,
        )?;
    }

    if let Some(author) = &updater.author {
        endpoint.author = Some(author.into());
    }

    if let Some(comment) = &updater.comment {
        endpoint.comment = Some(comment.into());
    }

    if let Some(disable) = &updater.disable {
        endpoint.disable = Some(*disable);
    }

    if endpoint.mailto.is_none() && endpoint.mailto_user.is_none() {
        http_bail!(
            BAD_REQUEST,
            "must at least provide one recipient, either in mailto or in mailto-user"
        );
    }

    config
        .config
        .set_data(name, SMTP_TYPENAME, &endpoint)
        .map_err(|e| {
            http_err!(
                INTERNAL_SERVER_ERROR,
                "could not save endpoint '{}': {e}",
                endpoint.name
            )
        })
}

/// Delete existing smtp endpoint
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns a `HttpError` if:
///   - an entity with the same name already exists (`400 Bad request`)
///   - the configuration could not be saved (`500 Internal server error`)
pub fn delete_endpoint(config: &mut Config, name: &str) -> Result<(), HttpError> {
    // Check if the endpoint exists
    let _ = get_endpoint(config, name)?;
    super::ensure_unused(config, name)?;

    super::remove_private_config_entry(config, name)?;
    config.config.sections.remove(name);

    Ok(())
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::api::test_helpers::*;
    use crate::endpoints::smtp::SmtpMode;

    pub fn add_smtp_endpoint_for_test(config: &mut Config, name: &str) -> Result<(), HttpError> {
        add_endpoint(
            config,
            &SmtpConfig {
                name: name.into(),
                mailto: Some(vec!["user1@example.com".into()]),
                mailto_user: None,
                from_address: "from@example.com".into(),
                author: Some("root".into()),
                comment: Some("Comment".into()),
                mode: Some(SmtpMode::StartTls),
                server: "localhost".into(),
                port: Some(555),
                username: Some("username".into()),
                ..Default::default()
            },
            &SmtpPrivateConfig {
                name: name.into(),
                password: Some("password".into()),
            },
        )?;

        assert!(get_endpoint(config, name).is_ok());
        Ok(())
    }

    #[test]
    fn test_smtp_create() -> Result<(), HttpError> {
        let mut config = empty_config();

        assert_eq!(get_endpoints(&config)?.len(), 0);
        add_smtp_endpoint_for_test(&mut config, "smtp-endpoint")?;

        // Endpoints must have a unique name
        assert!(add_smtp_endpoint_for_test(&mut config, "smtp-endpoint").is_err());
        assert_eq!(get_endpoints(&config)?.len(), 1);
        Ok(())
    }

    #[test]
    fn test_update_not_existing_returns_error() -> Result<(), HttpError> {
        let mut config = empty_config();

        assert!(update_endpoint(
            &mut config,
            "test",
            &Default::default(),
            &Default::default(),
            None,
            None,
        )
        .is_err());

        Ok(())
    }

    #[test]
    fn test_update_invalid_digest_returns_error() -> Result<(), HttpError> {
        let mut config = empty_config();
        add_smtp_endpoint_for_test(&mut config, "sendmail-endpoint")?;

        assert!(update_endpoint(
            &mut config,
            "sendmail-endpoint",
            &Default::default(),
            &Default::default(),
            None,
            Some(&[0; 32]),
        )
        .is_err());

        Ok(())
    }

    #[test]
    fn test_update() -> Result<(), HttpError> {
        let mut config = empty_config();
        add_smtp_endpoint_for_test(&mut config, "smtp-endpoint")?;

        let digest = config.digest;

        update_endpoint(
            &mut config,
            "smtp-endpoint",
            &SmtpConfigUpdater {
                mailto: Some(vec!["user2@example.com".into(), "user3@example.com".into()]),
                mailto_user: Some(vec!["root@pam".into()]),
                from_address: Some("root@example.com".into()),
                author: Some("newauthor".into()),
                comment: Some("new comment".into()),
                mode: Some(SmtpMode::Insecure),
                server: Some("pali".into()),
                port: Some(444),
                username: Some("newusername".into()),
                ..Default::default()
            },
            &Default::default(),
            None,
            Some(&digest),
        )?;

        let endpoint = get_endpoint(&config, "smtp-endpoint")?;

        assert_eq!(
            endpoint.mailto,
            Some(vec![
                "user2@example.com".to_string(),
                "user3@example.com".to_string()
            ])
        );
        assert_eq!(endpoint.mailto_user, Some(vec!["root@pam".to_string(),]));
        assert_eq!(endpoint.from_address, "root@example.com".to_string());
        assert_eq!(endpoint.author, Some("newauthor".to_string()));
        assert_eq!(endpoint.comment, Some("new comment".to_string()));

        // Test property deletion
        update_endpoint(
            &mut config,
            "smtp-endpoint",
            &Default::default(),
            &Default::default(),
            Some(&[
                DeleteableSmtpProperty::Author,
                DeleteableSmtpProperty::MailtoUser,
                DeleteableSmtpProperty::Port,
                DeleteableSmtpProperty::Username,
                DeleteableSmtpProperty::Comment,
            ]),
            None,
        )?;

        let endpoint = get_endpoint(&config, "smtp-endpoint")?;

        assert_eq!(endpoint.author, None);
        assert_eq!(endpoint.comment, None);
        assert_eq!(endpoint.port, None);
        assert_eq!(endpoint.username, None);
        assert_eq!(endpoint.mailto_user, None);

        Ok(())
    }

    #[test]
    fn test_delete() -> Result<(), HttpError> {
        let mut config = empty_config();
        add_smtp_endpoint_for_test(&mut config, "smtp-endpoint")?;

        delete_endpoint(&mut config, "smtp-endpoint")?;
        assert!(delete_endpoint(&mut config, "smtp-endpoint").is_err());
        assert_eq!(get_endpoints(&config)?.len(), 0);

        Ok(())
    }
}
