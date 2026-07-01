use std::collections::{BTreeSet, VecDeque};
use std::path::{Path, PathBuf};

use petgraph::Direction;
use petgraph::algo::tarjan_scc;
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};

use crate::graph::normalize_against;
use crate::{DependencyKind, Location, ModuleGraph};

/// Dead-code reachability result for a module graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeadCodeReport {
    /// Entry point files found in the graph.
    pub entry_points: Vec<PathBuf>,
    /// Entry point files requested by the caller but absent from the graph.
    pub missing_entry_points: Vec<PathBuf>,
    /// Files reachable from at least one entry point.
    pub reachable_files: Vec<PathBuf>,
    /// Files not reachable from any entry point.
    pub dead_files: Vec<DeadFile>,
}

/// An unreachable Dart file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeadFile {
    /// Unreachable file path.
    pub path: PathBuf,
    /// Whether Dart Decimate can suggest deletion from graph evidence alone.
    pub safe_to_delete: bool,
}

/// A circular dependency reported as one strongly connected component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyCycle {
    /// Files participating in the cycle, sorted for deterministic output.
    pub files: Vec<PathBuf>,
}

/// A cycle composed only of Dart `export` edges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReExportCycle {
    /// Files participating in the re-export cycle, sorted for deterministic output.
    pub files: Vec<PathBuf>,
}

/// A directory-based architecture boundary rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryRule {
    /// Source directory covered by the rule, relative to the project root unless absolute.
    pub from: PathBuf,
    /// Directory that matching source files must not depend on.
    pub disallow: PathBuf,
}

impl BoundaryRule {
    /// Create a boundary rule.
    pub fn new(from: impl Into<PathBuf>, disallow: impl Into<PathBuf>) -> Self {
        Self {
            from: from.into(),
            disallow: disallow.into(),
        }
    }
}

/// A graph edge that violates a boundary rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryViolation {
    /// File containing the violating directive.
    pub from_path: PathBuf,
    /// File resolved from the violating directive.
    pub to_path: PathBuf,
    /// Rule source directory.
    pub from_boundary: PathBuf,
    /// Rule disallowed target directory.
    pub disallowed_boundary: PathBuf,
    /// Raw import/export URI from the source file.
    pub specifier: String,
    /// Whether the dependency came from an import or export directive.
    pub kind: DependencyKind,
    /// Location of the directive in the source file.
    pub location: Location,
}

/// Traverse the graph from entry points and flag unreachable files.
#[must_use]
pub fn find_dead_code<P>(
    graph: &ModuleGraph,
    entry_points: impl IntoIterator<Item = P>,
) -> DeadCodeReport
where
    P: AsRef<Path>,
{
    let mut reachable_nodes = BTreeSet::new();
    let mut found_entry_points = BTreeSet::new();
    let mut missing_entry_points = BTreeSet::new();

    for entry_point in entry_points {
        let entry_path = normalize_against(graph.root(), entry_point.as_ref());
        let Some(start) = graph.node_index(&entry_path) else {
            missing_entry_points.insert(entry_path);
            continue;
        };

        found_entry_points.insert(entry_path);
        traverse_reachable(graph, start, &mut reachable_nodes);
    }

    let reachable_files = reachable_nodes
        .iter()
        .map(|node| graph.graph()[*node].path.clone())
        .collect::<BTreeSet<_>>();

    let mut dead_files = graph
        .graph()
        .node_indices()
        .filter_map(|node| {
            if reachable_nodes.contains(&node) {
                return None;
            }
            Some(DeadFile {
                path: graph.graph()[node].path.clone(),
                safe_to_delete: true,
            })
        })
        .collect::<Vec<_>>();
    dead_files.sort_by(|left, right| left.path.cmp(&right.path));

    DeadCodeReport {
        entry_points: found_entry_points.into_iter().collect(),
        missing_entry_points: missing_entry_points.into_iter().collect(),
        reachable_files: reachable_files.into_iter().collect(),
        dead_files,
    }
}

/// Detect circular dependencies using petgraph's Tarjan SCC implementation.
#[must_use]
pub fn detect_cycles(graph: &ModuleGraph) -> Vec<DependencyCycle> {
    let mut cycles = tarjan_scc(graph.graph())
        .into_iter()
        .filter_map(|component| cycle_from_component(graph, component))
        .collect::<Vec<_>>();

    cycles.sort_by(|left, right| left.files.cmp(&right.files));
    cycles
}

