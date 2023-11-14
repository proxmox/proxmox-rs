use serde::Deserialize;

use proxmox_schema::{ObjectSchema, Schema, StringSchema};
use proxmox_section_config::{SectionConfig, SectionConfigPlugin};

use crate::context::{common, Context};

const PBS_USER_CFG_FILENAME: &str = "/etc/proxmox-backup/user.cfg";
const PBS_NODE_CFG_FILENAME: &str = "/etc/proxmox-backup/node.cfg";

// FIXME: Switch to the actual schema when possible in terms of dependency.
// It's safe to assume that the config was written with the actual schema restrictions, so parsing
// it with the less restrictive schema should be enough for the purpose of getting the mail address.
const DUMMY_ID_SCHEMA: Schema = StringSchema::new("dummy ID").min_length(3).schema();
const DUMMY_EMAIL_SCHEMA: Schema = StringSchema::new("dummy email").schema();
const DUMMY_USER_SCHEMA: ObjectSchema = ObjectSchema {
    description: "minimal PBS user",
    properties: &[
        ("userid", false, &DUMMY_ID_SCHEMA),
        ("email", true, &DUMMY_EMAIL_SCHEMA),
    ],
    additional_properties: true,
    default_key: None,
};

#[derive(Deserialize)]
struct DummyPbsUser {
    pub email: Option<String>,
}

/// Extract the root user's email address from the PBS user config.
fn lookup_mail_address(content: &str, username: &str) -> Option<String> {
    let mut config = SectionConfig::new(&DUMMY_ID_SCHEMA).allow_unknown_sections(true);
    let user_plugin = SectionConfigPlugin::new(
        "user".to_string(),
        Some("userid".to_string()),
        &DUMMY_USER_SCHEMA,
    );
    config.register_plugin(user_plugin);

    match config.parse(PBS_USER_CFG_FILENAME, content) {
        Ok(parsed) => {
            parsed.sections.get(username)?;
            match parsed.lookup::<DummyPbsUser>("user", username) {
                Ok(user) => common::normalize_for_return(user.email.as_deref()),
                Err(err) => {
                    log::error!("unable to parse {PBS_USER_CFG_FILENAME}: {err}");
                    None
                }
            }
        }
        Err(err) => {
            log::error!("unable to parse {PBS_USER_CFG_FILENAME}: {err}");
            None
        }
    }
}

#[derive(Debug)]
pub struct PBSContext;

pub static PBS_CONTEXT: PBSContext = PBSContext;

impl Context for PBSContext {
    fn lookup_email_for_user(&self, user: &str) -> Option<String> {
        let content = common::attempt_file_read(PBS_USER_CFG_FILENAME);
        content.and_then(|content| lookup_mail_address(&content, user))
    }

    fn default_sendmail_author(&self) -> String {
        "Proxmox Backup Server".into()
    }

    fn default_sendmail_from(&self) -> String {
        let content = common::attempt_file_read(PBS_NODE_CFG_FILENAME);
        content
            .and_then(|content| common::lookup_datacenter_config_key(&content, "email-from"))
            .unwrap_or_else(|| String::from("root"))
    }

    fn http_proxy_config(&self) -> Option<String> {
        let content = common::attempt_file_read(PBS_NODE_CFG_FILENAME);
        content.and_then(|content| common::lookup_datacenter_config_key(&content, "http-proxy"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const USER_CONFIG: &str = "
user: root@pam
	email root@example.com

user: test@pbs
	enable true
	expire 0
    ";

    #[test]
    fn test_parse_mail() {
        assert_eq!(
            lookup_mail_address(USER_CONFIG, "root@pam"),
            Some("root@example.com".to_string())
        );
        assert_eq!(lookup_mail_address(USER_CONFIG, "test@pbs"), None);
    }

    const NODE_CONFIG: &str = "
default-lang: de
email-from: root@example.com
http-proxy: http://localhost:1234
    ";

    #[test]
    fn test_parse_node_config() {
        assert_eq!(
            common::lookup_datacenter_config_key(NODE_CONFIG, "email-from"),
            Some("root@example.com".to_string())
        );
        assert_eq!(
            common::lookup_datacenter_config_key(NODE_CONFIG, "http-proxy"),
            Some("http://localhost:1234".to_string())
        );
        assert_eq!(
            common::lookup_datacenter_config_key(NODE_CONFIG, "foo"),
            None
        );
    }
}
