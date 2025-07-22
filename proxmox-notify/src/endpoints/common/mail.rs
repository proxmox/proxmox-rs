use crate::context;

/// Get list of email recipients from a list of email addresses and users.
///
/// Any user passed in the user list will be looked up in the user configuration to
/// obtain this user's email address. The list of returned email addresses does
/// not contain any duplicates.
pub(crate) fn get_recipients(email_addrs: &[String], users: &[String]) -> Vec<String> {
    let mut recipients = Vec::new();

    for addr in email_addrs {
        if !recipients.contains(addr) {
            recipients.push(addr.clone());
        }
    }

    for user in users {
        match context::context().lookup_email_for_user(user) {
            Some(address) => {
                if !recipients.contains(&address) {
                    recipients.push(address);
                }
            }
            None => tracing::warn!(
                "'{user}' does not have a configured email address in the user configuration - \
                not sending an email to this user"
            ),
        }
    }
    recipients
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_recipients() {
        let emails = Vec::from(
            [
                "test1@example.com",
                "test2@example.com",
                "test2@example.com",
            ]
            .map(Into::into),
        );
        let users =
            Vec::from(["user1@pve", "user2@pve", "user2@pve", "user3@invalid"].map(Into::into));

        let expected = [
            "test1@example.com",
            "test2@example.com",
            "user1@example.com",
            "user2@example.com",
        ];

        let addrs = get_recipients(&emails, &users);

        assert_eq!(addrs, expected);
    }
}
