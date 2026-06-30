use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use super::format::display_path;
use super::types::{
    AuditAttribution, AuditAttributionCounts, AuditRiskLevel, Finding, JsonReport, ReportCommand,
    Severity,
};

/// Refresh Fallow-style changed-code risk fields for an audit report.
pub fn apply_audit_risk(root: &Path, changed_files: &[PathBuf], report: &mut JsonReport) {
    if report.command != ReportCommand::Audit {
        return;
    }

    let changed = changed_files
        .iter()
        .map(|path| display_path(root, path))
        .collect::<BTreeSet<_>>();
    let attribution = audit_attribution(&report.findings, &changed);
    let risk_score = risk_score(&attribution);
    let risk_level = risk_level(&attribution);

    report.summary.risk_score = Some(risk_score);
    report.summary.risk_level = Some(risk_level);
    report.summary.attribution = Some(attribution);
}

fn audit_attribution(findings: &[Finding], changed: &BTreeSet<String>) -> AuditAttribution {
    let mut introduced = AttributionBucket::default();
    let mut pre_existing = AttributionBucket::default();

    for finding in findings {
        if touches_changed_file(finding, changed) {
            introduced.add(finding);
        } else {
            pre_existing.add(finding);
        }
    }

    AuditAttribution {
        introduced: introduced.finish(),
        pre_existing: pre_existing.finish(),
    }
}

fn touches_changed_file(finding: &Finding, changed: &BTreeSet<String>) -> bool {
    changed.contains(&finding.path)
        || finding.files.iter().any(|path| changed.contains(path))
        || finding
            .edge
            .as_ref()
            .is_some_and(|edge| changed.contains(&edge.from) || changed.contains(&edge.to))
}

#[derive(Default)]
struct AttributionBucket {
    findings: usize,
    error_findings: usize,
    warning_findings: usize,
    safe_to_delete: usize,
    files: BTreeSet<String>,
}

impl AttributionBucket {
    fn add(&mut self, finding: &Finding) {
        self.findings += 1;
        match finding.severity {
            Severity::Error => self.error_findings += 1,
            Severity::Warning => self.warning_findings += 1,
        }
        if finding.safe_to_delete {
            self.safe_to_delete += 1;
        }
        add_finding_files(finding, &mut self.files);
    }

    fn finish(self) -> AuditAttributionCounts {
        AuditAttributionCounts {
            findings: self.findings,
            error_findings: self.error_findings,
            warning_findings: self.warning_findings,
            safe_to_delete: self.safe_to_delete,
            files: self.files.len(),
        }
    }
}

fn add_finding_files(finding: &Finding, files: &mut BTreeSet<String>) {
    if !finding.path.is_empty() {
        files.insert(finding.path.clone());
    }
    files.extend(finding.files.iter().cloned());
    if let Some(edge) = &finding.edge {
        files.insert(edge.from.clone());
        files.insert(edge.to.clone());
    }
}

fn risk_score(attribution: &AuditAttribution) -> usize {
    let introduced = &attribution.introduced;
    let pre_existing = &attribution.pre_existing;
    introduced
        .error_findings
        .saturating_mul(30)
        .saturating_add(introduced.warning_findings.saturating_mul(15))
        .saturating_add(introduced.safe_to_delete.saturating_mul(10))
        .saturating_add(pre_existing.error_findings.saturating_mul(10))
        .saturating_add(pre_existing.warning_findings.saturating_mul(4))
        .saturating_add(pre_existing.safe_to_delete.saturating_mul(2))
        .min(100)
}

fn risk_level(attribution: &AuditAttribution) -> AuditRiskLevel {
    if attribution.introduced.error_findings > 0 {
        AuditRiskLevel::Fail
    } else if attribution.introduced.findings > 0 || attribution.pre_existing.findings > 0 {
        AuditRiskLevel::Warn
    } else {
        AuditRiskLevel::Pass
    }
}
