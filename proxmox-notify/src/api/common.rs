use proxmox_http_error::HttpError;

use super::http_err;
use crate::{Bus, Config, Notification};

/// Send a notification to a given target.
///
/// The caller is responsible for any needed permission checks.
/// Returns an `anyhow::Error` in case of an error.
pub fn send(config: &Config, notification: &Notification) -> Result<(), HttpError> {
    let bus = Bus::from_config(config).map_err(|err| {
        http_err!(
            INTERNAL_SERVER_ERROR,
            "Could not instantiate notification bus: {err}"
        )
    })?;

    bus.send(notification);

    Ok(())
}

/// Test target (group or single endpoint) identified by its `name`.
///
/// The caller is responsible for any needed permission checks.
/// Returns an `anyhow::Error` if sending via the endpoint failed.
pub fn test_target(config: &Config, endpoint: &str) -> Result<(), HttpError> {
    let bus = Bus::from_config(config).map_err(|err| {
        http_err!(
            INTERNAL_SERVER_ERROR,
            "Could not instantiate notification bus: {err}"
        )
    })?;

    bus.test_target(endpoint).map_err(|err| match err {
        crate::Error::TargetDoesNotExist(endpoint) => {
            http_err!(NOT_FOUND, "endpoint '{endpoint}' does not exist")
        }
        _ => http_err!(INTERNAL_SERVER_ERROR, "Could not test target: {err}"),
    })?;

    Ok(())
}

/// Return all entities (targets, groups, filters) that are linked to the entity.
/// For instance, if a group 'grp1' contains the targets 'a', 'b' and 'c',
/// where grp1 has 'filter1' and 'a' has 'filter2' as filters, then
/// the result for 'grp1' would be [grp1, a, b, c, filter1, filter2].
/// The result will always contain the entity that was passed as a parameter.
/// If the entity does not exist, the result will only contain the entity.
pub fn get_referenced_entities(config: &Config, entity: &str) -> Result<Vec<String>, HttpError> {
    let entities = super::get_referenced_entities(config, entity);
    Ok(Vec::from_iter(entities))
}
