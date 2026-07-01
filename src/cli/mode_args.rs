use clap::{Arg, ArgAction, ArgMatches};

use crate::config::{DartDecimateConfig, private_type_leaks_enabled};

use super::entry_points::EntryPointMode;

pub(super) fn production_arg() -> Arg {
    Arg::new("production")
        .long("production")
        .help("Infer reachability from production Dart entry points only")
        .action(ArgAction::SetTrue)
}

pub(super) fn no_production_arg() -> Arg {
    Arg::new("no-production")
        .long("no-production")
        .help("Disable production-only reachability from config defaults")
        .conflicts_with("production")
        .action(ArgAction::SetTrue)
}

pub(super) fn include_entry_exports_arg() -> Arg {
    Arg::new("include-entry-exports")
        .long("include-entry-exports")
        .help("Report unused public declarations exposed through entry libraries")
        .action(ArgAction::SetTrue)
}

pub(super) fn private_type_leaks_arg() -> Arg {
    Arg::new("private-type-leaks")
        .long("private-type-leaks")
        .help("Report exported signatures that expose same-library private Dart types")
        .action(ArgAction::SetTrue)
}

pub(super) fn production(matches: &ArgMatches, config: &DartDecimateConfig) -> bool {
    if matches.get_flag("no-production") {
        false
    } else if matches.get_flag("production") {
        true
    } else {
        config.production
    }
}

pub(super) fn production_mode(production: bool) -> EntryPointMode {
    if production {
        EntryPointMode::Production
    } else {
        EntryPointMode::All
    }
}

pub(super) fn include_entry_exports(matches: &ArgMatches, config: &DartDecimateConfig) -> bool {
    matches
        .try_get_one::<bool>("include-entry-exports")
        .ok()
        .flatten()
        .copied()
        .unwrap_or_default()
        || config.include_entry_exports
}

pub(super) fn private_type_leaks(matches: &ArgMatches, config: &DartDecimateConfig) -> bool {
    matches
        .try_get_one::<bool>("private-type-leaks")
        .ok()
        .flatten()
        .copied()
        .unwrap_or_default()
        || private_type_leaks_enabled(&config.rules)
}
