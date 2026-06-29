use std::path::Path;

use super::format::display_path;
use super::{Finding, FindingAction, FindingKind, Severity};
use crate::{UnusedWidgetParam, WidgetReport};

pub(super) fn add_widget_findings(root: &Path, report: &WidgetReport, findings: &mut Vec<Finding>) {
    findings.extend(
        report
            .unused_params
            .iter()
            .map(|unused| unused_widget_param_finding(root, unused)),
    );
}

fn unused_widget_param_finding(root: &Path, unused: &UnusedWidgetParam) -> Finding {
    let path = display_path(root, &unused.path);
    let target_symbol = format!("{}.{}", unused.widget_class, unused.param_name);
    Finding {
        rule_id: "decimate/unused-widget-param".to_owned(),
        fingerprint: Some(format!("unused-widget-param:{path}:{target_symbol}")),
        kind: FindingKind::UnusedWidgetParam,
        severity: Severity::Warning,
        message: format!(
            "Flutter widget parameter {} is never read by {}",
            unused.param_name, unused.widget_class
        ),
        path: path.clone(),
        line: unused.location.line,
        column: unused.location.column,
        safe_to_delete: false,
        files: Vec::new(),
        edge: None,
        actions: vec![
            FindingAction::new(
                "review-widget-param",
                "Review widget callers before removing this constructor parameter",
                false,
            )
            .with_target_path(path.clone())
            .with_target_symbol(target_symbol)
            .with_decimate_args(["inspect", "--format", "json", "--file", path.as_str()])
            .with_suppression_comment("// decimate-ignore-next-line unused-widget-param"),
        ],
    }
}
