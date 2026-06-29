use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tree_sitter::{Node, Parser};

use crate::graph::normalize_against;
use crate::{DeadCodeReport, Location, ScannedProject};

/// Flutter widget constructor parameter analysis.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WidgetReport {
    /// Dart files included in widget analysis.
    pub analyzed_files: usize,
    /// Widget field-formal parameters that are never read.
    pub unused_params: Vec<UnusedWidgetParam>,
}

/// A widget constructor field-formal parameter that is not used by the widget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnusedWidgetParam {
    /// Dart file containing the widget declaration.
    pub path: PathBuf,
    /// Widget class that owns the parameter.
    pub widget_class: String,
    /// Flutter widget base class.
    pub widget_kind: WidgetClassKind,
    /// Constructor field-formal parameter name.
    pub param_name: String,
    /// Location of the field-formal identifier.
    pub location: Location,
}

/// Supported Flutter widget base classes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WidgetClassKind {
    StatelessWidget,
    StatefulWidget,
    ConsumerWidget,
    ConsumerStatefulWidget,
    HookWidget,
    HookConsumerWidget,
}

/// Errors returned while analyzing Flutter widget parameters.
#[derive(Debug, Error)]
pub enum WidgetAnalysisError {
    /// A Dart file could not be read.
    #[error("failed to read Dart file {path}: {source}")]
    ReadFile {
        /// File path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Tree-Sitter rejected the Dart grammar.
    #[error("failed to load Dart grammar: {0}")]
    Language(#[from] tree_sitter::LanguageError),
    /// Tree-Sitter did not produce a parse tree.
    #[error("tree-sitter did not return a parse tree for {path}")]
    ParseCancelled {
        /// Path being parsed.
        path: PathBuf,
    },
    /// The source parsed with syntax errors.
    #[error("Dart syntax errors found in {path}")]
    Syntax {
        /// Path being parsed.
        path: PathBuf,
    },
}

/// Detect unused Flutter widget constructor field-formal parameters.
///
/// # Errors
///
/// Returns [`WidgetAnalysisError`] if a scanned Dart file cannot be read or
/// parsed during widget analysis.
pub fn analyze_widgets(
    project: &ScannedProject,
    dead_code: Option<&DeadCodeReport>,
) -> Result<WidgetReport, WidgetAnalysisError> {
    let dead_files = dead_code
        .map(|report| {
            report
                .dead_files
                .iter()
                .map(|dead| dead.path.clone())
                .collect::<BTreeSet<_>>()
        })
        .unwrap_or_default();
    let paths = project
        .files
        .iter()
        .map(|file| normalize_against(&project.root, &file.path))
        .filter(|path| path.starts_with(&project.root))
        .filter(|path| !dead_files.contains(path))
        .filter(|path| !is_generated_path(path) && !is_test_path(path))
        .collect::<Vec<_>>();

    let mut unused_params = paths
        .par_iter()
        .map(|path| analyze_file(path))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    unused_params.sort_by(|left, right| {
        (
            &left.path,
            left.location.line,
            left.location.column,
            &left.widget_class,
            &left.param_name,
        )
            .cmp(&(
                &right.path,
                right.location.line,
                right.location.column,
                &right.widget_class,
                &right.param_name,
            ))
    });

    Ok(WidgetReport {
        analyzed_files: paths.len(),
        unused_params,
    })
}

fn analyze_file(path: &Path) -> Result<Vec<UnusedWidgetParam>, WidgetAnalysisError> {
    let source = fs::read_to_string(path).map_err(|source| WidgetAnalysisError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;
    let tree = parse_tree(path, &source)?;
    let root = tree.root_node();
    if root.has_error() {
        return Err(WidgetAnalysisError::Syntax {
            path: path.to_path_buf(),
        });
    }

    Ok(unused_params_in_source(path, root, &source))
}

fn parse_tree(path: &Path, source: &str) -> Result<tree_sitter::Tree, WidgetAnalysisError> {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_dart::LANGUAGE.into())?;
    parser
        .parse(source, None)
        .ok_or_else(|| WidgetAnalysisError::ParseCancelled {
            path: path.to_path_buf(),
        })
}

fn unused_params_in_source(path: &Path, root: Node<'_>, source: &str) -> Vec<UnusedWidgetParam> {
    let mut classes = Vec::new();
    collect_class_declarations(root, &mut classes);
    let states = state_classes_by_widget(&classes, source);
    let mut unused = Vec::new();

    for class in classes {
        let Some(widget_kind) = widget_kind(class, source) else {
            continue;
        };
        let Some(widget_class) = field_text(class, "name", source) else {
            continue;
        };
        let Some(body) = class.child_by_field_name("body") else {
            continue;
        };
        for param in constructor_field_params(class, &widget_class, source) {
            if widget_body_uses_param(body, &param.name, source)
                || states.get(&widget_class).is_some_and(|state_bodies| {
                    state_bodies
                        .iter()
                        .any(|state_body| state_body_uses_param(*state_body, &param.name, source))
                })
            {
                continue;
            }
            unused.push(UnusedWidgetParam {
                path: path.to_path_buf(),
                widget_class: widget_class.clone(),
                widget_kind,
                param_name: param.name,
                location: param.location,
            });
        }
    }

    unused
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WidgetParamCandidate {
    name: String,
    location: Location,
}

fn collect_class_declarations<'tree>(node: Node<'tree>, classes: &mut Vec<Node<'tree>>) {
    if node.kind() == "class_declaration" {
        classes.push(node);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_class_declarations(child, classes);
    }
}

fn state_classes_by_widget<'tree>(
    classes: &[Node<'tree>],
    source: &str,
) -> BTreeMap<String, Vec<Node<'tree>>> {
    let mut states = BTreeMap::<String, Vec<Node<'tree>>>::new();
    for class in classes {
        let Some(widget_class) = state_widget_class(*class, source) else {
            continue;
        };
        if let Some(body) = class.child_by_field_name("body") {
            states.entry(widget_class).or_default().push(body);
        }
    }
    states
}

fn widget_kind(class: Node<'_>, source: &str) -> Option<WidgetClassKind> {
    let base = superclass_base_name(class, source)?;
    match base.as_str() {
        "StatelessWidget" => Some(WidgetClassKind::StatelessWidget),
        "StatefulWidget" => Some(WidgetClassKind::StatefulWidget),
        "ConsumerWidget" => Some(WidgetClassKind::ConsumerWidget),
        "ConsumerStatefulWidget" => Some(WidgetClassKind::ConsumerStatefulWidget),
        "HookWidget" => Some(WidgetClassKind::HookWidget),
        "HookConsumerWidget" => Some(WidgetClassKind::HookConsumerWidget),
        _ => None,
    }
}

fn state_widget_class(class: Node<'_>, source: &str) -> Option<String> {
    let type_text = superclass_type_text(class, source)?;
    let compact = strip_whitespace(&type_text);
    let base = simple_type_name(compact.split('<').next().unwrap_or(&compact));
    if !matches!(base.as_str(), "State" | "ConsumerState") {
        return None;
    }
    let generic = compact
        .split_once('<')
        .and_then(|(_, rest)| rest.rsplit_once('>').map(|(inside, _)| inside))?;
    generic.split(',').next().map(simple_type_name)
}

fn superclass_base_name(class: Node<'_>, source: &str) -> Option<String> {
    superclass_type_text(class, source).map(|text| {
        let compact = strip_whitespace(&text);
        simple_type_name(compact.split('<').next().unwrap_or(&compact))
    })
}

fn superclass_type_text(class: Node<'_>, source: &str) -> Option<String> {
    let superclass = class.child_by_field_name("superclass")?;
    let type_text = superclass
        .child_by_field_name("type")?
        .utf8_text(source.as_bytes())
        .ok()
        .map(str::to_owned)?;
    if type_text.contains('<') {
        return Some(type_text);
    }
    superclass
        .utf8_text(source.as_bytes())
        .ok()
        .and_then(|text| {
            let without_extends = text.trim().strip_prefix("extends")?.trim();
            Some(
                without_extends
                    .split(" with ")
                    .next()
                    .unwrap_or(without_extends)
                    .to_owned(),
            )
        })
        .or(Some(type_text))
}

fn strip_whitespace(text: &str) -> String {
    text.chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn simple_type_name(text: &str) -> String {
    text.trim_end_matches('?')
        .rsplit('.')
        .next()
        .unwrap_or(text)
        .to_owned()
}

fn constructor_field_params(
    class: Node<'_>,
    widget_class: &str,
    source: &str,
) -> Vec<WidgetParamCandidate> {
    let mut signatures = Vec::new();
    collect_nodes_in(class, CONSTRUCTOR_SIGNATURES, &mut signatures);
    let mut params = BTreeMap::<String, WidgetParamCandidate>::new();
    for signature in signatures {
        if constructor_owner(signature, source).as_deref() != Some(widget_class) {
            continue;
        }
        let Some(parameters) = signature.child_by_field_name("parameters") else {
            continue;
        };
        let mut constructor_params = Vec::new();
        collect_nodes(parameters, "constructor_param", &mut constructor_params);
        for param in constructor_params {
            if !is_named_parameter(param, signature, source) {
                continue;
            }
            let Some(candidate) = field_formal_candidate(param, source) else {
                continue;
            };
            if candidate.name == "key" {
                continue;
            }
            params.entry(candidate.name.clone()).or_insert(candidate);
        }
    }
    params.into_values().collect()
}

const CONSTRUCTOR_SIGNATURES: &[&str] =
    &["constructor_signature", "constant_constructor_signature"];

fn constructor_owner(signature: Node<'_>, source: &str) -> Option<String> {
    field_text(signature, "name", source).and_then(|name| name.split('.').next().map(str::to_owned))
}

fn is_named_parameter(param: Node<'_>, signature: Node<'_>, source: &str) -> bool {
    let mut parent = param.parent();
    while let Some(node) = parent {
        if same_node(node, signature) {
            return false;
        }
        if node.kind() == "optional_formal_parameters" {
            return node
                .utf8_text(source.as_bytes())
                .ok()
                .is_some_and(|text| text.trim_start().starts_with('{'));
        }
        parent = node.parent();
    }
    false
}

fn field_formal_candidate(param: Node<'_>, source: &str) -> Option<WidgetParamCandidate> {
    let text = param.utf8_text(source.as_bytes()).ok()?;
    let offset = text.find("this.")? + "this.".len();
    let name = text[offset..]
        .chars()
        .take_while(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '$'))
        .collect::<String>();
    if name.is_empty() {
        return None;
    }
    let min_byte = param.start_byte() + offset;
    let location = identifier_after(param, &name, min_byte, source).map_or_else(
        || param.start_position().into(),
        |node| node.start_position().into(),
    );
    Some(WidgetParamCandidate { name, location })
}

fn widget_body_uses_param(body: Node<'_>, name: &str, source: &str) -> bool {
    let mut found = false;
    visit_named(body, &mut |node| {
        if !found && is_body_identifier_use(node, name, source) {
            found = true;
        }
    });
    found
}

fn is_body_identifier_use(node: Node<'_>, name: &str, source: &str) -> bool {
    if !matches!(node.kind(), "identifier" | "identifier_dollar_escaped") {
        return false;
    }
    if node.utf8_text(source.as_bytes()).ok() != Some(name) {
        return false;
    }
    if has_ancestor_kind(node, BODY_USAGE_SKIP_ANCESTORS) {
        return false;
    }
    let Some(parent) = node.parent() else {
        return true;
    };
    if parent.kind() == "label" || name_field_of(parent, node) {
        return false;
    }
    !(parent.kind() == "identifier_list" && has_ancestor_kind(parent, &["declaration"]))
}

const BODY_USAGE_SKIP_ANCESTORS: &[&str] = &[
    "constructor_signature",
    "constant_constructor_signature",
    "factory_constructor_signature",
    "redirecting_factory_constructor_signature",
    "constructor_param",
    "super_formal_parameter",
    "type",
    "typed_identifier",
];

fn state_body_uses_param(body: Node<'_>, name: &str, source: &str) -> bool {
    let mut found = false;
    visit_named(body, &mut |node| {
        if !found && is_widget_member_access(node, name, source) {
            found = true;
        }
    });
    found
}

fn is_widget_member_access(node: Node<'_>, name: &str, source: &str) -> bool {
    if !matches!(
        node.kind(),
        "member_expression" | "null_aware_member_expression" | "assignable_expression"
    ) {
        return false;
    }
    let Some(property) = node.child_by_field_name("property") else {
        return false;
    };
    if property.utf8_text(source.as_bytes()).ok() != Some(name) {
        return false;
    }
    let Some(object) = node.child_by_field_name("object") else {
        return false;
    };
    matches!(
        object.utf8_text(source.as_bytes()).ok(),
        Some("widget" | "oldWidget")
    )
}

fn collect_nodes_in<'tree>(node: Node<'tree>, kinds: &[&str], nodes: &mut Vec<Node<'tree>>) {
    if kinds.contains(&node.kind()) {
        nodes.push(node);
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_nodes_in(child, kinds, nodes);
    }
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

fn visit_named(node: Node<'_>, visitor: &mut impl FnMut(Node<'_>)) {
    visitor(node);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        visit_named(child, visitor);
    }
}

