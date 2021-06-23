use std::convert::TryInto;
use std::io::BufRead;
use std::iter::{Iterator, Peekable};
use std::str::SplitAsciiWhitespace;

use anyhow::{bail, format_err, Error};

use crate::repositories::{APTRepository, APTRepositoryFileType, APTRepositoryOption};

use super::APTRepositoryParser;

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
        tokens: &mut Peekable<SplitAsciiWhitespace>,
    ) -> Result<(), Error> {
        let mut option = match tokens.peek() {
            Some(token) => {
                match token.strip_prefix('[') {
                    Some(option) => option,
                    None => return Ok(()), // doesn't look like options
                }
            }
            None => return Ok(()),
        };

        tokens.next(); // avoid reading the beginning twice

        let mut finished = false;
        loop {
            if let Some(stripped) = option.strip_suffix(']') {
                option = stripped;
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

            option = match tokens.next() {
                Some(option) => option,
                None => bail!("options not closed by ']'"),
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

        let mut tokens = line.split_ascii_whitespace().peekable();

        match tokens.next() {
            Some(package_type) => {
                repo.types.push(package_type.try_into()?);
            }
            None => return Ok(None), // empty line
        }

        Self::parse_options(&mut repo.options, &mut tokens)?;

        // the rest of the line is just '<uri> <suite> [<components>...]'
        let mut tokens = tokens.map(str::to_string);
        repo.uris
            .push(tokens.next().ok_or_else(|| format_err!("missing URI"))?);
        repo.suites
            .push(tokens.next().ok_or_else(|| format_err!("missing suite"))?);
        repo.components.extend(tokens);

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
