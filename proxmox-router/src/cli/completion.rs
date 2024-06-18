use std::collections::HashMap;

use proxmox_schema::*;

use super::help_command_def;
use super::{
    shellword_split_unclosed, CliCommand, CliCommandMap, CommandLineInterface, CompletionFunction,
};

fn record_done_argument(
    done: &mut HashMap<String, String>,
    parameters: ParameterSchema,
    key: &str,
    value: &str,
) {
    if let Some((_, schema)) = parameters.lookup(key) {
        match schema {
            Schema::Array(_) => { /* do nothing ?? */ }
            _ => {
                done.insert(key.to_owned(), value.to_owned());
            }
        }
    }
}

fn get_property_completion(
    schema: &Schema,
    name: &str,
    completion_functions: &HashMap<String, CompletionFunction>,
    arg: &str,
    param: &HashMap<String, String>,
) -> Vec<String> {
    if let Some(callback) = completion_functions.get(name) {
        let list = (callback)(arg, param);
        let mut completions = Vec::new();
        for value in list {
            if value.starts_with(arg) {
                completions.push(value);
            }
        }
        return completions;
    }

    match schema {
        Schema::String(StringSchema {
            format: Some(ApiStringFormat::Enum(variants)),
            ..
        }) => {
            let mut completions = Vec::new();
            for variant in variants.iter() {
                if variant.value.starts_with(arg) {
                    completions.push(variant.value.to_string());
                }
            }
            return completions;
        }
        Schema::Boolean(BooleanSchema { .. }) => {
            let mut completions = Vec::new();
            let mut lowercase_arg = arg.to_string();
            lowercase_arg.make_ascii_lowercase();
            for value in ["0", "1", "yes", "no", "true", "false", "on", "off"].iter() {
                if value.starts_with(&lowercase_arg) {
                    completions.push((*value).to_string());
                }
            }
            return completions;
        }
        Schema::Array(ArraySchema { items, .. }) => {
            if let Schema::String(_) = items {
                return get_property_completion(items, name, completion_functions, arg, param);
            }
        }
        _ => {}
    }

    Vec::new()
}

fn get_simple_completion(
    cli_cmd: &CliCommand,
    global_option_schemas: &HashMap<&'static str, &'static Schema>,
    global_option_completions: HashMap<&'static str, CompletionFunction>,
    done: &mut HashMap<String, String>,
    arg_param: &[&str], // we remove done arguments
    args: &[String],
) -> Vec<String> {
    let mut completions: HashMap<String, CompletionFunction> = global_option_completions
        .into_iter()
        .map(|(key, value)| (key.to_string(), value))
        .collect();
    completions.extend(
        cli_cmd
            .completion_functions
            .iter()
            .map(|(key, value)| (key.clone(), *value)),
    );
    get_simple_completion_do(
        cli_cmd,
        global_option_schemas,
        &completions,
        done,
        arg_param,
        args,
    )
}

