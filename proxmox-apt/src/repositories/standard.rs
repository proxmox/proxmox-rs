//! Standard Proxmox APT repositories, materialized from [`HOST_PRODUCTS`] x [`CHANNELS`]
//! and [`CEPH_RELEASES`] x [`CHANNELS`] at first use.
//!
//! # Adding a Ceph release
//! 1. New `APTRepoType` variant + `Deserialize` arm (in api-types).
//! 2. Classify in `APTRepoType::is_ceph_release` (exhaustive; compile-fails until done).
//! 3. Add to `KNOWN_NONHOST_REPO_TYPES` and the `_force_exhaustive_repo_type` test; the
//!    `open_enum_variants_round_trip` test then also exercises the new kebab token.
//! 4. Add the three `pub const CEPH_<RELEASE>_*` handles on `APTRepositoryHandle`.
//! 5. Add a row to [`CEPH_RELEASES`].
//!
//! # Adding a host product
//! 1. New `HostProduct` and `APTRepoType` variants + `Deserialize` arms.
//! 2. Classify in `APTRepoType::is_host_product`.
//! 3. Update the `_force_exhaustive_*` tests for both types.
//! 4. Add a row to [`HOST_PRODUCTS`].

use std::sync::LazyLock;

use proxmox_apt_api_types::{
    APTRepoComponent, APTRepoType, APTRepository, APTRepositoryFileType, APTRepositoryHandle,
    APTRepositoryOption, APTRepositoryPackageType, APTStandardRepository, DebianCodename,
    HostProduct,
};

// TODO: parameterize for brand-registered keyrings.
const PROXMOX_KEYRING: &str = "/usr/share/keyrings/proxmox-archive-keyring.gpg";

/// Accepted keyrings for the signed-by-key fallback; a slice lets rows tolerate keyring rotation.
const STANDARD_KEYRINGS: &[&str] = &[PROXMOX_KEYRING];

const ENTERPRISE_DOMAIN: &str = "https://enterprise.proxmox.com/debian";
const DOWNLOAD_DOMAIN: &str = "http://download.proxmox.com/debian";

/// One release channel offered for every host product and Ceph release.
struct Channel {
    component: APTRepoComponent,
    /// Wire-form kebab string written to apt sources files; on-disk format.
    kebab: &'static str,
    title: &'static str,
    domain: &'static str,
    host_description: &'static str,
    /// `{name}` is substituted with the Ceph release name at use.
    ceph_description_template: &'static str,
}

/// Channels every host product and Ceph release offers in lock-step.
const CHANNELS: &[Channel] = &[
    Channel {
        component: APTRepoComponent::Enterprise,
        kebab: "enterprise",
        title: "Enterprise",
        domain: ENTERPRISE_DOMAIN,
        host_description: "This is the default, stable, and recommended repository, available \
            for all Proxmox subscription users.",
        ceph_description_template:
            "This repository holds the production-ready Proxmox Ceph {name} packages.",
    },
    Channel {
        component: APTRepoComponent::NoSubscription,
        kebab: "no-subscription",
        title: "No-Subscription",
        domain: DOWNLOAD_DOMAIN,
        host_description: "This is the recommended repository for testing and non-production \
            use. Its packages are not as heavily tested and validated as the production ready \
            enterprise repository. You don't need a subscription key to access this repository.",
        ceph_description_template:
            "This repository holds the Proxmox Ceph {name} packages intended for \
             non-production use.",
    },
    Channel {
        component: APTRepoComponent::Test,
        kebab: "test",
        title: "Test",
        domain: DOWNLOAD_DOMAIN,
        host_description: "This repository contains the latest packages and is primarily used \
            for test labs and by developers to test new features.",
        ceph_description_template:
            "This repository contains the Ceph {name} packages before they are moved to \
             the main repository.",
    },
];

impl Channel {
    /// Render the Ceph release channel description for a given release.
    fn ceph_description(&self, name: &str) -> String {
        self.ceph_description_template.replace("{name}", name)
    }
}

/// One Proxmox host product; expands to one [`StandardRepoEntry`] per channel.
struct HostProductDef {
    repo_type: APTRepoType,
    host_product: HostProduct,
    /// Short name used in URI suffixes, `deb_component`, and the enterprise file path.
    slug: &'static str,
    /// PVE only: detect the legacy pre-product-prefix URI form (no `/pve` suffix).
    has_legacy_uri: bool,
}

