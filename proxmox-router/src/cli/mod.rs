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
#[deprecated = "use proxmox_log::init_cli_logger instead"]
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
    #[allow(clippy::should_implement_trait)]
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

    /// Register options shared by all subcommands in this group.
    ///
    /// Can be called multiple times with different option types. The parameters are shown in help
    /// output as "Inherited group parameters" and can be placed anywhere on the command line
    /// (before or after the subcommand name).
    ///
    /// Note: global options are only processed by the [`CommandLine`] parser. The legacy
    /// [`run_cli_command`] path ignores them.
    pub fn global_option(mut self, opts: GlobalOptions) -> Self {
        if self.global_options.insert(opts.type_id, opts).is_some() {
            panic!("cannot add same option struct multiple times to command line interface");
        }
        self
    }

    /// Finish building and convert into a [`CommandLineInterface`].
    ///
    /// Shorthand for `.into()`.
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

/// Options that apply to an entire command group and all its subcommands.
///
/// Registered via [`CliCommandMap::global_option`], these parameters are parsed before subcommand
/// dispatch and shown under "Inherited group parameters" in `--help` output.
///
/// ```ignore
/// use proxmox_router::cli::{CliCommandMap, GlobalOptions};
/// use proxmox_schema::{api, ApiType};
///
/// #[api]
/// /// Options shared by all subcommands.
/// struct SharedArgs {
///     /// Path to config file.
///     #[serde(default)]
///     config: Option<String>,
/// }
///
/// let cmd = CliCommandMap::new()
///     .insert_help()
///     .global_option(GlobalOptions::of::<SharedArgs>())
///     .insert("sub1", sub1_cmd)
///     .insert("sub2", sub2_cmd);
/// ```
///
/// After parsing, retrieve the values via [`CliEnvironment::take_global_option`].
pub struct GlobalOptions {
    type_id: TypeId,
    schema: &'static Schema,
    parse: fn(env: &mut CliEnvironment, &mut HashMap<String, String>) -> Result<(), Error>,
    completion_functions: HashMap<String, CompletionFunction>,
}

impl GlobalOptions {
    /// Create a global option set from an API type.
    ///
    /// The type's schema properties become CLI parameters that are accepted at the command-group
    /// level and passed through to all subcommands.
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

/// CLI parser with support for global (group-level) options.
///
/// Unlike the legacy [`run_cli_command`] helper, this parser correctly handles
/// [`GlobalOptions`] registered on [`CliCommandMap`] nodes. Use [`CommandLine::parse`]
/// followed by [`Invocation::call`] to inspect global options between parsing and
/// command execution.
///
/// # Example
///
/// ```ignore
/// let cli = CommandLine::new(cmd_def).with_async(|f| proxmox_async::runtime::main(f));
/// let mut rpcenv = CliEnvironment::new();
/// let invocation = cli.parse(&mut rpcenv, std::env::args())?;
///
/// let globals: MyGlobalArgs = rpcenv.take_global_option().unwrap_or_default();
/// setup_logging(globals.verbose);
///
/// invocation.call(&mut rpcenv)?;
/// ```
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
    /// Create a new CLI parser for the given command interface.
    pub fn new(interface: CommandLineInterface) -> Self {
        Self {
            interface: Arc::new(interface),
            async_run: None,
        }
    }

    /// Set the executor for async API handlers.
    ///
    /// Required when any subcommand uses `ApiHandler::Async`. Typically:
    /// ```ignore
    /// .with_async(|future| proxmox_async::runtime::main(future))
    /// ```
    pub fn with_async(mut self, async_run: fn(ApiFuture) -> Result<Value, Error>) -> Self {
        self.async_run = Some(async_run);
        self
    }

    /// Parse the command line and return an [`Invocation`] without executing it.
    ///
    /// After parsing, global options are available in `rpcenv` via
    /// [`CliEnvironment::global_option`] or [`CliEnvironment::take_global_option`].
    /// Call [`Invocation::call`] to execute the resolved command handler.
    pub fn parse<A>(&self, rpcenv: &mut CliEnvironment, args: A) -> Result<Invocation<'_>, Error>
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
    fn handle_current_global_options(
        &mut self,
        args: Vec<String>,
        needs_subcommand: bool,
    ) -> Result<Vec<String>, Error> {
        let mut global_args = Vec::new();
        let args = getopts::ParseOptions::new(&mut global_args, &self.global_option_schemas)
            .deny_unknown(needs_subcommand)
            .stop_at_positional(needs_subcommand)
            .retain_unknown(!needs_subcommand)
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

        let mut args = self.handle_current_global_options(args, true)?;

        // now deal with the actual subcommand list
        if args.is_empty() {
            let mut cmds: Vec<&str> = cli.commands.keys().map(|s| s.as_str()).collect();
            cmds.sort();
            let list = cmds.join(", ");

            let err_msg = format!("no command specified.\nPossible commands: {list}");
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
        let args = self.handle_current_global_options(args, false)?;
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

/// A parsed command line ready for execution.
///
/// Returned by [`CommandLine::parse`]. Call [`Invocation::call`] to run the resolved
/// handler. Between parsing and calling, you can inspect or modify the [`CliEnvironment`]
/// (for example, to extract global options).
pub struct Invocation<'cli> {
    call: InvocationFn<'cli>,
}

impl Invocation<'_> {
    /// Execute the parsed command handler.
    pub fn call(self, rpcenv: &mut CliEnvironment) -> Result<(), Error> {
        (self.call)(rpcenv)
    }
}