fn get_simple_completion_do(
    cli_cmd: &CliCommand,
    global_option_schemas: &HashMap<&'static str, &'static Schema>,
    completion_functions: &HashMap<String, CompletionFunction>,
    done: &mut HashMap<String, String>,
    arg_param: &[&str], // we remove done arguments
    args: &[String],
) -> Vec<String> {
    //eprintln!("COMPL: {:?} {:?} {}", arg_param, args, args.len());

    if !arg_param.is_empty() {
        let prop_name = arg_param[0];
        if let Some((optional, schema)) = cli_cmd.info.parameters.lookup(prop_name) {
            let is_array_param = matches!(schema, Schema::Array(_));

            if (optional || is_array_param) && args[0].starts_with('-') {
                // argument parameter is optional (or array) , and arg
                // looks like an option, so assume its empty and
                // complete the rest
            } else {
                record_done_argument(done, cli_cmd.info.parameters, prop_name, &args[0]);
                if args.len() > 1 {
                    if is_array_param {
                        return get_simple_completion_do(
                            cli_cmd,
                            global_option_schemas,
                            completion_functions,
                            done,
                            arg_param,
                            &args[1..],
                        );
                    } else {
                        return get_simple_completion_do(
                            cli_cmd,
                            global_option_schemas,
                            completion_functions,
                            done,
                            &arg_param[1..],
                            &args[1..],
                        );
                    }
                }

                if args.len() == 1 {
                    return get_property_completion(
                        schema,
                        prop_name,
                        &completion_functions,
                        &args[0],
                        done,
                    );
                }

                return Vec::new();
            }
        } else {
            // unknown arg_param - should never happen
            return Vec::new();
        }
    }
    if args.is_empty() {
        return Vec::new();
    }

    // Try to parse all argumnets but last, record args already done
    if args.len() > 1 {
        let mut errors = ParameterError::new(); // we simply ignore any parsing errors here
        let (data, _remaining) = super::getopts::parse_argument_list(
            &args[0..args.len() - 1],
            cli_cmd.info.parameters,
            &mut errors,
        );
        for (key, value) in &data {
            record_done_argument(done, cli_cmd.info.parameters, key, value);
        }
    }

    let prefix = &args[args.len() - 1]; // match on last arg

    // complete option-name or option-value ?
    if !prefix.starts_with('-') && args.len() > 1 {
        let last = &args[args.len() - 2];
        if last.starts_with("--") && last.len() > 2 {
            let prop_name = &last[2..];
            if let Some(schema) = global_option_schemas.get(prop_name).copied().or_else(|| {
                cli_cmd
                    .info
                    .parameters
                    .lookup(prop_name)
                    .map(|(_, schema)| schema)
            }) {
                return get_property_completion(
                    schema,
                    prop_name,
                    &completion_functions,
                    prefix,
                    done,
                );
            }
            return Vec::new();
        }
    }

    let mut completions = Vec::new();
    for name in global_option_schemas.keys() {
        if done.contains_key(*name) {
            continue;
        }
        let option = String::from("--") + name;
        if option.starts_with(prefix) {
            completions.push(option);
        }
    }
    for (name, _optional, _schema) in cli_cmd.info.parameters.properties() {
        if done.contains_key(*name) {
            continue;
        }
        if cli_cmd.arg_param.contains(name) {
            continue;
        }
        let option = String::from("--") + name;
        if option.starts_with(prefix) {
            completions.push(option);
        }
    }
    completions
}

impl CommandLineInterface {
    fn get_help_completion(&self, help_cmd: &CliCommand, args: &[String]) -> Vec<String> {
        let mut done = HashMap::new();

        match self {
            CommandLineInterface::Simple(_) => get_simple_completion(
                help_cmd,
                &HashMap::new(),
                HashMap::new(),
                &mut done,
                &[],
                args,
            ),
            CommandLineInterface::Nested(map) => {
                if args.is_empty() {
                    let mut completions = Vec::new();
                    for cmd in map.commands.keys() {
                        completions.push(cmd.to_string());
                    }
                    return completions;
                }

                let first = &args[0];
                if args.len() > 1 {
                    if let Some(sub_cmd) = map.commands.get(first) {
                        // do exact match here
                        return sub_cmd.get_help_completion(help_cmd, &args[1..]);
                    }
                    return Vec::new();
                }

                if first.starts_with('-') {
                    return get_simple_completion(
                        help_cmd,
                        &HashMap::new(),
                        HashMap::new(),
                        &mut done,
                        &[],
                        args,
                    );
                }

                let mut completions = Vec::new();
                for cmd in map.commands.keys() {
                    if cmd.starts_with(first) {
                        completions.push(cmd.to_string());
                    }
                }
                completions
            }
        }
    }

    /// Helper to generate bash completions.
    ///
    /// This helper extracts the command line from environment variable
    /// set by ``bash``, namely ``COMP_LINE`` and ``COMP_POINT``. This is
    /// passed to ``get_completions()``. Returned values are printed to
    /// ``stdout``.
    pub fn print_bash_completion(&self) {
        let comp_point: usize = match std::env::var("COMP_POINT") {
            Ok(val) => match val.parse::<usize>() {
                Ok(i) => i,
                Err(_) => return,
            },
            Err(_) => return,
        };

        let cmdline = match std::env::var("COMP_LINE") {
            Ok(mut val) => {
                if let Some((byte_pos, _)) = val.char_indices().nth(comp_point) {
                    val.truncate(byte_pos);
                }
                val
            }
            Err(_) => return,
        };

        let (_start, completions) = self.get_completions(&cmdline, true);

        for item in completions {
            println!("{}", item);
        }
    }

