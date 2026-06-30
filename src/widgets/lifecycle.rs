use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tree_sitter::Node;

use crate::Location;

use super::{simple_type_name, state_widget_class, superclass_type_text, widget_kind};

/// A widget or `State` await without the required post-await mounted guard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingContextMountedAfterAwait {
    /// Dart file containing the await.
    pub path: PathBuf,
    /// Widget or `State` method containing the await.
    pub owner: String,
    /// Location of the await expression.
    pub location: Location,
}

/// A Riverpod notifier await without the required post-await mounted guard.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MissingRefMountedAfterAwait {
    /// Dart file containing the await.
    pub path: PathBuf,
    /// Notifier method containing the await.
    pub owner: String,
    /// Location of the await expression.
    pub location: Location,
}

/// A `ref.watch` call inside a Riverpod notifier mutation/helper method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiverpodWatchInNotifierMethod {
    /// Dart file containing the call.
    pub path: PathBuf,
    /// Notifier class containing the method.
    pub notifier_class: String,
    /// Method containing the call.
    pub method_name: String,
    /// Location of the `ref.watch` call.
    pub location: Location,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct LifecycleFindings {
    pub(super) missing_context_mounted_after_await: Vec<MissingContextMountedAfterAwait>,
    pub(super) missing_ref_mounted_after_await: Vec<MissingRefMountedAfterAwait>,
    pub(super) riverpod_watch_in_notifier_methods: Vec<RiverpodWatchInNotifierMethod>,
}

pub(super) fn lifecycle_findings(
    path: &Path,
    classes: &[Node<'_>],
    source: &str,
) -> LifecycleFindings {
    let mut findings = LifecycleFindings::default();
    for class in classes {
        let class_name = class_name(*class, source).unwrap_or_default();
        if widget_kind(*class, source).is_some() || state_widget_class(*class, source).is_some() {
            collect_context_guards(path, *class, &class_name, source, &mut findings);
        }
        if is_notifier_class(*class, &class_name, source) {
            collect_ref_rules(path, *class, &class_name, source, &mut findings);
        }
    }
    findings
}

fn collect_context_guards(
    path: &Path,
    class: Node<'_>,
    class_name: &str,
    source: &str,
    findings: &mut LifecycleFindings,
) {
    for method in class_methods(class) {
        let method_name = method_name(method, source).unwrap_or_else(|| "<method>".to_owned());
        let Some(body) = method.child_by_field_name("body") else {
            continue;
        };
        for await_node in unguarded_awaits(body, source, "context") {
            findings
                .missing_context_mounted_after_await
                .push(MissingContextMountedAfterAwait {
                    path: path.to_path_buf(),
                    owner: format!("{class_name}.{method_name}"),
                    location: await_node.start_position().into(),
                });
        }
    }
}

fn collect_ref_rules(
    path: &Path,
    class: Node<'_>,
    class_name: &str,
    source: &str,
    findings: &mut LifecycleFindings,
) {
    for method in class_methods(class) {
        let method_name = method_name(method, source).unwrap_or_else(|| "<method>".to_owned());
        let Some(body) = method.child_by_field_name("body") else {
            continue;
        };
        if method_name != "build" {
            for await_node in unguarded_awaits(body, source, "ref") {
                findings
                    .missing_ref_mounted_after_await
                    .push(MissingRefMountedAfterAwait {
                        path: path.to_path_buf(),
                        owner: format!("{class_name}.{method_name}"),
                        location: await_node.start_position().into(),
                    });
            }
            for watch in ref_watch_calls(body, source) {
                findings
                    .riverpod_watch_in_notifier_methods
                    .push(RiverpodWatchInNotifierMethod {
                        path: path.to_path_buf(),
                        notifier_class: class_name.to_owned(),
                        method_name: method_name.clone(),
                        location: watch.start_position().into(),
                    });
            }
        }
    }
}

fn class_methods(class: Node<'_>) -> Vec<Node<'_>> {
    let mut methods = Vec::new();
    collect_nodes(class, "method_declaration", &mut methods);
    methods
}

fn unguarded_awaits<'tree>(body: Node<'tree>, source: &str, receiver: &str) -> Vec<Node<'tree>> {
    let Some(block) = find_first_named_descendant(body, "block") else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    collect_unguarded_awaits_in_block(block, source, receiver, &mut findings);
    findings
}

fn collect_unguarded_awaits_in_block<'tree>(
    block: Node<'tree>,
    source: &str,
    receiver: &str,
    findings: &mut Vec<Node<'tree>>,
) {
    let statements = block_statements(block);
    for (index, statement) in statements.iter().copied().enumerate() {
        let awaits = direct_awaits(statement);
        if awaits.is_empty() {
            collect_child_block_awaits(statement, source, receiver, findings);
            continue;
        }
        if is_terminal_return_await(statement) {
            collect_child_block_awaits(statement, source, receiver, findings);
            continue;
        }
        let guarded = statements
            .get(index + 1)
            .copied()
            .is_some_and(|next| is_mounted_return_guard(next, source, receiver));
        if !guarded {
            findings.extend(awaits);
        }
        collect_child_block_awaits(statement, source, receiver, findings);
    }
}

