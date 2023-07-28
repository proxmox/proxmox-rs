use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::json;

use proxmox_http::client::sync::Client;
use proxmox_http::{HttpClient, HttpOptions, ProxyConfig};
use proxmox_schema::api_types::COMMENT_SCHEMA;
use proxmox_schema::{api, Updater};

use crate::context::context;
use crate::renderer::TemplateRenderer;
use crate::schema::ENTITY_NAME_SCHEMA;
use crate::{renderer, Endpoint, Error, Notification, Severity};

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
        let properties = notification.properties.as_ref();

        let title = renderer::render_template(
            TemplateRenderer::Plaintext,
            &notification.title,
            properties,
        )?;
        let message =
            renderer::render_template(TemplateRenderer::Plaintext, &notification.body, properties)?;

        // We don't have a TemplateRenderer::Markdown yet, so simply put everything
        // in code tags. Otherwise tables etc. are not formatted properly
        let message = format!("```\n{message}\n```");

        let body = json!({
            "title": &title,
            "message": &message,
            "priority": severity_to_priority(notification.severity),
            "extras": {
                "client::display": {
                    "contentType": "text/markdown"
                }
            }
        });

        let body = serde_json::to_vec(&body)
            .map_err(|err| Error::NotifyFailed(self.name().to_string(), err.into()))?;
        let extra_headers = HashMap::from([(
            "Authorization".into(),
            format!("Bearer {}", self.private_config.token),
        )]);

        let proxy_config = context()
            .http_proxy_config()
            .map(|url| ProxyConfig::parse_proxy_url(&url))
            .transpose()
            .map_err(|err| Error::NotifyFailed(self.name().to_string(), err.into()))?;

        let options = HttpOptions {
            proxy_config,
            ..Default::default()
        };

        let client = Client::new(options);
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

    fn filter(&self) -> Option<&str> {
        self.config.filter.as_deref()
    }
}
