use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::package_map::PackageMap;
use crate::{DartCombinator, DartFile, Location};

mod parts;

pub use parts::{InvalidPartReason, InvalidPartRelationship};
use parts::{add_orphan_part_relationships, add_part_dependency};

/// A directed dependency graph where nodes are Dart files.
pub type DependencyGraph = DiGraph<ModuleNode, DependencyEdge>;

/// A Dart file node in the module graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleNode {
    /// Normalized filesystem path for the Dart file.
    pub path: PathBuf,
}

/// A directed Dart library relationship edge in the module graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyEdge {
    /// Raw import/export/part/augment URI from the source file.
    pub specifier: String,
    /// Whether the dependency came from an import, export, part, or augment directive.
    pub kind: DependencyKind,
    /// Location of the directive in the source file.
    pub location: Location,
    /// Import/export visibility metadata carried by this edge.
    pub visibility: DependencyVisibility,
}

/// Dart import/export visibility metadata attached to a dependency edge.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyVisibility {
    /// Optional import prefix after `as`.
    pub prefix: Option<String>,
    /// Whether the import uses `deferred as`.
    pub deferred: bool,
    /// `show` and `hide` combinators applied to the import or export.
    pub combinators: Vec<DartCombinator>,
}

/// Import/export/part/augment dependency category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DependencyKind {
    /// An `import` directive.
    Import,
    /// An `export` directive.
    Export,
    /// A `part` directive.
    Part,
    /// A `library augment` directive.
    Augment,
}

/// A resolved graph edge with source and target paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedDependency {
    /// File containing the directive.
    pub from_path: PathBuf,
    /// File resolved from the directive.
    pub to_path: PathBuf,
    /// Raw import/export/part/augment URI from the source file.
    pub specifier: String,
    /// Whether the dependency came from an import, export, part, or augment directive.
    pub kind: DependencyKind,
    /// Location of the directive in the source file.
    pub location: Location,
    /// Import/export visibility metadata carried by this edge.
    pub visibility: DependencyVisibility,
}

/// A local dependency URI that did not match a parsed Dart file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnresolvedDependency {
    /// File containing the directive.
    pub from_path: PathBuf,
    /// Raw import/export/part/augment URI from the source file.
    pub specifier: String,
    /// Whether the dependency came from an import, export, part, or augment directive.
    pub kind: DependencyKind,
    /// Location of the directive in the source file.
    pub location: Location,
    /// Local filesystem path Decimate tried to resolve.
    pub attempted_path: PathBuf,
    /// Import/export visibility metadata carried by this unresolved edge.
    pub visibility: DependencyVisibility,
}

/// A Phase 2 module graph plus path lookup indexes.
#[derive(Debug, Clone)]
pub struct ModuleGraph {
    root: PathBuf,
    graph: DependencyGraph,
    nodes_by_path: BTreeMap<PathBuf, NodeIndex>,
    unresolved: Vec<UnresolvedDependency>,
    invalid_part_relationships: Vec<InvalidPartRelationship>,
    packages: PackageMap,
}

impl ModuleGraph {
    /// Project root used to resolve package and relative imports.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Underlying petgraph directed graph.
    #[must_use]
    pub fn graph(&self) -> &DependencyGraph {
        &self.graph
    }

    /// Number of Dart file nodes.
    #[must_use]
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Number of resolved dependency edges.
    #[must_use]
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Local dependencies that resolved to paths absent from the parsed input.
    #[must_use]
    pub fn unresolved(&self) -> &[UnresolvedDependency] {
        &self.unresolved
    }

    /// Resolved `part` edges whose target file does not declare a matching `part of`.
    #[must_use]
    pub fn invalid_part_relationships(&self) -> &[InvalidPartRelationship] {
        &self.invalid_part_relationships
    }

    /// Local package names discovered from Pub package resolution metadata.
    #[must_use]
    pub fn package_names(&self) -> Vec<&str> {
        self.packages.names()
    }

    /// Lookup a node by normalized path.
    #[must_use]
    pub fn node_index(&self, path: impl AsRef<Path>) -> Option<NodeIndex> {
        let path = normalize_against(&self.root, path.as_ref());
        self.nodes_by_path.get(&path).copied()
    }

    /// Return all resolved dependencies in graph edge order.
    #[must_use]
    pub fn dependencies(&self) -> Vec<ResolvedDependency> {
        self.graph
            .edge_references()
            .map(|edge| {
                let from = &self.graph[edge.source()].path;
                let to = &self.graph[edge.target()].path;
                let weight = edge.weight();
                ResolvedDependency {
                    from_path: from.clone(),
                    to_path: to.clone(),
                    specifier: weight.specifier.clone(),
                    kind: weight.kind,
                    location: weight.location,
                    visibility: weight.visibility.clone(),
                }
            })
            .collect()
    }
}

