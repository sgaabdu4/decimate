use clap::{Arg, ArgAction, ArgMatches, Command, parser::ValueSource, value_parser};

use crate::{DuplicateMode, DuplicateOptions};

pub(super) fn dupes_command(command: Command) -> Command {
    command
        .arg(
            Arg::new("mode")
                .long("mode")
                .value_name("MODE")
                .help("Detection mode")
                .default_value("mild")
                .value_parser(["strict", "mild", "weak", "semantic"]),
        )
        .arg(
            Arg::new("min-tokens")
                .long("min-tokens")
                .value_name("N")
                .help("Minimum tokens per clone")
                .default_value("50")
                .value_parser(value_parser!(usize)),
        )
        .arg(
            Arg::new("min-lines")
                .long("min-lines")
                .value_name("N")
                .help("Minimum lines per clone")
                .default_value("5")
                .value_parser(value_parser!(usize)),
        )
        .arg(
            Arg::new("min-occurrences")
                .long("min-occurrences")
                .value_name("N")
                .help("Minimum clone instances per group")
                .default_value("2")
                .value_parser(value_parser!(usize)),
        )
        .arg(
            Arg::new("top")
                .long("top")
                .value_name("N")
                .help("Show only the N largest clone groups")
                .value_parser(value_parser!(usize)),
        )
        .arg(
            Arg::new("skip-local")
                .long("skip-local")
                .help("Only report cross-directory duplicates")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("no-ignore-imports")
                .long("no-ignore-imports")
                .help("Count import/export/part/library directives")
                .action(ArgAction::SetTrue)
                .conflicts_with("ignore-imports"),
        )
        .arg(
            Arg::new("ignore-imports")
                .long("ignore-imports")
                .help("Ignore import/export/part/library directives")
                .action(ArgAction::SetTrue)
                .conflicts_with("no-ignore-imports"),
        )
}

pub(super) fn trace_clone_command(command: Command) -> Command {
    command.arg(
        Arg::new("fingerprint")
            .long("fingerprint")
            .alias("trace")
            .value_name("TRACE")
            .help("Clone fingerprint or FILE:LINE selector")
            .required(true),
    )
}

pub(super) fn duplicate_options_with_defaults(
    matches: &ArgMatches,
    mut options: DuplicateOptions,
) -> DuplicateOptions {
    if is_command_line(matches, "mode") {
        if let Some(mode) = matches.get_one::<String>("mode") {
            options.mode = DuplicateMode::parse(mode).unwrap_or(DuplicateMode::Mild);
        }
    }
    if is_command_line(matches, "min-tokens") {
        if let Some(min_tokens) = matches.get_one::<usize>("min-tokens") {
            options.min_tokens = *min_tokens;
        }
    }
    if is_command_line(matches, "min-lines") {
        if let Some(min_lines) = matches.get_one::<usize>("min-lines") {
            options.min_lines = *min_lines;
        }
    }
    if is_command_line(matches, "min-occurrences") {
        if let Some(min_occurrences) = matches.get_one::<usize>("min-occurrences") {
            options.min_occurrences = (*min_occurrences).max(2);
        }
    }
    if is_command_line(matches, "top") {
        options.top = matches.get_one::<usize>("top").copied();
    }
    if is_command_line(matches, "skip-local") {
        options.skip_local = matches.get_flag("skip-local");
    }
    if is_command_line(matches, "ignore-imports") {
        options.ignore_imports = matches.get_flag("ignore-imports");
    }
    if is_command_line(matches, "no-ignore-imports") {
        options.ignore_imports = !matches.get_flag("no-ignore-imports");
    }
    options
}

fn is_command_line(matches: &ArgMatches, id: &str) -> bool {
    matches.value_source(id) == Some(ValueSource::CommandLine)
}
