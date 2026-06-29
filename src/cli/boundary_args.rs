use std::path::PathBuf;

use clap::{Arg, ArgAction, Command, value_parser};

pub(super) fn boundary_command(command: Command) -> Command {
    command
        .arg(
            Arg::new("boundary")
                .long("boundary")
                .value_name("FROM:DISALLOW")
                .help("Flag imports from FROM into DISALLOW")
                .num_args(1)
                .action(ArgAction::Append),
        )
        .arg(
            Arg::new("boundary-coverage")
                .long("boundary-coverage")
                .help("Report Dart library files outside every configured architecture boundary")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("boundary-call")
                .long("boundary-call")
                .value_name("FROM:PATTERN")
                .help("Flag direct calls from FROM that match PATTERN")
                .num_args(1)
                .action(ArgAction::Append),
        )
        .arg(
            Arg::new("policy-pack")
                .long("policy-pack")
                .value_name("PATH")
                .help("Load a declarative policy rule pack")
                .value_parser(value_parser!(PathBuf))
                .num_args(1)
                .action(ArgAction::Append),
        )
        .arg(
            Arg::new("policy-violations")
                .long("policy-violations")
                .help("Run declarative policy rule-pack checks")
                .action(ArgAction::SetTrue),
        )
}
