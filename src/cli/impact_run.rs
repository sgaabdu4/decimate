use std::io::Write;
use std::path::PathBuf;

use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};

use crate::impact::{
    ImpactSort, impact_all_report, impact_report, render_impact_all_report, render_impact_report,
};

use super::common_args::{format_arg, root_arg};
use super::{CliError, OutputFormat};

pub(super) fn impact_command() -> Command {
    Command::new("impact")
        .about("Show Decimate's local value report")
        .arg(root_arg())
        .arg(format_arg())
        .arg(
            Arg::new("quiet")
                .long("quiet")
                .help("Suppress non-data prompts")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("all")
                .long("all")
                .help("Roll up every tracked local project")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("sort")
                .long("sort")
                .value_name("KEY")
                .help("Sort key for --all")
                .default_value("label")
                .value_parser(["label", "surfaced", "contained-commits", "record-count"]),
        )
        .arg(
            Arg::new("limit")
                .long("limit")
                .value_name("N")
                .help("Maximum projects to emit for --all")
                .default_value("20")
                .value_parser(value_parser!(usize)),
        )
}

pub(super) fn run_impact<W: Write>(
    subcommand: &ArgMatches,
    mut writer: W,
) -> Result<i32, CliError> {
    if subcommand.get_flag("all") {
        let report = impact_all_report(
            impact_sort(subcommand),
            subcommand.get_one::<usize>("limit").copied().unwrap_or(20),
        );
        match output_format(subcommand) {
            OutputFormat::Json => serde_json::to_writer_pretty(&mut writer, &report)?,
            OutputFormat::Human => {
                writer.write_all(render_impact_all_report(&report).as_bytes())?;
            }
        }
    } else {
        let root = subcommand
            .get_one::<PathBuf>("root")
            .cloned()
            .unwrap_or_else(|| PathBuf::from("."));
        let report = impact_report(root);
        match output_format(subcommand) {
            OutputFormat::Json => serde_json::to_writer_pretty(&mut writer, &report)?,
            OutputFormat::Human => writer.write_all(render_impact_report(&report).as_bytes())?,
        }
    }
    writeln!(writer)?;
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

fn impact_sort(subcommand: &ArgMatches) -> ImpactSort {
    match subcommand
        .get_one::<String>("sort")
        .map_or("label", String::as_str)
    {
        "surfaced" => ImpactSort::Surfaced,
        "contained-commits" => ImpactSort::ContainedCommits,
        "record-count" => ImpactSort::RecordCount,
        _ => ImpactSort::Label,
    }
}