    /// Compute possible completions for a partial command
    pub fn get_completions(&self, line: &str, skip_first: bool) -> (usize, Vec<String>) {
        let (mut args, start) = match shellword_split_unclosed(line, false) {
            (mut args, None) => {
                args.push("".into());
                (args, line.len())
            }
            (mut args, Some((start, arg, _quote))) => {
                args.push(arg);
                (args, start)
            }
        };

        if skip_first {
            if args.is_empty() {
                return (0, Vec::new());
            }

            args.remove(0); // no need for program name
        }

        let completions = if !args.is_empty() && args[0] == "help" {
            self.get_help_completion(&help_command_def(), &args[1..])
        } else {
            CompletionParser::default().get_completions(self, args)
        };

        (start, completions)
    }
}

#[derive(Default)]
struct CompletionParser {
    global_option_schemas: HashMap<&'static str, &'static Schema>,
    global_option_completions: HashMap<&'static str, CompletionFunction>,
    done_arguments: HashMap<String, String>,
}

impl CompletionParser {
    fn record_done_argument(&mut self, parameters: ParameterSchema, key: &str, value: &str) {
        if let Some(schema) = self
            .global_option_schemas
            .get(key)
            .copied()
            .or_else(|| parameters.lookup(key).map(|(_, schema)| schema))
        {
            match schema {
                Schema::Array(_) => { /* do nothing ?? */ }
                _ => {
                    self.done_arguments.insert(key.to_owned(), value.to_owned());
                }
            }
        }
    }

    /// Enable the current global options to be recognized by the argument parser.
    fn enable_global_options(&mut self, cli: &CliCommandMap) {
        for entry in cli.global_options.values() {
            for (name, schema) in entry.properties() {
                if self.global_option_schemas.insert(name, schema).is_some() {
                    panic!(
                        "duplicate option {name:?} in nested command line interface global options"
                    );
                }
                if let Some(cb) = entry.completion_functions.get(name) {
                    self.global_option_completions.insert(name, *cb);
                }
            }
        }
    }

    fn get_completions(mut self, cli: &CommandLineInterface, mut args: Vec<String>) -> Vec<String> {
        match cli {
            CommandLineInterface::Simple(cli_cmd) => {
                cli_cmd.fixed_param.iter().for_each(|(key, value)| {
                    self.record_done_argument(cli_cmd.info.parameters, key, value);
                });
                let args = match self.handle_current_global_options(args) {
                    Ok(GlobalArgs::Removed(args)) => args,
                    Ok(GlobalArgs::Completed(completion)) => return completion,
                    Err(_) => return Vec::new(),
                };
                get_simple_completion(
                    cli_cmd,
                    &self.global_option_schemas,
                    self.global_option_completions,
                    &mut self.done_arguments,
                    cli_cmd.arg_param,
                    &args,
                )
            }
            CommandLineInterface::Nested(map) => {
                super::command::replace_aliases(&mut args, &map.aliases);

                self.enable_global_options(map);
                let mut args = match self.handle_current_global_options(args) {
                    Ok(GlobalArgs::Removed(args)) => args,
                    Ok(GlobalArgs::Completed(completion)) => return completion,
                    Err(_) => return Vec::new(),
                };

                if args.len() == 1 || args.len() == 2 {
                    if let Some(arg0) = args[0].strip_prefix("--") {
                        if let Some(completion) =
                            self.try_complete_global_property(arg0, &args[1..])
                        {
                            return completion;
                        }
                    }
                }

                if args.len() <= 1 {
                    let filter = args.first().map(|s| s.as_str()).unwrap_or_default();

                    if filter.starts_with('-') {
                        return self
                            .global_option_schemas
                            .keys()
                            .filter_map(|k| {
                                let k = format!("--{k}");
                                k.starts_with(filter).then_some(k)
                            })
                            .collect();
                    }

                    let mut completion: Vec<String> = map
                        .commands
                        .keys()
                        .filter(|cmd| cmd.starts_with(filter))
                        .cloned()
                        .collect();
                    if filter.is_empty() {
                        completion.extend(
                            self.global_option_schemas
                                .keys()
                                .map(|key| format!("--{key}")),
                        );
                    }
                    return completion;
                }

                let first = args.remove(0);
                if let Some((_, sub_cmd)) = map.find_command(&first) {
                    return self.get_completions(sub_cmd, args);
                }

                Vec::new()
            }
        }
    }