/// Detect structural cycles made only of `export` edges.
#[must_use]
pub fn detect_re_export_cycles(graph: &ModuleGraph) -> Vec<ReExportCycle> {
    let mut export_graph = DiGraph::<NodeIndex, ()>::new();
    let mut original_to_export = std::collections::BTreeMap::new();

    for node in graph.graph().node_indices() {
        let mapped = export_graph.add_node(node);
        original_to_export.insert(node, mapped);
    }

    for edge in graph
        .graph()
        .edge_references()
        .filter(|edge| edge.weight().kind == DependencyKind::Export)
    {
        let Some(from) = original_to_export.get(&edge.source()).copied() else {
            continue;
        };
        let Some(to) = original_to_export.get(&edge.target()).copied() else {
            continue;
        };
        export_graph.add_edge(from, to, ());
    }

    let mut cycles = tarjan_scc(&export_graph)
        .into_iter()
        .filter_map(|component| re_export_cycle_from_component(graph, &export_graph, component))
        .collect::<Vec<_>>();
    cycles.sort_by(|left, right| left.files.cmp(&right.files));
    cycles
}

/// Check directory boundary rules against all resolved graph edges.
#[must_use]
pub fn check_architecture_boundaries(
    graph: &ModuleGraph,
    rules: &[BoundaryRule],
) -> Vec<BoundaryViolation> {
    let normalized_rules = rules
        .iter()
        .map(|rule| {
            (
                normalize_against(graph.root(), &rule.from),
                normalize_against(graph.root(), &rule.disallow),
            )
        })
        .collect::<Vec<_>>();

    let mut violations = graph
        .dependencies()
        .into_iter()
        .flat_map(|dependency| {
            normalized_rules
                .iter()
                .filter_map(move |(from_boundary, disallowed_boundary)| {
                    if dependency.from_path.starts_with(from_boundary)
                        && dependency.to_path.starts_with(disallowed_boundary)
                    {
                        return Some(BoundaryViolation {
                            from_path: dependency.from_path.clone(),
                            to_path: dependency.to_path.clone(),
                            from_boundary: from_boundary.clone(),
                            disallowed_boundary: disallowed_boundary.clone(),
                            specifier: dependency.specifier.clone(),
                            kind: dependency.kind,
                            location: dependency.location,
                        });
                    }

                    None
                })
        })
        .collect::<Vec<_>>();

    violations.sort_by(|left, right| {
        (
            &left.from_path,
            &left.to_path,
            &left.from_boundary,
            &left.disallowed_boundary,
        )
            .cmp(&(
                &right.from_path,
                &right.to_path,
                &right.from_boundary,
                &right.disallowed_boundary,
            ))
    });
    violations
}

fn traverse_reachable(graph: &ModuleGraph, start: NodeIndex, reachable: &mut BTreeSet<NodeIndex>) {
    let mut queue = VecDeque::from([start]);
    while let Some(node) = queue.pop_front() {
        if !reachable.insert(node) {
            continue;
        }
        for edge in graph.graph().edges(node) {
            queue.push_back(edge.target());
        }
        for edge in graph.graph().edges_directed(node, Direction::Incoming) {
            if edge.weight().kind == DependencyKind::Augment {
                queue.push_back(edge.source());
            }
        }
    }
}

fn cycle_from_component(graph: &ModuleGraph, component: Vec<NodeIndex>) -> Option<DependencyCycle> {
    let first = component.first().copied()?;
    let is_cycle = component.len() > 1 || component_has_self_loop(graph, first);
    if !is_cycle {
        return None;
    }

    let mut files = component
        .into_iter()
        .map(|node| graph.graph()[node].path.clone())
        .collect::<Vec<_>>();
    files.sort();

    Some(DependencyCycle { files })
}

fn component_has_self_loop(graph: &ModuleGraph, node: NodeIndex) -> bool {
    graph.graph().edges(node).any(|edge| edge.target() == node)
}

fn re_export_cycle_from_component(
    graph: &ModuleGraph,
    export_graph: &DiGraph<NodeIndex, ()>,
    component: Vec<NodeIndex>,
) -> Option<ReExportCycle> {
    let first = component.first().copied()?;
    let is_cycle = component.len() > 1 || export_component_has_self_loop(export_graph, first);
    if !is_cycle {
        return None;
    }

    let mut files = component
        .into_iter()
        .map(|node| graph.graph()[export_graph[node]].path.clone())
        .collect::<Vec<_>>();
    files.sort();

    Some(ReExportCycle { files })
}

fn export_component_has_self_loop(graph: &DiGraph<NodeIndex, ()>, node: NodeIndex) -> bool {
    graph.edges(node).any(|edge| edge.target() == node)
}

#[cfg(test)]
mod tests;
