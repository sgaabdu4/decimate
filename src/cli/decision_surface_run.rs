use std::io::Write;

use clap::{Arg, ArgMatches, Command, value_parser};

use crate::changed_scope::changed_files;
use crate::decision_surface::{
    decision_surface_report_for_command, render_decision_surface_report,
};
use crate::scan::{ScanOptions, scan_project_with_options};

use super::common_args::{config_arg, format_arg, root_arg, root_flag_arg, root_path};
use super::{CliError, OutputFormat};

pub(super) fn decision_surface_command() -> Command {
    decision_surface_command_named(
        "decision-surface",
        "Surface changed-code structural decisions for review",
    )
}

pub(super) fn review_command() -> Command {
    decision_surface_command_named(
        "review",
        "Review changed-code structural decisions without failing CI",
    )
}

pub(super) fn max_decisions_arg() -> Arg {
    Arg::new("max-decisions")
        .long("max-decisions")
        .value_name("N")
        .help("Maximum decisions to emit")
        .default_value("5")
        .value_parser(value_parser!(usize))
}

fn decision_surface_command_named(name: &'static str, about: &'static str) -> Command {
    Command::new(name)
        .about(about)
        .arg(root_arg())
        .arg(root_flag_arg())
        .arg(format_arg())
        .arg(config_arg())
        .arg(
            Arg::new("base")
                .long("base")
                .value_name("REF")
                .help("Git ref used to scope changed files")
                .required(true),
        )
        .arg(max_decisions_arg())
}

pub(super) fn run_decision_surface<W: Write>(
    subcommand: &ArgMatches,
    mut writer: W,
    command: &str,
) -> Result<i32, CliError> {
    let root = root_path(subcommand);
    let config = super::load_config(&root, subcommand)?;
    let project = scan_project_with_options(
        &root,
        &ScanOptions {
            ignore_patterns: config.ignore_patterns.clone(),
            ..ScanOptions::default()
        },
    )?;
    let Some(base) = subcommand.get_one::<String>("base") else {
        unreachable!("clap requires --base for decision-surface");
    };
    let changed = changed_files(&project.root, base)?;
    let max_decisions = subcommand
        .get_one::<usize>("max-decisions")
        .copied()
        .unwrap_or(5);
    let report =
        decision_surface_report_for_command(&project, base, &changed, max_decisions, command);

    match output_format(subcommand) {
        OutputFormat::Human => {
            writer.write_all(render_decision_surface_report(&report).as_bytes())?;
        }
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut writer, &report)?;
            writeln!(writer)?;
        }
    }
    Ok(0)
}

fn output_format(subcommand: &ArgMatches) -> OutputFormat {
    match subcommand
        .get_one::<String>("format")
        .map_or("human", String::as_str)
    {
        "json" => OutputFormat::Json,
        _ => OutputFormat::Human,
    }
}
