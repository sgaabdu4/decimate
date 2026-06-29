use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::graph::normalize_against;
use crate::{Location, ScannedProject};

mod detect;
use detect::{detect_in_source, is_ignored_path};

/// Security candidate detector options.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityOptions {
    /// Limit output to the N most frequently reported candidate groups.
    pub top: Option<usize>,
    /// Include attack-surface inventory entries.
    pub surface: bool,
    /// Enabled candidate categories. Empty means all categories.
    pub categories: BTreeSet<SecurityCategory>,
}

/// Security candidate report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityReport {
    /// Options used to compute this report.
    pub options: SecurityOptions,
    /// Dart files included in security detection.
    pub analyzed_files: usize,
    /// Grouped unverified security candidates.
    pub candidates: Vec<SecurityCandidate>,
    /// Raw security candidate occurrence count before `--top` truncation.
    pub total_occurrences: usize,
    /// Optional attack-surface inventory.
    pub attack_surface: Vec<AttackSurfaceEntry>,
}

/// One grouped unverified security candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityCandidate {
    /// Stable rule id.
    pub rule_id: String,
    /// Candidate category.
    pub category: SecurityCategory,
    /// API surface or sink family.
    pub sink: String,
    /// Detection confidence.
    pub confidence: SecurityConfidence,
    /// Candidate occurrences.
    pub occurrences: Vec<SecurityOccurrence>,
}

/// Security candidate category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecurityCategory {
    /// Secret-shaped literal or secret-named assignment.
    HardcodedSecret,
    /// Remote cleartext HTTP transport.
    InsecureTransport,
    /// TLS validation bypass.
    TlsBypass,
    /// `WebView` JavaScript or file access exposure.
    WebViewRisk,
    /// Process execution with shell or dynamic command material.
    ProcessExecution,
    /// Raw SQL with interpolation or dynamic query text.
    RawSql,
    /// Secret-like material written to plain local storage.
    PlainSecretStorage,
}

impl SecurityCategory {
    const fn rule_id(self) -> &'static str {
        match self {
            Self::HardcodedSecret => "decimate/security-hardcoded-secret",
            Self::InsecureTransport => "decimate/security-insecure-transport",
            Self::TlsBypass => "decimate/security-tls-bypass",
            Self::WebViewRisk => "decimate/security-webview-risk",
            Self::ProcessExecution => "decimate/security-process-execution",
            Self::RawSql => "decimate/security-raw-sql",
            Self::PlainSecretStorage => "decimate/security-plain-secret-storage",
        }
    }
}

impl SecurityOptions {
    fn includes_category(&self, category: SecurityCategory) -> bool {
        self.categories.is_empty() || self.categories.contains(&category)
    }
}

/// Detection confidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecurityConfidence {
    /// Low-confidence heuristic.
    Low,
    /// Medium-confidence heuristic.
    Medium,
    /// High-confidence known risky surface.
    High,
}

/// One security candidate occurrence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityOccurrence {
    /// Dart file path.
    pub path: PathBuf,
    /// Location of the candidate.
    pub location: Location,
    /// Matched expression or API surface.
    pub expression: String,
    /// Redacted source-line evidence.
    pub evidence: String,
}

/// Attack-surface inventory entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttackSurfaceEntry {
    /// Candidate category exposed on this surface.
    pub category: SecurityCategory,
    /// Dart file path.
    pub path: PathBuf,
    /// Location of the surface.
    pub location: Location,
    /// API surface or boundary.
    pub surface: String,
    /// Verification prompt for downstream agents.
    pub verification_prompt: String,
}

