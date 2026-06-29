use std::path::Path;

use super::format::display_path;
use super::{Finding, FindingAction, FindingKind, Severity};
use crate::{PrivateWidgetClass, UnusedWidgetParam, WidgetReport};

pub(super) fn add_widget_findings(root: &Path, report: &WidgetReport, findings: &mut Vec<Finding>) {
    findings.extend(
        report
            .private_widget_classes
            .iter()
            .map(|private| private_widget_class_finding(root, private)),
    );
    findings.extend(
        report
            .unused_params
            .iter()
            .map(|unused| unused_widget_param_finding(root, unused)),
    );
}

fn private_widget_class_finding(root: &Path, private: &PrivateWidgetClass) -> Finding {
    let path = display_path(root, &private.path);
    Finding {
        rule_id: "decimate/private-widget-class".to_owned(),
        fingerprint: Some(format!(
            "private-widget-class:{path}:{}",
            private.widget_class
        )),
        kind: FindingKind::PrivateWidgetClass,
        severity: Severity::Warning,
        message: format!(
            "Flutter widget class {} is private; extract widgets as public classes",
            private.widget_class
        ),
        path: path.clone(),
        line: private.location.line,
        column: private.location.column,
        safe_to_delete: false,
        files: Vec::new(),
        edge: None,
        actions: vec![
            FindingAction::new(
                "make-widget-public",
                "Rename the widget class to a public name or suppress the intentional private widget",
                false,
            )
            .with_target_path(path.clone())
            .with_target_symbol(private.widget_class.clone())
            .with_decimate_args(["inspect", "--format", "json", "--file", path.as_str()])
            .with_suppression_comment("// decimate-ignore-next-line private-widget-class"),
        ],
    }
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
