use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

use crate::graph::normalize_against;
use crate::scan::ScannedProject;

/// Errors returned while computing changed file scope.
#[derive(Debug, Error)]
pub enum ChangedScopeError {
    /// Git could not be executed.
    #[error("failed to run git diff for changed scope: {source}")]
    Git {
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Git returned a non-zero status.
    #[error("git diff failed for base {base:?}: {stderr}")]
    GitDiff {
        /// Base revision passed by the caller.
        base: String,
        /// Stderr from git.
        stderr: String,
    },
}

/// Return root-normalized files changed since `base`.
///
/// # Errors
///
/// Returns [`ChangedScopeError`] if `git diff` cannot be executed or the base
/// revision is invalid for this repository.
pub fn changed_files(root: &Path, base: &str) -> Result<Vec<PathBuf>, ChangedScopeError> {
    let output = Command::new("git")
        .args(["diff", "--name-only", "--diff-filter=ACMRTUXB", base, "--"])
        .current_dir(root)
        .output()
        .map_err(|source| ChangedScopeError::Git { source })?;

    if !output.status.success() {
        return Err(ChangedScopeError::GitDiff {
            base: base.to_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }

    let untracked = Command::new("git")
        .args(["ls-files", "--others", "--exclude-standard"])
        .current_dir(root)
        .output()
        .map_err(|source| ChangedScopeError::Git { source })?;

    if !untracked.status.success() {
        return Err(ChangedScopeError::GitDiff {
            base: base.to_owned(),
            stderr: String::from_utf8_lossy(&untracked.stderr).trim().to_owned(),
        });
    }

    let mut paths = changed_paths(root, &output.stdout);
    paths.extend(changed_paths(root, &untracked.stdout));
    Ok(paths.into_iter().collect())
}

/// Return changed files plus one resolved graph hop in either direction.
///
/// # Errors
///
/// Returns [`ChangedScopeError`] if changed-file discovery fails.
pub fn changed_file_scope(
    project: &ScannedProject,
    base: &str,
) -> Result<Vec<PathBuf>, ChangedScopeError> {
    let changed = changed_files(&project.root, base)?;
    Ok(changed_file_scope_from_changed(project, &changed))
}

/// Expand an already discovered changed-file list with one graph hop.
#[must_use]
pub fn changed_file_scope_from_changed(
    project: &ScannedProject,
    changed: &[PathBuf],
) -> Vec<PathBuf> {
    expand_related_files(project, changed.to_vec())
}

fn expand_related_files(project: &ScannedProject, changed: Vec<PathBuf>) -> Vec<PathBuf> {
    let changed_set = changed.into_iter().collect::<BTreeSet<_>>();
    let mut scope = changed_set.clone();

    for dependency in project.graph.dependencies() {
        if changed_set.contains(&dependency.from_path) {
            scope.insert(dependency.to_path);
        } else if changed_set.contains(&dependency.to_path) {
            scope.insert(dependency.from_path);
        }
    }

    scope.into_iter().collect()
}

fn changed_paths(root: &Path, stdout: &[u8]) -> BTreeSet<PathBuf> {
    stdout
        .split(|byte| *byte == b'\n')
        .filter_map(|line| std::str::from_utf8(line).ok())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .filter_map(|line| normalize_changed_path(root, line))
        .collect()
}

fn normalize_changed_path(root: &Path, line: &str) -> Option<PathBuf> {
    let path = normalize_against(root, Path::new(line));
    path.starts_with(root).then_some(path)
}
