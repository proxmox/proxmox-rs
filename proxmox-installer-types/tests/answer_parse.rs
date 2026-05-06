//! Wire-format guards for the answer file format.
//!
//! These tests catch accidental field renames, lost `deny_unknown_fields`
//! attributes, and dropped legacy aliases that would silently break upgrades
//! from older auto-installer answer files.

use proxmox_installer_types::answer::AutoInstallerConfig;

/// Modern, kebab-case answer file. Covers every field that has a legacy
/// snake_case alias so a rename or alias drop fails the kebab-case test
/// in tandem with the legacy test below.
const MODERN_ANSWER: &str = r#"
[global]
country = "at"
fqdn = "test.example.com"
keyboard = "en-us"
mailto = "ops@example.com"
timezone = "Europe/Vienna"
root-password-hashed = "$5$abcdefgh$abcdefghijklmnopqrstuvwxyz01234"
reboot-on-error = false
reboot-mode = "reboot"
root-ssh-keys = ["ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBzeJUiB6X test"]
subscription-key = "pve4c-1234567890"

[network]
source = "from-dhcp"

[disk-setup]
filesystem = "ext4"
disk-list = ["sda"]
filter-match = "any"

[post-installation-webhook]
url = "https://example.invalid/hook"
cert-fingerprint = "ab:cd:ef:12:34:56:78:90:a1:b2:c3:d4:e5:f6:7a:8b:9c:0d:aa:bb:cc:dd:ee:ff:21:43:65:87:09:af:bd:ce"

[first-boot]
source = "from-iso"
ordering = "fully-up"
"#;

/// Same content as MODERN_ANSWER but using the deprecated snake_case keys
/// for every field that has a legacy alias. Must round-trip into the same
/// struct when the `legacy` cargo feature is enabled.
#[cfg(feature = "legacy")]
const LEGACY_ANSWER: &str = r#"
[global]
country = "at"
fqdn = "test.example.com"
keyboard = "en-us"
mailto = "ops@example.com"
timezone = "Europe/Vienna"
root_password_hashed = "$5$abcdefgh$abcdefghijklmnopqrstuvwxyz01234"
reboot_on_error = false
reboot_mode = "reboot"
root_ssh_keys = ["ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIBzeJUiB6X test"]
subscription_key = "pve4c-1234567890"

[network]
source = "from-dhcp"

[disk-setup]
filesystem = "ext4"
disk_list = ["sda"]
filter_match = "any"

[post-installation-webhook]
url = "https://example.invalid/hook"
cert_fingerprint = "ab:cd:ef:12:34:56:78:90:a1:b2:c3:d4:e5:f6:7a:8b:9c:0d:aa:bb:cc:dd:ee:ff:21:43:65:87:09:af:bd:ce"

[first-boot]
source = "from-iso"
ordering = "fully-up"
"#;

#[test]
fn parses_modern_answer() {
    toml::from_str::<AutoInstallerConfig>(MODERN_ANSWER)
        .expect("kebab-case answer must parse without the legacy feature");
}

#[cfg(feature = "legacy")]
#[test]
fn parses_legacy_snake_case_answer() {
    let modern: AutoInstallerConfig = toml::from_str(MODERN_ANSWER)
        .expect("kebab-case fixture must parse with legacy feature too");
    let legacy: AutoInstallerConfig = toml::from_str(LEGACY_ANSWER)
        .expect("snake_case answer must parse with the legacy feature");
    assert_eq!(
        modern, legacy,
        "the kebab-case and snake_case fixtures must deserialize to the same value"
    );
}

#[test]
fn rejects_unknown_global_field() {
    let modified = MODERN_ANSWER.replace(
        "[global]\ncountry = \"at\"",
        "[global]\ncountry = \"at\"\nbogus-extra-field = \"oops\"",
    );
    assert!(
        toml::from_str::<AutoInstallerConfig>(&modified).is_err(),
        "GlobalOptions must reject unknown fields, otherwise typos and stale keys go unnoticed"
    );
}

#[test]
fn roundtrip_modern_answer() {
    let parsed: AutoInstallerConfig =
        toml::from_str(MODERN_ANSWER).expect("answer must parse");
    let reserialized = toml::to_string(&parsed).expect("answer must serialize");
    let reparsed: AutoInstallerConfig =
        toml::from_str(&reserialized).expect("re-serialized answer must parse again");
    assert_eq!(
        parsed, reparsed,
        "AutoInstallerConfig must survive a serialize/deserialize round-trip unchanged"
    );
}
