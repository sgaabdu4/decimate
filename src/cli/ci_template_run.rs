use std::io::Write;
use std::path::PathBuf;

use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};

use crate::ci_template::{
    CiReconcileReviewOptions, CiReviewProvider, CiTemplatePlatform, ci_reconcile_review_report,
    ci_template_report, render_ci_template, vendor_ci_template,
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

pub(super) fn ci_command() -> Command {
    Command::new("ci")
        .about("Run CI integration utilities")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(
            Command::new("reconcile-review")
                .about("Dry-run stale review reconciliation from a typed review envelope")
                .arg(
                    Arg::new("provider")
                        .long("provider")
                        .value_name("PROVIDER")
                        .help("Review provider")
                        .default_value("github")
                        .value_parser(["github", "gitlab"]),
                )
                .arg(
                    Arg::new("repo")
                        .long("repo")
                        .value_name("OWNER/REPO")
                        .help("GitHub repository slug"),
                )
                .arg(
                    Arg::new("project-id")
                        .long("project-id")
                        .value_name("ID")
                        .help("GitLab project id"),
                )
                .arg(Arg::new("pr").long("pr").value_name("NUMBER"))
                .arg(Arg::new("mr").long("mr").value_name("IID"))
                .arg(Arg::new("api-url").long("api-url").value_name("URL"))
                .arg(
                    Arg::new("envelope")
                        .long("envelope")
                        .value_name("PATH")
                        .help("Path to review-github or review-gitlab envelope JSON")
                        .required(true)
                        .value_parser(value_parser!(PathBuf)),
                )
                .arg(
                    Arg::new("dry-run")
                        .long("dry-run")
                        .help("Validate reconciliation without provider mutation")
                        .action(ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("format")
                        .long("format")
                        .value_name("FORMAT")
                        .help("Output format")
                        .default_value("json")
                        .value_parser(["json"]),
                ),
        )
}

pub(super) fn run_ci<W: Write>(subcommand: &ArgMatches, writer: W) -> Result<i32, CliError> {
    match subcommand.subcommand() {
        Some(("reconcile-review", command)) => run_ci_reconcile_review(command, writer),
        _ => unreachable!("clap requires a ci subcommand"),
    }
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

fn run_ci_reconcile_review<W: Write>(
    subcommand: &ArgMatches,
    mut writer: W,
) -> Result<i32, CliError> {
    let Some(envelope) = subcommand.get_one::<PathBuf>("envelope").cloned() else {
        return Err(CliError::MissingCiReviewEnvelope);
    };
    let report = ci_reconcile_review_report(CiReconcileReviewOptions {
        provider: review_provider(subcommand),
        envelope,
        dry_run: subcommand.get_flag("dry-run"),
        repo: subcommand.get_one::<String>("repo").cloned(),
        project_id: subcommand.get_one::<String>("project-id").cloned(),
        pr: subcommand.get_one::<String>("pr").cloned(),
        mr: subcommand.get_one::<String>("mr").cloned(),
        api_url: subcommand.get_one::<String>("api-url").cloned(),
    })?;
    serde_json::to_writer_pretty(&mut writer, &report)?;
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

fn review_provider(subcommand: &ArgMatches) -> CiReviewProvider {
    match subcommand
        .get_one::<String>("provider")
        .map_or("github", String::as_str)
    {
        "gitlab" => CiReviewProvider::Gitlab,
        _ => CiReviewProvider::Github,
    }
}
