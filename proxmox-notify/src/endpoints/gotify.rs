use std::collections::HashMap;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;

use proxmox_http::client::sync::Client;
use proxmox_http::{HttpClient, HttpOptions, ProxyConfig};
use proxmox_schema::api_types::COMMENT_SCHEMA;
use proxmox_schema::{api, Updater};

use crate::context::context;
use crate::renderer::TemplateType;
use crate::schema::ENTITY_NAME_SCHEMA;
use crate::{renderer, Content, Endpoint, Error, Notification, Origin, Severity};

const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

fn severity_to_priority(level: Severity) -> u32 {
    match level {
        Severity::Info => 1,
        Severity::Notice => 3,
        Severity::Warning => 5,
        Severity::Error => 9,
        Severity::Unknown => 3,
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
    }
)]
#[derive(Serialize, Deserialize, Updater, Default)]
#[serde(rename_all = "kebab-case")]
/// Config for  Gotify notification endpoints
pub struct GotifyConfig {
    /// Name of the endpoint.
    #[updater(skip)]
    pub name: String,
    /// Gotify Server URL.
    pub server: String,
    /// Comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Deprecated.
    #[serde(skip_serializing)]
    #[updater(skip)]
    pub filter: Option<String>,
    /// Disable this target.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable: Option<bool>,
    /// Origin of this config entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[updater(skip)]
    pub origin: Option<Origin>,
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

#[api]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// The set of properties that can be deleted from a gotify endpoint configuration.
pub enum DeleteableGotifyProperty {
    /// Delete `comment`
    Comment,
    /// Delete `disable`
    Disable,
}

impl Endpoint for GotifyEndpoint {
    fn send(&self, notification: &Notification) -> Result<(), Error> {
        let (title, message) = match &notification.content {
            Content::Template {
                template_name,
                data,
            } => {
                let rendered_title =
                    renderer::render_template(TemplateType::Subject, template_name, data)?;
                let rendered_message =
                    renderer::render_template(TemplateType::PlaintextBody, template_name, data)?;

                (rendered_title, rendered_message)
            }
            #[cfg(feature = "mail-forwarder")]
            Content::ForwardedMail { title, body, .. } => (title.clone(), body.clone()),
        };

        // We don't have a TemplateRenderer::Markdown yet, so simply put everything
        // in code tags. Otherwise tables etc. are not formatted properly
        let message = format!("```\n{message}\n```");

        let body = json!({
            "title": &title,
            "message": &message,
            "priority": severity_to_priority(notification.metadata.severity),
            "extras": {
                "client::display": {
                    "contentType": "text/markdown"
                }
            }
        });

        let body = serde_json::to_vec(&body)
            .map_err(|err| Error::NotifyFailed(self.name().to_string(), err.into()))?;
        let extra_headers = HashMap::from([
            (
                "Authorization".into(),
                format!("Bearer {}", self.private_config.token),
            ),
            ("X-Gotify-Key".into(), self.private_config.token.clone()),
        ]);

        let proxy_config = context()
            .http_proxy_config()
            .map(|url| ProxyConfig::parse_proxy_url(&url))
            .transpose()
            .map_err(|err| Error::NotifyFailed(self.name().to_string(), err.into()))?;

        let options = HttpOptions {
            proxy_config,
            ..Default::default()
        };

        let client = Client::new_with_timeout(options, HTTP_TIMEOUT);
        let uri = format!("{}/message", self.config.server);

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

    /// Check if the endpoint is disabled
    fn disabled(&self) -> bool {
        self.config.disable.unwrap_or_default()
    }
}
