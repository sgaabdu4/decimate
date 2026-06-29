use std::fmt::Write as _;

use super::types::JsonReport;

/// Render a concise human report.
#[must_use]
pub fn render_human_report(report: &JsonReport) -> String {
    let mut rendered = String::new();
    let _ = writeln!(
        rendered,
        "{:?} {}: {} findings across {} files",
        report.verdict,
        report.command.as_str(),
        report.summary.findings,
        report.summary.files
    );

    for finding in &report.findings {
        let _ = writeln!(
            rendered,
            "{}:{}:{} {} {}",
            finding.path, finding.line, finding.column, finding.rule_id, finding.message
        );
    }

    rendered
}
