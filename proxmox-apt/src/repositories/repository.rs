use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, format_err, Error};

use crate::repositories::standard::APTRepositoryHandleImpl;
use proxmox_apt_api_types::{
    APTRepository, APTRepositoryFileType, APTRepositoryHandle, APTRepositoryOption,
};

pub trait APTRepositoryImpl {
    /// Crates an empty repository.
    fn new(file_type: APTRepositoryFileType) -> Self;

    /// Changes the `enabled` flag and makes sure the `Enabled` option for
    /// `APTRepositoryPackageType::Sources` repositories is updated too.
    fn set_enabled(&mut self, enabled: bool);

    /// Makes sure that all basic properties of a repository are present and not obviously invalid.
    fn basic_check(&self) -> Result<(), Error>;

    /// Checks if the repository is the one referenced by the handle.
    fn is_referenced_repository(
        &self,
        handle: APTRepositoryHandle,
        product: &str,
        suite: &str,
    ) -> bool;

    /// Guess the origin from the repository's URIs.
    ///
    /// Intended to be used as a fallback for get_cached_origin.
    fn origin_from_uris(&self) -> Option<String>;

    /// Get the `Origin:` value from a cached InRelease file.
    fn get_cached_origin(&self, apt_lists_dir: &Path) -> Result<Option<String>, Error>;

    /// Writes a repository in the corresponding format followed by a blank.
    ///
    /// Expects that `basic_check()` for the repository was successful.
    fn write(&self, w: &mut dyn Write) -> Result<(), Error>;
}

impl APTRepositoryImpl for APTRepository {
    fn new(file_type: APTRepositoryFileType) -> Self {
        Self {
            types: vec![],
            uris: vec![],
            suites: vec![],
            components: vec![],
            options: vec![],
            comment: String::new(),
            file_type,
            enabled: true,
        }
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;

        if self.file_type == APTRepositoryFileType::Sources {
            let enabled_string = match enabled {
                true => "true".to_string(),
                false => "false".to_string(),
            };
            for option in self.options.iter_mut() {
                if option.key == "Enabled" {
                    option.values = vec![enabled_string];
                    return;
                }
            }
            self.options.push(APTRepositoryOption {
                key: "Enabled".to_string(),
                values: vec![enabled_string],
            });
        }
    }

    fn basic_check(&self) -> Result<(), Error> {
        if self.types.is_empty() {
            bail!("missing package type(s)");
        }
        if self.uris.is_empty() {
            bail!("missing URI(s)");
        }
        if self.suites.is_empty() {
            bail!("missing suite(s)");
        }

        for uri in self.uris.iter() {
            if !uri.contains(':') || uri.len() < 3 {
                bail!("invalid URI: '{uri}'");
            }
        }

        for suite in self.suites.iter() {
            if !suite.ends_with('/') && self.components.is_empty() {
                bail!("missing component(s)");
            } else if suite.ends_with('/') && !self.components.is_empty() {
                bail!("absolute suite '{suite}' does not allow component(s)");
            }
        }

        if self.file_type == APTRepositoryFileType::List {
            if self.types.len() > 1 {
                bail!("more than one package type");
            }
            if self.uris.len() > 1 {
                bail!("more than one URI");
            }
            if self.suites.len() > 1 {
                bail!("more than one suite");
            }
        }

        Ok(())
    }

    fn is_referenced_repository(
        &self,
        handle: APTRepositoryHandle,
        product: &str,
        suite: &str,
    ) -> bool {
        let (package_type, handle_uris, component) = handle.info(product);

        let mut found_uri = false;

        for uri in self.uris.iter() {
            let uri = uri.trim_end_matches('/');

            found_uri = found_uri || handle_uris.iter().any(|handle_uri| handle_uri == uri);
        }

        // In the past it was main instead of enterprise/no-subscription, and main now maps to
        // no-subscription. Note this only applies for Quincy.
        let found_component = if handle == APTRepositoryHandle::CephQuincyNoSubscription {
            self.components.contains(&component) || self.components.contains(&"main".to_string())
        } else {
            self.components.contains(&component)
        };

        self.types.contains(&package_type)
            && found_uri
            // using contains would require a &String
            && self.suites.iter().any(|self_suite| self_suite == suite)
            && found_component
    }

