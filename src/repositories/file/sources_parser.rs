use std::convert::TryInto;
use std::io::BufRead;
use std::iter::Iterator;

use anyhow::{bail, Error};

use crate::repositories::{
    APTRepository, APTRepositoryFileType, APTRepositoryOption, APTRepositoryPackageType,
};

use super::APTRepositoryParser;

pub struct APTSourcesFileParser<R: BufRead> {
    input: R,
    stanza_nr: usize,
    comment: String,
}

/// See `man sources.list` and `man deb822` for the format specification.
impl<R: BufRead> APTSourcesFileParser<R> {
    pub fn new(reader: R) -> Self {
        Self {
            input: reader,
            stanza_nr: 1,
            comment: String::new(),
        }
    }

    /// Based on APT's `StringToBool` in `strutl.cc`
    fn string_to_bool(string: &str, default: bool) -> bool {
        let string = string.trim_matches(|c| char::is_ascii_whitespace(&c));
        let string = string.to_lowercase();

        match &string[..] {
            "1" | "yes" | "true" | "with" | "on" | "enable" => true,
            "0" | "no" | "false" | "without" | "off" | "disable" => false,
            _ => default,
        }
    }

    /// Checks if `key` is valid according to deb822
    fn valid_key(key: &str) -> bool {
        if key.starts_with('-') {
            return false;
        };
        return key.chars().all(|c| matches!(c, '!'..='9' | ';'..='~'));
    }

    /// Try parsing a repository in stanza format from `lines`.
    ///
    /// Returns `Ok(None)` when no stanza can be found.
    ///
    /// Comments are added to `self.comments`. If a stanza can be found,
    /// `self.comment` is added to the repository's `comment` property.
    ///
    /// Fully commented out stanzas are treated as comments.
    fn parse_stanza(&mut self, lines: &str) -> Result<Option<APTRepository>, Error> {
        let mut repo = APTRepository::new(APTRepositoryFileType::Sources);

        // Values may be folded into multiple lines.
        // Those lines have to start with a space or a tab.
        let lines = lines.replace("\n ", " ");
        let lines = lines.replace("\n\t", " ");

        let mut got_something = false;

        for line in lines.lines() {
            let line = line.trim_matches(|c| char::is_ascii_whitespace(&c));
            if line.is_empty() {
                continue;
            }

            if let Some(commented_out) = line.strip_prefix('#') {
                self.comment = format!("{}{}\n", self.comment, commented_out);
                continue;
            }

            if let Some(mid) = line.find(':') {
                let (key, value_str) = line.split_at(mid);
                let value_str = &value_str[1..];
                let key = key.trim_matches(|c| char::is_ascii_whitespace(&c));

                if key.is_empty() {
                    bail!("option has no key: '{}'", line);
                }

                if value_str.is_empty() {
                    // ignored by APT
                    eprintln!("option has no value: '{}'", line);
                    continue;
                }

                if !Self::valid_key(key) {
                    // ignored by APT
                    eprintln!("option with invalid key '{}'", key);
                    continue;
                }

                let values: Vec<String> = value_str
                    .split_ascii_whitespace()
                    .map(|value| value.to_string())
                    .collect();

                match &key.to_lowercase()[..] {
                    "types" => {
                        if !repo.types.is_empty() {
                            eprintln!("key 'Types' was defined twice");
                        }
                        let mut types = Vec::<APTRepositoryPackageType>::new();
                        for package_type in values {
                            types.push((&package_type[..]).try_into()?);
                        }
                        repo.types = types;
                    }
                    "uris" => {
                        if !repo.uris.is_empty() {
                            eprintln!("key 'URIs' was defined twice");
                        }
                        repo.uris = values;
                    }
                    "suites" => {
                        if !repo.suites.is_empty() {
                            eprintln!("key 'Suites' was defined twice");
                        }
                        repo.suites = values;
                    }
                    "components" => {
                        if !repo.components.is_empty() {
                            eprintln!("key 'Components' was defined twice");
                        }
                        repo.components = values;
                    }
                    "enabled" => {
                        repo.set_enabled(Self::string_to_bool(value_str, true));
                    }
                    _ => repo.options.push(APTRepositoryOption {
                        key: key.to_string(),
                        values,
                    }),
                }
            } else {
                bail!("got invalid line - '{:?}'", line);
            }

            got_something = true;
        }

        if !got_something {
            return Ok(None);
        }

        repo.comment = std::mem::take(&mut self.comment);

        Ok(Some(repo))
    }

    /// Helper function for `parse_repositories`.
    fn try_parse_stanza(
        &mut self,
        lines: &str,
        repos: &mut Vec<APTRepository>,
    ) -> Result<(), Error> {
        match self.parse_stanza(lines) {
            Ok(Some(repo)) => {
                repos.push(repo);
                self.stanza_nr += 1;
            }
            Ok(None) => (),
            Err(err) => bail!("malformed entry in stanza {} - {}", self.stanza_nr, err),
        }

        Ok(())
    }
}

impl<R: BufRead> APTRepositoryParser for APTSourcesFileParser<R> {
    fn parse_repositories(&mut self) -> Result<Vec<APTRepository>, Error> {
        let mut repos = vec![];
        let mut lines = String::new();

        loop {
            let old_length = lines.len();
            match self.input.read_line(&mut lines) {
                Err(err) => bail!("input error - {}", err),
                Ok(0) => {
                    self.try_parse_stanza(&lines[..], &mut repos)?;
                    break;
                }
                Ok(_) => {
                    if (&lines[old_length..])
                        .trim_matches(|c| char::is_ascii_whitespace(&c))
                        .is_empty()
                    {
                        // detected end of stanza
                        self.try_parse_stanza(&lines[..], &mut repos)?;
                        lines.clear();
                    }
                }
            }
        }

        Ok(repos)
    }
}
