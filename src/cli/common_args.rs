use std::path::PathBuf;

use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};

use super::regression_args::regression_command;

pub(super) fn scan_command(command: Command) -> Command {
    scan_command_with_format(command, format_arg())
}

pub(super) fn report_command(command: Command) -> Command {
    super::scope_args::report_command(super::summary_args::summary_command(
        scan_command_with_format(command, report_format_arg()),
    ))
}

fn scan_command_with_format(command: Command, format: Arg) -> Command {
    command
        .arg(root_arg())
        .arg(root_flag_arg())
        .arg(format)
        .arg(quiet_arg())
        .arg(config_arg())
        .arg(entry_arg())
        .arg(super::mode_args::production_arg())
        .arg(super::mode_args::no_production_arg())
}

pub(super) fn symbol_options_command(command: Command) -> Command {
    include_entry_exports_command(command).arg(super::mode_args::private_type_leaks_arg())
}

pub(super) fn baseline_command(command: Command) -> Command {
    regression_command(report_command(command))
        .arg(baseline_arg())
        .arg(save_baseline_arg())
}

pub(super) fn root_arg() -> Arg {
    Arg::new("root")
        .value_name("ROOT")
        .help("Project root")
        .default_value(".")
        .value_parser(value_parser!(PathBuf))
}

pub(super) fn root_flag_arg() -> Arg {
    Arg::new("root-flag")
        .long("root")
        .value_name("ROOT")
        .help("Project root")
        .value_parser(value_parser!(PathBuf))
}

pub(super) fn root_path(matches: &ArgMatches) -> PathBuf {
    matches
        .try_get_one::<PathBuf>("root-flag")
        .ok()
        .flatten()
        .or_else(|| matches.try_get_one::<PathBuf>("root").ok().flatten())
        .cloned()
        .unwrap_or_else(|| PathBuf::from("."))
}

pub(super) fn format_arg() -> Arg {
    Arg::new("format")
        .long("format")
        .value_name("FORMAT")
        .help("Output format")
        .default_value("human")
        .value_parser(["human", "json"])
}

fn report_format_arg() -> Arg {
    format_arg().value_parser(["human", "json", "sarif"])
}

pub(super) fn config_arg() -> Arg {
    Arg::new("config")
        .long("config")
        .value_name("PATH")
        .help("Decimate config file")
        .value_parser(value_parser!(PathBuf))
}

fn quiet_arg() -> Arg {
    Arg::new("quiet")
        .long("quiet")
        .help("Suppress non-JSON progress output")
        .action(ArgAction::SetTrue)
}

pub(super) fn entry_arg() -> Arg {
    Arg::new("entry")
        .long("entry")
        .value_name("PATH")
        .help("Dart entry point for reachability")
        .num_args(1)
        .action(ArgAction::Append)
        .value_parser(value_parser!(PathBuf))
}

pub(super) fn audit_baseline_arg(id: &'static str, help: &'static str) -> Arg {
    Arg::new(id)
        .long(id)
        .value_name("PATH")
        .help(help)
        .value_parser(value_parser!(PathBuf))
}

fn include_entry_exports_command(command: Command) -> Command {
    command.arg(super::mode_args::include_entry_exports_arg())
}

fn baseline_arg() -> Arg {
    Arg::new("baseline")
        .long("baseline")
        .value_name("PATH")
        .help("Suppress findings already captured in a Decimate baseline")
        .value_parser(value_parser!(PathBuf))
}

fn save_baseline_arg() -> Arg {
    Arg::new("save-baseline")
        .long("save-baseline")
        .value_name("PATH")
        .help("Write current visible findings to a Decimate baseline")
        .value_parser(value_parser!(PathBuf))
}