const HOST_PRODUCTS: &[HostProductDef] = &[
    HostProductDef {
        repo_type: APTRepoType::Pve,
        host_product: HostProduct::Pve,
        slug: "pve",
        has_legacy_uri: true,
    },
    HostProductDef {
        repo_type: APTRepoType::Pbs,
        host_product: HostProduct::Pbs,
        slug: "pbs",
        has_legacy_uri: false,
    },
    HostProductDef {
        repo_type: APTRepoType::Pdm,
        host_product: HostProduct::Pdm,
        slug: "pdm",
        has_legacy_uri: false,
    },
    HostProductDef {
        repo_type: APTRepoType::Pmg,
        host_product: HostProduct::Pmg,
        slug: "pmg",
        has_legacy_uri: false,
    },
];

/// One Proxmox Ceph release; offered on PVE only and expands to one entry per channel.
struct CephRelease {
    repo_type: APTRepoType,
    /// Capitalized display name ("Squid"); URI/file slug comes from `repo_type` Display.
    name: &'static str,
    /// Suites this release ships for; empty means "every suite".
    suites: &'static [DebianCodename],
}

const CEPH_RELEASES: &[CephRelease] = &[
    CephRelease {
        repo_type: APTRepoType::CephSquid,
        name: "Squid",
        suites: &[DebianCodename::Trixie],
    },
    CephRelease {
        repo_type: APTRepoType::CephTentacle,
        name: "Tentacle",
        suites: &[DebianCodename::Trixie],
    },
];

/// One materialized standard-repo row, built from a [`HostProductDef`] or [`CephRelease`]
/// paired with one channel. Per-channel fields are owned strings, formatted on build.
struct StandardRepoEntry {
    repo_type: APTRepoType,
    component: APTRepoComponent,
    offered_on_host: HostProduct,
    name: String,
    description: String,
    package_type: APTRepositoryPackageType,
    canonical_uri: String,
    extra_detect_uris: Vec<String>,
    deb_component: String,
    /// Legacy deb_component spellings still accepted on detection. PVE/PBS/PMG bookworm wrote
    /// `pvetest`/`pbstest`/`pmgtest` (no hyphen) for the Test channel; trixie standardized to the
    /// hyphenated form. Upgraded hosts keep the old string until apt rewrites their sources.
    extra_detect_components: Vec<String>,
    signing_keys: &'static [&'static str],
    file_path_legacy: String,
    file_path_deb822: String,
    suites: &'static [DebianCodename],
}

/// Materialized table: host products' channels then Ceph releases' channels; order is
/// load-bearing for `standard_repositories()` (the web UI consumes it for dropdowns).
static STANDARD_REPOS: LazyLock<Vec<StandardRepoEntry>> = LazyLock::new(|| {
    let total = (HOST_PRODUCTS.len() + CEPH_RELEASES.len()) * CHANNELS.len();
    let mut v = Vec::with_capacity(total);
    for hp in HOST_PRODUCTS {
        for c in CHANNELS {
            v.push(host_product_entry(hp, c));
        }
    }
    for cr in CEPH_RELEASES {
        for c in CHANNELS {
            v.push(ceph_release_entry(cr, c));
        }
    }
    v
});

fn host_product_entry(hp: &HostProductDef, c: &Channel) -> StandardRepoEntry {
    let canonical_uri = format!("{}/{}", c.domain, hp.slug);
    let extra_detect_uris = if hp.has_legacy_uri {
        vec![c.domain.to_string()]
    } else {
        Vec::new()
    };
    let (file_path_legacy, file_path_deb822) = if c.component == APTRepoComponent::Enterprise {
        (
            format!("/etc/apt/sources.list.d/{}-enterprise.list", hp.slug),
            format!("/etc/apt/sources.list.d/{}-enterprise.sources", hp.slug),
        )
    } else {
        (
            "/etc/apt/sources.list".to_string(),
            "/etc/apt/sources.list.d/proxmox.sources".to_string(),
        )
    };
    let extra_detect_components = if c.component == APTRepoComponent::Test {
        vec![format!("{}test", hp.slug)]
    } else {
        Vec::new()
    };
    StandardRepoEntry {
        repo_type: hp.repo_type.clone(),
        component: c.component.clone(),
        offered_on_host: hp.host_product.clone(),
        name: c.title.to_string(),
        description: c.host_description.to_string(),
        package_type: APTRepositoryPackageType::Deb,
        canonical_uri,
        extra_detect_uris,
        deb_component: format!("{}-{}", hp.slug, c.kebab),
        extra_detect_components,
        signing_keys: STANDARD_KEYRINGS,
        file_path_legacy,
        file_path_deb822,
        suites: &[],
    }
}

