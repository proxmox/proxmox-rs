//! This endpoint implements a generic webhook target, allowing users to send notifications through
//! a highly customizable HTTP request.
//!
//! The configuration options include specifying the HTTP method, URL, headers, and body.
//! URLs, headers, and the body support template expansion using the [`handlebars`] templating engine.
//! For secure handling of passwords or tokens, these values can be stored as secrets.
//! Secrets are kept in a private configuration file, accessible only by root, and are not retrievable via the API.
//! Within templates, secrets can be referenced using `{{ secrets.<name> }}`.
//! Additionally, we take measures to prevent secrets from appearing in logs or error messages.
use std::time::Duration;

use handlebars::{
    Context as HandlebarsContext, Handlebars, Helper, HelperResult, Output, RenderContext,
    RenderError as HandlebarsRenderError,
};
use http::Request;
use percent_encoding::AsciiSet;
use proxmox_schema::property_string::PropertyString;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use proxmox_http::client::sync::Client;
use proxmox_http::{HttpClient, HttpOptions, ProxyConfig};
use proxmox_schema::api_types::{COMMENT_SCHEMA, HTTP_URL_SCHEMA};
use proxmox_schema::{api, ApiStringFormat, ApiType, Schema, StringSchema, Updater};

use crate::context::context;
use crate::renderer::TemplateType;
use crate::schema::ENTITY_NAME_SCHEMA;
use crate::{renderer, Content, Endpoint, Error, Notification, Origin};

/// This will be used as a section type in the public/private configuration file.
pub(crate) const WEBHOOK_TYPENAME: &str = "webhook";

const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

#[api]
#[derive(Serialize, Deserialize, Clone, Copy, Default)]
#[serde(rename_all = "kebab-case")]
/// HTTP Method to use.
pub enum HttpMethod {
    /// HTTP POST
    #[default]
    Post,
    /// HTTP PUT
    Put,
    /// HTTP GET
    Get,
}

// We only ever need a &str, so we rather implement this
// instead of Display.
impl From<HttpMethod> for &str {
    fn from(value: HttpMethod) -> Self {
        match value {
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Get => "GET",
        }
    }
}

#[api(
    properties: {
        name: {
            schema: ENTITY_NAME_SCHEMA,
        },
        url: {
            schema: HTTP_URL_SCHEMA,
        },
        comment: {
            optional: true,
            schema: COMMENT_SCHEMA,
        },
        header: {
            type: Array,
            items: {
                schema: KEY_AND_BASE64_VALUE_SCHEMA,
            },
            optional: true,
        },
        secret: {
            type: Array,
            items: {
                schema: KEY_AND_BASE64_VALUE_SCHEMA,
            },
            optional: true,
        },
    }
)]
#[derive(Serialize, Deserialize, Updater, Default, Clone)]
#[serde(rename_all = "kebab-case")]
/// Config for  Webhook notification endpoints
pub struct WebhookConfig {
    /// Name of the endpoint.
    #[updater(skip)]
    pub name: String,

    pub method: HttpMethod,

    /// Webhook URL. Supports templating.
    pub url: String,
    /// Array of HTTP headers. Each entry is a property string with a name and a value.
    /// The value property contains the header in base64 encoding. Supports templating.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[updater(serde(skip_serializing_if = "Option::is_none"))]
    pub header: Vec<PropertyString<KeyAndBase64Val>>,
    /// The HTTP body to send. Supports templating.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,

    /// Comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Disable this target.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable: Option<bool>,
    /// Origin of this config entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[updater(skip)]
    pub origin: Option<Origin>,
    /// Array of secrets. Each entry is a property string with a name and an optional value.
    /// The value property contains the secret in base64 encoding.
    /// For any API endpoints returning the endpoint config,
    /// only the secret name but not the value will be returned.
    /// When updating the config, also send all secrets that you want
    /// to keep, setting only the name but not the value. Can be accessed from templates.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[updater(serde(skip_serializing_if = "Option::is_none"))]
    pub secret: Vec<PropertyString<KeyAndBase64Val>>,
}

#[api(
    properties: {
        name: {
            schema: ENTITY_NAME_SCHEMA,
        },
        secret: {
            type: Array,
            items: {
                schema: KEY_AND_BASE64_VALUE_SCHEMA,
            },
            optional: true,
        },
    }
)]
#[derive(Serialize, Deserialize, Clone, Updater, Default)]
#[serde(rename_all = "kebab-case")]
/// Private configuration for Webhook notification endpoints.
/// This config will be saved to a separate configuration file with stricter
/// permissions (root:root 0600).
pub struct WebhookPrivateConfig {
    /// Name of the endpoint
    #[updater(skip)]
    pub name: String,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[updater(serde(skip_serializing_if = "Option::is_none"))]
    /// Array of secrets. Each entry is a property string with a name,
    /// and a value property. The value property contains the secret
    /// in base64 encoding. Can be accessed from templates.
    pub secret: Vec<PropertyString<KeyAndBase64Val>>,
}

