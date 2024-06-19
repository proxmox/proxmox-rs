//! Tools to create command line parsers
//!
//! This crate provides convenient helpers to create command line
//! parsers using Schema definitions.
//!
//! ## Features
//!
//! - Use declarative API schema to define the CLI
//! - Automatic parameter verification
//! - Automatically generate documentation and manual pages
//! - Automatically generate bash completion helpers
//! - Ability to create interactive commands (using ``rustyline``)
//! - Supports complex/nested commands

use std::collections::HashMap;
use std::io::{self, Write};

use anyhow::{bail, Error};

use crate::ApiMethod;

mod environment;
pub use environment::*;

mod shellword;
pub use shellword::*;

mod format;
pub use format::*;

mod text_table;
pub use text_table::*;

mod completion;

mod completion_helpers;
pub use completion_helpers::*;

mod getopts;
pub use getopts::*;

mod command;
pub use command::*;

mod readline;
pub use readline::*;

/// Completion function for single parameters.
///
/// Completion functions gets the current parameter value, and should
/// return a list of all possible values.
pub type CompletionFunction = fn(&str, &HashMap<String, String>) -> Vec<String>;

/// Initialize default logger for CLI binaries
pub fn init_cli_logger(env_var_name: &str, default_log_level: &str) {
    env_logger::Builder::from_env(
        env_logger::Env::new().filter_or(env_var_name, default_log_level),
    )
    .write_style(env_logger::WriteStyle::Never)
    .format_level(false)
    .format_module_path(false)
    .format_target(false)
    .format_timestamp(None)
    .init();
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
/// Use for simple yes or no questions, where booleans can be confusing, especially if there's a
/// default response to consider. The implementation provides query helper for the CLI.
pub enum Confirmation {
    Yes,
    No,
}

impl Confirmation {
    /// Get the formatted choice for the query prompt, with self being the highlighted (default)
    /// one displayed as upper case.
    pub fn default_choice_str(self) -> &'static str {
        match self {
            Self::Yes => "Y/n",
            Self::No => "y/N",
        }
    }

    /// Returns true if the answer is Yes
    pub fn is_yes(self) -> bool {
        self == Self::Yes
    }

    /// Returns true if the answer is No
    pub fn is_no(self) -> bool {
        self == Self::No
    }

    /// Parse an input string reference as yes or no confirmation.
    ///
    /// The input string is checked verbatim if it is exactly one of the single chars 'y', 'Y',
    /// 'n', or 'N'. You must trim the string before calling, if needed, or use one of the query
    /// helper functions.
    ///
    /// ```
    /// use proxmox_router::cli::Confirmation;
    ///
    /// let answer = Confirmation::from_str("y");
    /// assert!(answer.expect("valid").is_yes());
    ///
    /// let answer = Confirmation::from_str("N");
    /// assert!(answer.expect("valid").is_no());
    ///
    /// let answer = Confirmation::from_str("bogus");
    /// assert!(answer.is_err());
    /// ```
    pub fn from_str(input: &str) -> Result<Self, Error> {
        match input.trim() {
            "y" | "Y" => Ok(Self::Yes),
            "n" | "N" => Ok(Self::No),
            _ => bail!("unexpected choice '{input}'! Use 'y' or 'n'"),
        }
    }

    /// Parse a input string reference as yes or no confirmation, allowing a fallback default
    /// answer if the user enters an empty choice.
    ///
    /// The input string is checked verbatim if it is exactly one of the single chars 'y', 'Y',
    /// 'n', or 'N'. The empty string maps to the default. You must trim the string before calling,
    /// if needed, or use one of the query helper functions.
    ///
    /// ```
    /// use proxmox_router::cli::Confirmation;
    ///
    /// let answer = Confirmation::from_str_with_default("", Confirmation::No);
    /// assert!(answer.expect("valid").is_no());
    ///
    /// let answer = Confirmation::from_str_with_default("n", Confirmation::Yes);
    /// assert!(answer.expect("valid").is_no());
    ///
    /// let answer = Confirmation::from_str_with_default("yes", Confirmation::Yes);
    /// assert!(answer.is_err()); // full-word answer not allowed for now.
    /// ```
    pub fn from_str_with_default(input: &str, default: Self) -> Result<Self, Error> {
        match input.trim() {
            "y" | "Y" => Ok(Self::Yes),
            "n" | "N" => Ok(Self::No),
            "" => Ok(default),
            _ => bail!("unexpected choice '{input}'! Use enter for default or use 'y' or 'n'"),
        }
    }

    /// Print a query prompt with available yes no choices and returns the String the user enters.
    fn read_line(query: &str, choices: &str) -> Result<String, io::Error> {
        print!("{query} [{choices}]: ");

        io::stdout().flush()?;
        let stdin = io::stdin();
        let mut line = String::new();
        stdin.read_line(&mut line)?;
        Ok(line)
    }

    /// Print a query prompt and parse the white-space trimmed answer using `from_str`.
    pub fn query(query: &str) -> Result<Self, Error> {
        let line = Self::read_line(query, "y/n")?;
        Confirmation::from_str(line.trim())
    }

    /// Print a query prompt and parse the answer using `from_str_with_default`, falling back to the
    /// default_answer if the user provided an empty string.
    pub fn query_with_default(query: &str, default_answer: Self) -> Result<Self, Error> {
        let line = Self::read_line(query, default_answer.default_choice_str())?;
        Confirmation::from_str_with_default(line.trim(), default_answer)
    }
}

