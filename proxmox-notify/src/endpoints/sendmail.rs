use std::io::Write;
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

use proxmox_schema::api_types::COMMENT_SCHEMA;
use proxmox_schema::{api, Updater};

use crate::context;
use crate::endpoints::common::mail;
use crate::renderer::TemplateType;
use crate::schema::{EMAIL_SCHEMA, ENTITY_NAME_SCHEMA, USER_SCHEMA};
use crate::{renderer, Content, Endpoint, Error, Notification, Origin};

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
    },
)]
#[derive(Debug, Serialize, Deserialize, Updater, Default)]
#[serde(rename_all = "kebab-case")]
/// Config for Sendmail notification endpoints
pub struct SendmailConfig {
    /// Name of the endpoint
    #[updater(skip)]
    pub name: String,
    /// Mail address to send a mail to.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[updater(serde(skip_serializing_if = "Option::is_none"))]
    pub mailto: Vec<String>,
    /// Users to send a mail to. The email address of the user
    /// will be looked up in users.cfg.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[updater(serde(skip_serializing_if = "Option::is_none"))]
    pub mailto_user: Vec<String>,
    /// `From` address for sent E-Mails.
    /// If the parameter is not set, the plugin will fall back to the
    /// email-from setting from node.cfg (PBS).
    /// If that is also not set, the plugin will default to root@$hostname,
    /// where $hostname is the hostname of the node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from_address: Option<String>,
    /// Author of the mail. Defaults to 'Proxmox Backup Server ($hostname)'
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
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

#[api]
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// The set of properties that can be deleted from a sendmail endpoint configuration.
pub enum DeleteableSendmailProperty {
    /// Delete `author`
    Author,
    /// Delete `comment`
    Comment,
    /// Delete `disable`
    Disable,
    /// Delete `from-address`
    FromAddress,
    /// Delete `mailto`
    Mailto,
    /// Delete `mailto-user`
    MailtoUser,
}

/// A sendmail notification endpoint.
pub struct SendmailEndpoint {
    pub config: SendmailConfig,
}

impl Endpoint for SendmailEndpoint {
    fn send(&self, notification: &Notification) -> Result<(), Error> {
        let recipients = mail::get_recipients(
            self.config.mailto.as_slice(),
            self.config.mailto_user.as_slice(),
        );

        let recipients_str: Vec<&str> = recipients.iter().map(String::as_str).collect();
        let mailfrom = self
            .config
            .from_address
            .clone()
            .unwrap_or_else(|| context().default_sendmail_from());

        match &notification.content {
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

                let author = self
                    .config
                    .author
                    .clone()
                    .unwrap_or_else(|| context().default_sendmail_author());

                sendmail(
                    &recipients_str,
                    &subject,
                    Some(&text_part),
                    Some(&html_part),
                    Some(&mailfrom),
                    Some(&author),
                )
                .map_err(|err| Error::NotifyFailed(self.config.name.clone(), err.into()))
            }
            #[cfg(feature = "mail-forwarder")]
            Content::ForwardedMail { raw, uid, .. } => {
                forward(&recipients_str, &mailfrom, raw, *uid)
                    .map_err(|err| Error::NotifyFailed(self.config.name.clone(), err.into()))
            }
        }
    }

    fn name(&self) -> &str {
        &self.config.name
    }

    /// Check if the endpoint is disabled
    fn disabled(&self) -> bool {
        self.config.disable.unwrap_or_default()
    }
}