fn ceph_release_entry(cr: &CephRelease, c: &Channel) -> StandardRepoEntry {
    let repo_type_kebab = cr.repo_type.to_string();
    StandardRepoEntry {
        repo_type: cr.repo_type.clone(),
        component: c.component.clone(),
        offered_on_host: HostProduct::Pve,
        name: format!("Ceph {} {}", cr.name, c.title),
        description: c.ceph_description(cr.name),
        package_type: APTRepositoryPackageType::Deb,
        canonical_uri: format!("{}/{repo_type_kebab}", c.domain),
        extra_detect_uris: Vec::new(),
        deb_component: c.kebab.to_string(),
        extra_detect_components: Vec::new(),
        signing_keys: STANDARD_KEYRINGS,
        file_path_legacy: "/etc/apt/sources.list.d/ceph.list".to_string(),
        file_path_deb822: "/etc/apt/sources.list.d/ceph.sources".to_string(),
        suites: cr.suites,
    }
}

impl StandardRepoEntry {
    /// All URIs (canonical first) accepted as identifying this entry.
    fn all_detect_uris(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.canonical_uri.as_str())
            .chain(self.extra_detect_uris.iter().map(|s| s.as_str()))
    }

    /// Whether this entry applies on `suite`; empty `suites` means every suite.
    fn applies_to_suite(&self, suite: &DebianCodename) -> bool {
        self.suites.is_empty() || self.suites.iter().any(|s| s == suite)
    }

    /// Whether `c` matches this entry's canonical or any legacy deb_component spelling.
    fn matches_component(&self, c: &str) -> bool {
        c == self.deb_component || self.extra_detect_components.iter().any(|x| x == c)
    }
}

/// Handle of an entry; collapses `repo_type` to `None` for host products (wire form).
fn entry_handle(entry: &StandardRepoEntry) -> APTRepositoryHandle {
    APTRepositoryHandle::new(Some(entry.repo_type.clone()), entry.component.clone())
}

/// First table entry whose handle matches and is offered for `host_product`.
fn lookup_entry(
    handle: &APTRepositoryHandle,
    host_product: &HostProduct,
) -> Option<&'static StandardRepoEntry> {
    STANDARD_REPOS
        .iter()
        .find(|e| e.offered_on_host == *host_product && entry_handle(e) == *handle)
}

/// Whether any URI of `repo` resolves to a known URI of `entry`, either directly or via the
/// signed-by-key fallback for offline-mirror / staging setups (#5207). `suite` scopes the
/// signature lookup so a sibling-suite InRelease cannot falsely match.
fn entry_uri_matches(
    entry: &StandardRepoEntry,
    repo: &APTRepository,
    suite: &DebianCodename,
) -> bool {
    let suite_str = suite.to_string();
    repo.uris.iter().any(|u| {
        let u_trim = u.trim_end_matches('/');
        if entry.all_detect_uris().any(|known| known == u_trim) {
            return true;
        }
        entry
            .signing_keys
            .iter()
            .any(|key| crate::repositories::repository::is_signed_by_key(u_trim, &suite_str, key))
    })
}

/// Entry matching a parsed `repo` on `suite`, by package type, URI, component, and suite.
/// Requiring `repo.suites` to contain `suite` keeps a Bookworm-pinned config from being
/// reported as the standard handle for a Trixie query.
fn find_entry_for_repository(
    repo: &APTRepository,
    host_product: &HostProduct,
    suite: &DebianCodename,
) -> Option<&'static StandardRepoEntry> {
    let suite_str = suite.to_string();
    STANDARD_REPOS.iter().find(|e| {
        if e.offered_on_host != *host_product {
            return false;
        }
        if !repo.suites.iter().any(|s| *s == suite_str) {
            return false;
        }
        if !e.applies_to_suite(suite) {
            return false;
        }
        if !repo.types.contains(&e.package_type) {
            return false;
        }
        if !repo.components.iter().any(|c| e.matches_component(c)) {
            return false;
        }
        entry_uri_matches(e, repo, suite)
    })
}

pub trait APTStandardRepositoryImpl {
    fn from_handle_for(
        handle: APTRepositoryHandle,
        host_product: &HostProduct,
    ) -> APTStandardRepository;
}

impl APTStandardRepositoryImpl for APTStandardRepository {
    fn from_handle_for(
        handle: APTRepositoryHandle,
        host_product: &HostProduct,
    ) -> APTStandardRepository {
        let (name, description) = lookup_entry(&handle, host_product)
            .map(|e| (e.name.clone(), e.description.clone()))
            .unwrap_or_else(|| {
                let label = format!("Unknown repository ({handle})");
                (label.clone(), label)
            });
        APTStandardRepository {
            handle,
            status: None,
            name,
            description,
        }
    }
}

