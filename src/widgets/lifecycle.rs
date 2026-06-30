use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tree_sitter::Node;

use crate::Location;

use super::{state_widget_class, widget_kind};

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

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(super) struct LifecycleFindings {
    pub(super) missing_context_mounted_after_await: Vec<MissingContextMountedAfterAwait>,
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

fn class_methods(class: Node<'_>) -> Vec<Node<'_>> {
    let mut methods = Vec::new();
    collect_nodes(class, "method_declaration", &mut methods);
    methods
}

#[derive(Clone, Copy)]
enum GuardMode {
    Normal,
    Finally,
}

#[derive(Clone, Copy)]
enum GuardRequirement {
    LifecycleUse,
}

fn unguarded_awaits<'tree>(body: Node<'tree>, source: &str, receiver: &str) -> Vec<Node<'tree>> {
    let mut findings = Vec::new();
    collect_unguarded_awaits_in_function_body(
        body,
        source,
        receiver,
        GuardRequirement::LifecycleUse,
        &mut findings,
    );
    findings
}

fn collect_unguarded_awaits_in_function_body<'tree>(
    body: Node<'tree>,
    source: &str,
    receiver: &str,
    requirement: GuardRequirement,
    findings: &mut Vec<Node<'tree>>,
) {
    if body.kind() == "block" {
        collect_unguarded_awaits_in_block(
            body,
            source,
            receiver,
            GuardMode::Normal,
            requirement,
            false,
            findings,
        );
        return;
    }
    if let Some(block) = direct_named_child(body, "block") {
        collect_unguarded_awaits_in_block(
            block,
            source,
            receiver,
            GuardMode::Normal,
            requirement,
            false,
            findings,
        );
        return;
    }
    collect_expression_body_awaits(body, source, receiver, findings);
    collect_child_block_awaits(
        body,
        source,
        receiver,
        GuardMode::Normal,
        requirement,
        findings,
    );
}

fn collect_expression_body_awaits<'tree>(
    body: Node<'tree>,
    source: &str,
    receiver: &str,
    findings: &mut Vec<Node<'tree>>,
) {
    if !contains_await(body) || !contains_lifecycle_use(body, source, receiver) {
        return;
    }
    let mut awaits = Vec::new();
    collect_awaits(body, true, &mut awaits);
    findings.extend(awaits);
}

fn collect_unguarded_awaits_in_block<'tree>(
    block: Node<'tree>,
    source: &str,
    receiver: &str,
    mode: GuardMode,
    requirement: GuardRequirement,
    trailing_guard: bool,
    findings: &mut Vec<Node<'tree>>,
) {
    let statements = block_statements(block);
    for (index, statement) in statements.iter().copied().enumerate() {
        let awaits = direct_awaits(statement);
        if awaits.is_empty() {
            collect_child_block_awaits(statement, source, receiver, mode, requirement, findings);
            continue;
        }
        if is_terminal_return_await(statement) {
            collect_child_block_awaits(statement, source, receiver, mode, requirement, findings);
            continue;
        }
        let guarded = statements
            .get(index + 1)
            .copied()
            .is_some_and(|next| is_valid_post_await_guard(next, source, receiver, mode))
            || (index + 1 == statements.len() && trailing_guard);
        let required = match requirement {
            GuardRequirement::LifecycleUse => {
                requires_post_await_guard(statement, &statements[index + 1..], source, receiver)
            }
        };
        if !guarded && required {
            findings.extend(awaits);
        }
        collect_child_block_awaits(statement, source, receiver, mode, requirement, findings);
    }
}

fn requires_post_await_guard(
    statement: Node<'_>,
    following: &[Node<'_>],
    source: &str,
    receiver: &str,
) -> bool {
    contains_lifecycle_use(statement, source, receiver)
        || following
            .iter()
            .copied()
            .any(|statement| contains_lifecycle_use(statement, source, receiver))
}

fn collect_child_block_awaits<'tree>(
    node: Node<'tree>,
    source: &str,
    receiver: &str,
    mode: GuardMode,
    requirement: GuardRequirement,
    findings: &mut Vec<Node<'tree>>,
) {
    if node.kind() == "function_expression" {
        if let Some(body) = node.child_by_field_name("body") {
            collect_unguarded_awaits_in_function_body(
                body,
                source,
                receiver,
                GuardRequirement::LifecycleUse,
                findings,
            );
        }
        return;
    }
    if is_nested_non_closure_scope(node.kind()) {
        return;
    }
    if node.kind() == "try_statement" {
        collect_try_awaits(node, source, receiver, mode, requirement, findings);
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "block" {
            collect_unguarded_awaits_in_block(
                child,
                source,
                receiver,
                mode,
                requirement,
                false,
                findings,
            );
        } else {
            collect_child_block_awaits(child, source, receiver, mode, requirement, findings);
        }
    }
}