/// A Webhook notification endpoint.
pub struct WebhookEndpoint {
    pub config: WebhookConfig,
    pub private_config: WebhookPrivateConfig,
}

#[api]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Webhook configuration properties that can be deleted.
pub enum DeleteableWebhookProperty {
    /// Delete `comment`.
    Comment,
    /// Delete `disable`.
    Disable,
    /// Delete `header`.
    Header,
    /// Delete `body`.
    Body,
    /// Delete `secret`.
    Secret,
}

#[api]
#[derive(Serialize, Deserialize, Debug, Default, Clone)]
/// Datatype used to represent key-value pairs, the value
/// being encoded in base64.
pub struct KeyAndBase64Val {
    /// Name
    pub name: String,
    /// Base64 encoded value
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
}

impl KeyAndBase64Val {
    #[cfg(test)]
    pub fn new_with_plain_value(name: &str, value: &str) -> Self {
        let value = base64::encode(value);

        Self {
            name: name.into(),
            value: Some(value),
        }
    }

    /// Decode the contained value, returning the plaintext value
    ///
    /// Returns an error if the contained value is not valid base64-encoded
    /// text.
    pub fn decode_value(&self) -> Result<String, Error> {
        let value = self.value.as_deref().unwrap_or_default();
        let bytes = base64::decode(value).map_err(|_| {
            Error::Generic(format!(
                "could not decode base64 value with key '{}'",
                self.name
            ))
        })?;
        let value = String::from_utf8(bytes).map_err(|_| {
            Error::Generic(format!(
                "could not decode UTF8 string from base64, key '{}'",
                self.name
            ))
        })?;

        Ok(value)
    }
}

pub const KEY_AND_BASE64_VALUE_SCHEMA: Schema =
    StringSchema::new("String schema for pairs of keys and base64 encoded values")
        .format(&ApiStringFormat::PropertyString(
            &KeyAndBase64Val::API_SCHEMA,
        ))
        .schema();

impl Endpoint for WebhookEndpoint {
    /// Send a notification to a webhook endpoint.
    fn send(&self, notification: &Notification) -> Result<(), Error> {
        let request = self.build_request(notification)?;

        self.create_client()?
            .request(request)
            .map_err(|err| self.mask_secret_in_error(err))?;

        Ok(())
    }

    /// Return the name of the endpoint.
    fn name(&self) -> &str {
        &self.config.name
    }

    /// Check if the endpoint is disabled
    fn disabled(&self) -> bool {
        self.config.disable.unwrap_or_default()
    }
}

impl WebhookEndpoint {
    fn create_client(&self) -> Result<Client, Error> {
        let proxy_config = context()
            .http_proxy_config()
            .map(|url| ProxyConfig::parse_proxy_url(&url))
            .transpose()
            .map_err(|err| Error::NotifyFailed(self.name().to_string(), err.into()))?;

        let options = HttpOptions {
            proxy_config,
            ..Default::default()
        };

        Ok(Client::new_with_timeout(options, HTTP_TIMEOUT))
    }

    fn build_request(&self, notification: &Notification) -> Result<Request<String>, Error> {
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

        let mut fields = Map::new();

        for (field_name, field_value) in &notification.metadata.additional_fields {
            fields.insert(field_name.clone(), Value::String(field_value.to_string()));
        }

        let mut secrets = Map::new();

        for secret in &self.private_config.secret {
            let value = secret.decode_value()?;
            secrets.insert(secret.name.clone(), Value::String(value));
        }

        let data = json!({
            "title": &title,
            "message": &message,
            "severity": notification.metadata.severity,
            "timestamp": notification.metadata.timestamp,
            "fields": fields,
            "secrets": secrets,
        });

        let handlebars = setup_handlebars();
        let body_template = self.base64_decode(self.config.body.as_deref().unwrap_or_default())?;

        let body = handlebars
            .render_template(&body_template, &data)
            .map_err(|err| self.mask_secret_in_error(err))
            .map_err(|err| Error::Generic(format!("failed to render webhook body: {err}")))?;

        let url = handlebars
            .render_template(&self.config.url, &data)
            .map_err(|err| self.mask_secret_in_error(err))
            .map_err(|err| Error::Generic(format!("failed to render webhook url: {err}")))?;

        let method: &str = self.config.method.into();
        let mut builder = http::Request::builder().uri(url).method(method);

        for header in &self.config.header {
            let value = header.decode_value()?;

            let value = handlebars
                .render_template(&value, &data)
                .map_err(|err| self.mask_secret_in_error(err))
                .map_err(|err| {
                    Error::Generic(format!(
                        "failed to render header value template: {value}: {err}"
                    ))
                })?;

            builder = builder.header(header.name.clone(), value);
        }

        let request = builder
            .body(body)
            .map_err(|err| self.mask_secret_in_error(err))
            .map_err(|err| Error::Generic(format!("failed to build http request: {err}")))?;

        Ok(request)
    }

