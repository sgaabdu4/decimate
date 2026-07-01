use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::graph::normalize_against;
use crate::{DependencyKind, ScannedProject};

/// Stable decision-surface schema version.
pub const DECISION_SURFACE_SCHEMA_VERSION: &str = "dart-decimate.decision-surface.v1";

/// Advisory review surface for changed Dart/Flutter code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionSurfaceReport {
    /// Schema identifier.
    pub schema_version: String,
    /// Typed JSON envelope discriminator.
    pub kind: String,
    /// Tool name.
    pub tool: String,
    /// Command that produced this report.
    pub command: String,
    /// Git base used to find changed files.
    pub base: String,
    /// Numeric rollup.
    pub summary: DecisionSurfaceSummary,
    /// Ranked structural decisions that need reviewer judgment.
    pub decisions: Vec<DecisionSurfaceDecision>,
}

/// Decision-surface rollup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionSurfaceSummary {
    /// Changed files considered.
    pub changed_files: usize,
    /// Decisions emitted after ranking and truncation.
    pub decisions: usize,
}

/// One advisory structural decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionSurfaceDecision {
    /// Stable id for this decision.
    pub id: String,
    /// Decision category.
    pub category: DecisionSurfaceCategory,
    /// Lower values are more important.
    pub priority: usize,
    /// Review question to answer.
    pub question: String,
    /// Primary changed file.
    pub path: String,
    /// Related files, root-relative where possible.
    pub files: Vec<String>,
    /// Concrete evidence behind the decision.
    pub evidence: Vec<String>,
    /// Suggested reviewer expertise.
    pub recommended_expert: String,
    /// Read-only commands to run before changing code.
    pub suggested_commands: Vec<String>,
}

/// Decision category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DecisionSurfaceCategory {
    /// Boundary or coupling judgment.
    CouplingBoundary,
    /// Public API/export-surface judgment.
    PublicApiContract,
    /// Pub dependency ownership judgment.
    Dependency,
}

/// Build the decision surface from current graph facts and changed files.
#[must_use]
pub fn decision_surface_report(
    project: &ScannedProject,
    base: &str,
    changed_files: &[PathBuf],
    max_decisions: usize,
) -> DecisionSurfaceReport {
    decision_surface_report_for_command(
        project,
        base,
        changed_files,
        max_decisions,
        "decision-surface",
    )
}

/// Build the decision surface for a specific CLI command alias.
#[must_use]
pub fn decision_surface_report_for_command(
    project: &ScannedProject,
    base: &str,
    changed_files: &[PathBuf],
    max_decisions: usize,
    command: &str,
) -> DecisionSurfaceReport {
    let changed = changed_files
        .iter()
        .map(|path| normalize_against(&project.root, path))
        .collect::<BTreeSet<_>>();
    let mut decisions = BTreeMap::new();

    add_coupling_decisions(project, &changed, &mut decisions);
    add_public_api_decisions(project, &changed, &mut decisions);
    add_dependency_decisions(project, &changed, &mut decisions);

    let mut decisions = decisions.into_values().collect::<Vec<_>>();
    decisions.sort_by(|left, right| {
        (
            left.priority,
            left.category,
            &left.path,
            &left.question,
            &left.id,
        )
            .cmp(&(
                right.priority,
                right.category,
                &right.path,
                &right.question,
                &right.id,
            ))
    });
    decisions.truncate(max_decisions);

    DecisionSurfaceReport {
        schema_version: DECISION_SURFACE_SCHEMA_VERSION.to_owned(),
        kind: "decision-surface".to_owned(),
        tool: "dart-decimate".to_owned(),
        command: command.to_owned(),
        base: base.to_owned(),
        summary: DecisionSurfaceSummary {
            changed_files: changed.len(),
            decisions: decisions.len(),
        },
        decisions,
    }
}

/// Render a concise human decision-surface report.
#[must_use]
pub fn render_decision_surface_report(report: &DecisionSurfaceReport) -> String {
    let mut rendered = String::new();
    let _ = writeln!(
        rendered,
        "decision-surface base={} changed_files={} decisions={}",
        report.base, report.summary.changed_files, report.summary.decisions
    );
    for decision in &report.decisions {
        let _ = writeln!(
            rendered,
            "- {:?} {}: {}",
            decision.category, decision.path, decision.question
        );
    }
    rendered
}

fn add_coupling_decisions(
    project: &ScannedProject,
    changed: &BTreeSet<PathBuf>,
    decisions: &mut BTreeMap<String, DecisionSurfaceDecision>,
) {
    for dependency in project.graph.dependencies() {
        if !changed.contains(&dependency.from_path) && !changed.contains(&dependency.to_path) {
            continue;
        }

        let Some(from_zone) = architectural_zone(&project.root, &dependency.from_path) else {
            continue;
        };
        let Some(to_zone) = architectural_zone(&project.root, &dependency.to_path) else {
            continue;
        };
        if from_zone == to_zone {
            continue;
        }

        let from = display_path(&project.root, &dependency.from_path);
        let to = display_path(&project.root, &dependency.to_path);
        let path = if changed.contains(&dependency.from_path) {
            from.clone()
        } else {
            to.clone()
        };
        let id = format!("coupling-boundary:{from}:{to}:{}", dependency.specifier);
        decisions
            .entry(id.clone())
            .or_insert(DecisionSurfaceDecision {
                id,
                category: DecisionSurfaceCategory::CouplingBoundary,
                priority: 1,
                question: format!(
                    "Should {from_zone} code in {from} depend on {to_zone} code in {to}?"
                ),
                path,
                files: vec![from.clone(), to.clone()],
                evidence: vec![format!(
                    "{from} {} {to} via {}",
                    dependency_kind(dependency.kind),
                    dependency.specifier
                )],
                recommended_expert: "architecture".to_owned(),
                suggested_commands: vec![format!(
                    "dart-decimate inspect --format json --file {from}"
                )],
            });
    }
}

