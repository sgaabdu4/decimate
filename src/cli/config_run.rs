use std::io::Write;
use std::path::Path;

use clap::{Arg, Command};
use serde::Serialize;

use super::common_args::{config_arg, format_arg, root_arg, root_flag_arg, root_path};
use super::{CliError, OutputFormat, load_config, output_format};
use crate::config::{CONFIG_SCHEMA_VERSION, DecimateConfig};

pub(super) fn config_command() -> Command {
    Command::new("config")
        .about("Print the resolved Decimate configuration")
        .arg(root_arg())
        .arg(root_flag_arg())
        .arg(format_arg())
        .arg(config_arg())
        .arg(
            Arg::new("path")
                .long("path")
                .help("Print only the discovered config path")
                .action(clap::ArgAction::SetTrue),
        )
}

pub(super) fn run_config<W: Write>(
    subcommand: &clap::ArgMatches,
    mut writer: W,
) -> Result<i32, CliError> {
    let root = root_path(subcommand);
    let config = load_config(&root, subcommand)?;

    if subcommand.get_flag("path") {
        if let Some(path) = &config.path {
            writeln!(writer, "{}", path.display())?;
        }
        return Ok(0);
    }

    match output_format(subcommand, &config) {
        OutputFormat::Json => {
            serde_json::to_writer_pretty(
                &mut writer,
                &ConfigEnvelope {
                    schema_version: CONFIG_SCHEMA_VERSION,
                    path: config.path.as_deref(),
                    config: &config,
                },
            )?;
            writeln!(writer)?;
        }
        OutputFormat::Human => writer.write_all(render_config(&config).as_bytes())?,
    }

    Ok(0)
}

fn render_config(config: &DecimateConfig) -> String {
    let Some(path) = &config.path else {
        return "No Decimate config found\n".to_owned();
    };

    format!(
        "Config: {}\nEntries: {}\nBoundaries: {}\nIgnore patterns: {}\n",
        path.display(),
        config.entry_points.len(),
        config.boundaries.len(),
        config.ignore_patterns.len()
    )
}

#[derive(Debug, Serialize)]
struct ConfigEnvelope<'a> {
    schema_version: &'static str,
    path: Option<&'a Path>,
    config: &'a DecimateConfig,
}
