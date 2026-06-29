use clap::{Arg, ArgMatches, Command, parser::ValueSource, value_parser};

use crate::FeatureFlagOptions;

pub(super) fn flags_command(command: Command) -> Command {
    command.arg(
        Arg::new("top")
            .long("top")
            .value_name("N")
            .help("Show only the N most frequently referenced feature flags")
            .value_parser(value_parser!(usize)),
    )
}

pub(super) fn feature_flag_options_with_defaults(
    matches: &ArgMatches,
    mut options: FeatureFlagOptions,
) -> FeatureFlagOptions {
    if matches.value_source("top") == Some(ValueSource::CommandLine) {
        options.top = matches.get_one::<usize>("top").copied();
    }
    options
}
