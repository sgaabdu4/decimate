use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{
    BoundaryInventory, BoundaryRule, DartFile, LocalPubPackage, ScannedProject, boundary_inventory,
};

/// Stable JSON schema version for project-list reports.
pub const PROJECT_LIST_SCHEMA_VERSION: &str = "decimate.list.v1";

/// Sections included in a project-list report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectListOptions {
    sections: BTreeSet<ProjectListSection>,
}

/// Project-list report section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ProjectListSection {
    /// Parsed Dart files.
    Files,
    /// Effective entry points.
    EntryPoints,
    /// Discovered local pub packages.
    Workspaces,
    /// Active Decimate adapters.
    Plugins,
    /// Architecture boundary zones and rules.
    Boundaries,
}

impl ProjectListOptions {
    /// Include all list sections.
    #[must_use]
    pub fn all() -> Self {
        Self::from_sections([
            ProjectListSection::Files,
            ProjectListSection::EntryPoints,
            ProjectListSection::Workspaces,
            ProjectListSection::Plugins,
            ProjectListSection::Boundaries,
        ])
    }

    /// Include selected list sections.
    #[must_use]
    pub fn from_sections(sections: impl IntoIterator<Item = ProjectListSection>) -> Self {
        Self {
            sections: sections.into_iter().collect(),
        }
    }

    fn includes(&self, section: ProjectListSection) -> bool {
        self.sections.contains(&section)
    }
}

/// Project metadata report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectListReport {
    /// Schema identifier.
    pub schema_version: String,
    /// Tool name.
    pub tool: String,
    /// Command that produced this report.
    pub command: String,
    /// Numeric rollup.
    pub summary: ProjectListSummary,
    /// Parsed Dart files.
    pub files: Vec<ListedFile>,
    /// Effective entry points.
    pub entry_points: Vec<ListedEntryPoint>,
    /// Discovered local pub packages.
    pub workspaces: Vec<ListedWorkspace>,
    /// Active Decimate adapters.
    pub plugins: Vec<ListedPlugin>,
    /// Configured architecture boundaries.
    pub boundaries: ListedBoundaries,
}

/// Numeric project-list rollup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectListSummary {
    /// Dart files parsed.
    pub files: usize,
    /// Resolved graph edges.
    pub edges: usize,
    /// Local dependencies that did not resolve to parsed files.
    pub unresolved_dependencies: usize,
    /// Effective entry points.
    pub entry_points: usize,
    /// Discovered local pub packages.
    pub workspaces: usize,
    /// Active adapters.
    pub plugins: usize,
    /// Configured architecture boundary zones.
    pub boundary_zones: usize,
}

/// Parsed Dart file metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListedFile {
    /// Dart file path, root-relative where possible.
    pub path: String,
    /// Import directive count.
    pub imports: usize,
    /// Export directive count.
    pub exports: usize,
    /// Part directive count.
    pub parts: usize,
    /// Top-level declaration count.
    pub declarations: usize,
}

/// Entry-point metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListedEntryPoint {
    /// Entry-point path, root-relative where possible.
    pub path: String,
    /// Whether this came from config/CLI or Decimate heuristics.
    pub source: String,
}

/// Local pub package metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListedWorkspace {
    /// Pub package name.
    pub name: String,
    /// Package root, root-relative where possible.
    pub root: String,
    /// Pubspec path, root-relative where possible.
    pub pubspec_path: String,
}

/// Decimate adapter metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListedPlugin {
    /// Adapter name.
    pub name: String,
    /// Whether the adapter is active for this project.
    pub active: bool,
    /// Why Decimate selected or skipped it.
    pub reason: String,
}

/// Architecture boundary inventory.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListedBoundaries {
    /// Whether any boundaries are configured.
    pub configured: bool,
    /// Configured zones.
    pub zones: Vec<ListedBoundaryZone>,
    /// Configured access rules.
    pub rules: Vec<ListedBoundaryRule>,
    /// Dart library files outside every configured zone.
    pub uncovered_files: Vec<String>,
}

/// One listed architecture boundary zone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListedBoundaryZone {
    /// Zone path.
    pub path: String,
    /// Dart library file count under this zone.
    pub file_count: usize,
}

/// One listed architecture boundary access rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListedBoundaryRule {
    /// Source zone.
    pub from: String,
    /// Disallowed target zone.
    pub disallow: String,
    /// Dart library files covered by the source zone.
    pub covered_files: usize,
}

