//!
//! This library implements the [`Mail`] trait which makes it easy to send emails with attachments
//! and alternative html parts to one or multiple receivers via ``sendmail``.
//!

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Error};
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};

// Characters in this set will be encoded, so reproduce the inverse of the set described by RFC5987
// Section 3.2.1 `attr-char`, as that describes all characters that **don't** need encoding:
//
// https://datatracker.ietf.org/doc/html/rfc5987#section-3.2.1
//
// `CONTROLS` contains all control characters 0x00 - 0x1f and 0x7f as well as all non-ascii
// characters, so we need to add all characters here that aren't described in `attr-char` that are
// in the range 0x20-0x7e
const RFC5987SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'%')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'{')
    .add(b'}');

// base64 encode and hard-wrap the base64 encoded string every 72 characters. this improves
// compatibility.
fn encode_base64_formatted<T: AsRef<[u8]>>(raw: T) -> String {
    proxmox_base64::encode(raw)
        .chars()
        .enumerate()
        .flat_map(|(i, c)| {
            if i != 0 && i % 72 == 0 {
                Some('\n')
            } else {
                None
            }
            .into_iter()
            .chain(std::iter::once(c))
        })
        .collect::<String>()
}

struct Recipient {
    name: Option<String>,
    email: String,
}

impl Recipient {
    // Returns true if the name of the recipient is undefined or contains only ascii characters
    fn is_ascii(&self) -> bool {
        self.name.as_ref().map(|n| n.is_ascii()).unwrap_or(true)
    }

    fn format_recipient(&self) -> String {
        if let Some(name) = &self.name {
            if !name.is_ascii() {
                format!(
                    "=?utf-8?B?{}?= <{}>",
                    proxmox_base64::encode(name),
                    self.email
                )
            } else {
                format!("{name} <{}>", self.email)
            }
        } else {
            self.email.to_string()
        }
    }
}

struct Attachment<'a> {
    filename: String,
    mime: String,
    content: &'a [u8],
}

impl Attachment<'_> {
    fn format_attachment(&self, file_boundary: &str) -> String {
        use std::fmt::Write;

        let mut attachment = String::new();

        let encoded_filename = if self.filename.is_ascii() {
            &self.filename
        } else {
            &format!("=?utf-8?B?{}?=", proxmox_base64::encode(&self.filename))
        };

        let _ = writeln!(attachment, "\n--{file_boundary}");
        let _ = writeln!(
            attachment,
            "Content-Type: {}; name=\"{encoded_filename}\"",
            self.mime,
        );

        // both `filename` and `filename*` are included for additional compatability
        let _ = writeln!(
            attachment,
            "Content-Disposition: attachment; filename=\"{encoded_filename}\";\n\tfilename*=UTF-8''{}",
            utf8_percent_encode(&self.filename, RFC5987SET)
        );

        attachment.push_str("Content-Transfer-Encoding: base64\n\n");
        attachment.push_str(&encode_base64_formatted(self.content));

        attachment
    }
}

/// This struct is used to define mails that are to be sent via the `sendmail` command.
pub struct Mail<'a> {
    mail_author: String,
    mail_from: String,
    subject: String,
    to: Vec<Recipient>,
    body_txt: String,
    body_html: Option<String>,
    attachments: Vec<Attachment<'a>>,
    mask_participants: bool,
    noreply: Option<Recipient>,
}

impl<'a> Mail<'a> {
    /// Creates a new mail with a mail author, from address, subject line and a plain text body.
    ///
    /// Note: If the author's name or the subject line contains UTF-8 characters they will be
    /// appropriately encoded.
    pub fn new(mail_author: &str, mail_from: &str, subject: &str, body_txt: &str) -> Self {
        Self {
            mail_author: mail_author.to_string(),
            mail_from: mail_from.to_string(),
            subject: subject.to_string(),
            to: Vec::new(),
            body_txt: body_txt.to_string(),
            body_html: None,
            attachments: Vec::new(),
            mask_participants: true,
            noreply: None,
        }
    }

    /// Adds a recipient to the mail without specifying a name separately.
    ///
    /// Note: No formatting or encoding will be done here, the value will be passed to the `To:`
    /// header directly.
    pub fn add_recipient(&mut self, email: &str) {
        self.to.push(Recipient {
            name: None,
            email: email.to_string(),
        });
    }