/// Sends multi-part mail with text and/or html to a list of recipients
///
/// Includes the header `Auto-Submitted: auto-generated`, so that auto-replies
/// (i.e. OOO replies) won't trigger.
/// ``sendmail`` is used for sending the mail.
fn sendmail(
    mailto: &[&str],
    subject: &str,
    text: Option<&str>,
    html: Option<&str>,
    mailfrom: Option<&str>,
    author: Option<&str>,
) -> Result<(), Error> {
    use std::fmt::Write as _;

    if mailto.is_empty() {
        return Err(Error::Generic(
            "At least one recipient has to be specified!".into(),
        ));
    }
    let mailfrom = mailfrom.unwrap_or("root");
    let recipients = mailto.join(",");
    let author = author.unwrap_or("Proxmox Backup Server");

    let now = proxmox_time::epoch_i64();

    let mut sendmail_process = match Command::new("/usr/sbin/sendmail")
        .arg("-B")
        .arg("8BITMIME")
        .arg("-f")
        .arg(mailfrom)
        .arg("--")
        .args(mailto)
        .stdin(Stdio::piped())
        .spawn()
    {
        Err(err) => {
            return Err(Error::Generic(format!(
                "could not spawn sendmail process: {err}"
            )))
        }
        Ok(process) => process,
    };
    let mut is_multipart = false;
    if let (Some(_), Some(_)) = (text, html) {
        is_multipart = true;
    }

    let mut body = String::new();
    let boundary = format!("----_=_NextPart_001_{}", now);
    if is_multipart {
        body.push_str("Content-Type: multipart/alternative;\n");
        let _ = writeln!(body, "\tboundary=\"{}\"", boundary);
        body.push_str("MIME-Version: 1.0\n");
    } else if !subject.is_ascii() {
        body.push_str("MIME-Version: 1.0\n");
    }
    if !subject.is_ascii() {
        let _ = writeln!(body, "Subject: =?utf-8?B?{}?=", base64::encode(subject));
    } else {
        let _ = writeln!(body, "Subject: {}", subject);
    }
    let _ = writeln!(body, "From: {} <{}>", author, mailfrom);
    let _ = writeln!(body, "To: {}", &recipients);
    let rfc2822_date = proxmox_time::epoch_to_rfc2822(now)
        .map_err(|err| Error::Generic(format!("failed to format time: {err}")))?;
    let _ = writeln!(body, "Date: {}", rfc2822_date);
    body.push_str("Auto-Submitted: auto-generated;\n");

    if is_multipart {
        body.push('\n');
        body.push_str("This is a multi-part message in MIME format.\n");
        let _ = write!(body, "\n--{}\n", boundary);
    }
    if let Some(text) = text {
        body.push_str("Content-Type: text/plain;\n");
        body.push_str("\tcharset=\"UTF-8\"\n");
        body.push_str("Content-Transfer-Encoding: 8bit\n");
        body.push('\n');
        body.push_str(text);
        if is_multipart {
            let _ = write!(body, "\n--{}\n", boundary);
        }
    }
    if let Some(html) = html {
        body.push_str("Content-Type: text/html;\n");
        body.push_str("\tcharset=\"UTF-8\"\n");
        body.push_str("Content-Transfer-Encoding: 8bit\n");
        body.push('\n');
        body.push_str(html);
        if is_multipart {
            let _ = write!(body, "\n--{}--", boundary);
        }
    }

    if let Err(err) = sendmail_process
        .stdin
        .take()
        .unwrap()
        .write_all(body.as_bytes())
    {
        return Err(Error::Generic(format!(
            "couldn't write to sendmail stdin: {err}"
        )));
    };

    // wait() closes stdin of the child
    if let Err(err) = sendmail_process.wait() {
        return Err(Error::Generic(format!(
            "sendmail did not exit successfully: {err}"
        )));
    }

    Ok(())
}

/// Forwards an email message to a given list of recipients.
///
/// ``sendmail`` is used for sending the mail, thus `message` must be
/// compatible with that (the message is piped into stdin unmodified).
#[cfg(feature = "mail-forwarder")]
fn forward(mailto: &[&str], mailfrom: &str, message: &[u8], uid: Option<u32>) -> Result<(), Error> {
    use std::os::unix::process::CommandExt;

    if mailto.is_empty() {
        return Err(Error::Generic(
            "At least one recipient has to be specified!".into(),
        ));
    }

    let mut builder = Command::new("/usr/sbin/sendmail");

    builder
        .args([
            "-N", "never", // never send DSN (avoid mail loops)
            "-f", mailfrom, "--",
        ])
        .args(mailto)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    if let Some(uid) = uid {
        builder.uid(uid);
    }

    let mut process = builder
        .spawn()
        .map_err(|err| Error::Generic(format!("could not spawn sendmail process: {err}")))?;

    process
        .stdin
        .take()
        .unwrap()
        .write_all(message)
        .map_err(|err| Error::Generic(format!("couldn't write to sendmail stdin: {err}")))?;

    process
        .wait()
        .map_err(|err| Error::Generic(format!("sendmail did not exit successfully: {err}")))?;

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn email_without_recipients() {
        let result = sendmail(
            &[],
            "Subject2",
            None,
            Some("<b>HTML</b>"),
            None,
            Some("test1"),
        );
        assert!(result.is_err());
    }
}
