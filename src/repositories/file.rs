use std::convert::TryFrom;
use std::fmt::Display;
use std::path::{Path, PathBuf};

use anyhow::{bail, format_err, Error};
use serde::{Deserialize, Serialize};

use crate::repositories::release::{get_current_release_codename, DEBIAN_SUITES};
use crate::repositories::repository::{
    APTRepository, APTRepositoryFileType, APTRepositoryPackageType,
};

use proxmox::api::api;

mod list_parser;
use list_parser::APTListFileParser;

mod sources_parser;
use sources_parser::APTSourcesFileParser;

trait APTRepositoryParser {
    /// Parse all repositories including the disabled ones and push them onto
    /// the provided vector.
    fn parse_repositories(&mut self) -> Result<Vec<APTRepository>, Error>;
}

#[api(
    properties: {
        "file-type": {
            type: APTRepositoryFileType,
        },
        repositories: {
            description: "List of APT repositories.",
            type: Array,
            items: {
                type: APTRepository,
            },
        },
        digest: {
            description: "Digest for the content of the file.",
            optional: true,
            type: Array,
            items: {
                description: "Digest byte.",
                type: Integer,
            },
        },
    },
)]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Represents an abstract APT repository file.
pub struct APTRepositoryFile {
    /// The path to the file.
    pub path: String,

    /// The type of the file.
    pub file_type: APTRepositoryFileType,

    /// List of repositories in the file.
    pub repositories: Vec<APTRepository>,

    /// Digest of the original contents.
    pub digest: Option<[u8; 32]>,
}

#[api]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Error type for problems with APT repository files.
pub struct APTRepositoryFileError {
    /// The path to the problematic file.
    pub path: String,

    /// The error message.
    pub error: String,
}

impl Display for APTRepositoryFileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "proxmox-apt error for '{}' - {}", self.path, self.error)
    }
}

impl std::error::Error for APTRepositoryFileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

#[api]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
/// Additional information for a repository.
pub struct APTRepositoryInfo {
    /// Path to the defining file.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub path: String,

    /// Index of the associated respository within the file (starting from 0).
    pub index: usize,

    /// The property from which the info originates (e.g. "Suites")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub property: Option<String>,

    /// Info kind (e.g. "warning")
    pub kind: String,

    /// Info message
    pub message: String,
}

impl APTRepositoryFile {
    /// Creates a new `APTRepositoryFile` without parsing.
    ///
    /// If the file is hidden, the path points to a directory, or the extension
    /// is usually ignored by APT (e.g. `.orig`), `Ok(None)` is returned, while
    /// invalid file names yield an error.
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Option<Self>, APTRepositoryFileError> {
        let path: PathBuf = path.as_ref().to_path_buf();

        let new_err = |path_string: String, err: &str| APTRepositoryFileError {
            path: path_string,
            error: err.to_string(),
        };

        let path_string = path
            .clone()
            .into_os_string()
            .into_string()
            .map_err(|os_string| {
                new_err(
                    os_string.to_string_lossy().to_string(),
                    "path is not valid unicode",
                )
            })?;

        let new_err = |err| new_err(path_string.clone(), err);

        if path.is_dir() {
            return Ok(None);
        }

        let file_name = match path.file_name() {
            Some(file_name) => file_name
                .to_os_string()
                .into_string()
                .map_err(|_| new_err("invalid path"))?,
            None => return Err(new_err("invalid path")),
        };

        if file_name.starts_with('.') || file_name.ends_with('~') {
            return Ok(None);
        }

        let extension = match path.extension() {
            Some(extension) => extension
                .to_os_string()
                .into_string()
                .map_err(|_| new_err("invalid path"))?,
            None => return Err(new_err("invalid extension")),
        };

        // See APT's apt-pkg/init.cc
        if extension.starts_with("dpkg-")
            || extension.starts_with("ucf-")
            || matches!(
                extension.as_str(),
                "disabled" | "bak" | "save" | "orig" | "distUpgrade"
            )
        {
            return Ok(None);
        }

        let file_type = APTRepositoryFileType::try_from(&extension[..])
            .map_err(|_| new_err("invalid extension"))?;

        if !file_name
            .chars()
            .all(|x| x.is_ascii_alphanumeric() || x == '_' || x == '-' || x == '.')
        {
            return Err(new_err("invalid characters in file name"));
        }

        Ok(Some(Self {
            path: path_string,
            file_type,
            repositories: vec![],
            digest: None,
        }))
    }

    /// Check if the file exists.
    pub fn exists(&self) -> bool {
        PathBuf::from(&self.path).exists()
    }

    pub fn read_with_digest(&self) -> Result<(Vec<u8>, [u8; 32]), APTRepositoryFileError> {
        let content = std::fs::read(&self.path).map_err(|err| self.err(format_err!("{}", err)))?;

        let digest = openssl::sha::sha256(&content);

        Ok((content, digest))
    }

    /// Create an `APTRepositoryFileError`.
    pub fn err(&self, error: Error) -> APTRepositoryFileError {
        APTRepositoryFileError {
            path: self.path.clone(),
            error: error.to_string(),
        }
    }