fn identifier_after<'tree>(
    node: Node<'tree>,
    name: &str,
    min_byte: usize,
    source: &str,
) -> Option<Node<'tree>> {
    if node.kind() == "identifier"
        && node.start_byte() >= min_byte
        && node.utf8_text(source.as_bytes()).ok() == Some(name)
    {
        return Some(node);
    }
    let mut cursor = node.walk();
    node.named_children(&mut cursor)
        .find_map(|child| identifier_after(child, name, min_byte, source))
}

fn field_text(node: Node<'_>, field_name: &str, source: &str) -> Option<String> {
    node.child_by_field_name(field_name)
        .and_then(|child| child.utf8_text(source.as_bytes()).ok())
        .map(str::to_owned)
}

fn name_field_of(parent: Node<'_>, child: Node<'_>) -> bool {
    let mut cursor = parent.walk();
    parent
        .children_by_field_name("name", &mut cursor)
        .any(|field| same_node(field, child))
}

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

fn same_node(left: Node<'_>, right: Node<'_>) -> bool {
    left.kind() == right.kind()
        && left.start_byte() == right.start_byte()
        && left.end_byte() == right.end_byte()
}

fn is_generated_path(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    matches!(
        file_name,
        name if name.ends_with(".g.dart")
            || name.ends_with(".freezed.dart")
            || name.ends_with(".gen.dart")
            || name.ends_with(".gr.dart")
            || name.ends_with(".mocks.dart")
    )
}