    /// Parse out the current global options and return the remaining `args`.
    fn handle_current_global_options(
        &mut self,
        args: Vec<String>,
    ) -> Result<GlobalArgs, anyhow::Error> {
        let mut global_args = Vec::new();
        let args = super::getopts::ParseOptions::new(&mut global_args, &self.global_option_schemas)
            .stop_at_positional(true)
            .stop_at_unknown(true)
            .retain_separator(true)
            .parse(args)?;

        if args.is_empty() {
            // with no arguments remaining, the final global argument could need completion:
            if let Some((option, argument)) = global_args.last() {
                if let Some(completion) =
                    self.try_complete_global_property(option, &[argument.clone()])
                {
                    return Ok(GlobalArgs::Completed(completion));
                }
            }
        }

        // and merge them into the hash map
        for (option, argument) in global_args {
            self.done_arguments.insert(option, argument);
        }

        Ok(GlobalArgs::Removed(args))
    }

    fn try_complete_global_property(&self, arg0: &str, args: &[String]) -> Option<Vec<String>> {
        let cb = self.global_option_completions.get(arg0)?;
        let to_complete = args.first().map(|s| s.as_str()).unwrap_or_default();
        Some(cb(to_complete, &HashMap::new()))
    }
}

enum GlobalArgs {
    Removed(Vec<String>),
    Completed(Vec<String>),
}

#[cfg(test)]
mod test {
    use std::collections::HashMap;

    use anyhow::Error;
    use serde_json::Value;

    use proxmox_schema::{
        ApiStringFormat, ApiType, BooleanSchema, EnumEntry, ObjectSchema, Schema, StringSchema,
    };

    use crate::cli::{CliCommand, CliCommandMap, CommandLineInterface, GlobalOptions};
    use crate::{ApiHandler, ApiMethod, RpcEnvironment};

    fn dummy_method(
        _param: Value,
        _info: &ApiMethod,
        _rpcenv: &mut dyn RpcEnvironment,
    ) -> Result<Value, Error> {
        Ok(Value::Null)
    }

    const API_METHOD_SIMPLE1: ApiMethod = ApiMethod::new(
        &ApiHandler::Sync(&dummy_method),
        &ObjectSchema::new(
            "Simple API method with one required and one optionl argument.",
            &[
                (
                    "optional-arg",
                    true,
                    &BooleanSchema::new("Optional boolean argument.")
                        .default(false)
                        .schema(),
                ),
                (
                    "required-arg",
                    false,
                    &StringSchema::new("Required string argument.").schema(),
                ),
            ],
        ),
    );

    #[allow(dead_code)]
    struct GlobalOpts {
        global: String,
    }

