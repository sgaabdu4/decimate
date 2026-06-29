use std::path::PathBuf;

use clap::{Arg, Command, value_parser};

use super::common_args::{config_arg, entry_arg, format_arg};

pub(super) fn trace_file_command(command: Command) -> Command {
    command.arg(
        Arg::new("file")
            .long("file")
            .value_name("PATH")
            .help("Dart file to trace")
            .required(true)
            .value_parser(value_parser!(PathBuf)),
    )
}

pub(super) fn trace_symbol_command(command: Command) -> Command {
    command
        .arg(
            Arg::new("file")
                .long("file")
                .value_name("PATH")
                .help("Dart file declaring the symbol")
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("symbol")
                .long("symbol")
                .value_name("SYMBOL")
                .help("Top-level symbol to trace, or FILE:SYMBOL when --file is omitted")
                .required(true),
        )
}

pub(super) fn trace_command(command: Command) -> Command {
    command
        .arg(
            Arg::new("symbol")
                .value_name("FILE:SYMBOL")
                .help("Top-level symbol to trace")
                .required(true),
        )
        .arg(
            Arg::new("root")
                .long("root")
                .value_name("ROOT")
                .help("Project root")
                .default_value(".")
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(format_arg())
        .arg(config_arg())
        .arg(entry_arg())
        .arg(super::mode_args::production_arg())
        .arg(super::mode_args::no_production_arg())
}

pub(super) fn trace_dependency_command(command: Command) -> Command {
    command.arg(
        Arg::new("dependency")
            .long("dependency")
            .value_name("PACKAGE")
            .help("Pub dependency package to trace")
            .required(true),
    )
}
