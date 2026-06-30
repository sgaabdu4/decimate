use std::io::Write;

use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::unsupported::{render_unsupported_report, unsupported_report};

use super::CliError;

pub(super) fn migrate_command() -> Command {
    Command::new("migrate")
        .about("Report Dart support status for Fallow migration helpers")
        .arg(format_arg())
        .arg(
            Arg::new("dry-run")
                .long("dry-run")
                .help("Accepted for Fallow CLI compatibility")
                .action(ArgAction::SetTrue),
        )
}

pub(super) fn telemetry_command() -> Command {
    Command::new("telemetry")
        .about("Report Decimate telemetry support status")
        .subcommand_required(false)
        .arg_required_else_help(false)
        .arg(format_arg())
        .subcommand(unsupported_subcommand("status"))
        .subcommand(unsupported_subcommand("enable"))
        .subcommand(unsupported_subcommand("disable"))
}

pub(super) fn license_command() -> Command {
    Command::new("license")
        .about("Report Decimate license support status")
        .subcommand_required(false)
        .arg_required_else_help(false)
        .arg(format_arg())
        .subcommand(unsupported_subcommand("status"))
        .subcommand(unsupported_subcommand("activate"))
}

pub(super) fn run_migrate<W: Write>(subcommand: &ArgMatches, writer: W) -> Result<i32, CliError> {
    let report = unsupported_report(
        "migrate",
        "not-applicable",
        "Fallow migrate imports JS/TS tool configuration; Decimate has no Dart migration source to convert.",
        vec![
            "Use decimate init --format json to create Dart-native defaults.".to_owned(),
            "Use decimate config --format json to inspect the resolved local configuration."
                .to_owned(),
        ],
    );
    write_report(subcommand, writer, &report)?;
    Ok(2)
}

pub(super) fn run_telemetry<W: Write>(subcommand: &ArgMatches, writer: W) -> Result<i32, CliError> {
    let action = subcommand.subcommand_name().unwrap_or("status");
    let report = unsupported_report(
        format!("telemetry {action}"),
        "disabled",
        "Decimate does not collect telemetry and has no hosted telemetry backend.",
        vec!["No action is required; local analysis runs without telemetry.".to_owned()],
    );
    write_report(subcommand, writer, &report)?;
    Ok(if action == "enable" { 2 } else { 0 })
}

pub(super) fn run_license<W: Write>(subcommand: &ArgMatches, writer: W) -> Result<i32, CliError> {
    let action = subcommand.subcommand_name().unwrap_or("status");
    let report = unsupported_report(
        format!("license {action}"),
        "not-required",
        "This Decimate build has no hosted license service; local Dart analysis is available without activation.",
        vec![
            "Use cargo install or the npm wrapper once published for local installation."
                .to_owned(),
        ],
    );
    write_report(subcommand, writer, &report)?;
    Ok(if action == "activate" { 2 } else { 0 })
}

fn unsupported_subcommand(name: &'static str) -> Command {
    Command::new(name).arg(format_arg())
}

fn format_arg() -> Arg {
    Arg::new("format")
        .long("format")
        .value_name("FORMAT")
        .help("Output format")
        .default_value("json")
        .value_parser(["json", "human"])
}

fn write_report<W: Write>(
    subcommand: &ArgMatches,
    mut writer: W,
    report: &crate::unsupported::UnsupportedReport,
) -> Result<(), CliError> {
    match requested_format(subcommand) {
        "human" => writer.write_all(render_unsupported_report(report).as_bytes())?,
        _ => serde_json::to_writer_pretty(&mut writer, report)?,
    }
    writeln!(writer)?;
    Ok(())
}

fn requested_format(subcommand: &ArgMatches) -> &str {
    subcommand
        .subcommand()
        .and_then(|(_, command)| command.get_one::<String>("format"))
        .or_else(|| subcommand.get_one::<String>("format"))
        .map_or("json", String::as_str)
}
