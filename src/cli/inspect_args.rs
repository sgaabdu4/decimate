use std::path::PathBuf;

use clap::{Arg, Command, value_parser};

pub(super) fn inspect_command(command: Command) -> Command {
    command
        .arg(
            Arg::new("file")
                .long("file")
                .value_name("PATH")
                .help("Dart file to inspect")
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("symbol")
                .long("symbol")
                .value_name("SYMBOL")
                .help("Top-level symbol to inspect, or FILE:SYMBOL when --file is omitted"),
        )
}