    /// Builder-pattern method to conveniently add a recipient to an email without specifying a
    /// name separately.
    ///
    /// Note: No formatting or encoding will be done here, the value will be passed to the `To:`
    /// header directly.
    pub fn with_recipient(mut self, email: &str) -> Self {
        self.add_recipient(email);
        self
    }

    /// Adds a recipient to the mail with a name.
    ///
    /// Notes:
    ///
    /// - If the name contains UTF-8 characters it will be encoded. Then the possibly encoded name
    ///   and non-encoded email address will be passed to the `To:` header in this format:
    ///   `{encoded_name} <{email}>`
    /// - If multiple receivers are specified, they will be masked so as not to disclose them to
    ///   other receivers. This can be disabled via [`Mail::unmask_recipients`] or
    ///   [`Mail::with_unmasked_recipients`].
    pub fn add_recipient_and_name(&mut self, name: &str, email: &str) {
        self.to.push(Recipient {
            name: Some(name.to_string()),
            email: email.to_string(),
        });
    }

    /// Builder-style method to conveniently add a recipient with a name to an email.
    ///
    /// Notes:
    ///
    /// - If the name contains UTF-8 characters it will be encoded. Then the possibly encoded name
    ///   and non-encoded email address will be passed to the `To:` header in this format:
    ///   `{encoded_name} <{email}>`
    /// - If multiple receivers are specified, they will be masked so as not to disclose them to
    ///   other receivers. This can be disabled via [`Mail::unmask_recipients`] or
    ///   [`Mail::with_unmasked_recipients`].
    pub fn with_recipient_and_name(mut self, name: &str, email: &str) -> Self {
        self.add_recipient_and_name(name, email);
        self
    }

    /// Adds an attachment with a specified file name and mime-type to an email.
    ///
    /// Note: Adding attachments triggers `multipart/mixed` mode.
    pub fn add_attachment(&mut self, filename: &str, mime_type: &str, content: &'a [u8]) {
        self.attachments.push(Attachment {
            filename: filename.to_string(),
            mime: mime_type.to_string(),
            content,
        });
    }

    /// Builder-style method to conveniently add an attachment with a specific filename and
    /// mime-type to an email.
    ///
    /// Note: Adding attachements triggers `multipart/mixed` mode.
    pub fn with_attachment(mut self, filename: &str, mime_type: &str, content: &'a [u8]) -> Self {
        self.add_attachment(filename, mime_type, content);
        self
    }

    /// Set an alternative HTML part.
    ///
    /// Note: This triggers `multipart/alternative` mode. If both an HTML part and at least one
    /// attachment are specified, the `multipart/alternative` part will be nested within the first
    /// `multipart/mixed` part. This should ensure that the HTML is displayed properly by client's
    /// that prioritize it over the plain text part (should be the default for most clients) while
    /// also properly displaying the attachments.
    pub fn set_html_alt(&mut self, body_html: &str) {
        self.body_html.replace(body_html.to_string());
    }

    /// Builder-style method to add an alternative HTML part.
    ///
    /// Note: This triggers `multipart/alternative` mode. If both an HTML part and at least one
    /// attachment are specified, the `multipart/alternative` part will be nested within the first
    /// `multipart/mixed` part. This should ensure that the HTML is displayed properly by client's
    /// that prioritize it over the plain text part (should be the default for most clients) while
    /// also properly displaying the attachments.
    pub fn with_html_alt(mut self, body_html: &str) -> Self {
        self.set_html_alt(body_html);
        self
    }

    /// This function ensures that recipients of the mail are not masked. Being able to see all
    /// recipients of a mail can be helpful in, for example, notification scenarios.
    pub fn unmask_recipients(&mut self) {
        self.mask_participants = false;
    }

    /// Builder-style function that ensures that recipients of the mail are not masked. Being able
    /// to see all recipients of a mail can be helpful in, for example, notification scenarios.
    pub fn with_unmasked_recipients(mut self) -> Self {
        self.unmask_recipients();
        self
    }

