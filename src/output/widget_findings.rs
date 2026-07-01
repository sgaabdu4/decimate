use std::path::Path;

use super::format::display_path;
use super::{Finding, FindingAction, FindingKind, Severity};
use crate::{
    MissingContextMountedAfterAwait, PrivateWidgetClass, UnrenderedWidgetClass, UnusedWidgetParam,
    WidgetReport, WidgetTopLevelFunction,
};

pub(super) fn add_widget_findings(root: &Path, report: &WidgetReport, findings: &mut Vec<Finding>) {
    findings.extend(
        report
            .private_widget_classes
            .iter()
            .map(|private| private_widget_class_finding(root, private)),
    );
    findings.extend(
        report
            .top_level_functions
            .iter()
            .map(|function| top_level_function_finding(root, function)),
    );
    findings.extend(
        report
            .unused_params
            .iter()
            .map(|unused| unused_widget_param_finding(root, unused)),
    );
    findings.extend(
        report
            .unrendered_widgets
            .iter()
            .map(|widget| unrendered_widget_finding(root, widget)),
    );
    findings.extend(
        report
            .missing_context_mounted_after_await
            .iter()
            .map(|missing| missing_context_mounted_finding(root, missing)),
    );
}

fn private_widget_class_finding(root: &Path, private: &PrivateWidgetClass) -> Finding {
    let path = display_path(root, &private.path);
    Finding {
        rule_id: "dart-decimate/private-widget-class".to_owned(),
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
            .with_dart_decimate_args(["inspect", "--format", "json", "--file", path.as_str()])
            .with_suppression_comment("// dart-decimate-ignore-next-line private-widget-class"),
        ],
    }
}

fn top_level_function_finding(root: &Path, function: &WidgetTopLevelFunction) -> Finding {
    let path = display_path(root, &function.path);
    Finding {
        rule_id: "dart-decimate/widget-top-level-function-boundary".to_owned(),
        fingerprint: Some(format!(
            "widget-top-level-function-boundary:{path}:{}",
            function.function_name
        )),
        kind: FindingKind::WidgetTopLevelFunctionBoundary,
        severity: Severity::Warning,
        message: format!(
            "Top-level Flutter UI helper {} should be extracted to a widget class or moved behind an owning boundary",
            function.function_name
        ),
        path: path.clone(),
        line: function.location.line,
        column: function.location.column,
        safe_to_delete: false,
        files: Vec::new(),
        edge: None,
        actions: vec![
            FindingAction::new(
                "extract-widget-helper",
                "Move this top-level UI helper into a public widget class or another owning boundary",
                false,
            )
            .with_target_path(path.clone())
            .with_target_symbol(function.function_name.clone())
            .with_dart_decimate_args(["inspect", "--format", "json", "--file", path.as_str()])
            .with_suppression_comment(
                "// dart-decimate-ignore-next-line widget-top-level-function-boundary",
            ),
        ],
    }
}

fn unused_widget_param_finding(root: &Path, unused: &UnusedWidgetParam) -> Finding {
    let path = display_path(root, &unused.path);
    let target_symbol = format!("{}.{}", unused.widget_class, unused.param_name);
    Finding {
        rule_id: "dart-decimate/unused-widget-param".to_owned(),
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
            .with_dart_decimate_args(["inspect", "--format", "json", "--file", path.as_str()])
            .with_suppression_comment("// dart-decimate-ignore-next-line unused-widget-param"),
        ],
    }
}

fn unrendered_widget_finding(root: &Path, widget: &UnrenderedWidgetClass) -> Finding {
    let path = display_path(root, &widget.path);
    Finding {
        rule_id: "dart-decimate/unrendered-widget".to_owned(),
        fingerprint: Some(format!("unrendered-widget:{path}:{}", widget.widget_class)),
        kind: FindingKind::UnrenderedWidget,
        severity: Severity::Warning,
        message: format!(
            "Flutter widget class {} is never constructed from reachable production code",
            widget.widget_class
        ),
        path: path.clone(),
        line: widget.location.line,
        column: widget.location.column,
        safe_to_delete: false,
        files: Vec::new(),
        edge: None,
        actions: vec![
            FindingAction::new(
                "trace-widget-reachability",
                "Inspect reachable constructors and callers before removing this widget",
                false,
            )
            .with_target_path(path.clone())
            .with_target_symbol(widget.widget_class.clone())
            .with_dart_decimate_args(["inspect", "--format", "json", "--file", path.as_str()])
            .with_suppression_comment("// dart-decimate-ignore-next-line unrendered-widget"),
        ],
    }
}

fn missing_context_mounted_finding(
    root: &Path,
    missing: &MissingContextMountedAfterAwait,
) -> Finding {
    let path = display_path(root, &missing.path);
    Finding {
        rule_id: "dart-decimate/missing-context-mounted-after-await".to_owned(),
        fingerprint: Some(format!(
            "missing-context-mounted-after-await:{path}:{}:{}",
            missing.owner, missing.location.line
        )),
        kind: FindingKind::MissingContextMountedAfterAwait,
        severity: Severity::Warning,
        message: format!(
            "{} awaits work without an immediate `if (!context.mounted) return;` guard",
            missing.owner
        ),
        path: path.clone(),
        line: missing.location.line,
        column: missing.location.column,
        safe_to_delete: false,
        files: Vec::new(),
        edge: None,
        actions: vec![
            FindingAction::new(
                "add-context-mounted-guard",
                "Add `if (!context.mounted) return;` immediately after this await before using BuildContext",
                false,
            )
            .with_target_path(path.clone())
            .with_target_symbol(missing.owner.clone())
            .with_dart_decimate_args(["inspect", "--format", "json", "--file", path.as_str()])
            .with_suppression_comment(
                "// dart-decimate-ignore-next-line missing-context-mounted-after-await",
            ),
        ],
    }
}