    fn base64_decode(&self, s: &str) -> Result<String, Error> {
        // Also here, TODO: revisit Error variants for the *whole* crate.
        let s = base64::decode(s)
            .map_err(|err| Error::Generic(format!("could not decode base64 value: {err}")))?;

        String::from_utf8(s).map_err(|err| {
            Error::Generic(format!(
                "base64 encoded value did not contain valid utf8: {err}"
            ))
        })
    }

    /// Mask secrets in errors to avoid them showing up in error messages and log files
    ///
    /// Use this for any error from third-party code where you are not 100%
    /// sure whether it could leak the content of secrets in the error.
    /// For instance, the http client will contain the URL, including
    /// any URL parameters that could contain tokens.
    ///
    /// This function will only mask exact matches, but this should suffice
    /// for the majority of cases.
    fn mask_secret_in_error(&self, error: impl std::fmt::Display) -> Error {
        let mut s = error.to_string();

        for secret_value in &self.private_config.secret {
            match secret_value.decode_value() {
                Ok(value) => s = s.replace(&value, "<masked>"),
                Err(e) => return e,
            }
        }

        Error::Generic(s)
    }
}

fn setup_handlebars() -> Handlebars<'static> {
    let mut handlebars = Handlebars::new();

    handlebars.register_helper("url-encode", Box::new(handlebars_percent_encode));
    handlebars.register_helper("json", Box::new(handlebars_json));
    handlebars.register_helper("escape", Box::new(handlebars_escape));

    // There is no escape.
    handlebars.register_escape_fn(handlebars::no_escape);

    handlebars
}

fn handlebars_percent_encode(
    h: &Helper,
    _: &Handlebars,
    _: &HandlebarsContext,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param0 = h
        .param(0)
        .and_then(|v| v.value().as_str())
        .ok_or_else(|| HandlebarsRenderError::new("url-encode: missing parameter"))?;

    // See https://developer.mozilla.org/en-US/docs/Glossary/Percent-encoding
    const FRAGMENT: &AsciiSet = &percent_encoding::CONTROLS
        .add(b':')
        .add(b'/')
        .add(b'?')
        .add(b'#')
        .add(b'[')
        .add(b']')
        .add(b'@')
        .add(b'!')
        .add(b'$')
        .add(b'&')
        .add(b'\'')
        .add(b'(')
        .add(b')')
        .add(b'*')
        .add(b'+')
        .add(b',')
        .add(b';')
        .add(b'=')
        .add(b'%')
        .add(b' ');
    let a = percent_encoding::utf8_percent_encode(param0, FRAGMENT);

    out.write(&a.to_string())?;

    Ok(())
}

fn handlebars_json(
    h: &Helper,
    _: &Handlebars,
    _: &HandlebarsContext,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let param0 = h
        .param(0)
        .map(|v| v.value())
        .ok_or_else(|| HandlebarsRenderError::new("json: missing parameter"))?;

    let json = serde_json::to_string(param0)?;
    out.write(&json)?;

    Ok(())
}

fn handlebars_escape(
    h: &Helper,
    _: &Handlebars,
    _: &HandlebarsContext,
    _rc: &mut RenderContext,
    out: &mut dyn Output,
) -> HelperResult {
    let text = h
        .param(0)
        .and_then(|v| v.value().as_str())
        .ok_or_else(|| HandlebarsRenderError::new("escape: missing text parameter"))?;

    let val = Value::String(text.to_string());
    let json = serde_json::to_string(&val)?;
    out.write(&json[1..json.len() - 1])?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::Severity;

    #[test]
    fn test_build_request() -> Result<(), Error> {
        let data = HashMap::from_iter([
            ("hello".into(), "hello world".into()),
            ("test".into(), "escaped\nstring".into()),
        ]);

        let body_template = r#"
{{ fields.test }}
{{ escape fields.test }}

{{ json fields }}
{{ json fields.hello }}

{{ url-encode fields.hello }}

{{ json severity }}

"#;

        let expected_body = r#"
escaped
string
escaped\nstring

{"hello":"hello world","test":"escaped\nstring"}
"hello world"

hello%20world

"info"

"#;

        let endpoint = WebhookEndpoint {
            config: WebhookConfig {
                name: "test".into(),
                method: HttpMethod::Post,
                url: "http://localhost/{{ url-encode fields.hello }}".into(),
                header: vec![
                    KeyAndBase64Val::new_with_plain_value("X-Severity", "{{ severity }}").into(),
                ],
                body: Some(base64::encode(body_template)),
                ..Default::default()
            },
            private_config: WebhookPrivateConfig {
                name: "test".into(),
                ..Default::default()
            },
        };

        let notification = Notification::from_template(Severity::Info, "foo", json!({}), data);

        let request = endpoint.build_request(&notification)?;

        assert_eq!(request.uri(), "http://localhost/hello%20world");
        assert_eq!(request.body(), expected_body);
        assert_eq!(request.method(), "POST");

        assert_eq!(request.headers().get("X-Severity").unwrap(), "info");

        Ok(())
    }
}
