use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable schema version for hook management output.
pub const HOOKS_SCHEMA_VERSION: &str = "decimate.hooks.v1";

const HOOK_MARKER: &str = "decimate-managed-hook";

/// Hook installation target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookTarget {
    /// Git pre-commit hook under `.git/hooks/pre-commit`.
    Git,
}

impl HookTarget {
    /// CLI target string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Git => "git",
        }
    }
}

/// Hook management options.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookOptions {
    /// Hook target.
    pub target: HookTarget,
    /// Git base branch used by generated hooks.
    pub branch: String,
    /// Overwrite or remove existing unmanaged hook files.
    pub force: bool,
}

/// Machine-readable hook report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HooksReport {
    /// Schema identifier.
    pub schema_version: String,
    /// Typed JSON envelope discriminator.
    pub kind: String,
    /// Tool name.
    pub tool: String,
    /// Command that produced this report.
    pub command: String,
    /// Project root.
    pub root: PathBuf,
    /// Hook target.
    pub target: HookTarget,
    /// Git base branch used by generated hooks.
    pub branch: String,
    /// Target hook files.
    pub files: Vec<HookFile>,
}

/// Hook file status or mutation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookFile {
    /// Path relative to root.
    pub path: String,
    /// Whether the hook file exists.
    pub installed: bool,
    /// Whether Decimate owns the hook file content.
    pub managed: bool,
    /// Action performed.
    pub action: HookAction,
}

/// Hook action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookAction {
    /// Status only; no changes.
    Unchanged,
    /// Hook file was created.
    Created,
    /// Managed hook file was overwritten.
    Overwritten,
    /// Managed hook file was removed.
    Removed,
    /// Hook file did not exist.
    Missing,
}