    /// Parses the APT repositories configured in the file on disk, including
    /// disabled ones.
    ///
    /// Resets the current repositories and digest, even on failure.
    pub fn parse(&mut self) -> Result<(), APTRepositoryFileError> {
        self.repositories.clear();
        self.digest = None;

        let (content, digest) = self.read_with_digest()?;

        let mut parser: Box<dyn APTRepositoryParser> = match self.file_type {
            APTRepositoryFileType::List => Box::new(APTListFileParser::new(&content[..])),
            APTRepositoryFileType::Sources => Box::new(APTSourcesFileParser::new(&content[..])),
        };

        let repos = parser.parse_repositories().map_err(|err| self.err(err))?;

        for (n, repo) in repos.iter().enumerate() {
            repo.basic_check()
                .map_err(|err| self.err(format_err!("check for repository {} - {}", n + 1, err)))?;
        }

        self.repositories = repos;
        self.digest = Some(digest);

        Ok(())
    }

    /// Writes the repositories to the file on disk.
    ///
    /// If a digest is provided, checks that the current content of the file still
    /// produces the same one.
    pub fn write(&self) -> Result<(), APTRepositoryFileError> {
        if let Some(digest) = self.digest {
            if !self.exists() {
                return Err(self.err(format_err!("digest specified, but file does not exist")));
            }

            let (_, current_digest) = self.read_with_digest()?;
            if digest != current_digest {
                return Err(self.err(format_err!("digest mismatch")));
            }
        }

        if self.repositories.is_empty() {
            return std::fs::remove_file(&self.path)
                .map_err(|err| self.err(format_err!("unable to remove file - {}", err)));
        }

        let mut content = vec![];

        for (n, repo) in self.repositories.iter().enumerate() {
            repo.basic_check()
                .map_err(|err| self.err(format_err!("check for repository {} - {}", n + 1, err)))?;

            repo.write(&mut content)
                .map_err(|err| self.err(format_err!("writing repository {} - {}", n + 1, err)))?;
        }

        let path = PathBuf::from(&self.path);
        let dir = match path.parent() {
            Some(dir) => dir,
            None => return Err(self.err(format_err!("invalid path"))),
        };

        std::fs::create_dir_all(dir)
            .map_err(|err| self.err(format_err!("unable to create parent dir - {}", err)))?;

        let pid = std::process::id();
        let mut tmp_path = path.clone();
        tmp_path.set_extension("tmp");
        tmp_path.set_extension(format!("{}", pid));

        if let Err(err) = std::fs::write(&tmp_path, content) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(self.err(format_err!("writing {:?} failed - {}", path, err)));
        }

        if let Err(err) = std::fs::rename(&tmp_path, &path) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(self.err(format_err!("rename failed for {:?} - {}", path, err)));
        }

        Ok(())
    }

    /// Checks if old or unstable suites are configured and also that the
    /// `stable` keyword is not used.
    pub fn check_suites(&self) -> Result<Vec<APTRepositoryInfo>, Error> {
        let mut infos = vec![];

        for (n, repo) in self.repositories.iter().enumerate() {
            if !repo.types.contains(&APTRepositoryPackageType::Deb) {
                continue;
            }

            let mut add_info = |kind, message| {
                infos.push(APTRepositoryInfo {
                    path: self.path.clone(),
                    index: n,
                    property: Some("Suites".to_string()),
                    kind,
                    message,
                })
            };

            let current_suite = get_current_release_codename()?;

            let current_index = match DEBIAN_SUITES
                .iter()
                .position(|&suite| suite == current_suite)
            {
                Some(index) => index,
                None => bail!("unknown release {}", current_suite),
            };

            for (n, suite) in DEBIAN_SUITES.iter().enumerate() {
                if repo.has_suite_variant(suite) {
                    if n < current_index {
                        add_info(
                            "warning".to_string(),
                            format!("old suite '{}' configured!", suite),
                        );
                    }

                    if n == current_index + 1 {
                        add_info(
                            "ignore-pre-upgrade-warning".to_string(),
                            format!("suite '{}' should not be used in production!", suite),
                        );
                    }

                    if n > current_index + 1 {
                        add_info(
                            "warning".to_string(),
                            format!("suite '{}' should not be used in production!", suite),
                        );
                    }
                }
            }

            if repo.has_suite_variant("stable") {
                add_info(
                    "warning".to_string(),
                    "use the name of the stable distribution instead of 'stable'!".to_string(),
                );
            }
        }

        Ok(infos)
    }

    /// Checks for official URIs.
    pub fn check_uris(&self) -> Vec<APTRepositoryInfo> {
        let mut infos = vec![];

        for (n, repo) in self.repositories.iter().enumerate() {
            let mut origin = match repo.get_cached_origin() {
                Ok(option) => option,
                Err(_) => None,
            };

            if origin.is_none() {
                origin = repo.origin_from_uris();
            }

            if let Some(origin) = origin {
                infos.push(APTRepositoryInfo {
                    path: self.path.clone(),
                    index: n,
                    kind: "origin".to_string(),
                    property: None,
                    message: origin,
                });
            }
        }

        infos
    }
}
