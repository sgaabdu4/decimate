use std::io::Write;
use std::path::PathBuf;

use clap::{Arg, ArgAction, ArgMatches, Command, parser::ValueSource, value_parser};

use crate::coverage::{coverage_analysis_report, render_coverage_analysis_report};
use crate::output::json_runtime_coverage;
use crate::scan::{ScanOptions, scan_project_with_options};
use crate::{HealthOptions, LowTrafficThreshold};

use super::common_args::{config_arg, format_arg, root_arg};
use super::health_args::runtime_coverage_args;
use super::{CliError, OutputFormat, load_config, output_format};

pub(super) fn coverage_command() -> Command {
    Command::new("coverage")
        .about("Runtime coverage setup and analysis")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(coverage_analyze_command())
}

pub(super) fn run_coverage<W: Write>(subcommand: &ArgMatches, writer: W) -> Result<i32, CliError> {
    match subcommand.subcommand() {
        Some(("analyze", analyze)) => run_coverage_analyze(analyze, writer),
        _ => unreachable!("clap requires a coverage subcommand"),
    }
}

fn coverage_analyze_command() -> Command {
    runtime_coverage_args(
        Command::new("analyze")
            .about("Analyze local V8 or Istanbul runtime coverage")
            .arg(root_arg())
            .arg(format_arg())
            .arg(config_arg())
            .arg(
                Arg::new("cloud")
                    .long("cloud")
                    .help("Cloud runtime coverage is not supported yet")
                    .action(ArgAction::SetTrue),
            ),
        false,
    )
    .arg(
        Arg::new("top")
            .long("top")
            .value_name("N")
            .help("Show only the N highest runtime hot paths and findings")
            .value_parser(value_parser!(usize)),
    )
}

fn run_coverage_analyze<W: Write>(subcommand: &ArgMatches, mut writer: W) -> Result<i32, CliError> {
    if subcommand.get_flag("cloud") {
        return Err(CliError::UnsupportedCoverageCloud);
    }
    let root = subcommand
        .get_one::<PathBuf>("root")
        .cloned()
        .unwrap_or_else(|| PathBuf::from("."));
    require_runtime_coverage(subcommand)?;
    let config = load_config(&root, subcommand)?;
    let format = output_format(subcommand, &config);
    let health_options = coverage_health_options(subcommand, config.health_options());

    let project = scan_project_with_options(
        &root,
        &ScanOptions {
            ignore_patterns: config.ignore_patterns.clone(),
        },
    )?;
    let mut health = crate::analyze_health(&project, &health_options)?;
    let Some(runtime) = health.runtime_coverage.as_mut() else {
        return Err(CliError::MissingRuntimeCoverage);
    };
    if let Some(top) = health_options.top {
        runtime.hot_paths.truncate(top);
        runtime.findings.truncate(top);
        runtime.coverage_intelligence.truncate(top);
        runtime.blast_radius.truncate(top);
        runtime.importance.truncate(top);
    }
    let report = coverage_analysis_report(json_runtime_coverage(&project.root, runtime));

    match format {
        OutputFormat::Human => {
            writer.write_all(render_coverage_analysis_report(&report).as_bytes())?;
        }
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut writer, &report)?;
            writeln!(writer)?;
        }
    }

    Ok(0)
}

fn require_runtime_coverage(subcommand: &ArgMatches) -> Result<(), CliError> {
    if subcommand.get_one::<PathBuf>("runtime-coverage").is_some() {
        Ok(())
    } else {
        Err(CliError::MissingRuntimeCoverage)
    }
}

fn coverage_health_options(subcommand: &ArgMatches, mut options: HealthOptions) -> HealthOptions {
    options.coverage_path = None;
    options.coverage_gaps = false.into();
    options.max_crap = None;
    options.threshold_overrides.clear();
    options.file_scores = false.into();
    options.hotspots = false.into();
    options.targets = false.into();
    options.ownership = false.into();

    options.runtime_coverage_path = subcommand.get_one::<PathBuf>("runtime-coverage").cloned();
    if subcommand.value_source("min-invocations-hot") == Some(ValueSource::CommandLine)
        && let Some(value) = subcommand.get_one::<usize>("min-invocations-hot")
    {
        options.min_invocations_hot = (*value).max(1);
    }
    if subcommand.value_source("min-observation-volume") == Some(ValueSource::CommandLine)
        && let Some(value) = subcommand.get_one::<usize>("min-observation-volume")
    {
        options.min_observation_volume = (*value).max(1);
    }
    if subcommand.value_source("low-traffic-threshold") == Some(ValueSource::CommandLine)
        && let Some(value) = subcommand.get_one::<f64>("low-traffic-threshold")
    {
        options.low_traffic_threshold = LowTrafficThreshold::from_ratio(*value);
    }
    if let Some(top) = subcommand.get_one::<usize>("top") {
        options.top = Some(*top);
    }
    options
}
