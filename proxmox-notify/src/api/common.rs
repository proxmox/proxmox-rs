use crate::api::ApiError;
use crate::{Bus, Config, Notification};

/// Send a notification to a given target.
///
/// The caller is responsible for any needed permission checks.
/// Returns an `ApiError` in case of an error.
pub fn send(config: &Config, channel: &str, notification: &Notification) -> Result<(), ApiError> {
    let bus = Bus::from_config(config).map_err(|err| {
        ApiError::internal_server_error(
            "Could not instantiate notification bus",
            Some(Box::new(err)),
        )
    })?;

    bus.send(channel, notification);

    Ok(())
}

/// Test target (group or single endpoint) identified by its `name`.
///
/// The caller is responsible for any needed permission checks.
/// Returns an `ApiError` if sending via the endpoint failed.
pub fn test_target(config: &Config, endpoint: &str) -> Result<(), ApiError> {
    let bus = Bus::from_config(config).map_err(|err| {
        ApiError::internal_server_error(
            "Could not instantiate notification bus",
            Some(Box::new(err)),
        )
    })?;

    bus.test_target(endpoint).map_err(|err| match err {
        crate::Error::TargetDoesNotExist(endpoint) => {
            ApiError::not_found(format!("endpoint '{endpoint}' does not exist"), None)
        }
        _ => ApiError::internal_server_error(
            format!("Could not test target: {err}"),
            Some(Box::new(err)),
        ),
    })?;

    Ok(())
}

/// Return all entities (targets, groups, filters) that are linked to the entity.
/// For instance, if a group 'grp1' contains the targets 'a', 'b' and 'c',
/// where grp1 has 'filter1' and 'a' has 'filter2' as filters, then
/// the result for 'grp1' would be [grp1, a, b, c, filter1, filter2].
/// The result will always contain the entity that was passed as a parameter.
/// If the entity does not exist, the result will only contain the entity.
pub fn get_referenced_entities(config: &Config, entity: &str) -> Result<Vec<String>, ApiError> {
    let entities = super::get_referenced_entities(config, entity);
    Ok(Vec::from_iter(entities.into_iter()))
}