/// Errors produced while building a module graph.
#[derive(Debug, Error)]
pub enum GraphError {
    /// A directory could not be read during pubspec discovery.
    #[error("failed to read directory {path}: {source}")]
    ReadDir {
        /// Directory path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A directory entry could not be read during pubspec discovery.
    #[error("failed to read directory entry under {path}: {source}")]
    ReadDirEntry {
        /// Directory being scanned.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A file type could not be read during pubspec discovery.
    #[error("failed to read file type for {path}: {source}")]
    FileType {
        /// Entry path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A discovered `pubspec.yaml` could not be read.
    #[error("failed to read pubspec {path}: {source}")]
    ReadPubspec {
        /// Pubspec path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A discovered `pubspec.yaml` could not be parsed.
    #[error("failed to parse pubspec {path}: {source}")]
    ParsePubspec {
        /// Pubspec path.
        path: PathBuf,
        /// Underlying YAML parse error.
        source: serde_yaml_ng::Error,
    },
    /// A discovered `.dart_tool/package_config.json` could not be read.
    #[error("failed to read package config {path}: {source}")]
    ReadPackageConfig {
        /// Package config path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A discovered `.dart_tool/package_config.json` could not be parsed.
    #[error("failed to parse package config {path}: {source}")]
    ParsePackageConfig {
        /// Package config path.
        path: PathBuf,
        /// Underlying JSON parse error.
        source: serde_json::Error,
    },
    /// A pub workspace glob pattern was invalid.
    #[error("invalid pub workspace glob pattern {pattern}: {source}")]
    WorkspacePattern {
        /// Invalid pattern.
        pattern: String,
        /// Underlying glob pattern error.
        source: glob::PatternError,
    },
    /// A pub workspace glob expansion failed.
    #[error("failed to expand pub workspace glob {pattern}: {source}")]
    WorkspaceGlob {
        /// Pattern being expanded.
        pattern: String,
        /// Underlying glob error.
        source: glob::GlobError,
    },
}

/// Build a directed module graph from extracted Dart file facts.
///
/// Local relative imports and exports are resolved from the source file's
/// directory. `package:` imports resolve through local package names discovered
/// from `pubspec.yaml` files, including pub workspaces and path dependencies.
///
/// # Errors
///
/// Returns [`GraphError`] if a discovered `pubspec.yaml` cannot be read or
/// parsed, or if a workspace glob entry is invalid.
pub fn build_module_graph(
    root: impl AsRef<Path>,
    files: &[DartFile],
) -> Result<ModuleGraph, GraphError> {
    let root = normalize_path(root.as_ref());
    let packages = PackageMap::discover(&root)?;
    let mut graph = DependencyGraph::new();
    let mut nodes_by_path = BTreeMap::new();

    for file in files {
        let path = normalize_against(&root, &file.path);
        let node = graph.add_node(ModuleNode { path: path.clone() });
        nodes_by_path.insert(path, node);
    }

    let known_paths = nodes_by_path.keys().cloned().collect::<BTreeSet<_>>();
    let files_by_path = files
        .iter()
        .map(|file| (normalize_against(&root, &file.path), file))
        .collect::<BTreeMap<_, _>>();
    let mut unresolved = Vec::new();
    let mut invalid_part_relationships = Vec::new();
    let mut referenced_part_paths = BTreeSet::new();
    let mut part_owners = BTreeMap::new();

    for file in files {
        add_file_dependencies(
            &root,
            &packages,
            &known_paths,
            &nodes_by_path,
            &files_by_path,
            &mut graph,
            &mut unresolved,
            &mut invalid_part_relationships,
            &mut referenced_part_paths,
            &mut part_owners,
            file,
        );
    }

    add_orphan_part_relationships(
        &root,
        &packages,
        &files_by_path,
        &referenced_part_paths,
        &mut invalid_part_relationships,
    );
    invalid_part_relationships.sort_by(|left, right| {
        (
            &left.part_path,
            &left.library_path,
            left.location.line,
            &left.specifier,
        )
            .cmp(&(
                &right.part_path,
                &right.library_path,
                right.location.line,
                &right.specifier,
            ))
    });

    Ok(ModuleGraph {
        root,
        graph,
        nodes_by_path,
        unresolved,
        invalid_part_relationships,
        packages,
    })
}

#[expect(
    clippy::too_many_arguments,
    reason = "keeps per-file edge construction in one owner without hiding graph state"
)]
fn add_file_dependencies(
    root: &Path,
    packages: &PackageMap,
    known_paths: &BTreeSet<PathBuf>,
    nodes_by_path: &BTreeMap<PathBuf, NodeIndex>,
    files_by_path: &BTreeMap<PathBuf, &DartFile>,
    graph: &mut DependencyGraph,
    unresolved: &mut Vec<UnresolvedDependency>,
    invalid_part_relationships: &mut Vec<InvalidPartRelationship>,
    referenced_part_paths: &mut BTreeSet<PathBuf>,
    part_owners: &mut BTreeMap<PathBuf, PathBuf>,
    file: &DartFile,
) {
    let from_path = normalize_against(root, &file.path);
    let Some(from_index) = nodes_by_path.get(&from_path).copied() else {
        return;
    };

    if let Some(library) = &file.library
        && let Some(augment_uri) = &library.augment_uri
    {
        add_dependency(
            root,
            packages,
            known_paths,
            nodes_by_path,
            graph,
            unresolved,
            from_index,
            &from_path,
            augment_uri,
            DependencyKind::Augment,
            library.location,
            DependencyVisibility::default(),
        );
    }

    for import in &file.imports {
        add_dependency(
            root,
            packages,
            known_paths,
            nodes_by_path,
            graph,
            unresolved,
            from_index,
            &from_path,
            &import.uri,
            DependencyKind::Import,
            import.location,
            DependencyVisibility {
                prefix: import.prefix.clone(),
                deferred: import.deferred,
                combinators: import.combinators.clone(),
            },
        );
    }

    for export in &file.exports {
        add_dependency(
            root,
            packages,
            known_paths,
            nodes_by_path,
            graph,
            unresolved,
            from_index,
            &from_path,
            &export.uri,
            DependencyKind::Export,
            export.location,
            DependencyVisibility {
                combinators: export.combinators.clone(),
                ..DependencyVisibility::default()
            },
        );
    }

    for part in &file.parts {
        add_part_dependency(
            root,
            packages,
            known_paths,
            nodes_by_path,
            files_by_path,
            graph,
            unresolved,
            invalid_part_relationships,
            referenced_part_paths,
            part_owners,
            from_index,
            &from_path,
            file,
            &part.uri,
            part.location,
        );
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "keeps edge construction single-owner without creating a shallow context wrapper"
)]
fn add_dependency(
    root: &Path,
    packages: &PackageMap,
    known_paths: &BTreeSet<PathBuf>,
    nodes_by_path: &BTreeMap<PathBuf, NodeIndex>,
    graph: &mut DependencyGraph,
    unresolved: &mut Vec<UnresolvedDependency>,
    from_index: NodeIndex,
    from_path: &Path,
    specifier: &str,
    kind: DependencyKind,
    location: Location,
    visibility: DependencyVisibility,
) {
    let Some(target) = resolve_local_uri(root, packages, from_path, specifier) else {
        return;
    };
    let target_path = target.path;

    if !known_paths.contains(&target_path) {
        if !target.local {
            return;
        }

        unresolved.push(UnresolvedDependency {
            from_path: from_path.to_path_buf(),
            specifier: specifier.to_owned(),
            kind,
            location,
            attempted_path: target_path,
            visibility,
        });
        return;
    }

    let Some(target_index) = nodes_by_path.get(&target_path).copied() else {
        return;
    };

    graph.add_edge(
        from_index,
        target_index,
        DependencyEdge {
            specifier: specifier.to_owned(),
            kind,
            location,
            visibility,
        },
    );
}

pub(super) struct ResolvedTarget {
    pub(super) path: PathBuf,
    pub(super) local: bool,
}

pub(super) fn resolve_local_uri(
    root: &Path,
    packages: &PackageMap,
    from_path: &Path,
    specifier: &str,
) -> Option<ResolvedTarget> {
    if specifier.starts_with("dart:") {
        return None;
    }

    if let Some(rest) = specifier.strip_prefix("package:") {
        let (package, path) = rest.split_once('/')?;
        return packages
            .resolve(package, path)
            .map(|resolution| ResolvedTarget {
                path: resolution.path,
                local: resolution.local,
            });
    }

    if specifier.contains(':') {
        return None;
    }

    let base = from_path.parent().unwrap_or(root);
    Some(ResolvedTarget {
        path: normalize_path(&base.join(specifier)),
        local: true,
    })
}

pub(crate) fn normalize_against(root: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() || path.starts_with(root) {
        normalize_path(path)
    } else {
        normalize_path(&root.join(path))
    }
}

pub(crate) fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();

    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    normalized.push(component.as_os_str());
                }
            }
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir | Component::Normal(_) => normalized.push(component.as_os_str()),
        }
    }

    if normalized.as_os_str().is_empty() {
        return PathBuf::from(".");
    }

    normalized
}

#[cfg(test)]
mod augment_tests;
#[cfg(test)]
mod tests;
