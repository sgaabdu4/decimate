use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::format::display_path;
use super::{Finding, FindingAction, FindingKind, Severity};
use crate::{
    AttackSurfaceEntry, SecurityCandidate, SecurityCategory, SecurityConfidence, SecurityReport,
};

/// Security candidate serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonSecurityCandidate {
    /// Stable rule id.
    pub rule_id: String,
    /// Stable candidate fingerprint.
    pub fingerprint: String,
    /// Candidate category.
    pub category: SecurityCategory,
    /// API surface or sink family.
    pub sink: String,
    /// Detection confidence.
    pub confidence: SecurityConfidence,
    /// Candidate occurrences.
    pub occurrences: Vec<JsonSecurityOccurrence>,
}

/// One security candidate occurrence serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonSecurityOccurrence {
    /// Dart file path, root-relative where possible.
    pub path: String,
    /// 1-based line.
    pub line: usize,
    /// 0-based byte column.
    pub column: usize,
    /// Matched expression or API surface.
    pub expression: String,
    /// Redacted source-line evidence.
    pub evidence: String,
}

/// Attack-surface entry serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonAttackSurfaceEntry {
    /// Candidate category exposed on this surface.
    pub category: SecurityCategory,
    /// Dart file path, root-relative where possible.
    pub path: String,
    /// 1-based line.
    pub line: usize,
    /// 0-based byte column.
    pub column: usize,
    /// API surface or boundary.
    pub surface: String,
    /// Verification prompt for downstream agents.
    pub verification_prompt: String,
}

pub(super) fn add_security_findings(
    root: &Path,
    report: &SecurityReport,
    findings: &mut Vec<Finding>,
) {
    findings.extend(
        report
            .candidates
            .iter()
            .map(|candidate| security_finding(root, candidate)),
    );
}

pub(super) fn json_security_candidates(
    root: &Path,
    report: &SecurityReport,
) -> Vec<JsonSecurityCandidate> {
    report
        .candidates
        .iter()
        .map(|candidate| JsonSecurityCandidate {
            rule_id: candidate.rule_id.clone(),
            fingerprint: security_fingerprint(candidate),
            category: candidate.category,
            sink: candidate.sink.clone(),
            confidence: candidate.confidence,
            occurrences: candidate
                .occurrences
                .iter()
                .map(|occurrence| JsonSecurityOccurrence {
                    path: display_path(root, &occurrence.path),
                    line: occurrence.location.line,
                    column: occurrence.location.column,
                    expression: occurrence.expression.clone(),
                    evidence: occurrence.evidence.clone(),
                })
                .collect(),
        })
        .collect()
}

pub(super) fn json_attack_surface(
    root: &Path,
    report: &SecurityReport,
) -> Vec<JsonAttackSurfaceEntry> {
    report
        .attack_surface
        .iter()
        .map(|entry| attack_surface_entry(root, entry))
        .collect()
}

fn security_finding(root: &Path, candidate: &SecurityCandidate) -> Finding {
    let first = &candidate.occurrences[0];
    let path = display_path(root, &first.path);
    let files = candidate
        .occurrences
        .iter()
        .map(|occurrence| display_path(root, &occurrence.path))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    Finding {
        rule_id: candidate.rule_id.clone(),
        fingerprint: Some(security_fingerprint(candidate)),
        kind: FindingKind::SecurityCandidate,
        severity: Severity::Error,
        message: format!(
            "Security review candidate for {} via {}",
            category_name(candidate.category),
            candidate.sink
        ),
        path: path.clone(),
        line: first.location.line,
        column: first.location.column,
        safe_to_delete: false,
        files,
        edge: None,
        actions: vec![
            FindingAction::new(
                "review-security-candidate",
                "Verify source control, reachability, and defensive controls before editing",
                false,
            )
            .with_target_path(path.clone())
            .with_target_symbol(candidate.sink.clone())
            .with_decimate_args(["inspect", "--format", "json", "--file", path.as_str()])
            .with_suppression_comment("// decimate-ignore-next-line security-sink"),
        ],
    }
}

fn attack_surface_entry(root: &Path, entry: &AttackSurfaceEntry) -> JsonAttackSurfaceEntry {
    JsonAttackSurfaceEntry {
        category: entry.category,
        path: display_path(root, &entry.path),
        line: entry.location.line,
        column: entry.location.column,
        surface: entry.surface.clone(),
        verification_prompt: entry.verification_prompt.clone(),
    }
}

fn security_fingerprint(candidate: &SecurityCandidate) -> String {
    let text = format!(
        "{}:{}:{}",
        candidate.rule_id,
        category_name(candidate.category),
        candidate.sink
    );
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("sec:{:08x}", hash & 0xffff_ffff)
}

const fn category_name(category: SecurityCategory) -> &'static str {
    match category {
        SecurityCategory::HardcodedSecret => "hardcoded secret",
        SecurityCategory::InsecureTransport => "insecure transport",
        SecurityCategory::TlsBypass => "TLS bypass",
        SecurityCategory::WebViewRisk => "WebView risk",
        SecurityCategory::ProcessExecution => "process execution",
        SecurityCategory::RawSql => "raw SQL",
        SecurityCategory::PlainSecretStorage => "plain secret storage",
    }
}