    fn origin_from_uris(&self) -> Option<String> {
        for uri in self.uris.iter() {
            if let Some(host) = host_from_uri(uri) {
                if host == "proxmox.com" || host.ends_with(".proxmox.com") {
                    return Some("Proxmox".to_string());
                }

                if host == "debian.org" || host.ends_with(".debian.org") {
                    return Some("Debian".to_string());
                }
            }
        }

        None
    }

    fn get_cached_origin(&self, apt_lists_dir: &Path) -> Result<Option<String>, Error> {
        for uri in self.uris.iter() {
            for suite in self.suites.iter() {
                let mut file = release_filename(apt_lists_dir, uri, suite, false);

                if !file.exists() {
                    file = release_filename(apt_lists_dir, uri, suite, true);
                    if !file.exists() {
                        continue;
                    }
                }

                let raw = std::fs::read(&file)
                    .map_err(|err| format_err!("unable to read {file:?} - {err}"))?;
                let reader = BufReader::new(&*raw);

                for line in reader.lines() {
                    let line =
                        line.map_err(|err| format_err!("unable to read {file:?} - {err}"))?;

                    if let Some(value) = line.strip_prefix("Origin:") {
                        return Ok(Some(
                            value
                                .trim_matches(|c| char::is_ascii_whitespace(&c))
                                .to_string(),
                        ));
                    }
                }
            }
        }

        Ok(None)
    }

    fn write(&self, w: &mut dyn Write) -> Result<(), Error> {
        match self.file_type {
            APTRepositoryFileType::List => write_one_line(self, w),
            APTRepositoryFileType::Sources => write_stanza(self, w),
        }
    }
}

/// Get the path to the cached (In)Release file.
fn release_filename(apt_lists_dir: &Path, uri: &str, suite: &str, detached: bool) -> PathBuf {
    let mut path = PathBuf::from(apt_lists_dir);

    let encoded_uri = uri_to_filename(uri);
    let filename = if detached { "Release" } else { "InRelease" };

    if suite == "/" {
        path.push(format!("{encoded_uri}_{filename}"));
    } else if suite == "./" {
        path.push(format!("{encoded_uri}_._{filename}"));
    } else {
        let normalized_suite = suite.replace('/', "_"); // e.g. for buster/updates
        path.push(format!("{encoded_uri}_dists_{normalized_suite}_{filename}",));
    }

    path
}

/// See APT's URItoFileName in contrib/strutl.cc
fn uri_to_filename(uri: &str) -> String {
    let mut filename = uri;

    if let Some(begin) = filename.find("://") {
        filename = &filename[(begin + 3)..];
    }

    if uri.starts_with("http://") || uri.starts_with("https://") {
        if let Some(begin) = filename.find('@') {
            filename = &filename[(begin + 1)..];
        }
    }

    // APT seems to only strip one final slash, so do the same
    filename = filename.strip_suffix('/').unwrap_or(filename);

    let encode_chars = "\\|{}[]<>\"^~_=!@#$%^&*";

    let mut encoded = String::with_capacity(filename.len());

    for b in filename.as_bytes().iter() {
        if *b <= 0x20 || *b >= 0x7F || encode_chars.contains(*b as char) {
            let mut hex = [0u8; 2];
            // unwrap: we're hex-encoding a single byte into a 2-byte slice
            hex::encode_to_slice([*b], &mut hex).unwrap();
            let hex = unsafe { std::str::from_utf8_unchecked(&hex) };
            encoded = format!("{encoded}%{hex}");
        } else {
            encoded.push(*b as char);
        }
    }

    encoded.replace('/', "_")
}

