use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::graph::normalize_against;
use crate::{BoundaryRule, Location, ScannedProject};

/// A Dart library file not covered by any configured architecture boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryCoverageGap {
    /// File outside every configured boundary path.
    pub path: PathBuf,
    /// Boundary paths Decimate considered as architecture zones.
    pub configured_boundaries: Vec<PathBuf>,
    /// Location used for report anchoring.
    pub location: Location,
}

/// Architecture boundary inventory for `decimate list --boundaries`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryInventory {
    /// Whether any boundary rules are configured.
    pub configured: bool,
    /// Directory zones inferred from configured rules.
    pub zones: Vec<BoundaryZone>,
    /// Configured access rules.
    pub rules: Vec<BoundaryAccessRule>,
    /// Dart library files outside every configured zone.
    pub uncovered_files: Vec<PathBuf>,
}

/// One configured boundary zone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryZone {
    /// Boundary path.
    pub path: PathBuf,
    /// Dart library files under this boundary path.
    pub file_count: usize,
}

/// One configured boundary access rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryAccessRule {
    /// Source zone.
    pub from: PathBuf,
    /// Disallowed target zone.
    pub disallow: PathBuf,
    /// Files covered by the source zone.
    pub covered_files: usize,
}

/// Build a boundary inventory from configured directory rules.
#[must_use]
pub fn boundary_inventory(project: &ScannedProject, rules: &[BoundaryRule]) -> BoundaryInventory {
    let zones = configured_boundary_paths(&project.root, rules);
    let library_files = library_files(project);
    let listed_zones = zones
        .iter()
        .map(|zone| BoundaryZone {
            path: zone.clone(),
            file_count: library_files
                .iter()
                .filter(|path| path.starts_with(zone))
                .count(),
        })
        .collect::<Vec<_>>();
    let rules = rules
        .iter()
        .map(|rule| {
            let from = normalize_against(&project.root, &rule.from);
            let disallow = normalize_against(&project.root, &rule.disallow);
            BoundaryAccessRule {
                covered_files: library_files
                    .iter()
                    .filter(|path| path.starts_with(&from))
                    .count(),
                from,
                disallow,
            }
        })
        .collect::<Vec<_>>();
    let uncovered_files = library_files
        .into_iter()
        .filter(|path| !zones.iter().any(|zone| path.starts_with(zone)))
        .collect::<Vec<_>>();

    BoundaryInventory {
        configured: !listed_zones.is_empty(),
        zones: listed_zones,
        rules,
        uncovered_files,
    }
}

/// Report Dart library files that do not belong to any configured boundary zone.
#[must_use]
pub fn detect_boundary_coverage(
    project: &ScannedProject,
    rules: &[BoundaryRule],
) -> Vec<BoundaryCoverageGap> {
    let configured_boundaries = configured_boundary_paths(&project.root, rules);
    if configured_boundaries.is_empty() {
        return Vec::new();
    }

    let mut gaps = project
        .files
        .iter()
        .filter_map(|file| {
            let path = normalize_against(&project.root, &file.path);
            if !is_library_source(&project.root, &path)
                || is_generated_path(&path)
                || configured_boundaries
                    .iter()
                    .any(|boundary| path.starts_with(boundary))
            {
                return None;
            }

            Some(BoundaryCoverageGap {
                path,
                configured_boundaries: configured_boundaries.clone(),
                location: file
                    .declarations
                    .first()
                    .map_or(Location { line: 1, column: 0 }, |declaration| {
                        declaration.location
                    }),
            })
        })
        .collect::<Vec<_>>();

    gaps.sort_by(|left, right| left.path.cmp(&right.path));
    gaps
}

fn library_files(project: &ScannedProject) -> Vec<PathBuf> {
    project
        .files
        .iter()
        .filter_map(|file| {
            let path = normalize_against(&project.root, &file.path);
            (is_library_source(&project.root, &path) && !is_generated_path(&path)).then_some(path)
        })
        .collect()
}

fn configured_boundary_paths(root: &Path, rules: &[BoundaryRule]) -> Vec<PathBuf> {
    let mut paths = rules
        .iter()
        .flat_map(|rule| [&rule.from, &rule.disallow])
        .map(|path| normalize_against(root, path))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn is_library_source(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root).is_ok_and(|relative| {
        relative
            .components()
            .next()
            .is_some_and(|component| component.as_os_str() == "lib")
    })
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

#[cfg(test)]
mod tests;
