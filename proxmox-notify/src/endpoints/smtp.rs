use std::time::Duration;

use lettre::message::header::{HeaderName, HeaderValue};
use lettre::message::{Mailbox, MultiPart, SinglePart};
use lettre::transport::smtp::client::{Tls, TlsParameters};
use lettre::{message::header::ContentType, Message, SmtpTransport, Transport};
use serde::{Deserialize, Serialize};

use proxmox_schema::api_types::COMMENT_SCHEMA;
use proxmox_schema::{api, Updater};

use crate::context::context;
use crate::endpoints::common::mail;
use crate::renderer::TemplateType;
use crate::schema::{EMAIL_SCHEMA, ENTITY_NAME_SCHEMA, USER_SCHEMA};
use crate::{renderer, Content, Endpoint, Error, Notification, Origin};

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
    /// Disable this target.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disable: Option<bool>,
    /// Origin of this config entry.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[updater(skip)]
    pub origin: Option<Origin>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeleteableSmtpProperty {
    Author,
    Comment,
    Disable,
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

        let mut email = match &notification.content {
            Content::Template {
                template_name,
                data,
            } => {
                let subject =
                    renderer::render_template(TemplateType::Subject, template_name, data)?;
                let html_part =
                    renderer::render_template(TemplateType::HtmlBody, template_name, data)?;
                let text_part =
                    renderer::render_template(TemplateType::PlaintextBody, template_name, data)?;

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
                use lettre::message::header::ContentTransferEncoding;
                use lettre::message::Body;

                let parsed_message = mail_parser::Message::parse(raw)
                    .ok_or_else(|| Error::Generic("could not parse forwarded email".to_string()))?;

                let root_part = parsed_message
                    .part(0)
                    .ok_or_else(|| Error::Generic("root message part not present".to_string()))?;

                let raw_body = parsed_message
                    .raw_message()
                    .get(root_part.offset_body..root_part.offset_end)
                    .ok_or_else(|| Error::Generic("could not get raw body content".to_string()))?;

                // We assume that the original message content is already properly
                // encoded, thus we add the original message body in 'Binary' encoding.
                // This prohibits lettre from trying to re-encode our raw body data.
                // lettre will automatically set the `Content-Transfer-Encoding: binary` header,
                // which we need to remove. The actual transfer encoding is later
                // copied from the original message headers.
                let body =
                    Body::new_with_encoding(raw_body.to_vec(), ContentTransferEncoding::Binary)
                        .map_err(|_| Error::Generic("could not create body".into()))?;
                let mut message = email_builder
                    .subject(title)
                    .body(body)
                    .map_err(|err| Error::NotifyFailed(self.name().into(), Box::new(err)))?;
                message
                    .headers_mut()
                    .remove_raw("Content-Transfer-Encoding");

                // Copy over all headers that are relevant to display the original body correctly.
                // Unfortunately this is a bit cumbersome, as we use separate crates for mail parsing (mail-parser)
                // and creating/sending mails (lettre).
                // Note: Other MIME-Headers, such as Content-{ID,Description,Disposition} are only used
                // for body-parts in multipart messages, so we can ignore them for the messages headers.
                // Since we send the original raw body, the part-headers will be included any way.
                for header in parsed_message.headers() {
                    let header_name = header.name.as_str();
                    // Email headers are case-insensitive, so convert to lowercase...
                    let value = match header_name.to_lowercase().as_str() {
                        "content-type" => {
                            if let mail_parser::HeaderValue::ContentType(ct) = header.value() {
                                // mail_parser does not give us access to the full decoded and unfolded
                                // header value, so we unfortunately need to reassemble it ourselves.
                                // Meh.
                                let mut value = ct.ctype().to_string();
                                if let Some(subtype) = ct.subtype() {
                                    value.push('/');
                                    value.push_str(subtype);
                                }
                                if let Some(attributes) = ct.attributes() {
                                    use std::fmt::Write;

                                    for attribute in attributes {
                                        let _ = write!(
                                            &mut value,
                                            "; {}=\"{}\"",
                                            attribute.0, attribute.1
                                        );
                                    }
                                }
                                Some(value)
                            } else {
                                None
                            }
                        }
                        "content-transfer-encoding" | "mime-version" => {
                            if let mail_parser::HeaderValue::Text(text) = header.value() {
                                Some(text.to_string())
                            } else {
                                None
                            }
                        }
                        _ => None,
                    };

                    if let Some(value) = value {
                        match HeaderName::new_from_ascii(header_name.into()) {
                            Ok(name) => {
                                let header = HeaderValue::new(name, value);
                                message.headers_mut().insert_raw(header);
                            }
                            Err(e) => log::error!("could not set header: {e}"),
                        }
                    }
                }

                message
            }
        };

        // `Auto-Submitted` is defined in RFC 5436 and describes how
        // an automatic response (f.e. ooo replies, etc.) should behave on the
        // emails. When using `Auto-Submitted: auto-generated` (or any value
        // other than `none`) automatic replies won't be triggered.
        email.headers_mut().insert_raw(HeaderValue::new(
            HeaderName::new_from_ascii_str("Auto-Submitted"),
            "auto-generated;".into(),
        ));

        transport
            .send(&email)
            .map_err(|err| Error::NotifyFailed(self.name().into(), err.into()))?;

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
