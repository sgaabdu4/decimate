use serde::{Deserialize, Serialize};

pub const UNSUPPORTED_SCHEMA_VERSION: &str = "dart-decimate.unsupported.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsupportedReport {
    pub schema_version: String,
    pub kind: String,
    pub tool: String,
    pub command: String,
    pub supported: bool,
    pub status: String,
    pub reason: String,
    pub alternatives: Vec<String>,
}

#[must_use]
pub fn unsupported_report(
    command: impl Into<String>,
    status: impl Into<String>,
    reason: impl Into<String>,
    alternatives: Vec<String>,
) -> UnsupportedReport {
    UnsupportedReport {
        schema_version: UNSUPPORTED_SCHEMA_VERSION.to_owned(),
        kind: "unsupported".to_owned(),
        tool: "dart-decimate".to_owned(),
        command: command.into(),
        supported: false,
        status: status.into(),
        reason: reason.into(),
        alternatives,
    }
}

#[must_use]
pub fn render_unsupported_report(report: &UnsupportedReport) -> String {
    let alternatives = if report.alternatives.is_empty() {
        String::new()
    } else {
        format!("\nAlternatives:\n- {}\n", report.alternatives.join("\n- "))
    };
    format!(
        "{}: {} ({}){}\n",
        report.command, report.reason, report.status, alternatives
    )
}
