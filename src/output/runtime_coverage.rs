use std::path::Path;

use serde::{Deserialize, Serialize};

use super::format::display_path;
use crate::{
    LowTrafficThreshold, RuntimeBlastRadius, RuntimeBlastRisk, RuntimeCoverageAction,
    RuntimeCoverageConfidence, RuntimeCoverageFinding, RuntimeCoverageFindingKind,
    RuntimeCoverageFormat, RuntimeCoverageIntelligence, RuntimeCoverageIntelligenceKind,
    RuntimeCoverageReport, RuntimeHotPath, RuntimeImportance, SourceMapConfidence,
};

/// Runtime coverage intelligence serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRuntimeCoverage {
    /// Runtime coverage verdict.
    pub verdict: String,
    /// High-level runtime coverage signals present in this report.
    pub signals: Vec<String>,
    /// Runtime coverage summary.
    pub summary: JsonRuntimeCoverageSummary,
    /// Runtime cleanup/review signals.
    pub findings: Vec<JsonRuntimeCoverageFinding>,
    /// Runtime hot paths.
    pub hot_paths: Vec<JsonRuntimeHotPath>,
    /// Static/runtime recommendations for agent review.
    pub coverage_intelligence: Vec<JsonRuntimeCoverageIntelligence>,
    /// Caller blast-radius rows for hot runtime paths.
    pub blast_radius: Vec<JsonRuntimeBlastRadius>,
    /// Runtime-weighted production importance rows.
    pub importance: Vec<JsonRuntimeImportance>,
    /// Agent-friendly candidate buckets.
    pub actionable: JsonRuntimeCoverageActionable,
    /// Runtime coverage input provenance.
    pub provenance: JsonRuntimeCoverageProvenance,
    /// Observation watermark.
    pub watermark: JsonRuntimeCoverageWatermark,
    /// Non-fatal runtime coverage warnings.
    pub warnings: Vec<String>,
}

/// Runtime coverage summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRuntimeCoverageSummary {
    /// Files observed in runtime coverage.
    pub observed_files: usize,
    /// Total observed runtime invocations.
    pub total_invocations: usize,
    /// Runtime hot path count.
    pub hot_paths: usize,
    /// Runtime finding count.
    pub findings: usize,
    /// `safe_to_delete` finding count.
    pub safe_to_delete: usize,
    /// `review_required` finding count.
    pub review_required: usize,
    /// Low-traffic finding count.
    pub low_traffic: usize,
    /// Coverage-unavailable finding count.
    pub coverage_unavailable: usize,
    /// Hot path threshold used for this run.
    pub min_invocations_hot: usize,
    /// Observation volume threshold used for this run.
    pub min_observation_volume: usize,
    /// Low-traffic threshold used for this run.
    pub low_traffic_threshold: LowTrafficThreshold,
}

/// Runtime coverage finding serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRuntimeCoverageFinding {
    /// Stable finding id.
    pub id: String,
    /// Runtime finding kind.
    pub kind: String,
    /// Dart file path, root-relative where possible.
    pub path: String,
    /// 1-based line.
    pub line: usize,
    /// Observed runtime invocations.
    pub invocations: usize,
    /// Fraction of all runtime observations.
    pub traffic_fraction: LowTrafficThreshold,
    /// Whether graph plus runtime evidence supports deletion.
    pub safe_to_delete: bool,
    /// Whether a human or agent should review before changing.
    pub review_required: bool,
    /// Runtime confidence for the signal.
    pub confidence: String,
    /// Agent-readable reason.
    pub reason: String,
}

/// Runtime hot path serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRuntimeHotPath {
    /// Dart file path, root-relative where possible.
    pub path: String,
    /// 1-based line when available.
    pub line: Option<usize>,
    /// Runtime function name when available.
    pub symbol: Option<String>,
    /// Observed runtime invocations.
    pub invocations: usize,
    /// Source map confidence.
    pub source_map_confidence: String,
}

