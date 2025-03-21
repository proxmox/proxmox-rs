use crate::context::Context;
use crate::renderer::TemplateSource;
use crate::Error;

#[derive(Debug)]
pub struct TestContext;

impl Context for TestContext {
    fn lookup_email_for_user(&self, _user: &str) -> Option<String> {
        Some("test@example.com".into())
    }

    fn default_sendmail_author(&self) -> String {
        "Proxmox VE".into()
    }

    fn default_sendmail_from(&self) -> String {
        "root".into()
    }

    fn http_proxy_config(&self) -> Option<String> {
        None
    }

    fn default_config(&self) -> &'static str {
        ""
    }

    fn lookup_template(
        &self,
        _filename: &str,
        _namespace: Option<&str>,
        _source: TemplateSource,
    ) -> Result<Option<String>, Error> {
        Ok(Some(String::new()))
    }
}
