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

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::Arc;

use anyhow::{bail, format_err, Error};
use serde::Deserialize;
use serde_json::Value;

use proxmox_schema::{ApiType, Schema};

use crate::{ApiFuture, ApiMethod};

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

    /// A set of options common to all subcommands. Only object schemas can be used here.
    pub(crate) global_options: HashMap<TypeId, GlobalOptions>,
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

    /// Builder style method to set extra options for the entire set of subcommands.
    /// Can be used multiple times.
    pub fn global_option(mut self, opts: GlobalOptions) -> Self {
        if self.global_options.insert(opts.type_id, opts).is_some() {
            panic!("cannot add same option struct multiple times to command line interface");
        }
        self
    }

    /// Builder style method to set extra options for the entire set of subcommands, taking a
    /// prepared `GlobalOptions` for potential
    /// Can be used multiple times.

    /// Finish the command line interface.
    pub fn build(self) -> CommandLineInterface {
        self.into()
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

/// Options covering an entire hierarchy set of subcommands.
pub struct GlobalOptions {
    type_id: TypeId,
    schema: &'static Schema,
    parse: fn(env: &mut CliEnvironment, &mut HashMap<String, String>) -> Result<(), Error>,
    completion_functions: HashMap<String, CompletionFunction>,
}

impl GlobalOptions {
    /// Get an entry for an API type `T`.
    pub fn of<T>() -> Self
    where
        T: Send + Sync + Any + ApiType + for<'a> Deserialize<'a>,
    {
        return Self {
            type_id: TypeId::of::<T>(),
            schema: &T::API_SCHEMA,
            parse: parse_option_entry::<T>,
            completion_functions: HashMap::new(),
        };

        /// Extract known parameters from the current argument hash and store the parsed `T` in the
        /// `CliEnvironment`'s extra args.
        fn parse_option_entry<T>(
            env: &mut CliEnvironment,
            args: &mut HashMap<String, String>,
        ) -> Result<(), Error>
        where
            T: Send + Sync + Any + ApiType + for<'a> Deserialize<'a>,
        {
            let schema: proxmox_schema::ParameterSchema = match &T::API_SCHEMA {
                Schema::Object(s) => s.into(),
                Schema::AllOf(s) => s.into(),
                // FIXME: ParameterSchema should impl `TryFrom<&'static Schema>`
                _ => panic!("non-object schema in command line interface"),
            };

            let mut params = Vec::new();
            for (name, _optional, _schema) in T::API_SCHEMA
                .any_object()
                .expect("non-object schema in command line interface")
                .properties()
            {
                let name = *name;
                if let Some(value) = args.remove(name) {
                    params.push((name.to_string(), value));
                }
            }
            let value = schema.parse_parameter_strings(&params, true)?;

            let value: T = serde_json::from_value(value)?;

            env.global_options
                .insert(TypeId::of::<T>(), Box::new(value));
            Ok(())
        }
    }

    /// Set completion functions.
    pub fn completion_cb(mut self, param_name: &str, cb: CompletionFunction) -> Self {
        self.completion_functions.insert(param_name.into(), cb);
        self
    }

    /// Get an `Iterator` over the properties of `T`.
    fn properties(&self) -> impl Iterator<Item = (&'static str, &'static Schema)> {
        self.schema
            .any_object()
            .expect("non-object schema in command line interface")
            .properties()
            .map(|(name, _optional, schema)| (*name, *schema))
    }
}

pub struct CommandLine {
    interface: Arc<CommandLineInterface>,
    async_run: Option<fn(ApiFuture) -> Result<Value, Error>>,
}

struct CommandLineParseState<'cli> {
    prefix: String,
    global_option_schemas: HashMap<&'static str, &'static Schema>,
    global_option_values: HashMap<String, String>,
    global_option_types: HashMap<TypeId, &'cli GlobalOptions>,
    async_run: Option<fn(ApiFuture) -> Result<Value, Error>>,
    interface: Arc<CommandLineInterface>,
}

impl CommandLine {
    pub fn new(interface: CommandLineInterface) -> Self {
        Self {
            interface: Arc::new(interface),
            async_run: None,
        }
    }

    pub fn with_async(mut self, async_run: fn(ApiFuture) -> Result<Value, Error>) -> Self {
        self.async_run = Some(async_run);
        self
    }

    pub fn parse<A>(&self, rpcenv: &mut CliEnvironment, args: A) -> Result<Invocation, Error>
    where
        A: IntoIterator<Item = String>,
    {
        let (prefix, args) = command::prepare_cli_command(&self.interface, args.into_iter());

        let state = CommandLineParseState {
            prefix,
            global_option_schemas: HashMap::new(),
            global_option_values: HashMap::new(),
            global_option_types: HashMap::new(),
            async_run: self.async_run,
            interface: Arc::clone(&self.interface),
        };

        state.parse_do(&self.interface, rpcenv, args)
    }
}