/// Get the host part from a given URI.
fn host_from_uri(uri: &str) -> Option<&str> {
    let host = uri.strip_prefix("http")?;
    let host = host.strip_prefix('s').unwrap_or(host);
    let mut host = host.strip_prefix("://")?;

    if let Some(end) = host.find('/') {
        host = &host[..end];
    }

    if let Some(begin) = host.find('@') {
        host = &host[(begin + 1)..];
    }

    if let Some(end) = host.find(':') {
        host = &host[..end];
    }

    Some(host)
}

/// Strips existing double quotes from the string first, and then adds double quotes at the
/// beginning and end if there is an ASCII whitespace in the `string`, which is not escaped by
/// `[]`.
fn quote_for_one_line(string: &str) -> String {
    let mut add_quotes = false;
    let mut wait_for_bracket = false;

    // easier to just quote the whole string, so ignore pre-existing quotes
    // currently, parsing removes them anyways, but being on the safe side is rather cheap
    let string = string.replace('"', "");

    for c in string.chars() {
        if wait_for_bracket {
            if c == ']' {
                wait_for_bracket = false;
            }
            continue;
        }

        if char::is_ascii_whitespace(&c) {
            add_quotes = true;
            break;
        }

        if c == '[' {
            wait_for_bracket = true;
        }
    }

    match add_quotes {
        true => format!("\"{string}\""),
        false => string,
    }
}

/// Writes a repository in one-line format.
///
/// Expects that `repo.file_type == APTRepositoryFileType::List`.
///
/// Expects that `basic_check()` for the repository was successful.
fn write_one_line(repo: &APTRepository, w: &mut dyn Write) -> Result<(), Error> {
    if repo.file_type != APTRepositoryFileType::List {
        bail!("not a .list repository");
    }

    if !repo.comment.is_empty() {
        for line in repo.comment.lines() {
            writeln!(w, "#{line}")?;
        }
    }

    if !repo.enabled {
        write!(w, "# ")?;
    }

    write!(w, "{} ", repo.types[0])?;

    if !repo.options.is_empty() {
        write!(w, "[ ")?;

        for option in repo.options.iter() {
            let (key, value) = (&option.key, option.values.join(","));
            write!(w, "{} ", quote_for_one_line(&format!("{key}={value}")))?;
        }

        write!(w, "] ")?;
    };

    write!(w, "{} ", quote_for_one_line(&repo.uris[0]))?;
    write!(w, "{} ", quote_for_one_line(&repo.suites[0]))?;

    let components = repo
        .components
        .iter()
        .map(|comp| quote_for_one_line(comp))
        .collect::<Vec<String>>()
        .join(" ");
    writeln!(w, "{components}")?;

    Ok(())
}

/// Writes a single stanza.
///
/// Expects that `repo.file_type == APTRepositoryFileType::Sources`.
fn write_stanza(repo: &APTRepository, w: &mut dyn Write) -> Result<(), Error> {
    if repo.file_type != APTRepositoryFileType::Sources {
        bail!("not a .sources repository");
    }

    if !repo.comment.is_empty() {
        for line in repo.comment.lines() {
            writeln!(w, "#{line}")?;
        }
    }

    write!(w, "Types:")?;
    repo.types
        .iter()
        .try_for_each(|package_type| write!(w, " {package_type}"))?;
    writeln!(w)?;

    writeln!(w, "URIs: {}", repo.uris.join(" "))?;
    writeln!(w, "Suites: {}", repo.suites.join(" "))?;

    if !repo.components.is_empty() {
        writeln!(w, "Components: {}", repo.components.join(" "))?;
    }

    for option in repo.options.iter() {
        writeln!(w, "{}: {}", option.key, option.values.join(" "))?;
    }

    Ok(())
}

#[test]
fn test_uri_to_filename() {
    let filename = uri_to_filename("https://some_host/some/path");
    assert_eq!(filename, "some%5fhost_some_path".to_string());
}
