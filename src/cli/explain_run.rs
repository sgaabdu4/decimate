use std::io::Write;

use clap::{Arg, ArgMatches, Command};
use serde::Serialize;

use super::common_args::format_arg;
use super::{CliError, OutputFormat, output_format_value};
use crate::{explain_issue, render_explain_report};

pub(super) fn explain_command() -> Command {
    Command::new("explain")
        .about("Explain one Decimate issue type")
        .arg(
            Arg::new("issue-type")
                .value_name("ISSUE_TYPE")
                .help("Issue type such as unused-export, unused exports, or code duplication")
                .required(true),
        )
        .arg(format_arg())
}

pub(super) fn run_explain<W: Write>(
    subcommand: &ArgMatches,
    mut writer: W,
) -> Result<i32, CliError> {
    let format = output_format_value(subcommand);
    let issue_type = subcommand
        .get_one::<String>("issue-type")
        .map_or("", String::as_str);

    match explain_issue(issue_type) {
        Ok(report) => {
            match format {
                OutputFormat::Json => {
                    serde_json::to_writer_pretty(&mut writer, &report)?;
                    writeln!(writer)?;
                }
                OutputFormat::Human => {
                    writer.write_all(render_explain_report(&report).as_bytes())?;
                }
            }
            Ok(0)
        }
        Err(error) => {
            let message = error.to_string();
            match format {
                OutputFormat::Json => {
                    serde_json::to_writer_pretty(
                        &mut writer,
                        &ExplainErrorEnvelope {
                            error: true,
                            message,
                            exit_code: 2,
                        },
                    )?;
                    writeln!(writer)?;
                }
                OutputFormat::Human => {
                    writeln!(writer, "{message}")?;
                }
            }
            Ok(2)
        }
    }
}

#[derive(Debug, Serialize)]
struct ExplainErrorEnvelope {
    error: bool,
    message: String,
    exit_code: i32,
}
