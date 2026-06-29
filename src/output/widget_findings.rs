use std::path::Path;

use super::format::display_path;
use super::{Finding, FindingAction, FindingKind, Severity};
use crate::{
    ManualRiverpodProvider, PrivateWidgetClass, UnrenderedWidgetClass, UnusedWidgetParam,
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
            .manual_riverpod_providers
            .iter()
            .map(|provider| manual_riverpod_provider_finding(root, provider)),
    );
    findings.extend(
        report
            .unrendered_widgets
            .iter()
            .map(|widget| unrendered_widget_finding(root, widget)),
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

fn top_level_function_finding(root: &Path, function: &WidgetTopLevelFunction) -> Finding {
    let path = display_path(root, &function.path);
    Finding {
        rule_id: "decimate/widget-top-level-function-boundary".to_owned(),
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
            .with_decimate_args(["inspect", "--format", "json", "--file", path.as_str()])
            .with_suppression_comment(
                "// decimate-ignore-next-line widget-top-level-function-boundary",
            ),
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

fn manual_riverpod_provider_finding(root: &Path, provider: &ManualRiverpodProvider) -> Finding {
    let path = display_path(root, &provider.path);
    Finding {
        rule_id: "decimate/manual-riverpod-provider".to_owned(),
        fingerprint: Some(format!(
            "manual-riverpod-provider:{path}:{}",
            provider.provider_name
        )),
        kind: FindingKind::ManualRiverpodProvider,
        severity: Severity::Warning,
        message: format!(
            "Riverpod provider {} uses manual {} wiring; prefer generated @riverpod providers",
            provider.provider_name, provider.provider_type
        ),
        path: path.clone(),
        line: provider.location.line,
        column: provider.location.column,
        safe_to_delete: false,
        files: Vec::new(),
        edge: None,
        actions: vec![
            FindingAction::new(
                "migrate-riverpod-codegen",
                "Replace the manual provider declaration with a generated @riverpod provider owner",
                false,
            )
            .with_target_path(path.clone())
            .with_target_symbol(provider.provider_name.clone())
            .with_decimate_args(["inspect", "--format", "json", "--file", path.as_str()])
            .with_suppression_comment("// decimate-ignore-next-line manual-riverpod-provider"),
        ],
    }
}

fn unrendered_widget_finding(root: &Path, widget: &UnrenderedWidgetClass) -> Finding {
    let path = display_path(root, &widget.path);
    Finding {
        rule_id: "decimate/unrendered-widget".to_owned(),
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
            .with_decimate_args(["inspect", "--format", "json", "--file", path.as_str()])
            .with_suppression_comment("// decimate-ignore-next-line unrendered-widget"),
        ],
    }
}
