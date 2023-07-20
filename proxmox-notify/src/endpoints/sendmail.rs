use crate::schema::{EMAIL_SCHEMA, ENTITY_NAME_SCHEMA, USER_SCHEMA};
use crate::{Endpoint, Error, Notification};

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
pub struct SendmailConfig {
    /// Name of the endpoint
    #[updater(skip)]
    pub name: String,
    /// Mail recipients
    pub mailto: Vec<String>,
    /// `From` address for the mail
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_address: Option<String>,
    /// Author of the mail
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Comment
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeleteableSendmailProperty {
    FromAddress,
    Author,
    Comment,
}

/// A sendmail notification endpoint.
pub struct SendmailEndpoint {
    pub config: SendmailConfig,
}

impl Endpoint for SendmailEndpoint {
    fn send(&self, notification: &Notification) -> Result<(), Error> {
        let recipients: Vec<&str> = self.config.mailto.iter().map(String::as_str).collect();

        // Note: OX has serious problems displaying text mails,
        // so we include html as well
        let html = format!(
            "<html><body><pre>\n{}\n<pre>",
            handlebars::html_escape(&notification.body)
        );

        // proxmox_sys::email::sendmail will set the author to
        // "Proxmox Backup Server" if it is not set.
        let author = self.config.author.as_deref().or(Some(""));

        proxmox_sys::email::sendmail(
            &recipients,
            &notification.title,
            Some(&notification.body),
            Some(&html),
            self.config.from_address.as_deref(),
            author,
        )
        .map_err(|err| Error::NotifyFailed(self.config.name.clone(), err.into()))
    }

    fn name(&self) -> &str {
        &self.config.name
    }
}
