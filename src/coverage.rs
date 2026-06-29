use serde::{Deserialize, Serialize};

use crate::output::JsonRuntimeCoverage;

/// Stable schema version for focused runtime coverage analysis.
pub const COVERAGE_ANALYSIS_SCHEMA_VERSION: &str = "decimate.coverage.v1";

/// Focused runtime coverage analysis output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageAnalysisReport {
    /// Schema identifier.
    pub schema_version: String,
    /// Typed envelope discriminator.
    pub kind: String,
    /// Tool name.
    pub tool: String,
    /// Command that produced this report.
    pub command: String,
    /// Runtime coverage intelligence.
    pub runtime_coverage: JsonRuntimeCoverage,
}

/// Build a focused runtime coverage report.
#[must_use]
pub fn coverage_analysis_report(runtime_coverage: JsonRuntimeCoverage) -> CoverageAnalysisReport {
    CoverageAnalysisReport {
        schema_version: COVERAGE_ANALYSIS_SCHEMA_VERSION.to_owned(),
        kind: "runtime-coverage".to_owned(),
        tool: "decimate".to_owned(),
        command: "coverage analyze".to_owned(),
        runtime_coverage,
    }
}

/// Render a concise human runtime coverage report.
#[must_use]
pub fn render_coverage_analysis_report(report: &CoverageAnalysisReport) -> String {
    let summary = &report.runtime_coverage.summary;
    format!(
        "Runtime coverage: {} observed files, {} invocations, {} hot paths, {} findings\n",
        summary.observed_files, summary.total_invocations, summary.hot_paths, summary.findings
    )
}
