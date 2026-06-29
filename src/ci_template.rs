use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Stable schema version for CI template output.
pub const CI_TEMPLATE_SCHEMA_VERSION: &str = "decimate.ci-template.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CiTemplatePlatform {
    Github,
    Gitlab,
}

impl CiTemplatePlatform {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::Gitlab => "gitlab",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiTemplateReport {
    pub schema_version: String,
    pub kind: String,
    pub tool: String,
    pub command: String,
    pub platform: CiTemplatePlatform,
    pub vendor: bool,
    pub files: Vec<CiTemplateFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiTemplateFile {
    pub path: String,
    pub executable: bool,
    pub content: String,
}

#[derive(Debug, Error)]
pub enum CiTemplateError {
    #[error("refusing to overwrite existing CI template file {path}; pass --force")]
    AlreadyExists { path: PathBuf },
    #[error("failed to create directory {path}: {source}")]
    CreateDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write CI template file {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to mark CI helper executable {path}: {source}")]
    Permissions {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[must_use]
pub fn ci_template_report(platform: CiTemplatePlatform, vendor: bool) -> CiTemplateReport {
    CiTemplateReport {
        schema_version: CI_TEMPLATE_SCHEMA_VERSION.to_owned(),
        kind: "ci-template".to_owned(),
        tool: "decimate".to_owned(),
        command: "ci-template".to_owned(),
        platform,
        vendor,
        files: template_files(platform, vendor),
    }
}

/// Write the selected CI template files under `root`.
///
/// # Errors
///
/// Returns [`CiTemplateError`] when a target file already exists without
/// `force`, or when directories, files, or executable permissions cannot be
/// written.
pub fn vendor_ci_template(
    root: impl AsRef<Path>,
    platform: CiTemplatePlatform,
    force: bool,
) -> Result<CiTemplateReport, CiTemplateError> {
    let report = ci_template_report(platform, true);
    let root = root.as_ref();
    for file in &report.files {
        let path = root.join(&file.path);
        write_template_file(&path, file, force)?;
    }
    Ok(report)
}

#[must_use]
pub fn render_ci_template(report: &CiTemplateReport) -> String {
    if !report.vendor {
        return report
            .files
            .first()
            .map_or_else(String::new, |file| file.content.clone());
    }

    report
        .files
        .iter()
        .map(|file| format!("# {}\n{}", file.path, file.content))
        .collect::<Vec<_>>()
        .join("\n")
}

fn write_template_file(
    path: &Path,
    file: &CiTemplateFile,
    force: bool,
) -> Result<(), CiTemplateError> {
    if path.exists() && !force {
        return Err(CiTemplateError::AlreadyExists {
            path: path.to_path_buf(),
        });
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| CiTemplateError::CreateDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, &file.content).map_err(|source| CiTemplateError::Write {
        path: path.to_path_buf(),
        source,
    })?;
    mark_executable(path, file.executable)
}

fn mark_executable(path: &Path, executable: bool) -> Result<(), CiTemplateError> {
    if !executable {
        return Ok(());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)
            .map_err(|source| CiTemplateError::Permissions {
                path: path.to_path_buf(),
                source,
            })?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).map_err(|source| CiTemplateError::Permissions {
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

fn template_files(platform: CiTemplatePlatform, vendor: bool) -> Vec<CiTemplateFile> {
    match (platform, vendor) {
        (CiTemplatePlatform::Github, _) => vec![template_file(
            ".github/workflows/decimate.yml",
            false,
            GITHUB_WORKFLOW,
        )],
        (CiTemplatePlatform::Gitlab, false) => {
            vec![template_file(".gitlab-ci.yml", false, GITLAB_CI)]
        }
        (CiTemplatePlatform::Gitlab, true) => vec![
            template_file("ci/gitlab-ci.yml", false, GITLAB_VENDORED_CI),
            template_file("ci/scripts/review.sh", true, GITLAB_REVIEW_SCRIPT),
            template_file("ci/scripts/comment.sh", true, GITLAB_COMMENT_SCRIPT),
        ],
    }
}

fn template_file(path: &str, executable: bool, content: &str) -> CiTemplateFile {
    CiTemplateFile {
        path: path.to_owned(),
        executable,
        content: content.to_owned(),
    }
}

const GITHUB_WORKFLOW: &str = r"name: Decimate

on:
  pull_request:
  push:
    branches: [main]

jobs:
  decimate:
    runs-on: ubuntu-latest
    permissions:
      contents: read
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install --locked decimate
      - run: decimate audit --format json --base origin/${{ github.base_ref || 'main' }}
";

const GITLAB_CI: &str = r#"stages:
  - quality

decimate:
  stage: quality
  image: rust:latest
  before_script:
    - cargo install --locked decimate
  script:
    - decimate audit --format json --base "origin/${CI_MERGE_REQUEST_TARGET_BRANCH_NAME:-main}"
  rules:
    - if: $CI_MERGE_REQUEST_IID
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
"#;

const GITLAB_VENDORED_CI: &str = r".decimate:
  stage: quality
  image: rust:latest
  before_script:
    - cargo install --locked decimate
  script:
    - ci/scripts/review.sh
  rules:
    - if: $CI_MERGE_REQUEST_IID
    - if: $CI_COMMIT_BRANCH == $CI_DEFAULT_BRANCH
";

const GITLAB_REVIEW_SCRIPT: &str = r#"#!/usr/bin/env sh
set -eu

BASE="origin/${CI_MERGE_REQUEST_TARGET_BRANCH_NAME:-main}"
decimate audit --format json --base "$BASE"
"#;

const GITLAB_COMMENT_SCRIPT: &str = r#"#!/usr/bin/env sh
set -eu

decimate review --format json --base "origin/${CI_MERGE_REQUEST_TARGET_BRANCH_NAME:-main}"
"#;