fn add_public_api_decisions(
    project: &ScannedProject,
    changed: &BTreeSet<PathBuf>,
    decisions: &mut BTreeMap<String, DecisionSurfaceDecision>,
) {
    for path in changed
        .iter()
        .filter(|path| is_public_library_path(project, path))
    {
        let display = display_path(&project.root, path);
        let id = format!("public-api-contract:{display}");
        decisions
            .entry(id.clone())
            .or_insert(DecisionSurfaceDecision {
                id,
                category: DecisionSurfaceCategory::PublicApiContract,
                priority: 2,
                question: format!("Does changing public library {display} alter the package API?"),
                path: display.clone(),
                files: vec![display.clone()],
                evidence: vec![format!(
                    "{display} is outside lib/src and is public to importers"
                )],
                recommended_expert: "api-owner".to_owned(),
                suggested_commands: vec![format!(
                    "dart-decimate inspect --format json --file {display}"
                )],
            });
    }

    for dependency in project
        .graph
        .dependencies()
        .into_iter()
        .filter(|dependency| dependency.kind == DependencyKind::Export)
    {
        if !changed.contains(&dependency.to_path)
            || !is_public_library_path(project, &dependency.from_path)
        {
            continue;
        }
        let entry = display_path(&project.root, &dependency.from_path);
        let target = display_path(&project.root, &dependency.to_path);
        let id = format!("public-api-contract:{entry}:{target}");
        decisions
            .entry(id.clone())
            .or_insert(DecisionSurfaceDecision {
                id,
                category: DecisionSurfaceCategory::PublicApiContract,
                priority: 2,
                question: format!(
                    "Does changing {target} alter the public API exposed by {entry}?"
                ),
                path: target.clone(),
                files: vec![target.clone(), entry.clone()],
                evidence: vec![format!(
                    "{entry} exports {target} via {}",
                    dependency.specifier
                )],
                recommended_expert: "api-owner".to_owned(),
                suggested_commands: vec![format!(
                    "dart-decimate inspect --format json --file {target}"
                )],
            });
    }
}

fn add_dependency_decisions(
    project: &ScannedProject,
    changed: &BTreeSet<PathBuf>,
    decisions: &mut BTreeMap<String, DecisionSurfaceDecision>,
) {
    for path in changed.iter().filter(|path| {
        path.file_name()
            .is_some_and(|file_name| file_name == "pubspec.yaml")
    }) {
        let display = display_path(&project.root, path);
        let id = format!("dependency:{display}");
        decisions
            .entry(id.clone())
            .or_insert(DecisionSurfaceDecision {
                id,
                category: DecisionSurfaceCategory::Dependency,
                priority: 3,
                question: format!(
                    "Does {display} declare the right runtime, dev, and override dependencies?"
                ),
                path: display.clone(),
                files: vec![display.clone()],
                evidence: vec![format!(
                    "{display} changed and owns Pub dependency declarations"
                )],
                recommended_expert: "pub-dependency-owner".to_owned(),
                suggested_commands: vec![format!(
                    "dart-decimate check --format json --file {display}"
                )],
            });
    }

    for file in project.files.iter().filter(|file| {
        let path = normalize_against(&project.root, &file.path);
        changed.contains(&path)
    }) {
        let path = normalize_against(&project.root, &file.path);
        let display = display_path(&project.root, &path);
        for package in package_imports(file) {
            let id = format!("dependency:{display}:{package}");
            decisions
                .entry(id.clone())
                .or_insert(DecisionSurfaceDecision {
                    id,
                    category: DecisionSurfaceCategory::Dependency,
                    priority: 3,
                    question: format!(
                        "Should {display} introduce or keep the Pub dependency {package}?"
                    ),
                    path: display.clone(),
                    files: vec![display.clone()],
                    evidence: vec![format!("{display} imports package:{package}/...")],
                    recommended_expert: "pub-dependency-owner".to_owned(),
                    suggested_commands: vec![format!(
                        "dart-decimate trace-dependency --format json --dependency {package}"
                    )],
                });
        }
    }
}

fn package_imports(file: &crate::DartFile) -> BTreeSet<String> {
    file.imports
        .iter()
        .map(|import| &import.uri)
        .chain(file.exports.iter().map(|export| &export.uri))
        .filter_map(|uri| uri.strip_prefix("package:"))
        .filter_map(|rest| rest.split_once('/').map(|(package, _)| package.to_owned()))
        .collect()
}

fn architectural_zone(root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(root).unwrap_or(path);
    let components = relative
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>();
    if components.first() != Some(&"lib") {
        return None;
    }

    for component in &components {
        if matches!(
            *component,
            "core" | "data" | "domain" | "presentation" | "ui"
        ) {
            return Some((*component).to_owned());
        }
    }

    None
}

fn is_public_library_path(project: &ScannedProject, path: &Path) -> bool {
    let relative = path.strip_prefix(&project.root).unwrap_or(path);
    let components = relative
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>();
    components.first() == Some(&"lib")
        && path
            .extension()
            .is_some_and(|extension| extension == "dart")
        && components
            .get(1)
            .is_none_or(|component| *component != "src")
}

fn dependency_kind(kind: DependencyKind) -> &'static str {
    match kind {
        DependencyKind::Import => "imports",
        DependencyKind::Export => "exports",
        DependencyKind::Part => "parts",
        DependencyKind::Augment => "augments",
    }
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
