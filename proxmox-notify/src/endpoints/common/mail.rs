use std::collections::HashSet;

use crate::context;

pub(crate) fn get_recipients(
    email_addrs: Option<&[String]>,
    users: Option<&[String]>,
) -> HashSet<String> {
    let mut recipients = HashSet::new();

    if let Some(mailto_addrs) = email_addrs {
        for addr in mailto_addrs {
            recipients.insert(addr.clone());
        }
    }
    if let Some(users) = users {
        for user in users {
            if let Some(addr) = context::context().lookup_email_for_user(user) {
                recipients.insert(addr);
            }
        }
    }
    recipients
}
