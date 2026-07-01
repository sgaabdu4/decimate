use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::graph::normalize_path;
use crate::output::{Finding, FindingKind};

/// Stable JSON schema version for fix reports.
pub const FIX_SCHEMA_VERSION: &str = "dart-decimate.fix.v1";

/// Safe-fix execution mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FixMode {
    /// Report planned fixes without writing files.
    DryRun,
    /// Apply planned fixes.
    Apply,
}

/// Report emitted by `dart-decimate fix`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixReport {
    /// Schema identifier.
    pub schema_version: String,
    /// Tool name.
    pub tool: String,
    /// Typed JSON envelope kind.
    pub kind: String,
    /// Whether this was a dry-run or apply run.
    pub mode: FixMode,
    /// Numeric rollup.
    pub summary: FixSummary,
    /// Planned or applied fixes.
    pub fixes: Vec<FixChange>,
    /// Auto-fixable findings that were not safe to execute.
    pub skipped: Vec<FixSkip>,
}

/// Numeric fix summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixSummary {
    /// Planned fix count.
    pub planned: usize,
    /// Applied fix count.
    pub applied: usize,
    /// Skipped fix count.
    pub skipped: usize,
}

/// One planned or applied fix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixChange {
    /// Stable action id.
    pub action: String,
    /// Root-relative file path.
    pub path: String,
    /// 1-based source line for the finding.
    pub line: usize,
    /// Human-readable fix description.
    pub description: String,
    /// Whether the fix was applied to disk.
    pub applied: bool,
}

/// One skipped fix candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FixSkip {
    /// Stable action id.
    pub action: String,
    /// Root-relative file path.
    pub path: String,
    /// 1-based source line for the finding.
    pub line: usize,
    /// Why the fix was skipped.
    pub reason: String,
}

/// Plan or apply safe fixes from already-filtered findings.
#[must_use]
pub fn fix_findings(
    root: &Path,
    findings: &[Finding],
    selected_actions: &BTreeSet<String>,
    mode: FixMode,
) -> FixReport {
    let mut candidates = candidates(root, findings, selected_actions);
    candidates.sort_by_key(candidate_order);

    let mut fixes = Vec::new();
    let mut skipped = Vec::new();
    for candidate in candidates {
        match validate_candidate(root, &candidate) {
            Ok(()) => {
                let applied = mode == FixMode::Apply
                    && apply_candidate(&candidate)
                        .map_err(|reason| {
                            skipped.push(FixSkip {
                                action: candidate.action.clone(),
                                path: candidate.path.clone(),
                                line: candidate.line,
                                reason,
                            });
                        })
                        .is_ok();
                if mode == FixMode::DryRun || applied {
                    fixes.push(FixChange {
                        action: candidate.action,
                        path: candidate.path,
                        line: candidate.line,
                        description: candidate.description,
                        applied,
                    });
                }
            }
            Err(reason) => skipped.push(FixSkip {
                action: candidate.action,
                path: candidate.path,
                line: candidate.line,
                reason,
            }),
        }
    }

    FixReport {
        schema_version: FIX_SCHEMA_VERSION.to_owned(),
        tool: "dart-decimate".to_owned(),
        kind: "fix".to_owned(),
        mode,
        summary: FixSummary {
            planned: fixes.len(),
            applied: fixes.iter().filter(|fix| fix.applied).count(),
            skipped: skipped.len(),
        },
        fixes,
        skipped,
    }
}

