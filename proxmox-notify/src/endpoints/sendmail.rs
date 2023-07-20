use crate::context::context;
use crate::renderer::TemplateRenderer;
use crate::schema::{EMAIL_SCHEMA, ENTITY_NAME_SCHEMA, USER_SCHEMA};
use crate::{renderer, Endpoint, Error, Notification};
use std::collections::HashSet;

use proxmox_schema::api_types::COMMENT_SCHEMA;
use proxmox_schema::{api, Updater};
use serde::{Deserialize, Serialize};

pub(crate) const SENDMAIL_TYPENAME: &str = "sendmail";

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
        filter: {
            optional: true,
            schema: ENTITY_NAME_SCHEMA,
        },
    },
)]
#[derive(Debug, Serialize, Deserialize, Updater, Default)]
#[serde(rename_all = "kebab-case")]
/// Config for Sendmail notification endpoints
pub struct SendmailConfig {
    /// Name of the endpoint
    #[updater(skip)]
    pub name: String,
    /// Mail recipients
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mailto: Option<Vec<String>>,
    /// Mail recipients
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mailto_user: Option<Vec<String>>,
    /// `From` address for the mail
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_address: Option<String>,
    /// Author of the mail
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Comment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Filter to apply
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeleteableSendmailProperty {
    FromAddress,
    Author,
    Comment,
    Filter,
    Mailto,
    MailtoUser,
}

/// A sendmail notification endpoint.
pub struct SendmailEndpoint {
    pub config: SendmailConfig,
}

impl Endpoint for SendmailEndpoint {
    fn send(&self, notification: &Notification) -> Result<(), Error> {
        let mut recipients = HashSet::new();

        if let Some(mailto_addrs) = self.config.mailto.as_ref() {
            for addr in mailto_addrs {
                recipients.insert(addr.clone());
            }
        }

        if let Some(users) = self.config.mailto_user.as_ref() {
            for user in users {
                if let Some(addr) = context().lookup_email_for_user(user) {
                    recipients.insert(addr);
                }
            }
        }

        let properties = notification.properties.as_ref();

        let subject = renderer::render_template(
            TemplateRenderer::Plaintext,
            &notification.title,
            properties,
        )?;
        let html_part =
            renderer::render_template(TemplateRenderer::Html, &notification.body, properties)?;
        let text_part =
            renderer::render_template(TemplateRenderer::Plaintext, &notification.body, properties)?;

        let author = self
            .config
            .author
            .clone()
            .unwrap_or_else(|| context().default_sendmail_author());

        let mailfrom = self
            .config
            .from_address
            .clone()
            .unwrap_or_else(|| context().default_sendmail_from());

        let recipients_str: Vec<&str> = recipients.iter().map(String::as_str).collect();

        proxmox_sys::email::sendmail(
            &recipients_str,
            &subject,
            Some(&text_part),
            Some(&html_part),
            Some(&mailfrom),
            Some(&author),
        )
        .map_err(|err| Error::NotifyFailed(self.config.name.clone(), err.into()))
    }

    fn name(&self) -> &str {
        &self.config.name
    }

    fn filter(&self) -> Option<&str> {
        self.config.filter.as_deref()
    }
}
