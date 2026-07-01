use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, SystemTime};

use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};

use super::common_args::{format_arg, root_arg, root_flag_arg, root_path};
use super::{CliError, run_from};

pub(super) fn watch_command() -> Command {
    Command::new("watch")
        .about("Watch Dart project files and rerun Dart Decimate check")
        .arg(root_arg())
        .arg(root_flag_arg())
        .arg(format_arg())
        .arg(
            Arg::new("no-clear")
                .long("no-clear")
                .help("Do not clear the terminal between watch runs")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("once")
                .long("once")
                .help("Run one check pass and exit")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("interval-ms")
                .long("interval-ms")
                .value_name("MS")
                .help("Polling interval for watch mode")
                .default_value("1000")
                .value_parser(value_parser!(u64)),
        )
}

pub(super) fn run_watch<W: Write>(subcommand: &ArgMatches, mut writer: W) -> Result<i32, CliError> {
    let root = root_path(subcommand);
    if subcommand.get_flag("once") {
        return run_check_once(&root, subcommand, writer);
    }
    let interval = Duration::from_millis(
        subcommand
            .get_one::<u64>("interval-ms")
            .copied()
            .unwrap_or(1000)
            .max(100),
    );
    let mut snapshot = watch_snapshot(&root)?;
    let _ = run_check_once(&root, subcommand, &mut writer)?;
    loop {
        thread::sleep(interval);
        let next = watch_snapshot(&root)?;
        if next == snapshot {
            continue;
        }
        snapshot = next;
        if !subcommand.get_flag("no-clear") && output_format(subcommand) != "json" {
            writer.write_all(b"\x1b[2J\x1b[H")?;
        }
        let _ = run_check_once(&root, subcommand, &mut writer)?;
    }
}

fn run_check_once<W: Write>(
    root: &Path,
    subcommand: &ArgMatches,
    mut writer: W,
) -> Result<i32, CliError> {
    let mut output = Vec::new();
    let code = run_from(
        [
            OsString::from("dart-decimate"),
            OsString::from("check"),
            root.as_os_str().to_os_string(),
            OsString::from("--format"),
            OsString::from(output_format(subcommand)),
        ],
        &mut output,
    )?;
    writer.write_all(&output)?;
    Ok(code)
}

fn output_format(subcommand: &ArgMatches) -> String {
    subcommand
        .get_one::<String>("format")
        .cloned()
        .unwrap_or_else(|| "human".to_owned())
}

fn watch_snapshot(root: &Path) -> Result<BTreeMap<PathBuf, SystemTime>, CliError> {
    let mut snapshot = BTreeMap::new();
    collect_watch_snapshot(root, root, &mut snapshot)?;
    Ok(snapshot)
}

fn collect_watch_snapshot(
    root: &Path,
    path: &Path,
    snapshot: &mut BTreeMap<PathBuf, SystemTime>,
) -> Result<(), CliError> {
    if ignored_watch_path(path) {
        return Ok(());
    }
    let metadata = fs::metadata(path)?;
    if metadata.is_file() {
        if watched_file(path) {
            snapshot.insert(
                path.strip_prefix(root).unwrap_or(path).to_path_buf(),
                metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            );
        }
        return Ok(());
    }
    if !metadata.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(path)? {
        collect_watch_snapshot(root, &entry?.path(), snapshot)?;
    }
    Ok(())
}

fn watched_file(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    path.extension().and_then(|extension| extension.to_str()) == Some("dart")
        || matches!(
            name,
            "pubspec.yaml"
                | "pubspec_overrides.yaml"
                | ".dart-decimaterc"
                | ".dart-decimaterc.json"
        )
}

fn ignored_watch_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| {
            matches!(
                name,
                ".git" | ".dart_tool" | "build" | "target" | "node_modules"
            )
        })
}