/// Runtime coverage recommendation serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRuntimeCoverageIntelligence {
    /// Stable recommendation id.
    pub id: String,
    /// Cross-surface function id.
    pub stable_id: String,
    /// Recommendation kind.
    pub kind: String,
    /// Dart file path, root-relative where possible.
    pub path: String,
    /// 1-based line when available.
    pub line: Option<usize>,
    /// Runtime function name when available.
    pub symbol: Option<String>,
    /// Suggested action.
    pub action: String,
    /// Priority from 0 to 100.
    pub priority: usize,
    /// Runtime confidence for the recommendation.
    pub confidence: String,
    /// Agent-readable reason.
    pub reason: String,
    /// Supporting signal labels.
    pub signals: Vec<String>,
}

/// Runtime blast-radius row serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRuntimeBlastRadius {
    /// Stable blast-radius id.
    pub id: String,
    /// Cross-surface function id.
    pub stable_id: String,
    /// Dart file path, root-relative where possible.
    pub path: String,
    /// 1-based line when available.
    pub line: Option<usize>,
    /// Runtime function name when available.
    pub symbol: Option<String>,
    /// Number of graph callers.
    pub caller_count: usize,
    /// Direct graph callers.
    pub callers: Vec<String>,
    /// Fraction of runtime traffic observed in caller files.
    pub traffic_weighted_caller_reach: LowTrafficThreshold,
    /// Low, medium, or high review risk.
    pub risk: String,
}

/// Runtime importance row serialized in JSON reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRuntimeImportance {
    /// Stable importance id.
    pub id: String,
    /// Cross-surface function id.
    pub stable_id: String,
    /// Dart file path, root-relative where possible.
    pub path: String,
    /// 1-based line when available.
    pub line: Option<usize>,
    /// Runtime function name when available.
    pub symbol: Option<String>,
    /// Observed runtime invocations.
    pub invocations: usize,
    /// Fraction of total runtime traffic.
    pub traffic_fraction: LowTrafficThreshold,
    /// Static cyclomatic complexity for the matching function.
    pub cyclomatic_complexity: usize,
    /// Number of graph callers.
    pub caller_count: usize,
    /// Runtime-weighted priority score.
    pub score: usize,
    /// Agent-readable reason.
    pub reason: String,
}

/// Agent-friendly runtime coverage candidate buckets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRuntimeCoverageActionable {
    /// Finding ids that are safe to delete.
    pub safe_to_delete: Vec<String>,
    /// Finding ids that require review before changing.
    pub review_required: Vec<String>,
    /// Low-traffic finding ids.
    pub low_traffic: Vec<String>,
    /// Coverage-unavailable finding ids.
    pub coverage_unavailable: Vec<String>,
}

/// Runtime coverage input provenance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRuntimeCoverageProvenance {
    /// Runtime coverage input path.
    pub source_path: String,
    /// Runtime coverage source format.
    pub source_format: String,
    /// Stable hash of the coverage input payload.
    pub source_hash: String,
    /// Local or cloud capture mode.
    pub mode: String,
    /// Capture quality derived from observation volume.
    pub capture_quality: String,
}

/// Runtime coverage observation watermark.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JsonRuntimeCoverageWatermark {
    /// Files observed in runtime coverage.
    pub observed_files: usize,
    /// Total observed runtime invocations.
    pub total_invocations: usize,
}

#[must_use]
pub fn json_runtime_coverage(root: &Path, report: &RuntimeCoverageReport) -> JsonRuntimeCoverage {
    let findings = report
        .findings
        .iter()
        .map(|finding| json_finding(root, finding))
        .collect::<Vec<_>>();
    let hot_paths = report
        .hot_paths
        .iter()
        .map(|path| json_hot_path(root, path))
        .collect::<Vec<_>>();
    let coverage_intelligence = report
        .coverage_intelligence
        .iter()
        .map(|row| json_coverage_intelligence(root, row))
        .collect();
    let blast_radius = report
        .blast_radius
        .iter()
        .map(|row| json_blast_radius(root, row))
        .collect();
    let importance = report
        .importance
        .iter()
        .map(|row| json_importance(root, row))
        .collect();
    JsonRuntimeCoverage {
        verdict: runtime_verdict(report),
        signals: runtime_signals(report),
        summary: runtime_summary(report),
        actionable: runtime_actionable(&findings),
        provenance: JsonRuntimeCoverageProvenance {
            source_path: display_path(root, &report.source_path),
            source_format: source_format(report.source_format).to_owned(),
            source_hash: report.source_hash.clone(),
            mode: "local".to_owned(),
            capture_quality: capture_quality(report).to_owned(),
        },
        watermark: JsonRuntimeCoverageWatermark {
            observed_files: report.observed_files,
            total_invocations: report.total_invocations,
        },
        findings,
        hot_paths,
        coverage_intelligence,
        blast_radius,
        importance,
        warnings: report.warnings.clone(),
    }
}

