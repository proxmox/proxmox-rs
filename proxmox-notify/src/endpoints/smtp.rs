use lettre::message::{Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{message::header::ContentType, Message, SmtpTransport, Transport};
use serde::{Deserialize, Serialize};
use std::time::Duration;

use proxmox_schema::api_types::COMMENT_SCHEMA;
use proxmox_schema::{api, Updater};

use crate::context::context;
use crate::endpoints::common::mail;
use crate::renderer::TemplateRenderer;
use crate::schema::{EMAIL_SCHEMA, ENTITY_NAME_SCHEMA, USER_SCHEMA};
use crate::{renderer, Content, Endpoint, Error, Notification};

pub(crate) const SMTP_TYPENAME: &str = "smtp";

const SMTP_PORT: u16 = 25;
const SMTP_SUBMISSION_STARTTLS_PORT: u16 = 587;
const SMTP_SUBMISSION_TLS_PORT: u16 = 465;
const SMTP_TIMEOUT: u16 = 5;

#[api]
#[derive(Debug, Serialize, Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
/// Connection security
pub enum SmtpMode {
    /// No encryption (insecure), plain SMTP
    Insecure,
    /// Upgrade to TLS after connecting
    #[serde(rename = "starttls")]
    StartTls,
    /// Use TLS-secured connection
    #[default]
    Tls,
}

#[api(
    properties: {
        name: {
            schema: ENTITY_NAME_SCHEMA,
        },
        mailto: {
            type: Array,
                items: {
                    schema: EMAIL_SCHEMA,
            },
            optional: true,
        },
        "mailto-user": {
            type: Array,
            items: {
                schema: USER_SCHEMA,
            },
            optional: true,
        },
        comment: {
            optional: true,
            schema: COMMENT_SCHEMA,
        },
    },
)]
#[derive(Debug, Serialize, Deserialize, Updater, Default)]
#[serde(rename_all = "kebab-case")]
/// Config for Sendmail notification endpoints
pub struct SmtpConfig {
    /// Name of the endpoint
    #[updater(skip)]
    pub name: String,
    /// Host name or IP of the SMTP relay
    pub server: String,
    /// Port to use when connecting to the SMTP relay
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<SmtpMode>,
    /// Username for authentication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    /// Mail recipients
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mailto: Option<Vec<String>>,
    /// Mail recipients
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mailto_user: Option<Vec<String>>,
    /// `From` address for the mail
    pub from_address: String,
    /// Author of the mail
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Comment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeleteableSmtpProperty {
    Author,
    Comment,
    Mailto,
    MailtoUser,
    Password,
    Port,
    Username,
}

#[api]
#[derive(Serialize, Deserialize, Clone, Updater, Debug)]
#[serde(rename_all = "kebab-case")]
/// Private configuration for SMTP notification endpoints.
/// This config will be saved to a separate configuration file with stricter
/// permissions (root:root 0600)
pub struct SmtpPrivateConfig {
    /// Name of the endpoint
    #[updater(skip)]
    pub name: String,
    /// Authentication token
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

/// A sendmail notification endpoint.
pub struct SmtpEndpoint {
    pub config: SmtpConfig,
    pub private_config: SmtpPrivateConfig,
}

impl Endpoint for SmtpEndpoint {
    fn send(&self, notification: &Notification) -> Result<(), Error> {
        let tls_parameters = TlsParameters::new(self.config.server.clone())
            .map_err(|err| Error::NotifyFailed(self.name().into(), Box::new(err)))?;

        let (port, tls) = match self.config.mode.unwrap_or_default() {
            SmtpMode::Insecure => {
                let port = self.config.port.unwrap_or(SMTP_PORT);
                (port, Tls::None)
            }
            SmtpMode::StartTls => {
                let port = self.config.port.unwrap_or(SMTP_SUBMISSION_STARTTLS_PORT);
                (port, Tls::Required(tls_parameters))
            }
            SmtpMode::Tls => {
                let port = self.config.port.unwrap_or(SMTP_SUBMISSION_TLS_PORT);
                (port, Tls::Wrapper(tls_parameters))
            }
        };

        let mut transport_builder = SmtpTransport::builder_dangerous(&self.config.server)
            .tls(tls)
            .port(port)
            .timeout(Some(Duration::from_secs(SMTP_TIMEOUT.into())));

        if let Some(username) = self.config.username.as_deref() {
            if let Some(password) = self.private_config.password.as_deref() {
                transport_builder = transport_builder.credentials((username, password).into());
            } else {
                return Err(Error::NotifyFailed(
                    self.name().into(),
                    Box::new(Error::Generic(
                        "username is set but no password was provided".to_owned(),
                    )),
                ));
            }
        }

        let transport = transport_builder.build();

        let recipients = mail::get_recipients(
            self.config.mailto.as_deref(),
            self.config.mailto_user.as_deref(),
        );
        let mail_from = self.config.from_address.clone();

        let parse_address = |addr: &str| -> Result<Mailbox, Error> {
            addr.parse()
                .map_err(|err| Error::NotifyFailed(self.name().into(), Box::new(err)))
        };

        let author = self
            .config
            .author
            .clone()
            .unwrap_or_else(|| context().default_sendmail_author());

        let mut email_builder =
            Message::builder().from(parse_address(&format!("{author} <{mail_from}>"))?);

        for recipient in recipients {
            email_builder = email_builder.to(parse_address(&recipient)?);
        }

        let email = match &notification.content {
            Content::Template {
                title_template,
                body_template,
                data,
            } => {
                let subject =
                    renderer::render_template(TemplateRenderer::Plaintext, title_template, data)?;
                let html_part =
                    renderer::render_template(TemplateRenderer::Html, body_template, data)?;
                let text_part =
                    renderer::render_template(TemplateRenderer::Plaintext, body_template, data)?;

                email_builder = email_builder.subject(subject);

                email_builder
                    .multipart(
                        MultiPart::alternative()
                            .singlepart(
                                SinglePart::builder()
                                    .header(ContentType::TEXT_PLAIN)
                                    .body(text_part),
                            )
                            .singlepart(
                                SinglePart::builder()
                                    .header(ContentType::TEXT_HTML)
                                    .body(html_part),
                            ),
                    )
                    .map_err(|err| Error::NotifyFailed(self.name().into(), Box::new(err)))?
            }
            #[cfg(feature = "mail-forwarder")]
            Content::ForwardedMail { ref raw, title, .. } => {
                email_builder = email_builder.subject(title);

                // Forwarded messages are embedded inline as 'message/rfc822'
                // this let's us avoid rewriting any headers (e.g. From)
                email_builder
                    .singlepart(
                        SinglePart::builder()
                            .header(ContentType::parse("message/rfc822").unwrap())
                            .body(raw.to_owned()),
                    )
                    .map_err(|err| Error::NotifyFailed(self.name().into(), Box::new(err)))?
            }
        };

        transport
            .send(&email)
            .map_err(|err| Error::NotifyFailed(self.name().into(), err.into()))?;

        Ok(())
    }

    fn name(&self) -> &str {
        &self.config.name
    }
}
