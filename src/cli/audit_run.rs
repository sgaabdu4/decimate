use std::path::PathBuf;

use clap::{Arg, ArgMatches};

use crate::changed_scope::{changed_file_scope_from_changed, changed_files};
use crate::output::{AnalysisResults, ReportCommand};
use crate::scan::ScannedProject;

use super::{CliError, CommandRequest};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum AuditGate {
    NewOnly,
    #[default]
    All,
}

pub(super) fn gate_arg() -> Arg {
    Arg::new("gate")
        .long("gate")
        .value_name("MODE")
        .value_parser(["new-only", "all"])
        .default_value("all")
        .help("Audit gate mode: all visible findings or only findings introduced by changed files")
}

pub(super) fn gate(command: ReportCommand, subcommand: &ArgMatches) -> AuditGate {
    if command != ReportCommand::Audit {
        return AuditGate::All;
    }
    match subcommand.get_one::<String>("gate").map(String::as_str) {
        Some("new-only") => AuditGate::NewOnly,
        _ => AuditGate::All,
    }
}

pub(super) fn apply_scope(
    project: &ScannedProject,
    request: &CommandRequest,
    results: &mut AnalysisResults,
) -> Result<Vec<PathBuf>, CliError> {
    if request.command != ReportCommand::Audit {
        return Ok(Vec::new());
    }

    let Some(base) = request.audit_base.as_deref() else {
        unreachable!("clap requires --base for audit");
    };
    let changed = changed_files(&project.root, base)?;
    results.file_scope = Some(changed_file_scope_from_changed(project, &changed));
    Ok(changed)
}
