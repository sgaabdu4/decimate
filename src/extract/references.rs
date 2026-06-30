use tree_sitter::Node;

use super::IdentifierReference;

pub(super) fn extract_identifier_references(
    root: Node<'_>,
    source: &str,
) -> Vec<IdentifierReference> {
    let mut references = Vec::new();
    collect_identifier_references(root, source, &mut references);
    references
}

fn collect_identifier_references(
    node: Node<'_>,
    source: &str,
    references: &mut Vec<IdentifierReference>,
) {
    if is_reference_identifier(node, source)
        && let Ok(name) = node.utf8_text(source.as_bytes())
    {
        references.push(IdentifierReference {
            name: name.to_owned(),
            location: node.start_position().into(),
        });
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_identifier_references(child, source, references);
    }
}

fn is_reference_identifier(node: Node<'_>, source: &str) -> bool {
    if !matches!(node.kind(), "identifier" | "type_identifier") {
        return false;
    }
    if node.utf8_text(source.as_bytes()).ok() == Some("_") {
        return false;
    }
    if has_ancestor_kind(node, &["import_or_export", "part_directive"]) {
        return false;
    }
    let Some(parent) = node.parent() else {
        return true;
    };
    if parent.kind() == "variable_pattern" || is_pattern_binding(node, source) {
        return false;
    }
    if is_pattern_field_label(node, source) {
        return false;
    }
    if parent.kind() == "type_alias" && is_type_alias_name(parent, node) {
        return false;
    }
    if DECLARATION_NAME_OWNER_KINDS.contains(&parent.kind()) && is_child_field(parent, node, "name")
    {
        return false;
    }
    true
}

const DECLARATION_NAME_OWNER_KINDS: &[&str] = &[
    "class_declaration",
    "constant_constructor_signature",
    "constructor_signature",
    "default_formal_parameter",
    "enum_declaration",
    "enum_constant",
    "extension_declaration",
    "extension_type_declaration",
    "factory_constructor_signature",
    "field_formal_parameter",
    "formal_parameter",
    "function_signature",
    "getter_signature",
    "initialized_identifier",
    "mixin_declaration",
    "normal_formal_parameter",
    "operator_signature",
    "redirecting_factory_constructor_signature",
    "setter_signature",
    "static_final_declaration",
    "type_alias",
];

fn has_ancestor_kind(node: Node<'_>, kinds: &[&str]) -> bool {
    let mut parent = node.parent();
    while let Some(ancestor) = parent {
        if kinds.contains(&ancestor.kind()) {
            return true;
        }
        parent = ancestor.parent();
    }
    false
}

fn is_child_field(parent: Node<'_>, child: Node<'_>, field_name: &str) -> bool {
    let mut cursor = parent.walk();
    parent
        .children_by_field_name(field_name, &mut cursor)
        .any(|field| same_node(field, child))
}

fn is_type_alias_name(parent: Node<'_>, child: Node<'_>) -> bool {
    let mut cursor = parent.walk();
    parent
        .named_children(&mut cursor)
        .find(|node| matches!(node.kind(), "identifier" | "type_identifier"))
        .is_some_and(|name| same_node(name, child))
}

fn is_pattern_field_label(node: Node<'_>, source: &str) -> bool {
    if !has_ancestor_kind(node, &["object_pattern", "record_pattern"]) {
        return false;
    }
    source
        .get(node.end_byte()..)
        .is_some_and(|suffix| suffix.trim_start().starts_with(':'))
}

fn is_pattern_binding(node: Node<'_>, source: &str) -> bool {
    if !has_ancestor_kind(
        node,
        &[
            "constant_pattern",
            "list_pattern",
            "map_pattern",
            "object_pattern",
            "record_pattern",
        ],
    ) {
        return false;
    }
    let Some(prefix) = source.get(..node.start_byte()) else {
        return false;
    };
    let token_prefix = prefix
        .rsplit(|character: char| {
            character.is_whitespace() || matches!(character, '(' | '[' | '{' | ':' | ',')
        })
        .find(|segment| !segment.is_empty())
        .unwrap_or_default();
    matches!(token_prefix, "final" | "var")
}

fn same_node(left: Node<'_>, right: Node<'_>) -> bool {
    left.kind() == right.kind()
        && left.start_byte() == right.start_byte()
        && left.end_byte() == right.end_byte()
}
