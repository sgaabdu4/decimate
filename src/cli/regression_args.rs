use std::path::PathBuf;

use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};

use crate::RegressionTolerance;
use crate::output::ReportCommand;

use super::CliError;

pub(super) const DEFAULT_REGRESSION_BASELINE: &str = ".dart-decimate/regression-baseline.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct RegressionRequestArgs {
    pub(super) tolerance: RegressionTolerance,
    pub(super) fail_on_regression: bool,
}

pub(super) fn regression_command(command: Command) -> Command {
    command
        .arg(regression_baseline_arg())
        .arg(save_regression_baseline_arg())
        .arg(fail_on_regression_arg())
        .arg(tolerance_arg())
}

pub(super) fn regression_baseline_path(
    command: ReportCommand,
    subcommand: &ArgMatches,
) -> Option<PathBuf> {
    if !supports_regression(command) {
        return None;
    }
    subcommand
        .get_one::<PathBuf>("regression-baseline")
        .cloned()
        .or_else(|| {
            subcommand
                .get_flag("fail-on-regression")
                .then(|| PathBuf::from(DEFAULT_REGRESSION_BASELINE))
        })
}

pub(super) fn save_regression_baseline_path(
    command: ReportCommand,
    subcommand: &ArgMatches,
) -> Option<PathBuf> {
    supports_regression(command)
        .then(|| {
            subcommand
                .get_one::<PathBuf>("save-regression-baseline")
                .cloned()
        })
        .flatten()
}

pub(super) fn regression_request_args(
    command: ReportCommand,
    subcommand: &ArgMatches,
) -> Result<RegressionRequestArgs, CliError> {
    if !supports_regression(command) {
        return Ok(RegressionRequestArgs {
            tolerance: RegressionTolerance::default(),
            fail_on_regression: false,
        });
    }
    Ok(RegressionRequestArgs {
        tolerance: subcommand
            .get_one::<String>("tolerance")
            .map_or(Ok(RegressionTolerance::default()), |value| {
                parse_tolerance(value)
            })?,
        fail_on_regression: subcommand.get_flag("fail-on-regression"),
    })
}

fn regression_baseline_arg() -> Arg {
    Arg::new("regression-baseline")
        .long("regression-baseline")
        .value_name("PATH")
        .help("Compare current finding counts against a regression baseline")
        .value_parser(value_parser!(PathBuf))
}

fn save_regression_baseline_arg() -> Arg {
    Arg::new("save-regression-baseline")
        .long("save-regression-baseline")
        .value_name("PATH")
        .help("Write current finding counts to a regression baseline")
        .value_parser(value_parser!(PathBuf))
}

fn fail_on_regression_arg() -> Arg {
    Arg::new("fail-on-regression")
        .long("fail-on-regression")
        .help("Exit non-zero only when finding counts exceed the regression baseline")
        .action(ArgAction::SetTrue)
}

fn tolerance_arg() -> Arg {
    Arg::new("tolerance")
        .long("tolerance")
        .value_name("COUNT_OR_PERCENT")
        .help("Allowed count increase for --fail-on-regression, such as 0, 2, or 10%")
}

fn supports_regression(command: ReportCommand) -> bool {
    matches!(
        command,
        ReportCommand::Check
            | ReportCommand::DeadCode
            | ReportCommand::Cycles
            | ReportCommand::Dupes
            | ReportCommand::Health
            | ReportCommand::Flags
            | ReportCommand::Security
    )
}

fn parse_tolerance(value: &str) -> Result<RegressionTolerance, CliError> {
    let value = value.trim();
    if let Some(percent) = value.strip_suffix('%') {
        return parse_percent_tolerance(percent).map(RegressionTolerance::PercentBasisPoints);
    }
    value
        .parse::<usize>()
        .map(RegressionTolerance::Absolute)
        .map_err(|_| CliError::Tolerance {
            value: value.to_owned(),
        })
}

fn parse_percent_tolerance(value: &str) -> Result<u32, CliError> {
    let value = value.trim();
    let Some((whole, fraction)) = value.split_once('.') else {
        return value
            .parse::<u32>()
            .map(|whole| whole.saturating_mul(100))
            .map_err(|_| CliError::Tolerance {
                value: format!("{value}%"),
            });
    };
    if fraction.len() > 2 || whole.is_empty() || fraction.is_empty() {
        return Err(CliError::Tolerance {
            value: format!("{value}%"),
        });
    }
    let whole = whole.parse::<u32>().map_err(|_| CliError::Tolerance {
        value: format!("{value}%"),
    })?;
    let fraction = format!("{fraction:0<2}")
        .parse::<u32>()
        .map_err(|_| CliError::Tolerance {
            value: format!("{value}%"),
        })?;
    Ok(whole.saturating_mul(100).saturating_add(fraction))
}