/// Render a human-readable fix report.
#[must_use]
pub fn render_fix_report(report: &FixReport) -> String {
    let mode = match report.mode {
        FixMode::DryRun => "dry-run",
        FixMode::Apply => "apply",
    };
    let mut output = format!(
        "Fix {mode}\nplanned: {}\napplied: {}\nskipped: {}\n",
        report.summary.planned, report.summary.applied, report.summary.skipped
    );
    for fix in &report.fixes {
        let status = if fix.applied { "applied" } else { "planned" };
        let _ = write!(
            output,
            "\n{status} {} {}:{}\n{}",
            fix.action, fix.path, fix.line, fix.description
        );
    }
    for skip in &report.skipped {
        let _ = write!(
            output,
            "\nskipped {} {}:{}\n{}",
            skip.action, skip.path, skip.line, skip.reason
        );
    }
    output.push('\n');
    output
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FixCandidate {
    action: String,
    path: String,
    absolute_path: PathBuf,
    line: usize,
    description: String,
    kind: FindingKind,
    safe_to_delete: bool,
    target_path: Option<String>,
    target_dependency: Option<String>,
    target_end_line: Option<usize>,
    config_key: Option<String>,
}

fn candidates(
    root: &Path,
    findings: &[Finding],
    selected_actions: &BTreeSet<String>,
) -> Vec<FixCandidate> {
    findings
        .iter()
        .flat_map(|finding| {
            finding
                .actions
                .iter()
                .filter(|action| action.auto_fixable)
                .filter(|action| {
                    selected_actions.is_empty() || selected_actions.contains(&action.action)
                })
                .map(|action| FixCandidate {
                    path: action
                        .target_path
                        .clone()
                        .unwrap_or_else(|| finding.path.clone()),
                    action: action.action.clone(),
                    absolute_path: resolve_finding_path(
                        root,
                        action.target_path.as_deref().unwrap_or(&finding.path),
                    ),
                    line: finding.line,
                    description: action.description.clone(),
                    kind: finding.kind,
                    safe_to_delete: finding.safe_to_delete,
                    target_path: action.target_path.clone(),
                    target_dependency: action.target_dependency.clone(),
                    target_end_line: action.target_end_line,
                    config_key: action.config_key.clone(),
                })
        })
        .collect()
}

fn validate_candidate(root: &Path, candidate: &FixCandidate) -> Result<(), String> {
    let normalized_root = normalize_path(root);
    if !candidate.absolute_path.starts_with(&normalized_root) {
        return Err("candidate path is outside the project root".to_owned());
    }
    match candidate.action.as_str() {
        "delete-file" => validate_delete_file(candidate),
        "remove-declaration" => validate_remove_declaration(candidate),
        "remove-suppression" => validate_remove_suppression(candidate),
        action if is_pubspec_dependency_action(action) => {
            validate_remove_pubspec_dependency(candidate)
        }
        _ => Err(format!("unsupported safe fix action {}", candidate.action)),
    }
}

fn validate_delete_file(candidate: &FixCandidate) -> Result<(), String> {
    if candidate.kind != FindingKind::DeadFile {
        return Err("delete-file only applies to dead-file findings".to_owned());
    }
    if candidate
        .absolute_path
        .extension()
        .is_none_or(|ext| ext != "dart")
    {
        return Err("delete-file only removes Dart files".to_owned());
    }
    let metadata =
        fs::symlink_metadata(&candidate.absolute_path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() {
        return Err("refusing to delete a symlink".to_owned());
    }
    if !metadata.is_file() {
        return Err("delete-file target is not a regular file".to_owned());
    }
    Ok(())
}

fn validate_remove_declaration(candidate: &FixCandidate) -> Result<(), String> {
    if !matches!(
        candidate.kind,
        FindingKind::UnusedExport | FindingKind::UnusedType
    ) {
        return Err("remove-declaration only applies to unused top-level symbols".to_owned());
    }
    if !candidate.safe_to_delete {
        return Err("unused symbol finding is not marked safe_to_delete".to_owned());
    }
    if candidate.target_path.is_none() {
        return Err("remove-declaration requires action.target_path".to_owned());
    }
    if candidate.target_end_line.unwrap_or(candidate.line) != candidate.line {
        return Err("multi-line declarations are not auto-fixable".to_owned());
    }
    validate_dart_source_target(candidate)?;
    let source = fs::read_to_string(&candidate.absolute_path).map_err(|error| error.to_string())?;
    validate_declaration_line(&source, candidate.line)
}

fn validate_remove_suppression(candidate: &FixCandidate) -> Result<(), String> {
    if candidate.kind != FindingKind::StaleSuppression {
        return Err("remove-suppression only applies to stale-suppression findings".to_owned());
    }
    if candidate.line == 0 {
        return Err("suppression line must be 1-based".to_owned());
    }
    if !candidate.absolute_path.is_file() {
        return Err("suppression file does not exist".to_owned());
    }
    Ok(())
}

fn validate_remove_pubspec_dependency(candidate: &FixCandidate) -> Result<(), String> {
    let section = dependency_section_for_kind(candidate.kind)?;
    if !candidate.safe_to_delete {
        return Err("unused dependency finding is not marked safe_to_delete".to_owned());
    }
    if candidate.target_path.is_none() {
        return Err("remove-pubspec-dependency requires action.target_path".to_owned());
    }
    if candidate.config_key.as_deref() != Some(section) {
        return Err(format!(
            "remove-pubspec-dependency requires action.config_key {section}"
        ));
    }
    let metadata =
        fs::symlink_metadata(&candidate.absolute_path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() {
        return Err("refusing to edit a symlinked pubspec.yaml".to_owned());
    }
    if !metadata.is_file() {
        return Err("pubspec dependency target is not a regular file".to_owned());
    }
    if candidate
        .absolute_path
        .file_name()
        .and_then(|name| name.to_str())
        != Some("pubspec.yaml")
    {
        return Err("remove-pubspec-dependency target must be pubspec.yaml".to_owned());
    }
    let source = fs::read_to_string(&candidate.absolute_path).map_err(|error| error.to_string())?;
    validate_pubspec_dependency_source(&source, candidate, section)
}

fn apply_candidate(candidate: &FixCandidate) -> Result<(), String> {
    match candidate.action.as_str() {
        "delete-file" => {
            fs::remove_file(&candidate.absolute_path).map_err(|error| error.to_string())
        }
        "remove-declaration" => remove_declaration(candidate),
        "remove-suppression" => remove_line(&candidate.absolute_path, candidate.line),
        action if is_pubspec_dependency_action(action) => remove_pubspec_dependency(candidate),
        _ => Err(format!("unsupported safe fix action {}", candidate.action)),
    }
}

fn remove_declaration(candidate: &FixCandidate) -> Result<(), String> {
    validate_remove_declaration(candidate)?;
    remove_line(&candidate.absolute_path, candidate.line)
}

fn remove_pubspec_dependency(candidate: &FixCandidate) -> Result<(), String> {
    let section = dependency_section_for_kind(candidate.kind)?;
    let source = fs::read_to_string(&candidate.absolute_path).map_err(|error| error.to_string())?;
    validate_pubspec_dependency_source(&source, candidate, section)?;
    remove_line(&candidate.absolute_path, candidate.line)
}

fn remove_line(path: &Path, line: usize) -> Result<(), String> {
    let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let mut lines = source
        .split_inclusive('\n')
        .map(str::to_owned)
        .collect::<Vec<_>>();
    if line == 0 || line > lines.len() {
        return Err("suppression line is outside the file".to_owned());
    }
    lines.remove(line - 1);
    fs::write(path, lines.concat()).map_err(|error| error.to_string())
}

fn validate_pubspec_dependency_source(
    source: &str,
    candidate: &FixCandidate,
    section: &str,
) -> Result<(), String> {
    let dependency = candidate
        .target_dependency
        .as_deref()
        .ok_or_else(|| "remove-pubspec-dependency requires action.target_dependency".to_owned())?;
    let lines = source.split_inclusive('\n').collect::<Vec<_>>();
    if candidate.line == 0 || candidate.line > lines.len() {
        return Err("dependency line is outside pubspec.yaml".to_owned());
    }
    validate_simple_pubspec_dependency_line(&lines, candidate.line - 1, dependency, section)
}

fn validate_simple_pubspec_dependency_line(
    lines: &[&str],
    index: usize,
    dependency: &str,
    section: &str,
) -> Result<(), String> {
    let line = lines[index];
    if line.contains('#') {
        return Err("dependency entries with comments are not auto-fixable".to_owned());
    }
    let indent = indentation(line)?;
    if indent == 0 {
        return Err(
            "dependency entry must be nested under dependencies or dev_dependencies".to_owned(),
        );
    }
    let actual_section = enclosing_pubspec_section(lines, index, indent)?;
    if actual_section != section {
        return Err(format!(
            "dependency entry is under {actual_section}, expected {section}"
        ));
    }
    let trimmed = line.trim();
    let (key, value) = trimmed
        .split_once(':')
        .ok_or_else(|| "dependency entry is not a YAML key-value line".to_owned())?;
    if key.trim() != dependency {
        return Err(format!(
            "dependency entry key {} does not match target_dependency {dependency}",
            key.trim()
        ));
    }
    let value = value.trim();
    if value.is_empty() {
        return Err("nested dependency entries are not auto-fixable".to_owned());
    }
    if value.contains('{')
        || value.contains('}')
        || value.contains('[')
        || value.contains(']')
        || value.contains("path:")
        || value.contains("git:")
        || value.contains("sdk:")
    {
        return Err("path/git/sdk/map dependency entries are not auto-fixable".to_owned());
    }
    Ok(())
}

fn validate_dart_source_target(candidate: &FixCandidate) -> Result<(), String> {
    let metadata =
        fs::symlink_metadata(&candidate.absolute_path).map_err(|error| error.to_string())?;
    if metadata.file_type().is_symlink() {
        return Err("refusing to edit a symlinked Dart file".to_owned());
    }
    if !metadata.is_file() {
        return Err("Dart declaration target is not a regular file".to_owned());
    }
    if candidate
        .absolute_path
        .extension()
        .is_none_or(|extension| extension != "dart")
    {
        return Err("remove-declaration target must be a Dart file".to_owned());
    }
    Ok(())
}

fn validate_declaration_line(source: &str, line: usize) -> Result<(), String> {
    let lines = source.split_inclusive('\n').collect::<Vec<_>>();
    if line == 0 || line > lines.len() {
        return Err("declaration line is outside the file".to_owned());
    }
    let trimmed = lines[line - 1].trim();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        return Err("declaration line is not executable Dart source".to_owned());
    }
    if trimmed.starts_with("import ")
        || trimmed.starts_with("export ")
        || trimmed.starts_with("part ")
        || trimmed.starts_with("library ")
    {
        return Err("remove-declaration refuses to edit Dart directives".to_owned());
    }
    if !(trimmed.ends_with(';') || trimmed.ends_with('}')) {
        return Err("declaration line is not a complete one-line declaration".to_owned());
    }
    Ok(())
}

fn enclosing_pubspec_section(
    lines: &[&str],
    index: usize,
    target_indent: usize,
) -> Result<String, String> {
    for line in lines[..index].iter().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = indentation(line)?;
        if indent >= target_indent {
            continue;
        }
        if indent != 0 {
            return Err("dependency entry is nested below another map".to_owned());
        }
        let (key, value) = trimmed
            .split_once(':')
            .ok_or_else(|| "dependency entry is not under a pubspec section".to_owned())?;
        if !value.trim().is_empty() {
            return Err(
                "dependency entry is not under dependencies or dev_dependencies".to_owned(),
            );
        }
        return Ok(key.trim().to_owned());
    }
    Err("dependency entry is not under dependencies or dev_dependencies".to_owned())
}

fn indentation(line: &str) -> Result<usize, String> {
    let mut spaces = 0;
    for character in line.chars() {
        match character {
            ' ' => spaces += 1,
            '\t' => return Err("tab-indented pubspec entries are not auto-fixable".to_owned()),
            _ => return Ok(spaces),
        }
    }
    Ok(spaces)
}

fn dependency_section_for_kind(kind: FindingKind) -> Result<&'static str, String> {
    match kind {
        FindingKind::UnusedDependency => Ok("dependencies"),
        FindingKind::UnusedDevDependency => Ok("dev_dependencies"),
        _ => Err(
            "remove-pubspec-dependency only applies to unused pub dependency findings".to_owned(),
        ),
    }
}

fn is_pubspec_dependency_action(action: &str) -> bool {
    action == "remove-pubspec-dependency"
}

fn resolve_finding_path(root: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        normalize_path(path)
    } else {
        normalize_path(&root.join(path))
    }
}

fn candidate_order(candidate: &FixCandidate) -> (u8, String, std::cmp::Reverse<usize>, String) {
    let group = match candidate.action.as_str() {
        "remove-suppression" => 0,
        action if action == "remove-declaration" || is_pubspec_dependency_action(action) => 1,
        _ => 2,
    };
    (
        group,
        candidate.path.clone(),
        std::cmp::Reverse(candidate.line),
        candidate.action.clone(),
    )
}