fn collect_child_block_awaits<'tree>(
    node: Node<'tree>,
    source: &str,
    receiver: &str,
    findings: &mut Vec<Node<'tree>>,
) {
    if is_nested_function_scope(node.kind()) {
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "block" {
            collect_unguarded_awaits_in_block(child, source, receiver, findings);
        } else {
            collect_child_block_awaits(child, source, receiver, findings);
        }
    }
}

fn block_statements(block: Node<'_>) -> Vec<Node<'_>> {
    let mut cursor = block.walk();
    block
        .named_children(&mut cursor)
        .filter(|child| is_statement_kind(child.kind()))
        .collect()
}

fn direct_awaits(statement: Node<'_>) -> Vec<Node<'_>> {
    let mut awaits = Vec::new();
    collect_awaits(statement, true, &mut awaits);
    awaits
}

fn collect_awaits<'tree>(node: Node<'tree>, root: bool, awaits: &mut Vec<Node<'tree>>) {
    if !root && (node.kind() == "block" || is_nested_function_scope(node.kind())) {
        return;
    }
    if node.kind() == "await_expression" {
        awaits.push(node);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_awaits(child, false, awaits);
    }
}

fn is_mounted_return_guard(node: Node<'_>, source: &str, receiver: &str) -> bool {
    if node.kind() != "if_statement" {
        return false;
    }
    let Ok(text) = node.utf8_text(source.as_bytes()) else {
        return false;
    };
    compact(text) == format!("if(!{receiver}.mounted)return;")
}

fn is_terminal_return_await(statement: Node<'_>) -> bool {
    statement.kind() == "return_statement" && contains_await(statement)
}

fn contains_await(node: Node<'_>) -> bool {
    if node.kind() == "await_expression" {
        return true;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor).any(contains_await)
}

fn ref_watch_calls<'tree>(body: Node<'tree>, source: &str) -> Vec<Node<'tree>> {
    let mut calls = Vec::new();
    collect_ref_watch_calls(body, source, true, &mut calls);
    calls
}

fn collect_ref_watch_calls<'tree>(
    node: Node<'tree>,
    source: &str,
    root: bool,
    calls: &mut Vec<Node<'tree>>,
) {
    if !root && is_nested_function_scope(node.kind()) {
        return;
    }
    if node.kind() == "call_expression"
        && node
            .child_by_field_name("function")
            .is_some_and(|function| is_ref_watch_member(function, source))
    {
        calls.push(node);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_ref_watch_calls(child, source, false, calls);
    }
}

fn is_ref_watch_member(node: Node<'_>, source: &str) -> bool {
    if !matches!(
        node.kind(),
        "member_expression" | "null_aware_member_expression" | "assignable_expression"
    ) {
        return false;
    }
    let Some(object) = node.child_by_field_name("object") else {
        return false;
    };
    let Some(property) = node.child_by_field_name("property") else {
        return false;
    };
    object.utf8_text(source.as_bytes()).ok() == Some("ref")
        && property.utf8_text(source.as_bytes()).ok() == Some("watch")
}

fn is_notifier_class(class: Node<'_>, _class_name: &str, source: &str) -> bool {
    let Some(type_text) = superclass_type_text(class, source) else {
        return false;
    };
    let compact = compact(&type_text);
    let base = simple_type_name(compact.split('<').next().unwrap_or(&compact));
    if base.starts_with("_$") {
        return true;
    }
    matches!(
        base.as_str(),
        "Notifier"
            | "AsyncNotifier"
            | "AutoDisposeNotifier"
            | "AutoDisposeAsyncNotifier"
            | "BuildlessNotifier"
            | "BuildlessAsyncNotifier"
            | "BuildlessAutoDisposeNotifier"
            | "BuildlessAutoDisposeAsyncNotifier"
    )
}

fn class_name(class: Node<'_>, source: &str) -> Option<String> {
    class
        .child_by_field_name("name")?
        .utf8_text(source.as_bytes())
        .ok()
        .map(str::to_owned)
}

fn method_name(method: Node<'_>, source: &str) -> Option<String> {
    let signature = method.child_by_field_name("signature")?;
    let inner = first_named_child(signature)?;
    field_text(inner, "name", source)
}

fn field_text(node: Node<'_>, field_name: &str, source: &str) -> Option<String> {
    node.child_by_field_name(field_name)
        .and_then(|child| child.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn collect_nodes<'tree>(node: Node<'tree>, kind: &str, nodes: &mut Vec<Node<'tree>>) {
    if node.kind() == kind {
        nodes.push(node);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_nodes(child, kind, nodes);
    }
}

fn find_first_named_descendant<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    if node.kind() == kind {
        return Some(node);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if let Some(found) = find_first_named_descendant(child, kind) {
            return Some(found);
        }
    }
    None
}

fn first_named_child(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).next()
}

fn is_statement_kind(kind: &str) -> bool {
    matches!(
        kind,
        "assert_statement"
            | "block"
            | "break_statement"
            | "continue_statement"
            | "do_statement"
            | "expression_statement"
            | "for_statement"
            | "if_statement"
            | "local_variable_declaration"
            | "return_statement"
            | "switch_statement"
            | "try_statement"
            | "while_statement"
            | "yield_statement"
    )
}

fn is_nested_function_scope(kind: &str) -> bool {
    matches!(
        kind,
        "function_expression"
            | "function_declaration"
            | "method_declaration"
            | "getter_declaration"
            | "setter_declaration"
    )
}

fn compact(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}