fn runtime_summary(report: &RuntimeCoverageReport) -> JsonRuntimeCoverageSummary {
    JsonRuntimeCoverageSummary {
        observed_files: report.observed_files,
        total_invocations: report.total_invocations,
        hot_paths: report.hot_paths.len(),
        findings: report.findings.len(),
        safe_to_delete: report
            .findings
            .iter()
            .filter(|finding| finding.safe_to_delete)
            .count(),
        review_required: report
            .findings
            .iter()
            .filter(|finding| finding.review_required)
            .count(),
        low_traffic: report
            .findings
            .iter()
            .filter(|finding| finding.kind == RuntimeCoverageFindingKind::LowTraffic)
            .count(),
        coverage_unavailable: report
            .findings
            .iter()
            .filter(|finding| finding.kind == RuntimeCoverageFindingKind::CoverageUnavailable)
            .count(),
        min_invocations_hot: report.min_invocations_hot,
        min_observation_volume: report.min_observation_volume,
        low_traffic_threshold: report.low_traffic_threshold,
    }
}

fn json_coverage_intelligence(
    root: &Path,
    row: &RuntimeCoverageIntelligence,
) -> JsonRuntimeCoverageIntelligence {
    JsonRuntimeCoverageIntelligence {
        id: row.id.clone(),
        stable_id: row.stable_id.clone(),
        kind: intelligence_kind(row.kind).to_owned(),
        path: display_path(root, &row.path),
        line: row.line,
        symbol: row.symbol.clone(),
        action: runtime_action(row.action).to_owned(),
        priority: row.priority,
        confidence: confidence(row.confidence).to_owned(),
        reason: row.reason.clone(),
        signals: row.signals.clone(),
    }
}

fn json_blast_radius(root: &Path, row: &RuntimeBlastRadius) -> JsonRuntimeBlastRadius {
    JsonRuntimeBlastRadius {
        id: row.id.clone(),
        stable_id: row.stable_id.clone(),
        path: display_path(root, &row.path),
        line: row.line,
        symbol: row.symbol.clone(),
        caller_count: row.caller_count,
        callers: row
            .callers
            .iter()
            .map(|caller| display_path(root, caller))
            .collect(),
        traffic_weighted_caller_reach: row.traffic_weighted_caller_reach,
        risk: blast_risk(row.risk).to_owned(),
    }
}

fn json_importance(root: &Path, row: &RuntimeImportance) -> JsonRuntimeImportance {
    JsonRuntimeImportance {
        id: row.id.clone(),
        stable_id: row.stable_id.clone(),
        path: display_path(root, &row.path),
        line: row.line,
        symbol: row.symbol.clone(),
        invocations: row.invocations,
        traffic_fraction: row.traffic_fraction,
        cyclomatic_complexity: row.cyclomatic_complexity,
        caller_count: row.caller_count,
        score: row.score,
        reason: row.reason.clone(),
    }
}

fn runtime_actionable(findings: &[JsonRuntimeCoverageFinding]) -> JsonRuntimeCoverageActionable {
    JsonRuntimeCoverageActionable {
        safe_to_delete: findings
            .iter()
            .filter(|finding| finding.safe_to_delete)
            .map(|finding| finding.id.clone())
            .collect(),
        review_required: findings
            .iter()
            .filter(|finding| finding.review_required)
            .map(|finding| finding.id.clone())
            .collect(),
        low_traffic: findings
            .iter()
            .filter(|finding| finding.kind == "low-traffic")
            .map(|finding| finding.id.clone())
            .collect(),
        coverage_unavailable: findings
            .iter()
            .filter(|finding| finding.kind == "coverage-unavailable")
            .map(|finding| finding.id.clone())
            .collect(),
    }
}

