use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::ScannedProject;
use crate::graph::normalize_against;
use crate::output::TRACE_SCHEMA_VERSION;

mod lex;
use lex::normalized_lines;

/// Dart duplicate-code detection mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DuplicateMode {
    /// Exact token match after comments and whitespace are removed.
    Strict,
    /// Default mode. Equivalent to strict for the current Dart tokenizer.
    Mild,
    /// Normalize string literal values.
    Weak,
    /// Normalize string literals, numeric literals, and non-keyword identifiers.
    Semantic,
}

impl DuplicateMode {
    /// Parse a CLI mode value.
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "strict" => Some(Self::Strict),
            "mild" => Some(Self::Mild),
            "weak" => Some(Self::Weak),
            "semantic" => Some(Self::Semantic),
            _ => None,
        }
    }
}

/// Duplicate-code detector options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateOptions {
    /// Detection mode.
    pub mode: DuplicateMode,
    /// Minimum token count per clone.
    pub min_tokens: usize,
    /// Minimum line count per clone.
    pub min_lines: usize,
    /// Minimum occurrences before a clone group is reported.
    pub min_occurrences: usize,
    /// Report only clones that cross directory boundaries.
    pub skip_local: bool,
    /// Ignore import/export/part/augment directives.
    pub ignore_imports: bool,
    /// Limit output to the N largest clone groups.
    pub top: Option<usize>,
}

impl Default for DuplicateOptions {
    fn default() -> Self {
        Self {
            mode: DuplicateMode::Mild,
            min_tokens: 50,
            min_lines: 5,
            min_occurrences: 2,
            skip_local: false,
            ignore_imports: true,
            top: None,
        }
    }
}

/// Code duplication report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DuplicateCodeReport {
    /// Options used to compute the report.
    pub options: DuplicateOptions,
    /// Reported clone groups.
    pub clone_groups: Vec<CodeClone>,
}

/// One duplicated Dart code block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeClone {
    /// Stable clone fingerprint.
    pub fingerprint: String,
    /// Matching block instances.
    pub instances: Vec<CodeCloneInstance>,
    /// Lines in the duplicated block.
    pub line_count: usize,
    /// Tokens in the duplicated block.
    pub token_count: usize,
}

/// One clone group instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeCloneInstance {
    /// Dart file path.
    pub path: PathBuf,
    /// 1-based start line.
    pub start_line: usize,
    /// 1-based end line.
    pub end_line: usize,
    /// 0-based byte column.
    pub column: usize,
}

/// Trace-clone JSON envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloneTraceReport {
    /// Schema identifier.
    pub schema_version: String,
    /// Typed JSON envelope discriminator.
    pub kind: String,
    /// Tool name.
    pub tool: String,
    /// Trace command name.
    pub command: String,
    /// Trace selector supplied by the caller.
    pub trace: String,
    /// Whether at least one clone group matched.
    pub found: bool,
    /// Matching clone groups.
    pub clone_groups: Vec<TraceCloneGroup>,
    /// Short trace interpretation.
    pub reason: String,
}

/// Clone group included in trace output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceCloneGroup {
    /// Stable clone fingerprint.
    pub fingerprint: String,
    /// Lines in the duplicated block.
    pub line_count: usize,
    /// Tokens in the duplicated block.
    pub token_count: usize,
    /// Matching block instances.
    pub instances: Vec<TraceCloneInstance>,
    /// Suggested extraction target.
    pub suggestion: String,
}

/// Clone instance included in trace output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceCloneInstance {
    /// Dart file path, root-relative where possible.
    pub path: String,
    /// 1-based start line.
    pub start_line: usize,
    /// 1-based end line.
    pub end_line: usize,
    /// 0-based byte column.
    pub column: usize,
}

