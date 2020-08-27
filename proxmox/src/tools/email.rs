//! Email related utilities.

use std::process::{Command, Stdio};
use anyhow::{bail, Error};
use std::io::Write;
use chrono::{DateTime, Local};
use crate::tools::time::time;


/// Sends multi-part mail with text and/or html to a list of recipients
///
/// ``sendmail`` is used for sending the mail.
pub fn sendmail(mailto: Vec<&str>,
                subject: &str,
                text: Option<&str>,
                html: Option<&str>,
                mailfrom: Option<&str>,
                author: Option<&str>) -> Result<(), Error> {
    let mail_regex = regex::Regex::new(r"^[a-zA-Z\.0-9-]+@[a-zA-Z\.0-9-]+$").unwrap();

    if mailto.is_empty() {
        bail!("At least one recipient has to be specified!")
    }

    for recipient in &mailto {
        if !mail_regex.is_match(recipient) {
            bail!("'{}' is not a valid email address", recipient)
        }
    }

    let mailfrom = mailfrom.unwrap_or("root");
    if !mailfrom.eq("root") && !mail_regex.is_match(mailfrom) {
        bail!("'{}' is not a valid email address", mailfrom)
    }

    let recipients = mailto.join(",");
    let author = author.unwrap_or("Proxmox Backup Server");

    let now: DateTime<Local> = Local::now();

    let mut sendmail_process = match Command::new("/usr/sbin/sendmail")
        .arg("-B")
        .arg("8BITMIME")
        .arg("-f")
        .arg(mailfrom)
        .arg("--")
        .arg(&recipients)
        .stdin(Stdio::piped())
        .spawn() {
        Err(err) => bail!("could not spawn sendmail process: {}", err),
        Ok(process) => process
    };
    let mut is_multipart = false;
    if let (Some(_), Some(_)) = (text, html) {
        is_multipart = true;
    }

    let mut body = String::new();
    let boundary = format!("----_=_NextPart_001_{}", time()?);
    if is_multipart {
        body.push_str("Content-Type: multipart/alternative;\n");
        body.push_str(&format!("\tboundary=\"{}\"\n", boundary));
        body.push_str("MIME-Version: 1.0\n");
    } else if !subject.is_ascii() {
        body.push_str("MIME-Version: 1.0\n");
    }
    if !subject.is_ascii() {
        body.push_str(&format!("Subject: =?utf-8?B?{}?=\n", base64::encode(subject)));
    } else {
        body.push_str(&format!("Subject: {}\n", subject));
    }
    body.push_str(&format!("From: {} <{}>\n", author, mailfrom));
    body.push_str(&format!("To: {}\n", &recipients));
    body.push_str(&format!("Date: {}\n", now.to_rfc2822()));
    if is_multipart {
        body.push('\n');
        body.push_str("This is a multi-part message in MIME format.\n");
        body.push_str(&format!("\n--{}\n", boundary));
    }
    if let Some(text) = text {
        body.push_str("Content-Type: text/plain;\n");
        body.push_str("\tcharset=\"UTF-8\"\n");
        body.push_str("Content-Transfer-Encoding: 8bit\n");
        body.push('\n');
        body.push_str(text);
        if is_multipart {
            body.push_str(&format!("\n--{}\n", boundary));
        }
    }
    if let Some(html) = html {
        body.push_str("Content-Type: text/html;\n");
        body.push_str("\tcharset=\"UTF-8\"\n");
        body.push_str("Content-Transfer-Encoding: 8bit\n");
        body.push('\n');
        body.push_str(html);
        if is_multipart {
            body.push_str(&format!("\n--{}--", boundary));
        }
    }

    if let Err(err) = sendmail_process.stdin.take().unwrap().write_all(body.as_bytes()) {
        bail!("couldn't write to sendmail stdin: {}", err)
    };

    // wait() closes stdin of the child
    if let Err(err) = sendmail_process.wait() {
        bail!("sendmail did not exit successfully: {}", err)
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use crate::tools::email::sendmail;

    #[test]
    fn test1() {
        let result = sendmail(
            vec!["somenotvalidemail!", "somealmostvalid email"],
            "Subject1",
            Some("TEXT"),
            Some("<b>HTML</b>"),
            Some("bim@bam.bum"),
            Some("test1"));
        assert!(result.is_err());
    }

    #[test]
    fn test2() {
        let result = sendmail(
            vec![],
            "Subject2",
            None,
            Some("<b>HTML</b>"),
            None,
            Some("test1"));
        assert!(result.is_err());
    }

    #[test]
    fn test3() {
        let result = sendmail(
            vec!["a@b.c"],
            "Subject3",
            None,
            Some("<b>HTML</b>"),
            Some("notv@lid.com!"),
            Some("test1"));
        assert!(result.is_err());
    }
}