/// Build a project-list report.
#[must_use]
pub fn project_list_report(
    project: &ScannedProject,
    packages: &[LocalPubPackage],
    entry_points: &[PathBuf],
    entry_source: &str,
    boundaries: &[BoundaryRule],
    options: &ProjectListOptions,
) -> ProjectListReport {
    let plugins = list_plugins(packages);
    let boundary_inventory = boundary_inventory(project, boundaries);
    ProjectListReport {
        schema_version: PROJECT_LIST_SCHEMA_VERSION.to_owned(),
        tool: "decimate".to_owned(),
        command: "list".to_owned(),
        summary: ProjectListSummary {
            files: project.files.len(),
            edges: project.graph.edge_count(),
            unresolved_dependencies: project.graph.unresolved().len(),
            entry_points: entry_points.len(),
            workspaces: packages.len(),
            plugins: plugins.iter().filter(|plugin| plugin.active).count(),
            boundary_zones: boundary_inventory.zones.len(),
        },
        files: if options.includes(ProjectListSection::Files) {
            list_files(&project.root, &project.files)
        } else {
            Vec::new()
        },
        entry_points: if options.includes(ProjectListSection::EntryPoints) {
            list_entry_points(&project.root, entry_points, entry_source)
        } else {
            Vec::new()
        },
        workspaces: if options.includes(ProjectListSection::Workspaces) {
            list_workspaces(&project.root, packages)
        } else {
            Vec::new()
        },
        plugins: if options.includes(ProjectListSection::Plugins) {
            plugins
        } else {
            Vec::new()
        },
        boundaries: if options.includes(ProjectListSection::Boundaries) {
            list_boundaries(&project.root, &boundary_inventory)
        } else {
            ListedBoundaries::default()
        },
    }
}

fn list_files(root: &Path, files: &[DartFile]) -> Vec<ListedFile> {
    files
        .iter()
        .map(|file| ListedFile {
            path: display_path(root, &file.path),
            imports: file.imports.len(),
            exports: file.exports.len(),
            parts: file.parts.len(),
            declarations: file.declarations.len(),
        })
        .collect()
}

fn list_entry_points(
    root: &Path,
    entry_points: &[PathBuf],
    entry_source: &str,
) -> Vec<ListedEntryPoint> {
    entry_points
        .iter()
        .map(|path| ListedEntryPoint {
            path: display_path(root, path),
            source: entry_source.to_owned(),
        })
        .collect()
}

fn list_workspaces(root: &Path, packages: &[LocalPubPackage]) -> Vec<ListedWorkspace> {
    packages
        .iter()
        .map(|package| ListedWorkspace {
            name: package.name.clone(),
            root: display_path(root, &package.root),
            pubspec_path: display_path(root, &package.pubspec_path),
        })
        .collect()
}

fn list_plugins(packages: &[LocalPubPackage]) -> Vec<ListedPlugin> {
    let flutter = packages.iter().any(is_flutter_package);
    vec![
        ListedPlugin {
            name: "dart".to_owned(),
            active: true,
            reason: "Dart files are parsed with tree-sitter-dart".to_owned(),
        },
        ListedPlugin {
            name: "flutter".to_owned(),
            active: flutter,
            reason: if flutter {
                "A local pubspec declares the Flutter SDK".to_owned()
            } else {
                "No local pubspec declares the Flutter SDK".to_owned()
            },
        },
        ListedPlugin {
            name: "pub-workspace".to_owned(),
            active: packages.len() > 1,
            reason: if packages.len() > 1 {
                "Multiple local pub packages were discovered".to_owned()
            } else {
                "Only one local pub package was discovered".to_owned()
            },
        },
    ]
}

fn list_boundaries(root: &Path, inventory: &BoundaryInventory) -> ListedBoundaries {
    ListedBoundaries {
        configured: inventory.configured,
        zones: inventory
            .zones
            .iter()
            .map(|zone| ListedBoundaryZone {
                path: display_path(root, &zone.path),
                file_count: zone.file_count,
            })
            .collect(),
        rules: inventory
            .rules
            .iter()
            .map(|rule| ListedBoundaryRule {
                from: display_path(root, &rule.from),
                disallow: display_path(root, &rule.disallow),
                covered_files: rule.covered_files,
            })
            .collect(),
        uncovered_files: inventory
            .uncovered_files
            .iter()
            .map(|path| display_path(root, path))
            .collect(),
    }
}

fn is_flutter_package(package: &LocalPubPackage) -> bool {
    fs::read_to_string(&package.pubspec_path).is_ok_and(|source| {
        source
            .lines()
            .any(|line| line.trim_start().starts_with("flutter:"))
    })
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => value.to_str(),
            std::path::Component::RootDir => Some(""),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}
