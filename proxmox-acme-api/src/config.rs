use std::borrow::Cow;

use crate::types::KnownAcmeDirectory;

/// List of known ACME directorties.
pub const KNOWN_ACME_DIRECTORIES: &[KnownAcmeDirectory] = &[
    KnownAcmeDirectory {
        name: Cow::Borrowed("Let's Encrypt V2"),
        url: Cow::Borrowed("https://acme-v02.api.letsencrypt.org/directory"),
    },
    KnownAcmeDirectory {
        name: Cow::Borrowed("Let's Encrypt V2 Staging"),
        url: Cow::Borrowed("https://acme-staging-v02.api.letsencrypt.org/directory"),
    },
];

/// Default ACME directorties.
pub const DEFAULT_ACME_DIRECTORY_ENTRY: &KnownAcmeDirectory = &KNOWN_ACME_DIRECTORIES[0];
