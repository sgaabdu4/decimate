use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tree_sitter::{Node, Parser};

use crate::graph::normalize_against;
use crate::{DeadCodeReport, Location, ScannedProject};

mod params;
mod top_level;

use params::constructor_params;
use top_level::top_level_widget_functions;

/// Flutter widget framework analysis.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WidgetReport {
    /// Dart files included in widget analysis.
    pub analyzed_files: usize,
    /// Widget constructor parameters that are never read.
    pub unused_params: Vec<UnusedWidgetParam>,
    /// Private Flutter widget classes.
    pub private_widget_classes: Vec<PrivateWidgetClass>,
    /// Top-level Flutter widget helper functions.
    pub top_level_functions: Vec<WidgetTopLevelFunction>,
}

/// A widget constructor parameter that is not used by the widget.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnusedWidgetParam {
    /// Dart file containing the widget declaration.
    pub path: PathBuf,
    /// Widget class that owns the parameter.
    pub widget_class: String,
    /// Flutter widget base class.
    pub widget_kind: WidgetClassKind,
    /// Constructor parameter name.
    pub param_name: String,
    /// Location of the constructor parameter identifier.
    pub location: Location,
}

/// A private class that extends a Flutter widget base.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateWidgetClass {
    /// Dart file containing the widget declaration.
    pub path: PathBuf,
    /// Private widget class name.
    pub widget_class: String,
    /// Flutter widget base class.
    pub widget_kind: WidgetClassKind,
    /// Location of the class identifier.
    pub location: Location,
}

/// A top-level function that should be owned by a widget class or helper owner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WidgetTopLevelFunction {
    /// Dart file containing the function declaration.
    pub path: PathBuf,
    /// Top-level function name.
    pub function_name: String,
    /// Function return type when declared.
    pub return_type: Option<String>,
    /// Location of the function identifier.
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

/// Errors returned while analyzing Flutter widgets.
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

/// Detect Flutter widget framework issues.
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

    let file_findings = paths
        .par_iter()
        .map(|path| analyze_file(path))
        .collect::<Result<Vec<_>, _>>()?;

    let mut unused_params = Vec::new();
    let mut private_widget_classes = Vec::new();
    let mut top_level_functions = Vec::new();
    for mut findings in file_findings {
        unused_params.append(&mut findings.unused_params);
        private_widget_classes.append(&mut findings.private_widget_classes);
        top_level_functions.append(&mut findings.top_level_functions);
    }
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
    private_widget_classes.sort_by(|left, right| {
        (
            &left.path,
            left.location.line,
            left.location.column,
            &left.widget_class,
        )
            .cmp(&(
                &right.path,
                right.location.line,
                right.location.column,
                &right.widget_class,
            ))
    });
    top_level_functions.sort_by(|left, right| {
        (
            &left.path,
            left.location.line,
            left.location.column,
            &left.function_name,
        )
            .cmp(&(
                &right.path,
                right.location.line,
                right.location.column,
                &right.function_name,
            ))
    });

    Ok(WidgetReport {
        analyzed_files: paths.len(),
        unused_params,
        private_widget_classes,
        top_level_functions,
    })
}

fn analyze_file(path: &Path) -> Result<FileWidgetFindings, WidgetAnalysisError> {
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

    Ok(findings_in_source(path, root, &source))
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

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct FileWidgetFindings {
    unused_params: Vec<UnusedWidgetParam>,
    private_widget_classes: Vec<PrivateWidgetClass>,
    top_level_functions: Vec<WidgetTopLevelFunction>,
}

fn findings_in_source(path: &Path, root: Node<'_>, source: &str) -> FileWidgetFindings {
    let mut classes = Vec::new();
    collect_class_declarations(root, &mut classes);
    let states = state_classes_by_widget(&classes, source);
    let mut findings = FileWidgetFindings::default();
    let has_widget_class = classes
        .iter()
        .any(|class| widget_kind(*class, source).is_some());
    findings.top_level_functions = top_level_widget_functions(path, root, source, has_widget_class);

    for class in classes {
        let Some(widget_kind) = widget_kind(class, source) else {
            continue;
        };
        let Some(name_node) = class.child_by_field_name("name") else {
            continue;
        };
        let Ok(widget_class) = name_node.utf8_text(source.as_bytes()).map(str::to_owned) else {
            continue;
        };
        if widget_class.starts_with('_') {
            findings.private_widget_classes.push(PrivateWidgetClass {
                path: path.to_path_buf(),
                widget_class: widget_class.clone(),
                widget_kind,
                location: name_node.start_position().into(),
            });
        }
        let Some(body) = class.child_by_field_name("body") else {
            continue;
        };
        for param in constructor_params(class, &widget_class, source) {
            if widget_body_uses_param(body, &param.field_name, source)
                || states.get(&widget_class).is_some_and(|state_bodies| {
                    state_bodies.iter().any(|state_body| {
                        state_body_uses_param(*state_body, &param.field_name, source)
                    })
                })
            {
                continue;
            }
            findings.unused_params.push(UnusedWidgetParam {
                path: path.to_path_buf(),
                widget_class: widget_class.clone(),
                widget_kind,
                param_name: param.name,
                location: param.location,
            });
        }
    }

    findings
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
    "initializers",
    "initializer_list_entry",
    "field_initializer",
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

fn visit_named(node: Node<'_>, visitor: &mut impl FnMut(Node<'_>)) {
    visitor(node);
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        visit_named(child, visitor);
    }
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
mod tests;
