use std::io::{self, Read};
use std::path::Path;

use crate::ScannedProject;
use crate::security_gate::{self, SecurityDiffSource, SecurityGateError, SecurityGateMode};

use super::entry_points::entry_points_for_check;
use super::{CliError, CommandRequest, resolve_report_path};

pub(super) fn apply_security_gate(
    project: &ScannedProject,
    request: &CommandRequest,
    report: &mut crate::JsonReport,
) -> Result<(), CliError> {
    let root = &project.root;
    if matches!(
        request.security_gate,
        Some(SecurityGateMode::New | SecurityGateMode::NewlyReachable)
    ) && request.security_diff.is_none()
        && request.security_changed_since.is_none()
    {
        return Err(SecurityGateError::MissingDiffFile.into());
    }
    if request.security_gate.is_none() && request.security_diff.is_none() {
        return Ok(());
    }
    if let Some(scope) = changed_line_scope(root, request)? {
        if request.security_gate == Some(SecurityGateMode::NewlyReachable) {
            let entry_points =
                entry_points_for_check(project, &request.entry_points, request.entry_point_mode());
            security_gate::apply_changed_reachability_gate(
                root,
                &project.graph,
                report,
                entry_points,
                &scope,
            );
        } else {
            security_gate::apply_changed_line_gate(report, &scope);
        }
    }
    Ok(())
}

fn changed_line_scope(
    root: &Path,
    request: &CommandRequest,
) -> Result<Option<security_gate::ChangedLineScope>, CliError> {
    let Some(source) = request.security_diff.as_ref() else {
        return match request.security_changed_since.as_deref() {
            Some(base) => Ok(Some(security_gate::changed_lines_from_git(root, base)?)),
            None => Ok(None),
        };
    };
    match source {
        SecurityDiffSource::File(path) => Ok(Some(security_gate::changed_lines_from_diff_file(
            root,
            &resolve_report_path(root, path),
        )?)),
        SecurityDiffSource::Stdin => {
            let mut source = String::new();
            io::stdin().read_to_string(&mut source)?;
            Ok(Some(security_gate::changed_lines_from_diff(root, &source)))
        }
    }
}
