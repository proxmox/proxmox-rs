//! Provides Command Line Interface to API methods

use std::collections::HashMap;

use super::ApiMethodInfo;

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
}

/// A reference to an API method. Note that when coming from the command line, it is possible to
/// match some parameters as positional parameters rather than argument switches, therefor this
/// contains an ordered list of positional parameters.
///
/// Note that we currently do not support optional positional parameters.
// XXX: If we want optional positional parameters - should we make an enum or just say the
// parameter name should have brackets around it?
pub struct Method<Body: 'static> {
    pub method: &'static (dyn ApiMethodInfo<Body> + Send + Sync),
    pub positional_args: &'static [&'static str],
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
}