fn is_test_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with("_test.dart"))
        || path.components().any(|component| {
            component.as_os_str().to_str().is_some_and(|segment| {
                matches!(segment, "test" | "integration_test" | "test_driver")
            })
        })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn flags_unused_stateless_widget_field_formal() -> Result<(), Box<dyn std::error::Error>> {
        let source = r"
class UserCard extends StatelessWidget {
  const UserCard({super.key, required this.title, required this.subtitle});
  final String title;
  final String subtitle;
  Widget build(BuildContext context) => Text(title);
}
";
        let unused = parse_unused(source)?;

        assert_eq!(unused.len(), 1);
        assert_eq!(unused[0].widget_class, "UserCard");
        assert_eq!(unused[0].param_name, "subtitle");
        assert_eq!(unused[0].location.line, 3);
        Ok(())
    }

    #[test]
    fn respects_widget_and_state_usages() -> Result<(), Box<dyn std::error::Error>> {
        let source = r"
class UsedInBuild extends StatelessWidget {
  const UsedInBuild({super.key, required this.title});
  final String title;
  Widget build(BuildContext context) => Text('$title');
}
class UsedViaState extends StatefulWidget {
  const UsedViaState({super.key, required this.count});
  final int count;
  State<UsedViaState> createState() => _UsedViaStateState();
}
class _UsedViaStateState extends State<UsedViaState> {
  Widget build(BuildContext context) => Text('${widget.count}');
}
";
        let unused = parse_unused(source)?;

        assert!(unused.is_empty(), "{unused:?}");
        Ok(())
    }

    #[test]
    fn recognizes_consumer_and_hook_widget_bases() -> Result<(), Box<dyn std::error::Error>> {
        let source = r"
class A extends ConsumerWidget {
  const A({super.key, required this.value});
  final String value;
  Widget build(BuildContext context, WidgetRef ref) => const SizedBox();
}
class B extends HookConsumerWidget {
  const B({super.key, required this.value});
  final String value;
  Widget build(BuildContext context, WidgetRef ref) => Text(value);
}
class C extends ConsumerStatefulWidget {
  const C({super.key, required this.value});
  final String value;
  ConsumerState<C> createState() => _CState();
}
class _CState extends ConsumerState<C> {
  Widget build(BuildContext context) => Text(oldWidget.value);
}
";
        let unused = parse_unused(source)?;

        assert_eq!(unused.len(), 1);
        assert_eq!(unused[0].widget_class, "A");
        assert_eq!(unused[0].widget_kind, WidgetClassKind::ConsumerWidget);
        Ok(())
    }

    fn parse_unused(source: &str) -> Result<Vec<UnusedWidgetParam>, WidgetAnalysisError> {
        let tree = parse_tree(Path::new("lib/widgets.dart"), source)?;
        Ok(unused_params_in_source(
            Path::new("lib/widgets.dart"),
            tree.root_node(),
            source,
        ))
    }
}
