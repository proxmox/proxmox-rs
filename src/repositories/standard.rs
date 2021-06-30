use std::convert::TryFrom;
use std::fmt::Display;

use anyhow::{bail, Error};
use serde::{Deserialize, Serialize};

use crate::repositories::repository::{
    APTRepository, APTRepositoryFileType, APTRepositoryPackageType,
};

use proxmox::api::api;

#[api(
    properties: {
        handle: {
            description: "Handle referencing a standard repository.",
            type: String,
        },
    },
)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// Reference to a standard repository and configuration status.
pub struct APTStandardRepository {
    /// Handle referencing a standard repository.
    pub handle: APTRepositoryHandle,

    /// Configuration status of the associated repository.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<bool>,

    /// Full name of the repository.
    pub name: String,
}

#[api]
#[derive(Debug, Copy, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
/// Handles for Proxmox repositories.
pub enum APTRepositoryHandle {
    /// The enterprise repository for production use.
    Enterprise,
    /// The repository that can be used without subscription.
    NoSubscription,
    /// The test repository.
    Test,
    /// Ceph Pacific repository.
    CephPacific,
    /// Ceph Pacific test repository.
    CephPacificTest,
    /// Ceph Octoput repository.
    CephOctopus,
    /// Ceph Octoput test repository.
    CephOctopusTest,
}

impl TryFrom<&str> for APTRepositoryHandle {
    type Error = Error;

    fn try_from(string: &str) -> Result<Self, Error> {
        match string {
            "enterprise" => Ok(APTRepositoryHandle::Enterprise),
            "no-subscription" => Ok(APTRepositoryHandle::NoSubscription),
            "test" => Ok(APTRepositoryHandle::Test),
            "ceph-pacific" => Ok(APTRepositoryHandle::CephPacific),
            "ceph-pacific-test" => Ok(APTRepositoryHandle::CephPacificTest),
            "ceph-octopus" => Ok(APTRepositoryHandle::CephOctopus),
            "ceph-octopus-test" => Ok(APTRepositoryHandle::CephOctopusTest),
            _ => bail!("unknown repository handle '{}'", string),
        }
    }
}

impl Display for APTRepositoryHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            APTRepositoryHandle::Enterprise => write!(f, "enterprise"),
            APTRepositoryHandle::NoSubscription => write!(f, "no-subscription"),
            APTRepositoryHandle::Test => write!(f, "test"),
            APTRepositoryHandle::CephPacific => write!(f, "ceph-pacific"),
            APTRepositoryHandle::CephPacificTest => write!(f, "ceph-pacific-test"),
            APTRepositoryHandle::CephOctopus => write!(f, "ceph-octopus"),
            APTRepositoryHandle::CephOctopusTest => write!(f, "ceph-octopus-test"),
        }
    }
}

impl APTRepositoryHandle {
    /// Get the full name of the repository.
    pub fn name(self) -> String {
        match self {
            APTRepositoryHandle::Enterprise => "Enterprise Repository",
            APTRepositoryHandle::NoSubscription => "No-Subscription Repository",
            APTRepositoryHandle::Test => "Test Repository",
            APTRepositoryHandle::CephPacific => "Ceph Pacific Repository",
            APTRepositoryHandle::CephPacificTest => "Ceph Pacific Test Repository",
            APTRepositoryHandle::CephOctopus => "Ceph Octopus Repository",
            APTRepositoryHandle::CephOctopusTest => "Ceph Octopus Test Repository",
        }
        .to_string()
    }

    /// Get the standard file path for the repository referenced by the handle.
    pub fn path(self, product: &str) -> String {
        match self {
            APTRepositoryHandle::Enterprise => {
                format!("/etc/apt/sources.list.d/{}-enterprise.list", product)
            }
            APTRepositoryHandle::NoSubscription => "/etc/apt/sources.list".to_string(),
            APTRepositoryHandle::Test => "/etc/apt/sources.list".to_string(),
            APTRepositoryHandle::CephPacific => "/etc/apt/sources.list.d/ceph.list".to_string(),
            APTRepositoryHandle::CephPacificTest => "/etc/apt/sources.list.d/ceph.list".to_string(),
            APTRepositoryHandle::CephOctopus => "/etc/apt/sources.list.d/ceph.list".to_string(),
            APTRepositoryHandle::CephOctopusTest => "/etc/apt/sources.list.d/ceph.list".to_string(),
        }
    }

    /// Get package type, URI and the component associated with the handle.
    pub fn info(self, product: &str) -> (APTRepositoryPackageType, String, String) {
        match self {
            APTRepositoryHandle::Enterprise => (
                APTRepositoryPackageType::Deb,
                format!("https://enterprise.proxmox.com/debian/{}", product),
                format!("{}-enterprise", product),
            ),
            APTRepositoryHandle::NoSubscription => (
                APTRepositoryPackageType::Deb,
                format!("http://download.proxmox.com/debian/{}", product),
                format!("{}-no-subscription", product),
            ),
            APTRepositoryHandle::Test => (
                APTRepositoryPackageType::Deb,
                format!("http://download.proxmox.com/debian/{}", product),
                format!("{}test", product),
            ),
            APTRepositoryHandle::CephPacific => (
                APTRepositoryPackageType::Deb,
                "http://download.proxmox.com/debian/ceph-pacific".to_string(),
                "main".to_string(),
            ),
            APTRepositoryHandle::CephPacificTest => (
                APTRepositoryPackageType::Deb,
                "http://download.proxmox.com/debian/ceph-pacific".to_string(),
                "test".to_string(),
            ),
            APTRepositoryHandle::CephOctopus => (
                APTRepositoryPackageType::Deb,
                "http://download.proxmox.com/debian/ceph-octopus".to_string(),
                "main".to_string(),
            ),
            APTRepositoryHandle::CephOctopusTest => (
                APTRepositoryPackageType::Deb,
                "http://download.proxmox.com/debian/ceph-octopus".to_string(),
                "test".to_string(),
            ),
        }
    }

    /// Get the standard repository referenced by the handle.
    ///
    /// An URI in the result is not '/'-terminated (under the assumption that no valid
    /// product name is).
    pub fn to_repository(self, product: &str, suite: &str) -> APTRepository {
        let (package_type, uri, component) = self.info(product);

        APTRepository {
            types: vec![package_type],
            uris: vec![uri],
            suites: vec![suite.to_string()],
            components: vec![component],
            options: vec![],
            comment: String::new(),
            file_type: APTRepositoryFileType::List,
            enabled: true,
        }
    }
}
