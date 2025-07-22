use std::collections::HashSet;

use crate::context;

pub(crate) fn get_recipients(email_addrs: &[String], users: &[String]) -> HashSet<String> {
    let mut recipients = HashSet::new();

    for addr in email_addrs {
        recipients.insert(addr.clone());
    }

    for user in users {
        match context::context().lookup_email_for_user(user) {
            Some(address) => {
                recipients.insert(address);
            }
            None => tracing::warn!(
                "'{user}' does not have a configured email address in the user configuration - \
                not sending an email to this user"
            ),
        }
    }
    recipients
}
