use std::convert::TryInto;
use std::io::BufRead;
use std::iter::Iterator;

use anyhow::{bail, format_err, Error};

use crate::repositories::{APTRepository, APTRepositoryFileType, APTRepositoryOption};

use super::APTRepositoryParser;

// TODO convert %-escape characters. Also adapt printing back accordingly,
// because at least '%' needs to be re-escaped when printing.
/// See APT's ParseQuoteWord in contrib/strutl.cc
///
/// Doesn't split on whitespace when between `[]` or `""` and strips `"` from the word.
///
/// Currently, %-escaped characters are not interpreted, but passed along as is.
struct SplitQuoteWord {
    rest: String,
    position: usize,
}

impl SplitQuoteWord {
    pub fn new(string: String) -> Self {
        Self {
            rest: string,
            position: 0,
        }
    }
}

impl Iterator for SplitQuoteWord {
    type Item = Result<String, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        let rest = &self.rest[self.position..];

        let mut start = None;
        let mut wait_for = None;

        for (n, c) in rest.chars().enumerate() {
            self.position += 1;

            if let Some(wait_for_char) = wait_for {
                if wait_for_char == c {
                    wait_for = None;
                }
                continue;
            }

            if char::is_ascii_whitespace(&c) {
                if let Some(start) = start {
                    return Some(Ok(rest[start..n].replace('"', "")));
                }
                continue;
            }

            if start == None {
                start = Some(n);
            }

            if c == '"' {
                wait_for = Some('"');
            }

            if c == '[' {
                wait_for = Some(']');
            }
        }

        if let Some(wait_for) = wait_for {
            return Some(Err(format_err!("missing terminating '{}'", wait_for)));
        }

        if let Some(start) = start {
            return Some(Ok(rest[start..].replace('"', "")));
        }

        None
    }
}

pub struct APTListFileParser<R: BufRead> {
    input: R,
    line_nr: usize,
    comment: String,
}

impl<R: BufRead> APTListFileParser<R> {
    pub fn new(reader: R) -> Self {
        Self {
            input: reader,
            line_nr: 0,
            comment: String::new(),
        }
    }

    /// Helper to parse options from the existing token stream.
    ///
    /// Also returns `Ok(())` if there are no options.
    ///
    /// Errors when options are invalid or not closed by `']'`.
    fn parse_options(
        options: &mut Vec<APTRepositoryOption>,
        tokens: &mut SplitQuoteWord,
    ) -> Result<(), Error> {
        let mut finished = false;

        loop {
            let mut option = match tokens.next() {
                Some(token) => token?,
                None => bail!("options not closed by ']'"),
            };

            if let Some(stripped) = option.strip_suffix(']') {
                option = stripped.to_string();
                if option.is_empty() {
                    break;
                }
                finished = true; // but still need to handle the last one
            };

            if let Some(mid) = option.find('=') {
                let (key, mut value_str) = option.split_at(mid);
                value_str = &value_str[1..];

                if key.is_empty() {
                    bail!("option has no key: '{}'", option);
                }

                if value_str.is_empty() {
                    bail!("option has no value: '{}'", option);
                }

                let values: Vec<String> = value_str
                    .split(',')
                    .map(|value| value.to_string())
                    .collect();

                options.push(APTRepositoryOption {
                    key: key.to_string(),
                    values,
                });
            } else if !option.is_empty() {
                bail!("got invalid option - '{}'", option);
            }

            if finished {
                break;
            }
        }

        Ok(())
    }

    /// Parse a repository or comment in one-line format.
    ///
    /// Commented out repositories are also detected and returned with the
    /// `enabled` property set to `false`.
    ///
    /// If the line contains a repository, `self.comment` is added to the
    /// `comment` property.
    ///
    /// If the line contains a comment, it is added to `self.comment`.
    fn parse_one_line(&mut self, mut line: &str) -> Result<Option<APTRepository>, Error> {
        line = line.trim_matches(|c| char::is_ascii_whitespace(&c));

        // check for commented out repository first
        if let Some(commented_out) = line.strip_prefix('#') {
            if let Ok(Some(mut repo)) = self.parse_one_line(commented_out) {
                repo.set_enabled(false);
                return Ok(Some(repo));
            }
        }

        let mut repo = APTRepository::new(APTRepositoryFileType::List);

        // now handle "real" comment
        if let Some(comment_start) = line.find('#') {
            let (line_start, comment) = line.split_at(comment_start);
            self.comment = format!("{}{}\n", self.comment, &comment[1..]);
            line = line_start;
        }

        // e.g. quoted "deb" is not accepted by APT, so no need for quote word parsing here
        line = match line.split_once(|c| char::is_ascii_whitespace(&c)) {
            Some((package_type, rest)) => {
                repo.types.push(package_type.try_into()?);
                rest
            }
            None => return Ok(None), // empty line
        };

        line = line.trim_start_matches(|c| char::is_ascii_whitespace(&c));

        let has_options = match line.strip_prefix('[') {
            Some(rest) => {
                // avoid the start of the options to be interpreted as the start of a quote word
                line = rest;
                true
            }
            None => false,
        };

        let mut tokens = SplitQuoteWord::new(line.to_string());

        if has_options {
            Self::parse_options(&mut repo.options, &mut tokens)?;
        }

        // the rest of the line is just '<uri> <suite> [<components>...]'
        repo.uris
            .push(tokens.next().ok_or_else(|| format_err!("missing URI"))??);
        repo.suites.push(
            tokens
                .next()
                .ok_or_else(|| format_err!("missing suite"))??,
        );
        for token in tokens {
            repo.components.push(token?);
        }

        repo.comment = std::mem::take(&mut self.comment);

        Ok(Some(repo))
    }
}

impl<R: BufRead> APTRepositoryParser for APTListFileParser<R> {
    fn parse_repositories(&mut self) -> Result<Vec<APTRepository>, Error> {
        let mut repos = vec![];
        let mut line = String::new();

        loop {
            self.line_nr += 1;
            line.clear();

            match self.input.read_line(&mut line) {
                Err(err) => bail!("input error - {}", err),
                Ok(0) => break,
                Ok(_) => match self.parse_one_line(&line) {
                    Ok(Some(repo)) => repos.push(repo),
                    Ok(None) => continue,
                    Err(err) => bail!("malformed entry on line {} - {}", self.line_nr, err),
                },
            }
        }

        Ok(repos)
    }
}
