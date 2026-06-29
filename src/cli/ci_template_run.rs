use std::io::Write;
use std::path::PathBuf;

use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};

use crate::ci_template::{
    CiTemplatePlatform, ci_template_report, render_ci_template, vendor_ci_template,
};

use super::CliError;

pub(super) fn ci_template_command() -> Command {
    Command::new("ci-template")
        .about("Print or vendor CI integration templates")
        .arg(
            Arg::new("platform")
                .value_name("PLATFORM")
                .help("CI platform")
                .default_value("github")
                .value_parser(["github", "gitlab"]),
        )
        .arg(
            Arg::new("format")
                .long("format")
                .value_name("FORMAT")
                .help("Output format")
                .default_value("yaml")
                .value_parser(["yaml", "json"]),
        )
        .arg(
            Arg::new("vendor")
                .long("vendor")
                .help("Write vendored CI files under --root")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("root")
                .long("root")
                .value_name("PATH")
                .help("Project root for --vendor")
                .default_value(".")
                .value_parser(value_parser!(PathBuf)),
        )
        .arg(
            Arg::new("force")
                .long("force")
                .help("Overwrite existing vendored files")
                .action(ArgAction::SetTrue),
        )
}

pub(super) fn run_ci_template<W: Write>(
    subcommand: &ArgMatches,
    mut writer: W,
) -> Result<i32, CliError> {
    let platform = platform(subcommand);
    let vendor = subcommand.get_flag("vendor");
    let report = if vendor {
        let root = subcommand
            .get_one::<PathBuf>("root")
            .cloned()
            .unwrap_or_else(|| PathBuf::from("."));
        vendor_ci_template(root, platform, subcommand.get_flag("force"))?
    } else {
        ci_template_report(platform, false)
    };

    match output_format(subcommand) {
        CiTemplateOutputFormat::Json => serde_json::to_writer_pretty(&mut writer, &report)?,
        CiTemplateOutputFormat::Yaml => writer.write_all(render_ci_template(&report).as_bytes())?,
    }
    writeln!(writer)?;
    Ok(0)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CiTemplateOutputFormat {
    Yaml,
    Json,
}

fn output_format(subcommand: &ArgMatches) -> CiTemplateOutputFormat {
    match subcommand
        .get_one::<String>("format")
        .map_or("yaml", String::as_str)
    {
        "json" => CiTemplateOutputFormat::Json,
        _ => CiTemplateOutputFormat::Yaml,
    }
}

fn platform(subcommand: &ArgMatches) -> CiTemplatePlatform {
    match subcommand
        .get_one::<String>("platform")
        .map_or("github", String::as_str)
    {
        "gitlab" => CiTemplatePlatform::Gitlab,
        _ => CiTemplatePlatform::Github,
    }
}
