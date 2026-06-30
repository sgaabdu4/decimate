use clap::ArgMatches;

use crate::config::DecimateConfig;
use crate::output::ReportCommand;
use crate::{DuplicateOptions, FeatureFlagOptions, HealthOptions, SecurityOptions};

use super::CliError;
use super::dupes_args::duplicate_options_with_defaults;
use super::flags_args::feature_flag_options_with_defaults;
use super::health_args::health_options_with_defaults;
use super::security_args::security_options_with_defaults;

pub(super) fn duplicate_options_for(
    command: ReportCommand,
    subcommand: &ArgMatches,
    config: &DecimateConfig,
) -> Result<DuplicateOptions, CliError> {
    if matches!(
        command,
        ReportCommand::Check
            | ReportCommand::Audit
            | ReportCommand::Dupes
            | ReportCommand::TraceClone
    ) {
        duplicate_options_with_defaults(subcommand, config.duplicate_options())
    } else if command == ReportCommand::Inspect {
        Ok(config.duplicate_options())
    } else {
        Ok(DuplicateOptions::default())
    }
}

pub(super) fn health_options_for(
    command: ReportCommand,
    subcommand: &ArgMatches,
    config: &DecimateConfig,
) -> HealthOptions {
    if matches!(
        command,
        ReportCommand::Check | ReportCommand::Audit | ReportCommand::Health
    ) {
        health_options_with_defaults(subcommand, config.health_options())
    } else if command == ReportCommand::Inspect {
        config.health_options()
    } else {
        HealthOptions::default()
    }
}

pub(super) fn feature_flag_options_for(
    command: ReportCommand,
    subcommand: &ArgMatches,
    config: &DecimateConfig,
) -> FeatureFlagOptions {
    if command == ReportCommand::Flags {
        feature_flag_options_with_defaults(subcommand, config.feature_flag_options())
    } else if matches!(
        command,
        ReportCommand::Check | ReportCommand::Audit | ReportCommand::Inspect
    ) {
        config.feature_flag_options()
    } else {
        FeatureFlagOptions::default()
    }
}

pub(super) fn security_options_for(
    command: ReportCommand,
    subcommand: &ArgMatches,
    config: &DecimateConfig,
) -> SecurityOptions {
    if command == ReportCommand::Security {
        security_options_with_defaults(subcommand, config.security_options())
    } else if matches!(
        command,
        ReportCommand::Check | ReportCommand::Audit | ReportCommand::Inspect
    ) {
        config.security_options()
    } else {
        SecurityOptions::default()
    }
}