/// Errors returned while detecting duplicated code.
#[derive(Debug, Error)]
pub enum DuplicateCodeError {
    /// A Dart file could not be read.
    #[error("failed to read Dart file {path}: {source}")]
    ReadFile {
        /// File path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CloneOccurrence {
    path: PathBuf,
    start_line: usize,
    end_line: usize,
    column: usize,
    parent: PathBuf,
}

/// Detect duplicated Dart code blocks.
///
/// # Errors
///
/// Returns [`DuplicateCodeError`] if a scanned Dart file cannot be read.
pub fn detect_duplicates(
    project: &ScannedProject,
    options: &DuplicateOptions,
) -> Result<DuplicateCodeReport, DuplicateCodeError> {
    let mut by_fingerprint = BTreeMap::<String, Vec<(CloneOccurrence, usize)>>::new();

    for file in &project.files {
        let path = normalize_against(&project.root, &file.path);
        if !path.starts_with(&project.root) || is_ignored_path(&path) {
            continue;
        }
        let source = fs::read_to_string(&path).map_err(|source| DuplicateCodeError::ReadFile {
            path: path.clone(),
            source,
        })?;
        let lines = normalized_lines(&source, options);
        if lines.len() < options.min_lines {
            continue;
        }

        for window in lines.windows(options.min_lines) {
            let token_count = window.iter().map(|line| line.token_count).sum::<usize>();
            if token_count < options.min_tokens {
                continue;
            }
            let text = window
                .iter()
                .map(|line| line.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            let Some(first) = window.first() else {
                continue;
            };
            let Some(last) = window.last() else {
                continue;
            };
            let fingerprint = fingerprint(&text);
            by_fingerprint.entry(fingerprint).or_default().push((
                CloneOccurrence {
                    parent: path.parent().map_or_else(PathBuf::new, Path::to_path_buf),
                    path: path.clone(),
                    start_line: first.line,
                    end_line: last.line,
                    column: first.column,
                },
                token_count,
            ));
        }
    }

    let mut clone_groups = by_fingerprint
        .into_iter()
        .filter_map(|(fingerprint, occurrences)| {
            clone_group_from_occurrences(&fingerprint, occurrences, options)
        })
        .collect::<Vec<_>>();

    clone_groups.sort_by(|left, right| {
        (
            std::cmp::Reverse(left.instances.len()),
            std::cmp::Reverse(left.line_count),
            &left.instances[0].path,
            left.instances[0].start_line,
            &left.fingerprint,
        )
            .cmp(&(
                std::cmp::Reverse(right.instances.len()),
                std::cmp::Reverse(right.line_count),
                &right.instances[0].path,
                right.instances[0].start_line,
                &right.fingerprint,
            ))
    });
    clone_groups = collapse_overlapping_groups(clone_groups);
    if let Some(top) = options.top {
        clone_groups.truncate(top);
    }

    Ok(DuplicateCodeReport {
        options: options.clone(),
        clone_groups,
    })
}

/// Trace clone groups by fingerprint or `FILE:LINE`.
#[must_use]
pub fn trace_clone(
    project: &ScannedProject,
    report: &DuplicateCodeReport,
    trace: &str,
) -> CloneTraceReport {
    let clone_groups = report
        .clone_groups
        .iter()
        .filter(|group| clone_matches(project, group, trace))
        .map(|group| trace_group(&project.root, group))
        .collect::<Vec<_>>();
    let found = !clone_groups.is_empty();

    CloneTraceReport {
        schema_version: TRACE_SCHEMA_VERSION.to_owned(),
        kind: "trace-clone".to_owned(),
        tool: "decimate".to_owned(),
        command: "trace-clone".to_owned(),
        trace: trace.to_owned(),
        found,
        reason: if found {
            "clone trace matched one or more duplicate code groups"
        } else {
            "clone trace did not match any duplicate code group"
        }
        .to_owned(),
        clone_groups,
    }
}

/// Render a concise human clone trace.
#[must_use]
pub fn render_clone_trace(report: &CloneTraceReport) -> String {
    format!(
        "trace-clone {}: found={} groups={}\n{}\n",
        report.trace,
        report.found,
        report.clone_groups.len(),
        report.reason
    )
}

fn collapse_overlapping_groups(groups: Vec<CodeClone>) -> Vec<CodeClone> {
    let mut collapsed = Vec::new();
    for group in groups {
        if !collapsed
            .iter()
            .any(|existing| groups_overlap(existing, &group))
        {
            collapsed.push(group);
        }
    }
    collapsed
}

fn groups_overlap(left: &CodeClone, right: &CodeClone) -> bool {
    right.instances.iter().all(|right_instance| {
        left.instances.iter().any(|left_instance| {
            left_instance.path == right_instance.path
                && ranges_overlap(
                    left_instance.start_line,
                    left_instance.end_line,
                    right_instance.start_line,
                    right_instance.end_line,
                )
        })
    })
}

fn ranges_overlap(
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
) -> bool {
    left_start <= right_end && right_start <= left_end
}

fn clone_group_from_occurrences(
    fingerprint: &str,
    occurrences: Vec<(CloneOccurrence, usize)>,
    options: &DuplicateOptions,
) -> Option<CodeClone> {
    let mut seen = BTreeSet::<(PathBuf, usize, usize)>::new();
    let mut instances = Vec::new();
    let mut token_count = 0;
    let mut parents = BTreeSet::new();

    for (occurrence, tokens) in occurrences {
        if !seen.insert((
            occurrence.path.clone(),
            occurrence.start_line,
            occurrence.end_line,
        )) {
            continue;
        }
        parents.insert(occurrence.parent);
        token_count = token_count.max(tokens);
        instances.push(CodeCloneInstance {
            path: occurrence.path,
            start_line: occurrence.start_line,
            end_line: occurrence.end_line,
            column: occurrence.column,
        });
    }

    if instances.len() < options.min_occurrences || (options.skip_local && parents.len() < 2) {
        return None;
    }
    instances.sort_by(|left, right| {
        (&left.path, left.start_line, left.end_line).cmp(&(
            &right.path,
            right.start_line,
            right.end_line,
        ))
    });

    Some(CodeClone {
        fingerprint: fingerprint.to_owned(),
        instances,
        line_count: options.min_lines,
        token_count,
    })
}

fn is_ignored_path(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if matches!(
        file_name,
        name if name.ends_with(".g.dart")
            || name.ends_with(".freezed.dart")
            || name.ends_with(".gen.dart")
            || name.ends_with(".gr.dart")
            || name.ends_with(".mocks.dart")
    ) {
        return true;
    }

    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some("test" | "integration_test" | "test_driver" | "__tests__" | "__mocks__")
        )
    })
}