    /// Set the receiver that is used when the mail is send in masked mode. `Undisclosed <noreply>`
    /// by default.
    pub fn set_masked_mail_and_name(&mut self, name: &str, email: &str) {
        self.noreply = Some(Recipient {
            email: email.to_owned(),
            name: Some(name.to_owned()),
        });
    }

    /// Builder-style method to set the receiver when the mail is send in masked mode. `Undisclosed
    /// <noreply>` by default.
    pub fn with_masked_receiver(mut self, name: &str, email: &str) -> Self {
        self.set_masked_mail_and_name(name, email);
        self
    }

    /// Sends the email. This will fail if no recipients have been added.
    ///
    /// Note: An `Auto-Submitted: auto-generated` header is added to avoid triggering OOO and
    /// similar mails.
    pub fn send(&self) -> Result<(), Error> {
        if self.to.is_empty() {
            bail!("no recipients provided for the mail, cannot send it.");
        }

        let now = proxmox_time::epoch_i64();
        let body = self.format_mail(now)?;

        let mut sendmail_process = Command::new("/usr/sbin/sendmail")
            .arg("-B")
            .arg("8BITMIME")
            .arg("-f")
            .arg(&self.mail_from)
            .arg("--")
            .args(self.to.iter().map(|p| &p.email).collect::<Vec<&String>>())
            .stdin(Stdio::piped())
            .spawn()
            .with_context(|| "could not spawn sendmail process")?;

        sendmail_process
            .stdin
            .as_ref()
            .unwrap()
            .write_all(body.as_bytes())
            .with_context(|| "couldn't write to sendmail stdin")?;

        sendmail_process
            .wait()
            .with_context(|| "sendmail did not exit successfully")?;

        Ok(())
    }

