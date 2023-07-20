use std::collections::HashMap;

use crate::schema::ENTITY_NAME_SCHEMA;
use crate::{Endpoint, Error, Notification, Severity};

use proxmox_schema::api_types::COMMENT_SCHEMA;
use serde::{Deserialize, Serialize};

use proxmox_http::client::sync::Client;
use proxmox_http::{HttpClient, HttpOptions};
use proxmox_schema::{api, Updater};

#[derive(Serialize)]
struct GotifyMessageBody<'a> {
    title: &'a str,
    message: &'a str,
    priority: u32,
}

fn severity_to_priority(level: Severity) -> u32 {
    match level {
        Severity::Info => 1,
        Severity::Notice => 3,
        Severity::Warning => 5,
        Severity::Error => 9,
    }
}

pub(crate) const GOTIFY_TYPENAME: &str = "gotify";

#[api(
    properties: {
        name: {
            schema: ENTITY_NAME_SCHEMA,
        },
        comment: {
            optional: true,
            schema: COMMENT_SCHEMA,
        },
        filter: {
            optional: true,
            schema: ENTITY_NAME_SCHEMA,
        },
    }
)]
#[derive(Serialize, Deserialize, Updater, Default)]
#[serde(rename_all = "kebab-case")]
/// Config for  Gotify notification endpoints
pub struct GotifyConfig {
    /// Name of the endpoint
    #[updater(skip)]
    pub name: String,
    /// Gotify Server URL
    pub server: String,
    /// Comment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Filter to apply
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
}

#[api()]
#[derive(Serialize, Deserialize, Clone, Updater)]
#[serde(rename_all = "kebab-case")]
/// Private configuration for Gotify notification endpoints.
/// This config will be saved to a separate configuration file with stricter
/// permissions (root:root 0600)
pub struct GotifyPrivateConfig {
    /// Name of the endpoint
    #[updater(skip)]
    pub name: String,
    /// Authentication token
    pub token: String,
}

/// A Gotify notification endpoint.
pub struct GotifyEndpoint {
    pub config: GotifyConfig,
    pub private_config: GotifyPrivateConfig,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeleteableGotifyProperty {
    Comment,
    Filter,
}

impl Endpoint for GotifyEndpoint {
    fn send(&self, notification: &Notification) -> Result<(), Error> {
        // TODO: What about proxy configuration?
        let client = Client::new(HttpOptions::default());

        let uri = format!("{}/message", self.config.server);

        let body = GotifyMessageBody {
            title: &notification.title,
            message: &notification.body,
            priority: severity_to_priority(notification.severity),
        };

        let body = serde_json::to_vec(&body)
            .map_err(|err| Error::NotifyFailed(self.name().to_string(), err.into()))?;
        let extra_headers = HashMap::from([(
            "Authorization".into(),
            format!("Bearer {}", self.private_config.token),
        )]);

        client
            .post(
                &uri,
                Some(body.as_slice()),
                Some("application/json"),
                Some(&extra_headers),
            )
            .map_err(|err| Error::NotifyFailed(self.name().to_string(), err.into()))?;

        Ok(())
    }

    fn name(&self) -> &str {
        &self.config.name
    }

    fn filter(&self) -> Option<&str> {
        self.config.filter.as_deref()
    }
}