impl<'cli> CommandLineParseState<'cli> {
    fn parse_do(
        self,
        cli: &'cli CommandLineInterface,
        rpcenv: &mut CliEnvironment,
        args: Vec<String>,
    ) -> Result<Invocation<'cli>, Error> {
        match cli {
            CommandLineInterface::Simple(cli) => self.parse_simple(cli, rpcenv, args),
            CommandLineInterface::Nested(cli) => self.parse_nested(cli, rpcenv, args),
        }
    }

    /// Parse out the current global options and return the remaining `args`.
    fn handle_current_global_options(&mut self, args: Vec<String>) -> Result<Vec<String>, Error> {
        let mut global_args = Vec::new();
        let args = getopts::ParseOptions::new(&mut global_args, &self.global_option_schemas)
            .stop_at_positional(true)
            .deny_unknown(true)
            .parse(args)?;
        // and merge them into the hash map
        for (option, argument) in global_args {
            self.global_option_values.insert(option, argument);
        }

        Ok(args)
    }

    /// Enable the current global options to be recognized by the argument parser.
    fn enable_global_options(&mut self, cli: &'cli CliCommandMap) {
        for entry in cli.global_options.values() {
            self.global_option_types
                .extend(cli.global_options.iter().map(|(id, entry)| (*id, entry)));
            for (name, schema) in entry.properties() {
                if self.global_option_schemas.insert(name, schema).is_some() {
                    panic!(
                        "duplicate option {name:?} in nested command line interface global options"
                    );
                }
            }
        }
    }

    fn parse_nested(
        mut self,
        cli: &'cli CliCommandMap,
        rpcenv: &mut CliEnvironment,
        mut args: Vec<String>,
    ) -> Result<Invocation<'cli>, Error> {
        use std::fmt::Write as _;

        command::replace_aliases(&mut args, &cli.aliases);

        self.enable_global_options(cli);

        let mut args = self.handle_current_global_options(args)?;

        // now deal with the actual subcommand list
        if args.is_empty() {
            let mut cmds: Vec<&str> = cli.commands.keys().map(|s| s.as_str()).collect();
            cmds.sort();
            let list = cmds.join(", ");

            let err_msg = format!("no command specified.\nPossible commands: {}", list);
            print_nested_usage_error(&self.prefix, cli, &err_msg);
            return Err(format_err!("{}", err_msg));
        }

        let (_, sub_cmd) = match cli.find_command(&args[0]) {
            Some(cmd) => cmd,
            None => {
                let err_msg = format!("no such command '{}'", args[0]);
                print_nested_usage_error(&self.prefix, cli, &err_msg);
                return Err(format_err!("{}", err_msg));
            }
        };

        let _ = write!(&mut self.prefix, " {}", args.remove(0));

        self.parse_do(sub_cmd, rpcenv, args)
    }

    fn parse_simple(
        mut self,
        cli: &'cli CliCommand,
        rpcenv: &mut CliEnvironment,
        args: Vec<String>,
    ) -> Result<Invocation<'cli>, Error> {
        let args = self.handle_current_global_options(args)?;
        self.build_global_options(&mut *rpcenv)?;
        let interface = Arc::clone(&self.interface);
        Ok(Invocation {
            call: Box::new(move |rpcenv| {
                command::set_help_context(Some(interface));
                let out = command::handle_simple_command(
                    &self.prefix,
                    cli,
                    args,
                    rpcenv,
                    self.async_run,
                    self.global_option_types.values().copied(),
                );
                command::set_help_context(None);
                out
            }),
        })
    }

    fn build_global_options(&mut self, env: &mut CliEnvironment) -> Result<(), Error> {
        for entry in self.global_option_types.values() {
            (entry.parse)(env, &mut self.global_option_values)?;
        }

        Ok(())
    }
}

type InvocationFn<'cli> =
    Box<dyn FnOnce(&mut CliEnvironment) -> Result<(), Error> + Send + Sync + 'cli>;

/// After parsing the command line, this is responsible for calling the API method with its
/// parameters, and gives the user a chance to adapt the RPC environment before doing so.
pub struct Invocation<'cli> {
    call: InvocationFn<'cli>,
}

impl Invocation<'_> {
    pub fn call(self, rpcenv: &mut CliEnvironment) -> Result<(), Error> {
        (self.call)(rpcenv)
    }
}
