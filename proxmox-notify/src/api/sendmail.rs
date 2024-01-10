use proxmox_http_error::HttpError;

use crate::api::{http_bail, http_err};
use crate::endpoints::sendmail::{
    DeleteableSendmailProperty, SendmailConfig, SendmailConfigUpdater, SENDMAIL_TYPENAME,
};
use crate::Config;

/// Get a list of all sendmail endpoints.
///
/// The caller is responsible for any needed permission checks.
/// Returns a list of all sendmail endpoints or a `HttpError` if the config is
/// erroneous (`500 Internal server error`).
pub fn get_endpoints(config: &Config) -> Result<Vec<SendmailConfig>, HttpError> {
    config
        .config
        .convert_to_typed_array(SENDMAIL_TYPENAME)
        .map_err(|e| http_err!(NOT_FOUND, "Could not fetch endpoints: {e}"))
}

/// Get sendmail endpoint with given `name`.
///
/// The caller is responsible for any needed permission checks.
/// Returns the endpoint or a `HttpError` if the endpoint was not found (`404 Not found`).
pub fn get_endpoint(config: &Config, name: &str) -> Result<SendmailConfig, HttpError> {
    config
        .config
        .lookup(SENDMAIL_TYPENAME, name)
        .map_err(|_| http_err!(NOT_FOUND, "endpoint '{name}' not found"))
}

/// Add a new sendmail endpoint.
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns a `HttpError` if:
///   - an entity with the same name already exists (`400 Bad request`)
///   - the configuration could not be saved (`500 Internal server error`)
///   - mailto *and* mailto_user are both set to `None`
pub fn add_endpoint(config: &mut Config, endpoint: &SendmailConfig) -> Result<(), HttpError> {
    super::ensure_unique(config, &endpoint.name)?;

    if endpoint.mailto.is_none() && endpoint.mailto_user.is_none() {
        http_bail!(
            BAD_REQUEST,
            "must at least provide one recipient, either in mailto or in mailto-user"
        );
    }

    config
        .config
        .set_data(&endpoint.name, SENDMAIL_TYPENAME, endpoint)
        .map_err(|e| {
            http_err!(
                INTERNAL_SERVER_ERROR,
                "could not save endpoint '{}': {e}",
                endpoint.name
            )
        })
}