    impl<'de> serde::Deserialize<'de> for GlobalOpts {
        fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            unreachable!("not used in tests, implemented to satisfy `.global_option` constraint");
        }
    }

    impl ApiType for GlobalOpts {
        const API_SCHEMA: Schema = ObjectSchema::new(
            "Global options.",
            &[(
                "global",
                true,
                &StringSchema::new("A global option.")
                    .format(&ApiStringFormat::Enum(&[
                        EnumEntry::new("one", "Option one."),
                        EnumEntry::new("two", "Option two."),
                    ]))
                    .schema(),
            )],
        )
        .schema();
    }

    fn complete_global(arg: &str, _param: &HashMap<String, String>) -> Vec<String> {
        eprintln!("GOT HERE WITH {arg:?}");
        ["one", "two"]
            .into_iter()
            .filter(|v| v.starts_with(arg))
            .map(str::to_string)
            .collect()
    }

    fn get_complex_test_cmddef() -> CommandLineInterface {
        let sub_def = CliCommandMap::new()
            .insert("l1c1", CliCommand::new(&API_METHOD_SIMPLE1))
            .insert("l1c2", CliCommand::new(&API_METHOD_SIMPLE1));

        let cmd_def = CliCommandMap::new()
            .global_option(
                GlobalOptions::of::<GlobalOpts>().completion_cb("global", complete_global),
            )
            .insert_help()
            .insert("l0sub", CommandLineInterface::Nested(sub_def))
            .insert("l0c1", CliCommand::new(&API_METHOD_SIMPLE1))
            .insert(
                "l0c2",
                CliCommand::new(&API_METHOD_SIMPLE1).arg_param(&["required-arg"]),
            )
            .insert(
                "l0c3",
                CliCommand::new(&API_METHOD_SIMPLE1).arg_param(&["required-arg", "optional-arg"]),
            );

        cmd_def.into()
    }

    fn test_completions(cmd_def: &CommandLineInterface, line: &str, start: usize, expect: &[&str]) {
        let mut expect: Vec<String> = expect.iter().map(|s| s.to_string()).collect();
        expect.sort();

        let (completion_start, mut completions) = cmd_def.get_completions(line, false);
        completions.sort();

        assert_eq!((start, expect), (completion_start, completions));
    }

    #[test]
    fn test_nested_completion() {
        let cmd_def = get_complex_test_cmddef();

        test_completions(
            &cmd_def,
            "",
            0,
            &["--global", "help", "l0c1", "l0c2", "l0c3", "l0sub"],
        );

        test_completions(
            &cmd_def,
            "l0c1 ",
            5,
            &["--global", "--optional-arg", "--required-arg"],
        );

        test_completions(
            &cmd_def,
            "l0c1 -",
            5,
            &["--global", "--optional-arg", "--required-arg"],
        );

        test_completions(
            &cmd_def,
            "l0c1 --",
            5,
            &["--global", "--optional-arg", "--required-arg"],
        );

        test_completions(&cmd_def, "l0c1 ---", 5, &[]);

        test_completions(&cmd_def, "l0c1 x", 5, &[]);

        test_completions(&cmd_def, "l0c1 --r", 5, &["--required-arg"]);

        test_completions(&cmd_def, "l0c1 --required-arg", 5, &["--required-arg"]);

        test_completions(
            &cmd_def,
            "l0c1 --required-arg -",
            20,
            // Note: --required-arg is not finished, so it still pops up
            &["--global", "--required-arg", "--optional-arg"],
        );

        test_completions(
            &cmd_def,
            "l0c1 --required-arg test -",
            25,
            &["--global", "--optional-arg"],
        );

        test_completions(
            &cmd_def,
            "l0c1 --global test -",
            19,
            &["--required-arg", "--optional-arg"],
        );

        test_completions(
            &cmd_def,
            "l0c1 --required-arg test --optional-arg ",
            40,
            &["0", "1", "false", "no", "true", "yes", "on", "off"],
        );

        test_completions(
            &cmd_def,
            "l0c1 --required-arg test --optional-arg f",
            40,
            &["false"],
        );

        test_completions(
            &cmd_def,
            "l0c1 --required-arg test --optional-arg F",
            40,
            &["false"],
        );

        test_completions(
            &cmd_def,
            "l0c1 --required-arg test --optional-arg Yes",
            40,
            &["yes"],
        );

        test_completions(&cmd_def, "l0sub ", 6, &["--global", "l1c1", "l1c2"]);
        test_completions(&cmd_def, "l0sub -", 6, &["--global"]);
        test_completions(&cmd_def, "l0sub --global ", 15, &["one", "two"]);
        test_completions(&cmd_def, "l0sub --global o", 15, &["one"]);
        test_completions(&cmd_def, "l0sub --global one", 15, &["one"]);
        test_completions(
            &cmd_def,
            "l0sub --global one ",
            19,
            &["--global", "l1c1", "l1c2"],
        );
    }

    #[test]
    fn test_help_completion() {
        let cmd_def = get_complex_test_cmddef();

        test_completions(&cmd_def, "h", 0, &["help"]);

        test_completions(
            &cmd_def,
            "help ",
            5,
            &["help", "l0sub", "l0c1", "l0c3", "l0c2"],
        );

        test_completions(&cmd_def, "help l0", 5, &["l0sub", "l0c1", "l0c3", "l0c2"]);

        test_completions(&cmd_def, "help -", 5, &["--verbose"]);

        test_completions(&cmd_def, "help l0c2", 5, &["l0c2"]);

        test_completions(&cmd_def, "help l0c2 ", 10, &["--verbose"]);

        test_completions(&cmd_def, "help l0c2 --verbose -", 20, &[]);

        test_completions(&cmd_def, "help l0s", 5, &["l0sub"]);

        test_completions(&cmd_def, "help l0sub ", 11, &["l1c1", "l1c2"]);

        test_completions(&cmd_def, "help l0sub l1c2 -", 16, &["--verbose"]);

        test_completions(&cmd_def, "help l0sub l1c2 --verbose -", 26, &[]);

        test_completions(&cmd_def, "help l0sub l1c3", 11, &[]);
    }
}