fn fingerprint(text: &str) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("dup:{:08x}", hash & 0xffff_ffff)
}

fn clone_matches(project: &ScannedProject, group: &CodeClone, trace: &str) -> bool {
    if trace == group.fingerprint {
        return true;
    }
    let Some((path, line)) = parse_file_line(trace) else {
        return false;
    };
    let path = normalize_against(&project.root, &path);
    group.instances.iter().any(|instance| {
        instance.path == path && line >= instance.start_line && line <= instance.end_line
    })
}

fn parse_file_line(trace: &str) -> Option<(PathBuf, usize)> {
    let (path, line) = trace.rsplit_once(':')?;
    Some((PathBuf::from(path), line.parse().ok()?))
}

fn trace_group(root: &Path, group: &CodeClone) -> TraceCloneGroup {
    TraceCloneGroup {
        fingerprint: group.fingerprint.clone(),
        line_count: group.line_count,
        token_count: group.token_count,
        instances: group
            .instances
            .iter()
            .map(|instance| TraceCloneInstance {
                path: display_path(root, &instance.path),
                start_line: instance.start_line,
                end_line: instance.end_line,
                column: instance.column,
            })
            .collect(),
        suggestion: format!(
            "Extract the {} duplicated lines into one shared function, method, widget, or mixin owner",
            group.line_count
        ),
    }
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests;
