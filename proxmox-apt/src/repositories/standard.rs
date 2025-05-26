use proxmox_apt_api_types::{
    APTRepository, APTRepositoryFileType, APTRepositoryHandle, APTRepositoryPackageType,
    APTStandardRepository,
};

pub trait APTStandardRepositoryImpl {
    fn from_handle(handle: APTRepositoryHandle) -> APTStandardRepository;
}

impl APTStandardRepositoryImpl for APTStandardRepository {
    fn from_handle(handle: APTRepositoryHandle) -> APTStandardRepository {
        APTStandardRepository {
            handle,
            status: None,
            name: handle.name(),
            description: handle.description(),
        }
    }
}

pub trait APTRepositoryHandleImpl {
    /// Get the description for the repository.
    fn description(self) -> String;
    /// Get the display name of the repository.
    fn name(self) -> String;
    /// Get the standard file path for the repository referenced by the handle.
    fn path(self, product: &str) -> String;
    /// Get package type, possible URIs and the component associated with the handle.
    ///
    /// The first URI is the preferred one.
    fn info(self, product: &str) -> (APTRepositoryPackageType, Vec<String>, String);
    /// Get the standard repository referenced by the handle.
    ///
    /// An URI in the result is not '/'-terminated (under the assumption that no valid
    /// product name is).
    fn to_repository(self, product: &str, suite: &str) -> APTRepository;
}

impl APTRepositoryHandleImpl for APTRepositoryHandle {
    fn description(self) -> String {
        match self {
            APTRepositoryHandle::Enterprise => {
                "This is the default, stable, and recommended repository, available for all \
                Proxmox subscription users."
            }
            APTRepositoryHandle::NoSubscription => {
                "This is the recommended repository for testing and non-production use. \
                Its packages are not as heavily tested and validated as the production ready \
                enterprise repository. You don't need a subscription key to access this repository."
            }
            APTRepositoryHandle::Test => {
                "This repository contains the latest packages and is primarily used for test labs \
                and by developers to test new features."
            }
            APTRepositoryHandle::CephSquidEnterprise => {
                "This repository holds the production-ready Proxmox Ceph Squid packages."
            }
            APTRepositoryHandle::CephSquidNoSubscription => {
                "This repository holds the Proxmox Ceph Squid packages intended for \
                non-production use."
            }
            APTRepositoryHandle::CephSquidTest => {
                "This repository contains the Ceph Squid packages before they are moved to the \
                main repository."
            }
        }
        .to_string()
    }

    fn name(self) -> String {
        match self {
            APTRepositoryHandle::Enterprise => "Enterprise",
            APTRepositoryHandle::NoSubscription => "No-Subscription",
            APTRepositoryHandle::Test => "Test",
            APTRepositoryHandle::CephSquidEnterprise => "Ceph Squid Enterprise",
            APTRepositoryHandle::CephSquidNoSubscription => "Ceph Squid No-Subscription",
            APTRepositoryHandle::CephSquidTest => "Ceph Squid Test",
        }
        .to_string()
    }

    fn path(self, product: &str) -> String {
        match self {
            APTRepositoryHandle::Enterprise => {
                format!("/etc/apt/sources.list.d/{}-enterprise.list", product)
            }
            APTRepositoryHandle::NoSubscription => "/etc/apt/sources.list".to_string(),
            APTRepositoryHandle::Test => "/etc/apt/sources.list".to_string(),
            APTRepositoryHandle::CephSquidEnterprise
            | APTRepositoryHandle::CephSquidNoSubscription
            | APTRepositoryHandle::CephSquidTest => "/etc/apt/sources.list.d/ceph.list".to_string(),
        }
    }

    fn info(self, product: &str) -> (APTRepositoryPackageType, Vec<String>, String) {
        match self {
            APTRepositoryHandle::Enterprise => (
                APTRepositoryPackageType::Deb,
                match product {
                    "pve" => vec![
                        "https://enterprise.proxmox.com/debian/pve".to_string(),
                        "https://enterprise.proxmox.com/debian".to_string(),
                    ],
                    _ => vec![format!("https://enterprise.proxmox.com/debian/{}", product)],
                },
                format!("{}-enterprise", product),
            ),
            APTRepositoryHandle::NoSubscription => (
                APTRepositoryPackageType::Deb,
                match product {
                    "pve" => vec![
                        "http://download.proxmox.com/debian/pve".to_string(),
                        "http://download.proxmox.com/debian".to_string(),
                    ],
                    _ => vec![format!("http://download.proxmox.com/debian/{}", product)],
                },
                format!("{}-no-subscription", product),
            ),
            APTRepositoryHandle::Test => (
                APTRepositoryPackageType::Deb,
                match product {
                    "pve" => vec![
                        "http://download.proxmox.com/debian/pve".to_string(),
                        "http://download.proxmox.com/debian".to_string(),
                    ],
                    _ => vec![format!("http://download.proxmox.com/debian/{}", product)],
                },
                match product {
                    "pdm" => format!("{}-test", product),
                    _ => format!("{}test", product),
                },
            ),
            APTRepositoryHandle::CephSquidEnterprise => (
                APTRepositoryPackageType::Deb,
                vec!["https://enterprise.proxmox.com/debian/ceph-squid".to_string()],
                "enterprise".to_string(),
            ),
            APTRepositoryHandle::CephSquidNoSubscription => (
                APTRepositoryPackageType::Deb,
                vec!["http://download.proxmox.com/debian/ceph-squid".to_string()],
                "no-subscription".to_string(),
            ),
            APTRepositoryHandle::CephSquidTest => (
                APTRepositoryPackageType::Deb,
                vec!["http://download.proxmox.com/debian/ceph-squid".to_string()],
                "test".to_string(),
            ),
        }
    }

    fn to_repository(self, product: &str, suite: &str) -> APTRepository {
        let (package_type, uris, component) = self.info(product);

        use crate::repositories::DebianCodename;
        let file_type = match DebianCodename::try_from(suite) {
            Ok(codename) if codename >= DebianCodename::Trixie => APTRepositoryFileType::Sources,
            _ => APTRepositoryFileType::List,
        };

        APTRepository {
            types: vec![package_type],
            uris: vec![uris.into_iter().next().unwrap()],
            suites: vec![suite.to_string()],
            components: vec![component],
            options: vec![],
            comment: String::new(),
            file_type,
            enabled: true,
        }
    }
}
