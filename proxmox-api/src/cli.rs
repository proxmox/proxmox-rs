//! Provides Command Line Interface to API methods

use std::collections::HashMap;
use std::str::FromStr;

use failure::{bail, format_err, Error};
use serde::Serialize;
use serde_json::Value;

use super::{ApiMethodInfo, ApiOutput, Parameter};

type MethodInfoRef<Body> = &'static (dyn ApiMethodInfo<Body> + Send + Sync);

/// A CLI root node.
pub struct App<Body: 'static> {
    name: &'static str,
    command: Option<Command<Body>>,
}

impl<Body: 'static> App<Body> {
    /// Create a new empty App instance.
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            command: None,
        }
    }

    /// Directly connect this instance to a single API method.
    ///
    /// This is a builder method and will panic if there's already a method registered!
    pub fn method(mut self, method: Method<Body>) -> Self {
        assert!(
            self.command.is_none(),
            "app {} already has a comman!",
            self.name
        );

        self.command = Some(Command::Method(method));
        self
    }

    /// Add a subcommand to this instance.
    ///
    /// This is a builder method and will panic if the subcommand already exists or no subcommands
    /// may be added.
    pub fn subcommand(mut self, name: &'static str, subcommand: Command<Body>) -> Self {
        match self
            .command
            .get_or_insert_with(|| Command::SubCommands(SubCommands::new()))
        {
            Command::SubCommands(ref mut commands) => {
                commands.add_subcommand(name, subcommand);
                self
            }
            _ => panic!("app {} cannot have subcommands!", self.name),
        }
    }

    /// Resolve a list of parameters to a method and a parameter json value.
    pub fn resolve(&self, args: &[&str]) -> Result<(MethodInfoRef<Body>, Value), Error> {
        self.command
            .as_ref()
            .ok_or_else(|| format_err!("no commands available"))?
            .resolve(args.iter())
    }

    /// Run a command through this command line interface.
    pub fn run(&self, args: &[&str]) -> ApiOutput<Body> {
        let (method, params) = self.resolve(args)?;
        let handler = method.handler();
        futures::executor::block_on(handler(params))
    }
}

/// A node in the CLI command router. This is either
pub enum Command<Body: 'static> {
    Method(Method<Body>),
    SubCommands(SubCommands<Body>),
}

impl<Body: 'static> Command<Body> {
    /// Create a Command entry pointing to an API method
    pub fn method(
        method: &'static (dyn ApiMethodInfo<Body> + Send + Sync),
        positional_args: &'static [&'static str],
    ) -> Self {
        Command::Method(Method::new(method, positional_args))
    }

    /// Create a new empty subcommand entry.
    pub fn new() -> Self {
        Command::SubCommands(SubCommands::new())
    }

    fn resolve(&self, args: std::slice::Iter<&str>) -> Result<(MethodInfoRef<Body>, Value), Error> {
        match self {
            Command::Method(method) => method.resolve(args),
            Command::SubCommands(subcmd) => subcmd.resolve(args),
        }
    }
}

pub struct SubCommands<Body: 'static> {
    commands: HashMap<&'static str, Command<Body>>,
}

impl<Body: 'static> SubCommands<Body> {
    /// Create a new empty SubCommands hash.
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    /// Add a subcommand.
    ///
    /// Note that it is illegal for the subcommand to already exist, which will cause a panic.
    pub fn add_subcommand(&mut self, name: &'static str, command: Command<Body>) -> &mut Self {
        let old = self.commands.insert(name, command);
        assert!(old.is_none(), "subcommand '{}' already exists", name);
        self
    }

    /// Builder method to add a subcommand.
    ///
    /// Note that it is illegal for the subcommand to already exist, which will cause a panic.
    pub fn subcommand(mut self, name: &'static str, command: Command<Body>) -> Self {
        self.add_subcommand(name, command);
        self
    }

    fn resolve(
        &self,
        mut args: std::slice::Iter<&str>,
    ) -> Result<(MethodInfoRef<Body>, Value), Error> {
        match args.next() {
            None => bail!("missing subcommand"),
            Some(arg) => match self.commands.get(arg) {
                None => bail!("no such subcommand: {}", arg),
                Some(cmd) => cmd.resolve(args),
            },
        }
    }
}

/// A reference to an API method. Note that when coming from the command line, it is possible to
/// match some parameters as positional parameters rather than argument switches, therefor this
/// contains an ordered list of positional parameters.
///
/// Note that we currently do not support optional positional parameters.
// XXX: If we want optional positional parameters - should we make an enum or just say the
// parameter name should have brackets around it?
pub struct Method<Body: 'static> {
    pub method: MethodInfoRef<Body>,
    pub positional_args: &'static [&'static str],
    //pub formatter: Option<()>, // TODO: output formatter
}