/// Errors returned while detecting security candidates.
#[derive(Debug, Error)]
pub enum SecurityError {
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
struct CandidateGroup {
    rule_id: String,
    category: SecurityCategory,
    sink: String,
    confidence: SecurityConfidence,
    occurrences: Vec<SecurityOccurrence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DetectedSecurityCandidate {
    category: SecurityCategory,
    sink: String,
    confidence: SecurityConfidence,
    occurrence: SecurityOccurrence,
}

/// Detect unverified local security review candidates in Dart and Flutter code.
///
/// # Errors
///
/// Returns [`SecurityError`] if a scanned Dart file cannot be read.
pub fn analyze_security(
    project: &ScannedProject,
    options: &SecurityOptions,
) -> Result<SecurityReport, SecurityError> {
    let mut groups = BTreeMap::<(SecurityCategory, String), CandidateGroup>::new();
    let mut analyzed_files = 0;

    for file in &project.files {
        let path = normalize_against(&project.root, &file.path);
        if !path.starts_with(&project.root) || is_ignored_path(&path) {
            continue;
        }
        analyzed_files += 1;
        let source = fs::read_to_string(&path).map_err(|source| SecurityError::ReadFile {
            path: path.clone(),
            source,
        })?;
        for detected in detect_in_source(&path, &source)
            .into_iter()
            .filter(|candidate| options.includes_category(candidate.category))
        {
            let key = (detected.category, detected.sink.clone());
            let group = groups.entry(key).or_insert_with(|| CandidateGroup {
                rule_id: detected.category.rule_id().to_owned(),
                category: detected.category,
                sink: detected.sink.clone(),
                confidence: detected.confidence,
                occurrences: Vec::new(),
            });
            group.confidence = group.confidence.max(detected.confidence);
            group.occurrences.push(detected.occurrence);
        }
    }

    let total_occurrences = groups
        .values()
        .map(|group| group.occurrences.len())
        .sum::<usize>();
    let mut candidates = groups
        .into_values()
        .map(SecurityCandidate::from)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        (
            std::cmp::Reverse(left.occurrences.len()),
            left.category,
            &left.sink,
        )
            .cmp(&(
                std::cmp::Reverse(right.occurrences.len()),
                right.category,
                &right.sink,
            ))
    });
    if let Some(top) = options.top {
        candidates.truncate(top);
    }
    let attack_surface = if options.surface {
        attack_surface_for(&candidates)
    } else {
        Vec::new()
    };

    Ok(SecurityReport {
        options: options.clone(),
        analyzed_files,
        candidates,
        total_occurrences,
        attack_surface,
    })
}

impl From<CandidateGroup> for SecurityCandidate {
    fn from(group: CandidateGroup) -> Self {
        let mut seen = BTreeSet::new();
        let mut occurrences = group
            .occurrences
            .into_iter()
            .filter(|occurrence| {
                seen.insert((
                    occurrence.path.clone(),
                    occurrence.location.line,
                    occurrence.location.column,
                    occurrence.expression.clone(),
                ))
            })
            .collect::<Vec<_>>();
        occurrences.sort_by(|left, right| {
            (&left.path, left.location.line, left.location.column).cmp(&(
                &right.path,
                right.location.line,
                right.location.column,
            ))
        });
        Self {
            rule_id: group.rule_id,
            category: group.category,
            sink: group.sink,
            confidence: group.confidence,
            occurrences,
        }
    }
}

fn attack_surface_for(candidates: &[SecurityCandidate]) -> Vec<AttackSurfaceEntry> {
    candidates
        .iter()
        .flat_map(|candidate| {
            candidate
                .occurrences
                .iter()
                .map(|occurrence| AttackSurfaceEntry {
                    category: candidate.category,
                    path: occurrence.path.clone(),
                    location: occurrence.location,
                    surface: candidate.sink.clone(),
                    verification_prompt: verification_prompt(candidate.category).to_owned(),
                })
        })
        .collect()
}

const fn verification_prompt(category: SecurityCategory) -> &'static str {
    match category {
        SecurityCategory::HardcodedSecret => {
            "Verify whether this literal is a real secret and rotate it if confirmed."
        }
        SecurityCategory::InsecureTransport => {
            "Verify whether this remote HTTP endpoint can expose sensitive traffic."
        }
        SecurityCategory::TlsBypass => {
            "Verify whether certificate validation can be bypassed outside trusted development code."
        }
        SecurityCategory::WebViewRisk => {
            "Verify whether untrusted content can execute JavaScript or access local files."
        }
        SecurityCategory::ProcessExecution => {
            "Verify whether attacker-controlled input can influence the executable, arguments, or shell."
        }
        SecurityCategory::RawSql => {
            "Verify whether untrusted input can alter SQL text instead of using parameters."
        }
        SecurityCategory::PlainSecretStorage => {
            "Verify whether secret material is persisted outside secure storage."
        }
    }
}

#[cfg(test)]
mod tests;