/// Update existing sendmail endpoint
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns a `HttpError` if:
///   - the configuration could not be saved (`500 Internal server error`)
///   - mailto *and* mailto_user are both set to `None`
pub fn update_endpoint(
    config: &mut Config,
    name: &str,
    updater: &SendmailConfigUpdater,
    delete: Option<&[DeleteableSendmailProperty]>,
    digest: Option<&[u8]>,
) -> Result<(), HttpError> {
    super::verify_digest(config, digest)?;

    let mut endpoint = get_endpoint(config, name)?;

    if let Some(delete) = delete {
        for deleteable_property in delete {
            match deleteable_property {
                DeleteableSendmailProperty::FromAddress => endpoint.from_address = None,
                DeleteableSendmailProperty::Author => endpoint.author = None,
                DeleteableSendmailProperty::Comment => endpoint.comment = None,
                DeleteableSendmailProperty::Mailto => endpoint.mailto = None,
                DeleteableSendmailProperty::MailtoUser => endpoint.mailto_user = None,
                DeleteableSendmailProperty::Disable => endpoint.disable = None,
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
        endpoint.from_address = Some(from_address.into());
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
        .set_data(name, SENDMAIL_TYPENAME, &endpoint)
        .map_err(|e| {
            http_err!(
                INTERNAL_SERVER_ERROR,
                "could not save endpoint '{}': {e}",
                endpoint.name
            )
        })
}

/// Delete existing sendmail endpoint
///
/// The caller is responsible for any needed permission checks.
/// The caller also responsible for locking the configuration files.
/// Returns a `HttpError` if:
///   - an entity with the same name already exists (`400 Bad request`)
///   - a referenced filter does not exist (`400 Bad request`)
///   - the configuration could not be saved (`500 Internal server error`)
pub fn delete_endpoint(config: &mut Config, name: &str) -> Result<(), HttpError> {
    // Check if the endpoint exists
    let _ = get_endpoint(config, name)?;
    super::ensure_safe_to_delete(config, name)?;

    config.config.sections.remove(name);

    Ok(())
}

#[cfg(all(feature = "pve-context", test))]
pub mod tests {
    use super::*;
    use crate::api::test_helpers::*;

    pub fn add_sendmail_endpoint_for_test(
        config: &mut Config,
        name: &str,
    ) -> Result<(), HttpError> {
        add_endpoint(
            config,
            &SendmailConfig {
                name: name.into(),
                mailto: Some(vec!["user1@example.com".into()]),
                mailto_user: None,
                from_address: Some("from@example.com".into()),
                author: Some("root".into()),
                comment: Some("Comment".into()),
                filter: None,
                ..Default::default()
            },
        )?;

        assert!(get_endpoint(config, name).is_ok());
        Ok(())
    }

    #[test]
    fn test_sendmail_create() -> Result<(), HttpError> {
        let mut config = empty_config();

        add_sendmail_endpoint_for_test(&mut config, "sendmail-endpoint")?;

        // Endpoints must have a unique name
        assert!(add_sendmail_endpoint_for_test(&mut config, "sendmail-endpoint").is_err());
        Ok(())
    }

    #[test]
    fn test_update_not_existing_returns_error() -> Result<(), HttpError> {
        let mut config = empty_config();

        assert!(update_endpoint(&mut config, "test", &Default::default(), None, None,).is_err());

        Ok(())
    }

    #[test]
    fn test_update_invalid_digest_returns_error() -> Result<(), HttpError> {
        let mut config = empty_config();
        add_sendmail_endpoint_for_test(&mut config, "sendmail-endpoint")?;

        assert!(update_endpoint(
            &mut config,
            "sendmail-endpoint",
            &SendmailConfigUpdater {
                mailto: Some(vec!["user2@example.com".into(), "user3@example.com".into()]),
                mailto_user: None,
                from_address: Some("root@example.com".into()),
                author: Some("newauthor".into()),
                comment: Some("new comment".into()),
                ..Default::default()
            },
            None,
            Some(&[0; 32]),
        )
        .is_err());

        Ok(())
    }

    #[test]
    fn test_sendmail_update() -> Result<(), HttpError> {
        let mut config = empty_config();
        add_sendmail_endpoint_for_test(&mut config, "sendmail-endpoint")?;

        let digest = config.digest;

        update_endpoint(
            &mut config,
            "sendmail-endpoint",
            &SendmailConfigUpdater {
                mailto: Some(vec!["user2@example.com".into(), "user3@example.com".into()]),
                mailto_user: Some(vec!["root@pam".into()]),
                from_address: Some("root@example.com".into()),
                author: Some("newauthor".into()),
                comment: Some("new comment".into()),
                ..Default::default()
            },
            None,
            Some(&digest),
        )?;

        let endpoint = get_endpoint(&config, "sendmail-endpoint")?;

        assert_eq!(
            endpoint.mailto,
            Some(vec![
                "user2@example.com".to_string(),
                "user3@example.com".to_string()
            ])
        );
        assert_eq!(endpoint.mailto_user, Some(vec!["root@pam".to_string(),]));
        assert_eq!(endpoint.from_address, Some("root@example.com".to_string()));
        assert_eq!(endpoint.author, Some("newauthor".to_string()));
        assert_eq!(endpoint.comment, Some("new comment".to_string()));

        // Test property deletion
        update_endpoint(
            &mut config,
            "sendmail-endpoint",
            &Default::default(),
            Some(&[
                DeleteableSendmailProperty::FromAddress,
                DeleteableSendmailProperty::Author,
            ]),
            None,
        )?;

        let endpoint = get_endpoint(&config, "sendmail-endpoint")?;

        assert_eq!(endpoint.from_address, None);
        assert_eq!(endpoint.author, None);

        Ok(())
    }

    #[test]
    fn test_sendmail_delete() -> Result<(), HttpError> {
        let mut config = empty_config();
        add_sendmail_endpoint_for_test(&mut config, "sendmail-endpoint")?;

        delete_endpoint(&mut config, "sendmail-endpoint")?;
        assert!(delete_endpoint(&mut config, "sendmail-endpoint").is_err());

        Ok(())
    }
}