fn json_finding(root: &Path, finding: &RuntimeCoverageFinding) -> JsonRuntimeCoverageFinding {
    let path = display_path(root, &finding.path);
    JsonRuntimeCoverageFinding {
        id: format!("runtime-coverage:{}:{path}", finding_kind(finding.kind)),
        kind: finding_kind(finding.kind).to_owned(),
        path,
        line: finding.line,
        invocations: finding.invocations,
        traffic_fraction: finding.traffic_fraction,
        safe_to_delete: finding.safe_to_delete,
        review_required: finding.review_required,
        confidence: confidence(finding.confidence).to_owned(),
        reason: finding.reason.clone(),
    }
}

fn json_hot_path(root: &Path, path: &RuntimeHotPath) -> JsonRuntimeHotPath {
    JsonRuntimeHotPath {
        path: display_path(root, &path.path),
        line: path.line,
        symbol: path.symbol.clone(),
        invocations: path.invocations,
        source_map_confidence: source_map_confidence(path.source_map_confidence).to_owned(),
    }
}

fn runtime_verdict(report: &RuntimeCoverageReport) -> String {
    if report.observed_files == 0 || !report.warnings.is_empty() {
        "warn".to_owned()
    } else {
        "pass".to_owned()
    }
}

fn runtime_signals(report: &RuntimeCoverageReport) -> Vec<String> {
    let mut signals = vec!["runtime-coverage".to_owned()];
    if !report.hot_paths.is_empty() {
        signals.push("hot-paths".to_owned());
    }
    if !report.coverage_intelligence.is_empty() {
        signals.push("coverage-intelligence".to_owned());
    }
    if !report.blast_radius.is_empty() {
        signals.push("blast-radius".to_owned());
    }
    if !report.importance.is_empty() {
        signals.push("importance".to_owned());
    }
    if report
        .findings
        .iter()
        .any(|finding| finding.kind == RuntimeCoverageFindingKind::LowTraffic)
    {
        signals.push("low-traffic".to_owned());
    }
    if report
        .findings
        .iter()
        .any(|finding| finding.kind == RuntimeCoverageFindingKind::CoverageUnavailable)
    {
        signals.push("coverage-unavailable".to_owned());
    }
    if report.total_invocations < report.min_observation_volume {
        signals.push("low-observation-volume".to_owned());
    }
    signals
}

fn intelligence_kind(kind: RuntimeCoverageIntelligenceKind) -> &'static str {
    match kind {
        RuntimeCoverageIntelligenceKind::HotPathTouched => "hot-path-touched",
        RuntimeCoverageIntelligenceKind::LowTraffic => "low-traffic",
        RuntimeCoverageIntelligenceKind::CoverageUnavailable => "coverage-unavailable",
    }
}

fn runtime_action(action: RuntimeCoverageAction) -> &'static str {
    match action {
        RuntimeCoverageAction::ReviewRuntime => "review-runtime",
        RuntimeCoverageAction::DeleteColdCode => "delete-cold-code",
    }
}

fn blast_risk(risk: RuntimeBlastRisk) -> &'static str {
    match risk {
        RuntimeBlastRisk::Low => "low",
        RuntimeBlastRisk::Medium => "medium",
        RuntimeBlastRisk::High => "high",
    }
}

fn capture_quality(report: &RuntimeCoverageReport) -> &'static str {
    if report.total_invocations == 0 {
        "low"
    } else if report.total_invocations >= report.min_observation_volume {
        "high"
    } else {
        "medium"
    }
}

fn source_format(format: RuntimeCoverageFormat) -> &'static str {
    match format {
        RuntimeCoverageFormat::Istanbul => "istanbul",
        RuntimeCoverageFormat::V8 => "v8",
        RuntimeCoverageFormat::Mixed => "mixed",
    }
}

fn finding_kind(kind: RuntimeCoverageFindingKind) -> &'static str {
    match kind {
        RuntimeCoverageFindingKind::LowTraffic => "low-traffic",
        RuntimeCoverageFindingKind::CoverageUnavailable => "coverage-unavailable",
    }
}

fn confidence(confidence: RuntimeCoverageConfidence) -> &'static str {
    match confidence {
        RuntimeCoverageConfidence::High => "high",
        RuntimeCoverageConfidence::Medium => "medium",
        RuntimeCoverageConfidence::Low => "low",
    }
}

fn source_map_confidence(confidence: SourceMapConfidence) -> &'static str {
    match confidence {
        SourceMapConfidence::Resolved => "resolved",
        SourceMapConfidence::Fallback => "fallback",
        SourceMapConfidence::Unresolved => "unresolved",
    }
}