/// Define a simple CLI command.
pub struct CliCommand {
    /// The Schema definition.
    pub info: &'static ApiMethod,
    /// Argument parameter list.
    ///
    /// Those parameters are expected to be passed as command line
    /// arguments in the specified order. All other parameters needs
    /// to be specified as ``--option <value>`` pairs.
    pub arg_param: &'static [&'static str],
    /// Predefined parameters.
    pub fixed_param: HashMap<&'static str, String>,
    /// Completion functions.
    ///
    /// Each parameter may have an associated completion function,
    /// which is called by the shell completion handler.
    pub completion_functions: HashMap<String, CompletionFunction>,
}

impl CliCommand {
    /// Create a new instance.
    pub fn new(info: &'static ApiMethod) -> Self {
        Self {
            info,
            arg_param: &[],
            fixed_param: HashMap::new(),
            completion_functions: HashMap::new(),
        }
    }

    /// Set argument parameter list.
    pub fn arg_param(mut self, names: &'static [&'static str]) -> Self {
        self.arg_param = names;
        self
    }

    /// Set fixed parameters.
    pub fn fixed_param(mut self, key: &'static str, value: String) -> Self {
        self.fixed_param.insert(key, value);
        self
    }

    /// Set completion functions.
    pub fn completion_cb(mut self, param_name: &str, cb: CompletionFunction) -> Self {
        self.completion_functions.insert(param_name.into(), cb);
        self
    }
}

/// Define nested CLI commands.
#[derive(Default)]
pub struct CliCommandMap {
    /// Each command has an unique name. The map associates names with
    /// command definitions.
    pub commands: HashMap<String, CommandLineInterface>,
    pub aliases: Vec<(Vec<&'static str>, Vec<&'static str>)>,
    /// List of options to suppress in generate_usage
    pub usage_skip_options: &'static [&'static str],
}

impl CliCommandMap {
    /// Create a new instance.
    pub fn new() -> Self {
        Default::default()
    }

    /// Insert another command.
    pub fn insert<C: Into<CommandLineInterface>>(mut self, name: &'static str, cli: C) -> Self {
        self.commands.insert(name.into(), cli.into());
        self
    }

    pub fn alias(mut self, old: &'static [&'static str], new: &'static [&'static str]) -> Self {
        self.aliases.push((Vec::from(old), Vec::from(new)));
        self
    }

    pub fn usage_skip_options(mut self, list: &'static [&'static str]) -> Self {
        self.usage_skip_options = list;
        self
    }

    /// Insert the help command.
    pub fn insert_help(mut self) -> Self {
        self.commands
            .insert(String::from("help"), help_command_def().into());
        self
    }

    fn find_command(&self, name: &str) -> Option<(String, &CommandLineInterface)> {
        if let Some(sub_cmd) = self.commands.get(name) {
            return Some((name.to_string(), sub_cmd));
        };

        let mut matches: Vec<&str> = vec![];

        for cmd in self.commands.keys() {
            if cmd.starts_with(name) {
                matches.push(cmd);
            }
        }

        if matches.len() != 1 {
            return None;
        }

        if let Some(sub_cmd) = self.commands.get(matches[0]) {
            return Some((matches[0].to_string(), sub_cmd));
        };

        None
    }
}

/// Define Complex command line interfaces.
pub enum CommandLineInterface {
    Simple(CliCommand),
    Nested(CliCommandMap),
}

impl From<CliCommand> for CommandLineInterface {
    fn from(cli_cmd: CliCommand) -> Self {
        CommandLineInterface::Simple(cli_cmd)
    }
}

impl From<CliCommandMap> for CommandLineInterface {
    fn from(list: CliCommandMap) -> Self {
        CommandLineInterface::Nested(list)
    }
}
