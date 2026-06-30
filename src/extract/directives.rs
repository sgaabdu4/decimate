use tree_sitter::Node;

use super::{
    DartCombinator, DartCombinatorKind, DartExport, DartImport, DartLibrary, DartPart, DartPartOf,
    collect_direct_identifier_children, extract_uri, field_text, find_first_named_descendant,
    first_named_child,
};

pub(super) fn extract_library_name(node: Node<'_>, source: &str) -> DartLibrary {
    DartLibrary {
        name: dotted_identifier_text(node, source),
        augment_uri: node
            .child_by_field_name("uri")
            .and_then(|uri| extract_uri(uri, source)),
        location: node.start_position().into(),
    }
}

pub(super) fn extract_directive(
    node: Node<'_>,
    source: &str,
    imports: &mut Vec<DartImport>,
    exports: &mut Vec<DartExport>,
) {
    let Some(kind_node) = first_named_child(&node) else {
        return;
    };
    let Some(uri_node) = directive_uri_node(kind_node) else {
        return;
    };
    let uris = extract_directive_uris(uri_node, source);
    if uris.is_empty() {
        return;
    }
    let location = node.start_position().into();
    let combinators = extract_combinators(kind_node, source);

    match kind_node.kind() {
        "library_import" => {
            let import_specification =
                find_first_named_descendant(kind_node, "import_specification");
            let prefix = import_specification
                .and_then(|specification| field_text(specification, "alias", source))
                .filter(|prefix| prefix != "_");
            let deferred = import_specification.is_some_and(|specification| {
                import_uses_deferred_as(specification, uri_node, source)
            });
            imports.extend(uris.into_iter().map(|uri| DartImport {
                uri,
                prefix: prefix.clone(),
                deferred,
                combinators: combinators.clone(),
                location,
            }));
        }
        "library_export" => {
            exports.extend(uris.into_iter().map(|uri| DartExport {
                uri,
                combinators: combinators.clone(),
                location,
            }));
        }
        _ => {}
    }
}

pub(super) fn extract_part_directive(node: Node<'_>, source: &str, parts: &mut Vec<DartPart>) {
    let Some(uri_node) = node.child_by_field_name("uri") else {
        return;
    };
    let Some(uri) = extract_uri(uri_node, source) else {
        return;
    };

    parts.push(DartPart {
        uri,
        location: node.start_position().into(),
    });
}

pub(super) fn extract_part_of_directive(node: Node<'_>, source: &str) -> DartPartOf {
    let uri = find_first_named_child(node, "uri").and_then(|uri| extract_uri(uri, source));
    DartPartOf {
        name: (uri.is_none())
            .then(|| dotted_identifier_text(node, source))
            .flatten(),
        uri,
        location: node.start_position().into(),
    }
}

fn directive_uri_node(node: Node<'_>) -> Option<Node<'_>> {
    match node.kind() {
        "library_import" => {
            let specification = find_first_named_descendant(node, "import_specification")?;
            specification.child_by_field_name("uri")
        }
        "library_export" => node.child_by_field_name("uri"),
        _ => None,
    }
}

pub(super) fn import_uses_deferred_as(
    specification: Node<'_>,
    uri_node: Node<'_>,
    _source: &str,
) -> bool {
    let mut saw_deferred = false;
    let mut cursor = specification.walk();
    for child in specification.children(&mut cursor) {
        if child.end_byte() <= uri_node.end_byte() || child.kind() == "comment" {
            continue;
        }
        match child.kind() {
            "deferred" => saw_deferred = true,
            "as" if saw_deferred => return true,
            _ if !child.is_extra() => saw_deferred = false,
            _ => {}
        }
    }
    false
}

fn extract_directive_uris(uri_node: Node<'_>, source: &str) -> Vec<String> {
    let mut uris = Vec::new();
    collect_directive_uris(uri_node, source, &mut uris);
    uris
}

fn collect_directive_uris(node: Node<'_>, source: &str, uris: &mut Vec<String>) {
    if node.kind() == "uri" {
        if let Some(uri) = extract_uri(node, source) {
            uris.push(uri);
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_directive_uris(child, source, uris);
    }
}

fn extract_combinators(node: Node<'_>, source: &str) -> Vec<DartCombinator> {
    let mut combinators = Vec::new();
    collect_combinators(node, source, &mut combinators);
    combinators
}

fn collect_combinators(node: Node<'_>, source: &str, combinators: &mut Vec<DartCombinator>) {
    if node.kind() == "combinator" {
        if let Some(combinator) = combinator_from_node(node, source) {
            combinators.push(combinator);
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_combinators(child, source, combinators);
    }
}

fn combinator_from_node(node: Node<'_>, source: &str) -> Option<DartCombinator> {
    let text = node.utf8_text(source.as_bytes()).ok()?;
    let kind = if text.trim_start().starts_with("show") {
        DartCombinatorKind::Show
    } else if text.trim_start().starts_with("hide") {
        DartCombinatorKind::Hide
    } else {
        return None;
    };
    let mut names = Vec::new();
    collect_direct_identifier_children(node, source, &mut names);

    Some(DartCombinator {
        kind,
        names,
        location: node.start_position().into(),
    })
}

fn dotted_identifier_text(node: Node<'_>, source: &str) -> Option<String> {
    let dotted = find_first_named_child(node, "dotted_identifier_list")?;
    let mut names = Vec::new();
    collect_direct_identifier_children(dotted, source, &mut names);
    (!names.is_empty()).then(|| names.join("."))
}

fn find_first_named_child<'tree>(node: Node<'tree>, kind: &str) -> Option<Node<'tree>> {
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find(|child| child.kind() == kind)
}
