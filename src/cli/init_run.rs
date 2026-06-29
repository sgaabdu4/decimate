use std::io::Write;
use std::path::PathBuf;

use clap::{Arg, ArgAction, ArgMatches, Command};

use crate::config::DecimateConfig;
use crate::init::{InitOptions, init_project, render_init_report};

use super::common_args::{format_arg, root_arg};
use super::{CliError, OutputFormat, output_format};

pub(super) fn init_command() -> Command {
    Command::new("init")
        .about("Create Decimate config and optional agent guidance")
        .arg(root_arg())
        .arg(format_arg())
        .arg(
            Arg::new("agents")
                .long("agents")
                .help("Write AGENTS.md guidance for downstream coding agents")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("force")
                .long("force")
                .help("Overwrite existing init files")
                .action(ArgAction::SetTrue),
        )
}

pub(super) fn run_init<W: Write>(subcommand: &ArgMatches, mut writer: W) -> Result<i32, CliError> {
    let root = subcommand
        .get_one::<PathBuf>("root")
        .cloned()
        .unwrap_or_else(|| PathBuf::from("."));
    let report = init_project(
        &root,
        InitOptions {
            force: subcommand.get_flag("force"),
            agents: subcommand.get_flag("agents"),
        },
    )?;

    match output_format(subcommand, &DecimateConfig::default()) {
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut writer, &report)?;
            writeln!(writer)?;
        }
        OutputFormat::Human => writer.write_all(render_init_report(&report).as_bytes())?,
    }
    Ok(0)
}