impl<Body: 'static> Method<Body> {
    /// Create a new reference to an API method.
    pub fn new(
        method: &'static (dyn ApiMethodInfo<Body> + Send + Sync),
        positional_args: &'static [&'static str],
    ) -> Self {
        Self {
            method,
            positional_args,
        }
    }

    fn resolve(
        &self,
        mut args: std::slice::Iter<&str>,
    ) -> Result<(MethodInfoRef<Body>, Value), Error> {
        let mut params = serde_json::Map::new();
        let mut positionals = self.positional_args.iter();

        let mut current_option = None;
        loop {
            match next_arg(&mut args) {
                Some(Arg::Opt(arg)) => {
                    if let Some(arg) = current_option {
                        self.add_parameter(&mut params, arg, None)?;
                    }

                    current_option = Some(arg);
                }
                Some(Arg::OptArg(arg, value)) => {
                    if let Some(arg) = current_option {
                        self.add_parameter(&mut params, arg, None)?;
                    }

                    self.add_parameter(&mut params, arg, Some(value))?;
                }
                Some(Arg::Positional(value)) => match current_option {
                    Some(arg) => self.add_parameter(&mut params, arg, Some(value))?,
                    None => match positionals.next() {
                        Some(arg) => self.add_parameter(&mut params, arg, Some(value))?,
                        None => bail!("unexpected positional parameter: '{}'", value),
                    },
                },
                None => {
                    if let Some(arg) = current_option {
                        self.add_parameter(&mut params, arg, None)?;
                    }
                    break;
                }
            }
        }
        assert!(
            current_option.is_none(),
            "current_option must have been dealt with"
        );

        let missing = positionals.fold(String::new(), |mut acc, more| {
            if acc.is_empty() {
                more.to_string()
            } else {
                acc.push_str(", ");
                acc.push_str(more);
                acc
            }
        });
        if !missing.is_empty() {
            bail!("missing positional parameters: {}", missing);
        }

        Ok((self.method, Value::Object(params)))
    }

    /// This should insert the parameter 'arg' with value 'value' into 'params'.
    /// This means we need to verify `arg` exists in self.method, `value` deserializes to its type,
    /// and then serialize it into the Value.
    fn add_parameter(
        &self,
        params: &mut serde_json::Map<String, Value>,
        arg: &str,
        value: Option<&str>,
    ) -> Result<(), Error> {
        let param_def = self
            .find_parameter(arg)
            .ok_or_else(|| format_err!("no such parameter: '{}'", arg))?;
        params.insert(arg.to_string(), param_def.parse_cli(arg, value)?);
        Ok(())
    }

    fn find_parameter(&self, name: &str) -> Option<&Parameter> {
        self.method.parameters().iter().find(|p| p.name == name)
    }
}

enum Arg<'a> {
    Positional(&'a str),
    Opt(&'a str),
    OptArg(&'a str, &'a str),
}

fn next_arg<'a>(args: &mut std::slice::Iter<&'a str>) -> Option<Arg<'a>> {
    args.next().map(|arg| {
        if arg.starts_with("--") {
            let arg = &arg[2..];

            match arg.find('=') {
                Some(idx) => Arg::OptArg(&arg[0..idx], &arg[idx + 1..]),
                None => Arg::Opt(arg),
            }
        } else {
            Arg::Positional(arg)
        }
    })
}

pub fn parse_cli_from_str<T>(name: &str, value: Option<&str>) -> Result<Value, Error>
where
    T: FromStr + Serialize,
    <T as FromStr>::Err: Into<Error>,
{
    let this: T = value
        .ok_or_else(|| format_err!("missing parameter value for '{}'", name))?
        .parse()
        .map_err(|e: <T as FromStr>::Err| e.into())?;
    Ok(serde_json::to_value(this)?)
}

/// We use this trait so we can keep the "mass implementation macro" for the ApiType trait simple
/// and specialize the CLI parameter parsing via this trait separately.
pub trait ParseCli {
    fn parse_cli(name: &str, value: Option<&str>) -> Result<Value, Error>;
}

/// This is a version of ParseCli with a default implementation falling to FromStr.
pub trait ParseCliFromStr
where
    Self: FromStr + Serialize,
    <Self as FromStr>::Err: Into<Error>,
{
    fn parse_cli(name: &str, value: Option<&str>) -> Result<Value, Error> {
        parse_cli_from_str::<Self>(name, value)
    }
}

impl<T> ParseCliFromStr for T
where
    T: FromStr + Serialize,
    <T as FromStr>::Err: Into<Error>,
{
}

#[macro_export]
macro_rules! no_cli_type {
    ($type:ty $(, $more:ty)*) => {
        impl $crate::cli::ParseCli for $type {
            fn parse_cli(name: &str, _value: Option<&str>) -> Result<Value, Error> {
                bail!(
                    "invalid type for command line interface found for parameter '{}'",
                    name
                );
            }
        }

        $crate::impl_parse_cli_from_str!{$($more),*}
    };
    () => {};
}

no_cli_type! {Vec<String>}

#[macro_export]
macro_rules! impl_parse_cli_from_str {
    ($type:ty $(, $more:ty)*) => {
        impl $crate::cli::ParseCli for $type {
            fn parse_cli(name: &str, value: Option<&str>) -> Result<Value, Error> {
                parse_cli_from_str::<$type>(name, value)
            }
        }

        $crate::impl_parse_cli_from_str!{$($more),*}
    };
    () => {};
}

impl_parse_cli_from_str! {bool}
impl_parse_cli_from_str! {isize, usize, i64, u64, i32, u32, i16, u16, i8, u8, f64, f32}

impl ParseCli for Value {
    fn parse_cli(name: &str, _value: Option<&str>) -> Result<Value, Error> {
        // FIXME: we could of course allow generic json parameters...?
        bail!(
            "found generic json parameter ('{}') in command line...",
            name
        );
    }
}

impl ParseCli for &str {
    fn parse_cli(name: &str, value: Option<&str>) -> Result<Value, Error> {
        Ok(Value::String(
            value
                .ok_or_else(|| format_err!("missing value for parameter '{}'", name))?
                .to_string(),
        ))
    }
}

impl ParseCli for String {
    fn parse_cli(name: &str, value: Option<&str>) -> Result<Value, Error> {
        Ok(Value::String(
            value
                .ok_or_else(|| format_err!("missing value for parameter '{}'", name))?
                .to_string(),
        ))
    }
}