/// Hook errors.
#[derive(Debug, Error)]
pub enum HooksError {
    /// Git hook target requires an initialized Git repository.
    #[error("cannot install git hook because {path} does not exist")]
    MissingGitDir {
        /// Missing `.git` directory.
        path: PathBuf,
    },
    /// Existing hook is not Decimate-managed and `force` was not set.
    #[error("refusing to overwrite unmanaged hook {path}; pass --force")]
    UnmanagedHook {
        /// Existing hook path.
        path: PathBuf,
    },
    /// Directory creation failed.
    #[error("failed to create hook directory {path}: {source}")]
    CreateDir {
        /// Directory path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Hook file write failed.
    #[error("failed to write hook {path}: {source}")]
    Write {
        /// Hook path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Hook file read failed.
    #[error("failed to read hook {path}: {source}")]
    Read {
        /// Hook path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Hook permission update failed.
    #[error("failed to mark hook executable {path}: {source}")]
    Permissions {
        /// Hook path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Hook removal failed.
    #[error("failed to remove hook {path}: {source}")]
    Remove {
        /// Hook path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
}

/// Inspect hook status.
///
/// # Errors
///
/// Returns [`HooksError`] if an existing hook cannot be read.
pub fn hooks_status(
    root: impl AsRef<Path>,
    options: &HookOptions,
) -> Result<HooksReport, HooksError> {
    let root = root.as_ref();
    let path = hook_path(root, options.target);
    let status = hook_status_file(root, &path, HookAction::Unchanged)?;
    Ok(report("hooks status", root, options, status))
}

/// Install a Decimate hook.
///
/// # Errors
///
/// Returns [`HooksError`] when the target is not available, the hook is
/// unmanaged without `force`, or file writes fail.
pub fn install_hooks(
    root: impl AsRef<Path>,
    options: &HookOptions,
) -> Result<HooksReport, HooksError> {
    let root = root.as_ref();
    let path = hook_path(root, options.target);
    ensure_git_dir(root)?;
    let action = write_git_hook(root, &path, options)?;
    let status = hook_status_file(root, &path, action)?;
    Ok(report("hooks install", root, options, status))
}

/// Uninstall a Decimate-managed hook.
///
/// # Errors
///
/// Returns [`HooksError`] when an unmanaged hook exists without `force`, or when
/// file removal fails.
pub fn uninstall_hooks(
    root: impl AsRef<Path>,
    options: &HookOptions,
) -> Result<HooksReport, HooksError> {
    let root = root.as_ref();
    let path = hook_path(root, options.target);
    let status = remove_git_hook(root, &path, options.force)?;
    Ok(report("hooks uninstall", root, options, status))
}

/// Render a human-readable hook report.
#[must_use]
pub fn render_hooks_report(report: &HooksReport) -> String {
    let Some(file) = report.files.first() else {
        return format!(
            "Hooks: {} target {}\n",
            report.command,
            report.target.as_str()
        );
    };
    let state = if file.installed {
        if file.managed {
            "installed"
        } else {
            "unmanaged"
        }
    } else {
        "not installed"
    };
    format!(
        "Hooks: {} target {} ({state})\n{}: {:?}\n",
        report.command,
        report.target.as_str(),
        file.path,
        file.action
    )
}

fn report(command: &str, root: &Path, options: &HookOptions, file: HookFile) -> HooksReport {
    HooksReport {
        schema_version: HOOKS_SCHEMA_VERSION.to_owned(),
        kind: "hooks".to_owned(),
        tool: "decimate".to_owned(),
        command: command.to_owned(),
        root: root.to_path_buf(),
        target: options.target,
        branch: options.branch.clone(),
        files: vec![file],
    }
}

fn ensure_git_dir(root: &Path) -> Result<(), HooksError> {
    let path = root.join(".git");
    if path.is_dir() {
        Ok(())
    } else {
        Err(HooksError::MissingGitDir { path })
    }
}

fn hook_path(root: &Path, target: HookTarget) -> PathBuf {
    match target {
        HookTarget::Git => root.join(".git/hooks/pre-commit"),
    }
}

fn hook_status_file(root: &Path, path: &Path, action: HookAction) -> Result<HookFile, HooksError> {
    let installed = path.is_file();
    let managed = installed && read_hook(path)?.contains(HOOK_MARKER);
    Ok(HookFile {
        path: relative_path(root, path),
        installed,
        managed,
        action,
    })
}

fn write_git_hook(
    root: &Path,
    path: &Path,
    options: &HookOptions,
) -> Result<HookAction, HooksError> {
    let action = if path.exists() {
        if !read_hook(path)?.contains(HOOK_MARKER) && !options.force {
            return Err(HooksError::UnmanagedHook {
                path: path.to_path_buf(),
            });
        }
        HookAction::Overwritten
    } else {
        HookAction::Created
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| HooksError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, git_hook_script(&options.branch)).map_err(|source| HooksError::Write {
        path: path.to_path_buf(),
        source,
    })?;
    mark_executable(path)?;
    let _ = root;
    Ok(action)
}

fn remove_git_hook(root: &Path, path: &Path, force: bool) -> Result<HookFile, HooksError> {
    if !path.exists() {
        return Ok(HookFile {
            path: relative_path(root, path),
            installed: false,
            managed: false,
            action: HookAction::Missing,
        });
    }
    let managed = read_hook(path)?.contains(HOOK_MARKER);
    if !managed && !force {
        return Err(HooksError::UnmanagedHook {
            path: path.to_path_buf(),
        });
    }
    fs::remove_file(path).map_err(|source| HooksError::Remove {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(HookFile {
        path: relative_path(root, path),
        installed: false,
        managed,
        action: HookAction::Removed,
    })
}

fn read_hook(path: &Path) -> Result<String, HooksError> {
    fs::read_to_string(path).map_err(|source| HooksError::Read {
        path: path.to_path_buf(),
        source,
    })
}

fn mark_executable(path: &Path) -> Result<(), HooksError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|source| HooksError::Permissions {
                path: path.to_path_buf(),
                source,
            })?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).map_err(|source| HooksError::Permissions {
            path: path.to_path_buf(),
            source,
        })
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

fn git_hook_script(branch: &str) -> String {
    format!(
        r#"#!/bin/sh
# {HOOK_MARKER}
set -eu
BASE="${{DECIMATE_BASE:-{branch}}}"
if ! command -v decimate >/dev/null 2>&1; then
  echo "decimate hook: decimate binary not found" >&2
  exit 2
fi
decimate audit . --base "$BASE" --format json --summary
"#
    )
}

fn relative_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
