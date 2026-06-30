use std::io::Write;
use std::path::PathBuf;

use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};

use crate::config::DecimateConfig;
use crate::hooks::{
    HookOptions, HookTarget, hooks_status, install_hooks, render_hooks_report, uninstall_hooks,
};

use super::common_args::{format_arg, root_arg, root_flag_arg, root_path};
use super::{CliError, OutputFormat, output_format};

pub(super) fn hooks_command() -> Command {
    Command::new("hooks")
        .about("Inspect, install, or remove Decimate-managed hooks")
        .subcommand_required(true)
        .arg_required_else_help(true)
        .subcommand(hook_subcommand("status", "Inspect Decimate hook status"))
        .subcommand(hook_subcommand("install", "Install Decimate-managed hooks").arg(force_arg()))
        .subcommand(hook_subcommand("uninstall", "Remove Decimate-managed hooks").arg(force_arg()))
}

pub(super) fn setup_hooks_command() -> Command {
    Command::new("setup-hooks")
        .about("Install or remove Decimate-managed agent hooks")
        .arg(root_arg())
        .arg(root_flag_arg())
        .arg(format_arg())
        .arg(branch_arg())
        .arg(force_arg())
        .arg(
            Arg::new("agent")
                .long("agent")
                .help("Install the repository-local agent hook target")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("dry-run")
                .long("dry-run")
                .help("Inspect planned agent hook state without writing files")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("uninstall")
                .long("uninstall")
                .help("Remove Decimate-managed agent hooks")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("user")
                .long("user")
                .help("User-level agent hooks are not supported by Decimate")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("gitignore-claude")
                .long("gitignore-claude")
                .help("Mutating .gitignore from setup-hooks is not supported")
                .action(ArgAction::SetTrue),
        )
}

pub(super) fn run_hooks<W: Write>(subcommand: &ArgMatches, writer: W) -> Result<i32, CliError> {
    match subcommand.subcommand() {
        Some(("status", command)) => {
            run_hook_report(command, writer, |root, options| hooks_status(root, options))
        }
        Some(("install", command)) => run_hook_report(command, writer, |root, options| {
            install_hooks(root, options)
        }),
        Some(("uninstall", command)) => run_hook_report(command, writer, |root, options| {
            uninstall_hooks(root, options)
        }),
        _ => unreachable!("clap requires a hooks subcommand"),
    }
}

pub(super) fn run_setup_hooks<W: Write>(
    subcommand: &ArgMatches,
    writer: W,
) -> Result<i32, CliError> {
    if subcommand.get_flag("user") {
        return Err(CliError::UnsupportedSetupHooksUser);
    }
    if subcommand.get_flag("gitignore-claude") {
        return Err(CliError::UnsupportedSetupHooksGitignoreClaude);
    }
    run_hook_report(subcommand, writer, |root, options| {
        let mut options = options.clone();
        options.target = HookTarget::Agent;
        if subcommand.get_flag("dry-run") {
            hooks_status(root, &options)
        } else if subcommand.get_flag("uninstall") {
            uninstall_hooks(root, &options)
        } else {
            install_hooks(root, &options)
        }
    })
}

fn hook_subcommand(name: &'static str, about: &'static str) -> Command {
    Command::new(name)
        .about(about)
        .arg(root_arg())
        .arg(root_flag_arg())
        .arg(format_arg())
        .arg(target_arg())
        .arg(branch_arg())
}

fn run_hook_report<W, F>(
    subcommand: &ArgMatches,
    mut writer: W,
    operation: F,
) -> Result<i32, CliError>
where
    W: Write,
    F: FnOnce(&PathBuf, &HookOptions) -> Result<crate::HooksReport, crate::HooksError>,
{
    let root = root_path(subcommand);
    let report = operation(&root, &hook_options(subcommand))?;
    match output_format(subcommand, &DecimateConfig::default()) {
        OutputFormat::Json => {
            serde_json::to_writer_pretty(&mut writer, &report)?;
            writeln!(writer)?;
        }
        OutputFormat::Human => writer.write_all(render_hooks_report(&report).as_bytes())?,
    }
    Ok(0)
}

fn hook_options(subcommand: &ArgMatches) -> HookOptions {
    HookOptions {
        target: hook_target(subcommand),
        branch: subcommand
            .get_one::<String>("branch")
            .cloned()
            .unwrap_or_else(|| "origin/main".to_owned()),
        force: subcommand
            .try_get_one::<bool>("force")
            .ok()
            .flatten()
            .copied()
            .unwrap_or(false),
    }
}

fn hook_target(subcommand: &ArgMatches) -> HookTarget {
    match subcommand
        .try_get_one::<String>("target")
        .ok()
        .flatten()
        .map_or("git", String::as_str)
    {
        "agent" => HookTarget::Agent,
        _ => HookTarget::Git,
    }
}

fn target_arg() -> Arg {
    Arg::new("target")
        .long("target")
        .value_name("TARGET")
        .help("Hook target")
        .default_value("git")
        .value_parser(["git", "agent"])
}

fn branch_arg() -> Arg {
    Arg::new("branch")
        .long("branch")
        .value_name("REF")
        .help("Git base ref used by installed hooks")
        .default_value("origin/main")
        .value_parser(value_parser!(String))
}

fn force_arg() -> Arg {
    Arg::new("force")
        .long("force")
        .help("Overwrite or remove unmanaged hook files")
        .action(ArgAction::SetTrue)
}
