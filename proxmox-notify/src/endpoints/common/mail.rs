use std::collections::HashSet;

use crate::context;

pub(crate) fn get_recipients(email_addrs: &[String], users: &[String]) -> HashSet<String> {
    let mut recipients = HashSet::new();

    for addr in email_addrs {
        recipients.insert(addr.clone());
    }

    for user in users {
        if let Some(addr) = context::context().lookup_email_for_user(user) {
            recipients.insert(addr);
        }
    }
    recipients
}
