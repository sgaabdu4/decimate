use std::io::Write;

use clap::{Arg, Command};

use crate::config::config_schema;
use crate::manifest::decimate_schema;
use crate::policy::rule_pack_schema;
use crate::report_schema::report_schema;

use super::CliError;

pub(super) fn config_schema_command() -> Command {
    schema_command(
        "config-schema",
        "Print the Decimate configuration JSON schema",
    )
}

pub(super) fn report_schema_command() -> Command {
    schema_command("report-schema", "Print the Decimate report JSON schema")
}

pub(super) fn rule_pack_schema_command() -> Command {
    schema_command(
        "rule-pack-schema",
        "Print the Decimate policy rule-pack JSON schema",
    )
}

pub(super) fn manifest_command() -> Command {
    schema_command("schema", "Print the Decimate CLI and issue manifest")
}

pub(super) fn run_config_schema<W: Write>(mut writer: W) -> Result<i32, CliError> {
    serde_json::to_writer_pretty(&mut writer, &config_schema())?;
    writeln!(writer)?;
    Ok(0)
}

pub(super) fn run_report_schema<W: Write>(mut writer: W) -> Result<i32, CliError> {
    serde_json::to_writer_pretty(&mut writer, &report_schema())?;
    writeln!(writer)?;
    Ok(0)
}

pub(super) fn run_rule_pack_schema<W: Write>(mut writer: W) -> Result<i32, CliError> {
    serde_json::to_writer_pretty(&mut writer, &rule_pack_schema())?;
    writeln!(writer)?;
    Ok(0)
}

pub(super) fn run_manifest<W: Write>(mut writer: W) -> Result<i32, CliError> {
    serde_json::to_writer_pretty(&mut writer, &decimate_schema())?;
    writeln!(writer)?;
    Ok(0)
}

fn schema_command(name: &'static str, about: &'static str) -> Command {
    Command::new(name).about(about).arg(schema_format_arg())
}

fn schema_format_arg() -> Arg {
    Arg::new("format")
        .long("format")
        .value_name("FORMAT")
        .help("Output format")
        .default_value("human")
        .value_parser(["human", "json"])
}