    /// Forwards an email message to a given list of recipients.
    ///
    /// `message` must be compatible with ``sendmail`` (the message is piped into stdin unmodified).
    #[cfg(feature = "mail-forwarder")]
    pub fn forward(
        mailto: &[&str],
        mailfrom: &str,
        message: &[u8],
        uid: Option<u32>,
    ) -> Result<(), Error> {
        use std::os::unix::process::CommandExt;

        if mailto.is_empty() {
            bail!("At least one recipient has to be specified!");
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

        let mut sendmail_process = builder
            .spawn()
            .with_context(|| "could not spawn sendmail process")?;

        sendmail_process
            .stdin
            .take()
            .unwrap()
            .write_all(message)
            .with_context(|| "couldn't write to sendmail stdin")?;

        sendmail_process
            .wait()
            .with_context(|| "sendmail did not exit successfully")?;

        Ok(())
    }

    fn format_mail(&self, now: i64) -> Result<String, Error> {
        use std::fmt::Write;

        let file_boundary = format!("----_=_NextPart_001_{now}");
        let html_boundary = format!("----_=_NextPart_002_{now}");

        let mut mail = self.format_header(now, &file_boundary, &html_boundary)?;
        mail.push_str(&self.format_body(&file_boundary, &html_boundary)?);

        if !self.attachments.is_empty() {
            mail.push_str(
                &self
                    .attachments
                    .iter()
                    .map(|a| a.format_attachment(&file_boundary))
                    .collect::<String>(),
            );

            write!(mail, "\n--{file_boundary}--")?;
        }

        Ok(mail)
    }

    fn format_header(
        &self,
        now: i64,
        file_boundary: &str,
        html_boundary: &str,
    ) -> Result<String, Error> {
        use std::fmt::Write;

        let mut header = String::new();

        let encoded_to = if self.to.len() > 1 && self.mask_participants {
            // if the receivers are masked, we know that they don't need to be encoded
            false
        } else {
            // check if there is a recipient that needs encoding
            self.to.iter().any(|r| !r.is_ascii())
        };

        if !self.attachments.is_empty() {
            header.push_str("Content-Type: multipart/mixed;\n");
            writeln!(header, "\tboundary=\"{file_boundary}\"")?;
            header.push_str("MIME-Version: 1.0\n");
        } else if self.body_html.is_some() {
            header.push_str("Content-Type: multipart/alternative;\n");
            writeln!(header, "\tboundary=\"{html_boundary}\"")?;
            header.push_str("MIME-Version: 1.0\n");
        } else if !self.subject.is_ascii()
            || !self.mail_author.is_ascii()
            || !self.body_txt.is_ascii()
            || encoded_to
        {
            header.push_str("MIME-Version: 1.0\n");
        }

        if !self.subject.is_ascii() {
            writeln!(
                header,
                "Subject: =?utf-8?B?{}?=",
                proxmox_base64::encode(&self.subject)
            )?;
        } else {
            writeln!(header, "Subject: {}", self.subject)?;
        };

        if !self.mail_author.is_ascii() {
            writeln!(
                header,
                "From: =?utf-8?B?{}?= <{}>",
                proxmox_base64::encode(&self.mail_author),
                self.mail_from
            )?;
        } else {
            writeln!(header, "From: {} <{}>", self.mail_author, self.mail_from)?;
        }

        let to = if self.to.len() > 1 && self.mask_participants {
            // don't disclose all recipients if the mail goes out to multiple
            self.noreply
                .as_ref()
                .map(|f| f.format_recipient())
                .unwrap_or_else(|| {
                    Recipient {
                        name: Some("Undisclosed".to_string()),
                        email: "noreply".to_string(),
                    }
                    .format_recipient()
                })
        } else {
            self.to
                .iter()
                .map(Recipient::format_recipient)
                .collect::<Vec<String>>()
                .join(", ")
        };

        writeln!(header, "To: {to}")?;

        let rfc2822_date = proxmox_time::epoch_to_rfc2822(now)
            .with_context(|| "could not convert epoch to rfc2822 date")?;
        writeln!(header, "Date: {rfc2822_date}")?;
        header.push_str("Auto-Submitted: auto-generated;\n");

        Ok(header)
    }

    fn format_body(&self, file_boundary: &str, html_boundary: &str) -> Result<String, Error> {
        use std::fmt::Write;

        let mut body = String::new();

        if self.body_html.is_some() && !self.attachments.is_empty() {
            body.push_str("\nThis is a multi-part message in MIME format.\n");
            writeln!(body, "\n--{file_boundary}")?;
            writeln!(
                body,
                "Content-Type: multipart/alternative; boundary=\"{html_boundary}\""
            )?;
            body.push_str("MIME-Version: 1.0\n");
            writeln!(body, "\n--{html_boundary}")?;
        } else if self.body_html.is_some() {
            body.push_str("\nThis is a multi-part message in MIME format.\n");
            writeln!(body, "\n--{html_boundary}")?;
        } else if self.body_html.is_none() && !self.attachments.is_empty() {
            body.push_str("\nThis is a multi-part message in MIME format.\n");
            writeln!(body, "\n--{file_boundary}")?;
        }

        body.push_str("Content-Type: text/plain;\n");
        body.push_str("\tcharset=\"UTF-8\"\n");

        if self.body_txt.is_ascii() {
            body.push_str("Content-Transfer-Encoding: 7bit\n\n");
            body.push_str(&self.body_txt);
        } else {
            body.push_str("Content-Transfer-Encoding: base64\n\n");
            body.push_str(&encode_base64_formatted(&self.body_txt));
        }

        if let Some(html) = &self.body_html {
            writeln!(body, "\n--{html_boundary}")?;
            body.push_str("Content-Type: text/html;\n");
            body.push_str("\tcharset=\"UTF-8\"\n");

            if html.is_ascii() {
                body.push_str("Content-Transfer-Encoding: 7bit\n\n");
                body.push_str(html);
            } else {
                body.push_str("Content-Transfer-Encoding: base64\n\n");
                body.push_str(&encode_base64_formatted(html));
            }

            write!(body, "\n--{html_boundary}--")?;
        }

        Ok(body)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Compare two multi-line strings, ignoring any line that starts with 'Date:'.
    ///
    /// The `Date` header is formatted in the local timezone, which means our
    /// tests are sensitive to the timezone of the machine running the tests.
    /// Simplest solution is to just ignore the date header.
    fn assert_lines_equal_ignore_date(s1: &str, s2: &str) {
        let lines1 = s1.lines();
        let lines2 = s2.lines();

        for (line1, line2) in lines1.zip(lines2) {
            if !(line1.starts_with("Date:") && line2.starts_with("Date:")) {
                assert_eq!(line1, line2);
            }
        }

        assert_eq!(s1.lines().count(), s2.lines().count());
    }

    #[test]
    fn email_without_recipients_fails() {
        let result = Mail::new("Sender", "mail@example.com", "hi", "body").send();
        assert!(result.is_err());
    }

    #[test]
    #[cfg(feature = "mail-forwarder")]
    fn forwarding_without_recipients_fails() {
        let result = Mail::forward(&[], "me@example.com", String::from("text").as_bytes(), None);
        assert!(result.is_err());
    }

    #[test]
    fn simple_ascii_text_mail() {
        let mail = Mail::new(
            "Sender Name",
            "mailfrom@example.com",
            "Subject Line",
            "This is just ascii text.\nNothing too special.",
        )
        .with_recipient_and_name("Receiver Name", "receiver@example.com");

        let body = mail.format_mail(0).expect("could not format mail");

        assert_lines_equal_ignore_date(
            &body,
            r#"Subject: Subject Line
From: Sender Name <mailfrom@example.com>
To: Receiver Name <receiver@example.com>
Date: Thu, 01 Jan 1970 01:00:00 +0100
Auto-Submitted: auto-generated;
Content-Type: text/plain;
	charset="UTF-8"
Content-Transfer-Encoding: 7bit

This is just ascii text.
Nothing too special."#,
        )
    }

    #[test]
    fn multiple_receiver_masked() {
        let mail = Mail::new(
            "Sender Name",
            "mailfrom@example.com",
            "Subject Line",
            "This is just ascii text.\nNothing too special.",
        )
        .with_recipient_and_name("Receiver Name", "receiver@example.com")
        .with_recipient("two@example.com")
        .with_recipient_and_name("mäx müstermänn", "mm@example.com");

        let body = mail.format_mail(0).expect("could not format mail");

        assert_lines_equal_ignore_date(
            &body,
            r#"Subject: Subject Line
From: Sender Name <mailfrom@example.com>
To: Undisclosed <noreply>
Date: Thu, 01 Jan 1970 01:00:00 +0100
Auto-Submitted: auto-generated;
Content-Type: text/plain;
	charset="UTF-8"
Content-Transfer-Encoding: 7bit

This is just ascii text.
Nothing too special."#,
        )
    }

    #[test]
    fn multiple_receiver_unmasked() {
        let mail = Mail::new(
            "Sender Name",
            "mailfrom@example.com",
            "Subject Line",
            "This is just ascii text.\nNothing too special.",
        )
        .with_recipient_and_name("Receiver Name", "receiver@example.com")
        .with_recipient("two@example.com")
        .with_recipient_and_name("mäx müstermänn", "mm@example.com")
        .with_unmasked_recipients();

        let body = mail.format_mail(0).expect("could not format mail");

        assert_lines_equal_ignore_date(
            &body,
            r#"MIME-Version: 1.0
Subject: Subject Line
From: Sender Name <mailfrom@example.com>
To: Receiver Name <receiver@example.com>, two@example.com, =?utf-8?B?bcOkeCBtw7xzdGVybcOkbm4=?= <mm@example.com>
Date: Thu, 01 Jan 1970 01:00:00 +0100
Auto-Submitted: auto-generated;
Content-Type: text/plain;
	charset="UTF-8"
Content-Transfer-Encoding: 7bit

This is just ascii text.
Nothing too special."#,
        )
    }

    #[test]
    fn multiple_receiver_custom_masked() {
        let mail = Mail::new(
            "Sender Name",
            "mailfrom@example.com",
            "Subject Line",
            "This is just ascii text.\nNothing too special.",
        )
        .with_recipient_and_name("Receiver Name", "receiver@example.com")
        .with_recipient("two@example.com")
        .with_recipient_and_name("mäx müstermänn", "mm@example.com")
        .with_masked_receiver("Example Receiver", "noanswer@example.com");

        let body = mail.format_mail(0).expect("could not format mail");

        assert_lines_equal_ignore_date(
            &body,
            r#"Subject: Subject Line
From: Sender Name <mailfrom@example.com>
To: Example Receiver <noanswer@example.com>
Date: Thu, 01 Jan 1970 01:00:00 +0100
Auto-Submitted: auto-generated;
Content-Type: text/plain;
	charset="UTF-8"
Content-Transfer-Encoding: 7bit

This is just ascii text.
Nothing too special."#,
        )
    }

    #[test]
    fn simple_utf8_text_mail() {
        let mail = Mail::new(
            "UTF-8 Sender Name 📧",
            "differentfrom@example.com",
            "Subject Line 🧑",
            "This utf-8 email should handle emojis\n🧑📧\nand weird german characters: öäüß\nand more.",
        )
        .with_recipient_and_name("Receiver Name📩", "receiver@example.com");

        let body = mail.format_mail(1732806251).expect("could not format mail");

        assert_lines_equal_ignore_date(
            &body,
            r#"MIME-Version: 1.0
Subject: =?utf-8?B?U3ViamVjdCBMaW5lIPCfp5E=?=
From: =?utf-8?B?VVRGLTggU2VuZGVyIE5hbWUg8J+Tpw==?= <differentfrom@example.com>
To: =?utf-8?B?UmVjZWl2ZXIgTmFtZfCfk6k=?= <receiver@example.com>
Date: Thu, 28 Nov 2024 16:04:11 +0100
Auto-Submitted: auto-generated;
Content-Type: text/plain;
	charset="UTF-8"
Content-Transfer-Encoding: base64

VGhpcyB1dGYtOCBlbWFpbCBzaG91bGQgaGFuZGxlIGVtb2ppcwrwn6eR8J+TpwphbmQgd2Vp
cmQgZ2VybWFuIGNoYXJhY3RlcnM6IMO2w6TDvMOfCmFuZCBtb3JlLg=="#,
        )
    }

    #[test]
    fn multipart_html_alternative() {
        let mail = Mail::new(
            "Sender Name",
            "from@example.com",
            "Subject Line",
            "Lorem Ipsum Dolor Sit\nAmet",
        )
        .with_recipient("receiver@example.com")
        .with_html_alt("<html lang=\"de-at\"><head></head><body>\n\t<pre>\n\t\tLorem Ipsum Dolor Sit Amet\n\t</pre>\n</body></html>");
        let body = mail.format_mail(1732806251).expect("could not format mail");
        assert_lines_equal_ignore_date(
            &body,
            r#"Content-Type: multipart/alternative;
	boundary="----_=_NextPart_002_1732806251"
MIME-Version: 1.0
Subject: Subject Line
From: Sender Name <from@example.com>
To: receiver@example.com
Date: Thu, 28 Nov 2024 16:04:11 +0100
Auto-Submitted: auto-generated;

This is a multi-part message in MIME format.

------_=_NextPart_002_1732806251
Content-Type: text/plain;
	charset="UTF-8"
Content-Transfer-Encoding: 7bit

Lorem Ipsum Dolor Sit
Amet
------_=_NextPart_002_1732806251
Content-Type: text/html;
	charset="UTF-8"
Content-Transfer-Encoding: 7bit

<html lang="de-at"><head></head><body>
	<pre>
		Lorem Ipsum Dolor Sit Amet
	</pre>
</body></html>
------_=_NextPart_002_1732806251--"#,
        )
    }

    #[test]
    fn multipart_plain_text_attachments_mixed() {
        let bin: [u8; 62] = [
            0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
            0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
            0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
            0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
            0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef,
        ];

        let mail = Mail::new(
            "Sender Name",
            "from@example.com",
            "Subject Line",
            "Lorem Ipsum Dolor Sit\nAmet",
        )
        .with_recipient_and_name("Receiver Name", "receiver@example.com")
        .with_attachment("deadbeef.bin", "application/octet-stream", &bin);

        let body = mail.format_mail(1732806251).expect("could not format mail");
        assert_lines_equal_ignore_date(
            &body,
            r#"Content-Type: multipart/mixed;
	boundary="----_=_NextPart_001_1732806251"
MIME-Version: 1.0
Subject: Subject Line
From: Sender Name <from@example.com>
To: Receiver Name <receiver@example.com>
Date: Thu, 28 Nov 2024 16:04:11 +0100
Auto-Submitted: auto-generated;

This is a multi-part message in MIME format.

------_=_NextPart_001_1732806251
Content-Type: text/plain;
	charset="UTF-8"
Content-Transfer-Encoding: 7bit

Lorem Ipsum Dolor Sit
Amet
------_=_NextPart_001_1732806251
Content-Type: application/octet-stream; name="deadbeef.bin"
Content-Disposition: attachment; filename="deadbeef.bin";
	filename*=UTF-8''deadbeef.bin
Content-Transfer-Encoding: base64

3q2+796tvu/erb7v3q3erb7v3q2+796tvu/erd6tvu/erb7v3q2+796t3q2+796tvu/erb7v
3q2+796tvu8=
------_=_NextPart_001_1732806251--"#,
        )
    }

    #[test]
    fn multipart_plain_text_html_alternative_attachments() {
        let bin: [u8; 62] = [
            0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
            0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
            0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
            0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
            0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef,
        ];

        let mail = Mail::new(
            "Sender Name",
            "from@example.com",
            "Subject Line",
            "Lorem Ipsum Dolor Sit\nAmet",
        )
        .with_recipient_and_name("Receiver Name", "receiver@example.com")
        .with_attachment("deadbeef.bin", "application/octet-stream", &bin)
        .with_attachment("🐄💀.bin", "image/bmp", &bin)
        .with_html_alt("<html lang=\"de-at\"><head></head><body>\n\t<pre>\n\t\tLorem Ipsum Dolor Sit Amet\n\t</pre>\n</body></html>");

        let body = mail.format_mail(1732806251).expect("could not format mail");

        assert_lines_equal_ignore_date(
            &body,
            r#"Content-Type: multipart/mixed;
	boundary="----_=_NextPart_001_1732806251"
MIME-Version: 1.0
Subject: Subject Line
From: Sender Name <from@example.com>
To: Receiver Name <receiver@example.com>
Date: Thu, 28 Nov 2024 16:04:11 +0100
Auto-Submitted: auto-generated;

This is a multi-part message in MIME format.

------_=_NextPart_001_1732806251
Content-Type: multipart/alternative; boundary="----_=_NextPart_002_1732806251"
MIME-Version: 1.0

------_=_NextPart_002_1732806251
Content-Type: text/plain;
	charset="UTF-8"
Content-Transfer-Encoding: 7bit

Lorem Ipsum Dolor Sit
Amet
------_=_NextPart_002_1732806251
Content-Type: text/html;
	charset="UTF-8"
Content-Transfer-Encoding: 7bit

<html lang="de-at"><head></head><body>
	<pre>
		Lorem Ipsum Dolor Sit Amet
	</pre>
</body></html>
------_=_NextPart_002_1732806251--
------_=_NextPart_001_1732806251
Content-Type: application/octet-stream; name="deadbeef.bin"
Content-Disposition: attachment; filename="deadbeef.bin";
	filename*=UTF-8''deadbeef.bin
Content-Transfer-Encoding: base64

3q2+796tvu/erb7v3q3erb7v3q2+796tvu/erd6tvu/erb7v3q2+796t3q2+796tvu/erb7v
3q2+796tvu8=
------_=_NextPart_001_1732806251
Content-Type: image/bmp; name="=?utf-8?B?8J+QhPCfkoAuYmlu?="
Content-Disposition: attachment; filename="=?utf-8?B?8J+QhPCfkoAuYmlu?=";
	filename*=UTF-8''%F0%9F%90%84%F0%9F%92%80.bin
Content-Transfer-Encoding: base64

3q2+796tvu/erb7v3q3erb7v3q2+796tvu/erd6tvu/erb7v3q2+796t3q2+796tvu/erb7v
3q2+796tvu8=
------_=_NextPart_001_1732806251--"#,
        )
    }

    #[test]
    fn test_format_mail_multipart() {
        let mail = Mail::new(
            "Fred Oobar",
            "foobar@example.com",
            "This is the subject",
            "This is the plain body",
        )
        .with_recipient_and_name("Tony Est", "test@example.com")
        .with_html_alt("<body>This is the HTML body</body>");

        let body = mail.format_mail(1718977850).expect("could not format mail");

        assert_lines_equal_ignore_date(
            &body,
            r#"Content-Type: multipart/alternative;
	boundary="----_=_NextPart_002_1718977850"
MIME-Version: 1.0
Subject: This is the subject
From: Fred Oobar <foobar@example.com>
To: Tony Est <test@example.com>
Date: Fri, 21 Jun 2024 15:50:50 +0200
Auto-Submitted: auto-generated;

This is a multi-part message in MIME format.

------_=_NextPart_002_1718977850
Content-Type: text/plain;
	charset="UTF-8"
Content-Transfer-Encoding: 7bit

This is the plain body
------_=_NextPart_002_1718977850
Content-Type: text/html;
	charset="UTF-8"
Content-Transfer-Encoding: 7bit

<body>This is the HTML body</body>
------_=_NextPart_002_1718977850--"#,
        );
    }

    #[test]
    fn multipart_plain_text_html_alternative_attachments_ascii_compat() {
        let bin: [u8; 62] = [
            0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
            0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
            0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
            0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef, 0xde, 0xad,
            0xbe, 0xef, 0xde, 0xad, 0xbe, 0xef,
        ];

        let mail = Mail::new(
            "Sender Näme",
            "from@example.com",
            "Subject Linë",
            "Lorem Ipsum Dolor Sit\nAmët",
        )
        .with_recipient_and_name("Receiver Näme", "receiver@example.com")
        .with_attachment("deadbeef.bin", "application/octet-stream", &bin)
        .with_attachment("🐄💀.bin", "image/bmp", &bin)
        .with_html_alt("<html lang=\"de-at\"><head></head><body>\n\t<pre>\n\t\tLorem Ipsum Dölor Sit Amet\n\t</pre>\n</body></html>");

        let body = mail.format_mail(1732806251).expect("could not format mail");

        assert!(body.is_ascii());

        assert_lines_equal_ignore_date(
            &body,
            r#"Content-Type: multipart/mixed;
	boundary="----_=_NextPart_001_1732806251"
MIME-Version: 1.0
Subject: =?utf-8?B?U3ViamVjdCBMaW7Dqw==?=
From: =?utf-8?B?U2VuZGVyIE7DpG1l?= <from@example.com>
To: =?utf-8?B?UmVjZWl2ZXIgTsOkbWU=?= <receiver@example.com>
Date: Thu, 28 Nov 2024 16:04:11 +0100
Auto-Submitted: auto-generated;

This is a multi-part message in MIME format.

------_=_NextPart_001_1732806251
Content-Type: multipart/alternative; boundary="----_=_NextPart_002_1732806251"
MIME-Version: 1.0

------_=_NextPart_002_1732806251
Content-Type: text/plain;
	charset="UTF-8"
Content-Transfer-Encoding: base64

TG9yZW0gSXBzdW0gRG9sb3IgU2l0CkFtw6t0
------_=_NextPart_002_1732806251
Content-Type: text/html;
	charset="UTF-8"
Content-Transfer-Encoding: base64

PGh0bWwgbGFuZz0iZGUtYXQiPjxoZWFkPjwvaGVhZD48Ym9keT4KCTxwcmU+CgkJTG9yZW0g
SXBzdW0gRMO2bG9yIFNpdCBBbWV0Cgk8L3ByZT4KPC9ib2R5PjwvaHRtbD4=
------_=_NextPart_002_1732806251--
------_=_NextPart_001_1732806251
Content-Type: application/octet-stream; name="deadbeef.bin"
Content-Disposition: attachment; filename="deadbeef.bin";
	filename*=UTF-8''deadbeef.bin
Content-Transfer-Encoding: base64

3q2+796tvu/erb7v3q3erb7v3q2+796tvu/erd6tvu/erb7v3q2+796t3q2+796tvu/erb7v
3q2+796tvu8=
------_=_NextPart_001_1732806251
Content-Type: image/bmp; name="=?utf-8?B?8J+QhPCfkoAuYmlu?="
Content-Disposition: attachment; filename="=?utf-8?B?8J+QhPCfkoAuYmlu?=";
	filename*=UTF-8''%F0%9F%90%84%F0%9F%92%80.bin
Content-Transfer-Encoding: base64

3q2+796tvu/erb7v3q3erb7v3q2+796tvu/erd6tvu/erb7v3q2+796t3q2+796tvu/erb7v
3q2+796tvu8=
------_=_NextPart_001_1732806251--"#,
        )
    }
}