pub trait APTRepositoryHandleImpl {
    /// Whether `repo` is the standard repository this handle references on `suite`.
    fn is_referenced_by(
        &self,
        repo: &APTRepository,
        host_product: &HostProduct,
        suite: &DebianCodename,
    ) -> bool;
    /// Fresh repository entry to write to disk; `None` if no table row matches.
    fn to_repository(
        &self,
        host_product: &HostProduct,
        suite: &DebianCodename,
    ) -> Option<APTRepository>;
    /// File path to write this handle's repository to; `None` under the same conditions as
    /// `to_repository`.
    fn file_path(&self, host_product: &HostProduct, suite: &DebianCodename) -> Option<String>;
}

impl APTRepositoryHandleImpl for APTRepositoryHandle {
    fn is_referenced_by(
        &self,
        repo: &APTRepository,
        host_product: &HostProduct,
        suite: &DebianCodename,
    ) -> bool {
        let Some(entry) = lookup_entry(self, host_product) else {
            return false;
        };
        if !entry.applies_to_suite(suite) {
            return false;
        }
        if !repo.types.contains(&entry.package_type) {
            return false;
        }
        let suite_str = suite.to_string();
        if !repo.suites.iter().any(|s| *s == suite_str) {
            return false;
        }
        if !repo.components.iter().any(|c| entry.matches_component(c)) {
            return false;
        }
        entry_uri_matches(entry, repo, suite)
    }

    fn to_repository(
        &self,
        host_product: &HostProduct,
        suite: &DebianCodename,
    ) -> Option<APTRepository> {
        let entry = lookup_entry(self, host_product)?;
        if !entry.applies_to_suite(suite) {
            return None;
        }
        let file_type = if *suite >= DebianCodename::Trixie {
            APTRepositoryFileType::Sources
        } else {
            APTRepositoryFileType::List
        };
        Some(APTRepository {
            types: vec![entry.package_type],
            uris: vec![entry.canonical_uri.clone()],
            suites: vec![suite.to_string()],
            components: vec![entry.deb_component.clone()],
            options: vec![APTRepositoryOption {
                key: "Signed-By".into(),
                values: vec![entry.signing_keys[0].to_string()],
            }],
            comment: String::new(),
            file_type,
            enabled: true,
        })
    }

    fn file_path(&self, host_product: &HostProduct, suite: &DebianCodename) -> Option<String> {
        let entry = lookup_entry(self, host_product)?;
        if !entry.applies_to_suite(suite) {
            return None;
        }
        let path = if *suite >= DebianCodename::Trixie {
            &entry.file_path_deb822
        } else {
            &entry.file_path_legacy
        };
        Some(path.clone())
    }
}

/// Standard repositories offered for `host_product` on `suite`, in table-declaration order.
pub fn standard_repos_offered_for(
    host_product: &HostProduct,
    suite: &DebianCodename,
) -> Vec<APTStandardRepository> {
    STANDARD_REPOS
        .iter()
        .filter(|e| e.offered_on_host == *host_product && e.applies_to_suite(suite))
        .map(|e| APTStandardRepository::from_handle_for(entry_handle(e), host_product))
        .collect()
}

/// Table-known handle for a parsed `repo` on `suite`, mapping it back to a standard
/// repository identifier; `None` if no row matches the queried suite.
pub fn find_handle_for_repository(
    repo: &APTRepository,
    host_product: &HostProduct,
    suite: &DebianCodename,
) -> Option<APTRepositoryHandle> {
    find_entry_for_repository(repo, host_product, suite).map(entry_handle)
}

/// Replace any legacy `deb_component` spellings on `repo` with the canonical form of the
/// matching standard entry. Returns `true` if any component was rewritten.
///
/// Used by write-path callers: PVE 8 wrote the test-channel deb822 component without a
/// hyphen (`pvetest` / `pbstest` / `pmgtest`); the unhyphenated form is no longer hosted on
/// `download.proxmox.com` and an `apt update` against a PVE 9 host that still carries the
/// legacy spelling will 404. Rewriting on the next API write fixes the file in place.
pub fn canonicalize_components_to_standard(
    repo: &mut APTRepository,
    host_product: &HostProduct,
    suite: &DebianCodename,
) -> bool {
    let Some(entry) = find_entry_for_repository(repo, host_product, suite) else {
        return false;
    };
    let mut changed = false;
    for c in repo.components.iter_mut() {
        if *c != entry.deb_component && entry.matches_component(c) {
            *c = entry.deb_component.clone();
            changed = true;
        }
    }
    changed
}