fn collect_try_awaits<'tree>(
    try_statement: Node<'tree>,
    source: &str,
    receiver: &str,
    mode: GuardMode,
    requirement: GuardRequirement,
    findings: &mut Vec<Node<'tree>>,
) {
    let body = try_statement.child_by_field_name("body");
    let has_finally_guard = receiver == "ref" && finally_has_positive_guard(try_statement, source);
    if let Some(block) = body {
        collect_unguarded_awaits_in_block(
            block,
            source,
            receiver,
            mode,
            requirement,
            has_finally_guard,
            findings,
        );
    }

    let mut cursor = try_statement.walk();
    for child in try_statement.named_children(&mut cursor) {
        if body.is_some_and(|body| same_node(body, child)) {
            continue;
        }
        if child.kind() == "finally_clause" {
            if let Some(block) = direct_named_child(child, "block") {
                collect_unguarded_awaits_in_block(
                    block,
                    source,
                    receiver,
                    GuardMode::Finally,
                    requirement,
                    false,
                    findings,
                );
            }
        } else {
            collect_child_block_awaits(child, source, receiver, mode, requirement, findings);
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

fn is_valid_post_await_guard(
    node: Node<'_>,
    source: &str,
    receiver: &str,
    mode: GuardMode,
) -> bool {
    match mode {
        GuardMode::Normal => is_mounted_return_guard(node, source, receiver),
        GuardMode::Finally => receiver == "ref" && is_mounted_positive_block_guard(node, source),
    }
}

fn is_mounted_return_guard(node: Node<'_>, source: &str, receiver: &str) -> bool {
    if node.kind() != "if_statement" {
        return false;
    }
    let Ok(text) = node.utf8_text(source.as_bytes()) else {
        return false;
    };
    let compact = compact(text);
    let unbraced = format!("if(!{receiver}.mounted)return");
    let braced = format!("if(!{receiver}.mounted){{return");
    (compact.starts_with(&unbraced) && compact.ends_with(';'))
        || (compact.starts_with(&braced) && compact.ends_with(";}"))
}

fn is_mounted_positive_block_guard(node: Node<'_>, source: &str) -> bool {
    if node.kind() != "if_statement" || node.child_by_field_name("alternative").is_some() {
        return false;
    }
    let Ok(text) = node.utf8_text(source.as_bytes()) else {
        return false;
    };
    let compact = compact(text);
    compact.starts_with("if(ref.mounted){") && compact.ends_with('}')
}

fn finally_has_positive_guard(try_statement: Node<'_>, source: &str) -> bool {
    let Some(finally) = direct_named_child(try_statement, "finally_clause") else {
        return false;
    };
    let Some(block) = direct_named_child(finally, "block") else {
        return false;
    };
    block_statements(block)
        .first()
        .copied()
        .is_some_and(|statement| is_mounted_positive_block_guard(statement, source))
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

fn contains_lifecycle_use(node: Node<'_>, source: &str, receiver: &str) -> bool {
    if receiver == "context" {
        contains_identifier(node, source, "context", true)
    } else {
        contains_identifier(node, source, "ref", true)
            || contains_identifier(node, source, "state", true)
    }
}

fn contains_identifier(node: Node<'_>, source: &str, name: &str, root: bool) -> bool {
    if !root && is_nested_function_scope(node.kind()) {
        return false;
    }
    if node.kind() == "identifier" && node.utf8_text(source.as_bytes()).ok() == Some(name) {
        return true;
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .any(|child| contains_identifier(child, source, name, false))
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

fn direct_named_child<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == kind)
}

fn first_named_child(node: Node<'_>) -> Option<Node<'_>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor).next()
}

fn same_node(left: Node<'_>, right: Node<'_>) -> bool {
    left.start_byte() == right.start_byte() && left.end_byte() == right.end_byte()
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

fn is_nested_non_closure_scope(kind: &str) -> bool {
    is_nested_function_scope(kind) && kind != "function_expression"
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
