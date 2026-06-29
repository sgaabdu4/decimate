use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_yaml_ng::{Mapping, Value};
use thiserror::Error;

use crate::dependency_scripts::package_used_in_tooling;
use crate::graph::normalize_path;
use crate::{DependencyKind, Location, scan::ScannedProject};

mod discovery;
mod overrides;
mod pubspec_entry;
use overrides::misconfigured_dependency_overrides;
pub use overrides::{DependencyOverrideMisconfigReason, MisconfiguredDependencyOverride};

/// Dart pub dependency hygiene findings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyHygieneReport {
    /// Declared dependencies not imported by Dart files in the declaring package.
    pub unused_dependencies: Vec<UnusedPackageDependency>,
    /// Dependency override entries whose key or value cannot be honored by Pub.
    pub misconfigured_dependency_overrides: Vec<MisconfiguredDependencyOverride>,
    /// Imported packages absent from the importing package's pubspec.
    pub unlisted_dependencies: Vec<UnlistedPackageDependency>,
}

/// A declared pub dependency that has no Dart import/export usage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnusedPackageDependency {
    /// Package declaring the dependency.
    pub package: String,
    /// Pubspec path that declares the dependency.
    pub pubspec_path: PathBuf,
    /// Dependency package name.
    pub dependency: String,
    /// Pubspec dependency section.
    pub section: DependencySection,
    /// Dependency hygiene issue class.
    pub issue: DependencyIssue,
    /// Best-effort location of the dependency key in `pubspec.yaml`.
    pub location: Location,
    /// Whether Decimate can suggest removal from current evidence alone.
    pub safe_to_delete: bool,
}

/// A package import/export missing from the declaring package's pubspec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnlistedPackageDependency {
    /// Package containing the import/export.
    pub package: String,
    /// Pubspec path for the package containing the import/export.
    pub pubspec_path: PathBuf,
    /// Dart file containing the import/export.
    pub path: PathBuf,
    /// Imported package name.
    pub dependency: String,
    /// Import/export URI.
    pub specifier: String,
    /// Whether the dependency came from an import or export directive.
    pub kind: DependencyKind,
    /// Pubspec section where the dependency is declared, if present in a wrong section.
    pub declared_section: Option<DependencySection>,
    /// Location of the import/export directive.
    pub location: Location,
}

/// A dependency declaration found in a local pubspec.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclaredPackageDependency {
    /// Package declaring the dependency.
    pub package: String,
    /// Pubspec path that declares the dependency.
    pub pubspec_path: PathBuf,
    /// Dependency package name.
    pub dependency: String,
    /// Pubspec dependency section.
    pub section: DependencySection,
    /// Best-effort location of the dependency key in `pubspec.yaml`.
    pub location: Location,
}

/// A local pub package discovered under the scan root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalPubPackage {
    /// Pub package name.
    pub name: String,
    /// Package root directory.
    pub root: PathBuf,
    /// Pubspec path.
    pub pubspec_path: PathBuf,
}

/// Pub dependency section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencySection {
    /// Runtime dependencies.
    Dependencies,
    /// Development-only dependencies.
    DevDependencies,
    /// Temporary dependency overrides.
    DependencyOverrides,
}

/// Dependency hygiene issue class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DependencyIssue {
    /// Runtime dependency has no Dart import/export usage.
    UnusedRuntimeDependency,
    /// Development dependency has no Dart import/export usage.
    UnusedDevDependency,
    /// Runtime dependency is imported only from development-only files.
    TestOnlyDependency,
    /// Dependency override is absent from the resolved lockfile package graph.
    UnusedDependencyOverride,
}

impl DependencySection {
    const fn as_pubspec_key(self) -> &'static str {
        match self {
            Self::Dependencies => "dependencies",
            Self::DevDependencies => "dev_dependencies",
            Self::DependencyOverrides => "dependency_overrides",
        }
    }
}

/// Errors returned while analyzing pub dependency hygiene.
#[derive(Debug, Error)]
pub enum DependencyHygieneError {
    /// A directory could not be read.
    #[error("failed to read directory {path}: {source}")]
    ReadDir {
        /// Directory path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A directory entry could not be read.
    #[error("failed to read directory entry under {path}: {source}")]
    ReadDirEntry {
        /// Directory being scanned.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A file type could not be read.
    #[error("failed to read file type for {path}: {source}")]
    FileType {
        /// Entry path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A pubspec could not be read.
    #[error("failed to read pubspec {path}: {source}")]
    ReadPubspec {
        /// Pubspec path.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A pubspec could not be parsed.
    #[error("failed to parse pubspec {path}: {source}")]
    ParsePubspec {
        /// Pubspec path.
        path: PathBuf,
        /// Underlying YAML parse error.
        source: serde_yaml_ng::Error,
    },
}

/// Analyze Dart `package:` imports against pubspec dependency declarations.
///
/// # Errors
///
/// Returns [`DependencyHygieneError`] if pubspec discovery or parsing fails.
pub fn analyze_dependency_hygiene(
    project: &ScannedProject,
) -> Result<DependencyHygieneReport, DependencyHygieneError> {
    let packages = discover_packages(&project.root)?;
    let mut used_by_package = BTreeMap::<PathBuf, BTreeMap<String, DependencyUsage>>::new();
    let mut unlisted_by_identity = BTreeMap::<(PathBuf, String), UnlistedPackageDependency>::new();

    for file in &project.files {
        let Some(owner) = owning_package(&packages, &file.path) else {
            continue;
        };

        for (specifier, kind, location) in file
            .imports
            .iter()
            .map(|import| (&import.uri, DependencyKind::Import, import.location))
            .chain(
                file.exports
                    .iter()
                    .map(|export| (&export.uri, DependencyKind::Export, export.location)),
            )
        {
            let Some(dependency) = package_name(specifier) else {
                continue;
            };
            if dependency == owner.name {
                continue;
            }

            let usage = used_by_package
                .entry(owner.root.clone())
                .or_default()
                .entry(dependency.clone())
                .or_default();
            usage.record(&owner.root, &file.path);

            if !owner.declares_dependency_for_path(&dependency, &file.path) {
                unlisted_by_identity
                    .entry((owner.root.clone(), dependency.clone()))
                    .or_insert_with(|| UnlistedPackageDependency {
                        package: owner.name.clone(),
                        pubspec_path: owner.pubspec_path.clone(),
                        path: file.path.clone(),
                        dependency,
                        specifier: specifier.clone(),
                        kind,
                        declared_section: owner.declared_section(specifier),
                        location,
                    });
            }
        }
    }

    let mut unused_dependencies = Vec::new();
    for package in &packages {
        let used = used_by_package.get(&package.root);
        for dependency in &package.dependencies {
            if dependency.name == package.name {
                continue;
            }
            let mut usage = used
                .and_then(|used| used.get(&dependency.name))
                .copied()
                .unwrap_or_default();
            if package_used_in_tooling(&package.root, &dependency.name) {
                usage.record_tooling();
            }
            let usage = usage.any().then_some(usage);
            if let Some(issue) =
                dependency_issue(dependency, usage.as_ref(), package.locked_packages.as_ref())
            {
                unused_dependencies.push(UnusedPackageDependency {
                    package: package.name.clone(),
                    pubspec_path: package.pubspec_path.clone(),
                    dependency: dependency.name.clone(),
                    section: dependency.section,
                    issue,
                    location: dependency.location,
                    safe_to_delete: dependency.safe_to_delete
                        && matches!(
                            issue,
                            DependencyIssue::UnusedRuntimeDependency
                                | DependencyIssue::UnusedDevDependency
                        ),
                });
            }
        }
    }

    unused_dependencies.sort_by(|left, right| {
        (
            &left.package,
            left.section.as_pubspec_key(),
            &left.dependency,
        )
            .cmp(&(
                &right.package,
                right.section.as_pubspec_key(),
                &right.dependency,
            ))
    });

    Ok(DependencyHygieneReport {
        unused_dependencies,
        misconfigured_dependency_overrides: packages
            .into_iter()
            .flat_map(|package| package.misconfigured_dependency_overrides)
            .collect(),
        unlisted_dependencies: unlisted_by_identity.into_values().collect(),
    })
}

/// Find local pubspec declarations for one dependency package.
///
/// # Errors
///
/// Returns [`DependencyHygieneError`] if pubspec discovery or parsing fails.
pub fn declared_package_dependencies(
    root: &Path,
    dependency: &str,
) -> Result<Vec<DeclaredPackageDependency>, DependencyHygieneError> {
    let packages = discover_packages(root)?;
    let mut declarations = Vec::new();
    for package in packages {
        for declared in package
            .dependencies
            .into_iter()
            .filter(|declared| declared.name == dependency)
        {
            declarations.push(DeclaredPackageDependency {
                package: package.name.clone(),
                pubspec_path: package.pubspec_path.clone(),
                dependency: declared.name,
                section: declared.section,
                location: declared.location,
            });
        }
    }

    declarations.sort_by(|left, right| {
        (
            &left.package,
            &left.pubspec_path,
            left.section.as_pubspec_key(),
            &left.dependency,
        )
            .cmp(&(
                &right.package,
                &right.pubspec_path,
                right.section.as_pubspec_key(),
                &right.dependency,
            ))
    });
    Ok(declarations)
}

/// List local pub packages under a root.
///
/// # Errors
///
/// Returns [`DependencyHygieneError`] if pubspec discovery or parsing fails.
pub fn local_pub_packages(root: &Path) -> Result<Vec<LocalPubPackage>, DependencyHygieneError> {
    discover_packages(root).map(|packages| {
        packages
            .into_iter()
            .map(|package| LocalPubPackage {
                name: package.name,
                root: package.root,
                pubspec_path: package.pubspec_path,
            })
            .collect()
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PubPackage {
    name: String,
    root: PathBuf,
    pubspec_path: PathBuf,
    dependencies: Vec<DeclaredDependency>,
    misconfigured_dependency_overrides: Vec<MisconfiguredDependencyOverride>,
    locked_packages: Option<BTreeSet<String>>,
}

impl PubPackage {
    fn declares_dependency_for_path(&self, dependency: &str, path: &Path) -> bool {
        let allow_dev_dependencies = allows_dev_dependency(&self.root, path);
        self.dependencies.iter().any(|declared| {
            declared.name == dependency
                && match declared.section {
                    DependencySection::Dependencies => true,
                    DependencySection::DevDependencies => allow_dev_dependencies,
                    DependencySection::DependencyOverrides => false,
                }
        })
    }

    fn declared_section(&self, specifier: &str) -> Option<DependencySection> {
        let dependency = package_name(specifier)?;
        self.dependencies
            .iter()
            .find(|declared| declared.name == dependency)
            .map(|declared| declared.section)
    }
}

fn allows_dev_dependency(package_root: &Path, path: &Path) -> bool {
    let relative = path.strip_prefix(package_root).unwrap_or(path);
    !matches!(
        relative.components().next(),
        Some(std::path::Component::Normal(name)) if name == "lib" || name == "bin"
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeclaredDependency {
    name: String,
    section: DependencySection,
    location: Location,
    safe_to_delete: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct DependencyUsage {
    production: bool,
    development: bool,
}

impl DependencyUsage {
    fn record(&mut self, package_root: &Path, path: &Path) {
        if allows_dev_dependency(package_root, path) {
            self.development = true;
        } else {
            self.production = true;
        }
    }

    fn record_tooling(&mut self) {
        self.development = true;
    }

    const fn any(self) -> bool {
        self.production || self.development
    }
}

fn dependency_issue(
    dependency: &DeclaredDependency,
    usage: Option<&DependencyUsage>,
    locked_packages: Option<&BTreeSet<String>>,
) -> Option<DependencyIssue> {
    match dependency.section {
        DependencySection::Dependencies => match usage {
            Some(usage) if usage.production => None,
            Some(usage) if usage.development => Some(DependencyIssue::TestOnlyDependency),
            Some(_) | None => Some(DependencyIssue::UnusedRuntimeDependency),
        },
        DependencySection::DevDependencies => usage
            .is_none()
            .then_some(DependencyIssue::UnusedDevDependency),
        DependencySection::DependencyOverrides => locked_packages.and_then(|locked| {
            (!locked.contains(&dependency.name))
                .then_some(DependencyIssue::UnusedDependencyOverride)
        }),
    }
}

fn discover_packages(root: &Path) -> Result<Vec<PubPackage>, DependencyHygieneError> {
    let mut pubspecs = Vec::new();
    discover_pubspecs(root, &mut pubspecs)?;
    let mut packages = pubspecs
        .into_iter()
        .filter_map(|path| read_package(&path).transpose())
        .collect::<Result<Vec<_>, _>>()?;
    packages.sort_by(|left, right| left.root.cmp(&right.root));
    Ok(packages)
}

fn discover_pubspecs(
    dir: &Path,
    pubspecs: &mut Vec<PathBuf>,
) -> Result<(), DependencyHygieneError> {
    let entries = fs::read_dir(dir).map_err(|source| DependencyHygieneError::ReadDir {
        path: dir.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| DependencyHygieneError::ReadDirEntry {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| DependencyHygieneError::FileType {
                path: path.clone(),
                source,
            })?;

        if file_type.is_dir() {
            if discovery::should_skip_dir(&path) {
                continue;
            }
            discover_pubspecs(&path, pubspecs)?;
        } else if file_type.is_file() && path.file_name().is_some_and(|name| name == "pubspec.yaml")
        {
            pubspecs.push(normalize_path(&path));
        }
    }

    Ok(())
}

fn read_package(path: &Path) -> Result<Option<PubPackage>, DependencyHygieneError> {
    let source =
        fs::read_to_string(path).map_err(|source| DependencyHygieneError::ReadPubspec {
            path: path.to_path_buf(),
            source,
        })?;
    let value = serde_yaml_ng::from_str::<Value>(&source).map_err(|source| {
        DependencyHygieneError::ParsePubspec {
            path: path.to_path_buf(),
            source,
        }
    })?;

    let Some(name) = string_field(&value, "name") else {
        return Ok(None);
    };

    let root = path
        .parent()
        .map_or_else(|| PathBuf::from("."), normalize_path);

    Ok(Some(PubPackage {
        name: name.to_owned(),
        root,
        pubspec_path: path.to_path_buf(),
        dependencies: declared_dependencies(&value, &source),
        misconfigured_dependency_overrides: misconfigured_dependency_overrides(
            &value, &source, name, path,
        ),
        locked_packages: read_locked_packages(path),
    }))
}

fn read_locked_packages(pubspec_path: &Path) -> Option<BTreeSet<String>> {
    let lock_path = pubspec_path.with_file_name("pubspec.lock");
    let source = fs::read_to_string(lock_path).ok()?;
    let value = serde_yaml_ng::from_str::<Value>(&source).ok()?;
    let packages = mapping_field(&value, "packages")?;
    Some(
        packages
            .keys()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect(),
    )
}

fn declared_dependencies(value: &Value, source: &str) -> Vec<DeclaredDependency> {
    let has_declared_sections = [
        DependencySection::Dependencies,
        DependencySection::DevDependencies,
        DependencySection::DependencyOverrides,
    ]
    .into_iter()
    .any(|section| mapping_field(value, section.as_pubspec_key()).is_some());
    let mut dependencies = [
        DependencySection::Dependencies,
        DependencySection::DevDependencies,
        DependencySection::DependencyOverrides,
    ]
    .into_iter()
    .filter_map(|section| {
        mapping_field(value, section.as_pubspec_key()).map(|mapping| (section, mapping))
    })
    .flat_map(|(section, mapping)| {
        mapping.iter().filter_map(move |(name, value)| {
            let name = name.as_str()?.to_owned();
            if section == DependencySection::DependencyOverrides
                && (!valid_dart_package_name(&name) || value.is_null())
            {
                return None;
            }
            let location = dependency_location(source, section, &name);
            Some(DeclaredDependency {
                safe_to_delete: pubspec_entry::is_simple_scalar_dependency(
                    source, section, &name, location,
                ),
                location,
                name,
                section,
            })
        })
    })
    .collect::<Vec<_>>();

    if dependencies.is_empty() && !has_declared_sections {
        dependencies = declared_dependencies_from_source(source);
    }

    dependencies
}

fn declared_dependencies_from_source(source: &str) -> Vec<DeclaredDependency> {
    let mut dependencies = Vec::new();
    let mut current_section = None;

    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();

        if indent == 0 {
            current_section = match trimmed.trim_end() {
                "dependencies:" => Some(DependencySection::Dependencies),
                "dev_dependencies:" => Some(DependencySection::DevDependencies),
                "dependency_overrides:" => Some(DependencySection::DependencyOverrides),
                _ => None,
            };
            continue;
        }

        let Some(section) = current_section else {
            continue;
        };
        if indent != 2 {
            continue;
        }
        let Some((name, _)) = trimmed.split_once(':') else {
            continue;
        };
        if name.trim().is_empty() {
            continue;
        }
        let location = Location {
            line: index + 1,
            column: indent,
        };
        dependencies.push(DeclaredDependency {
            name: name.trim().to_owned(),
            section,
            location,
            safe_to_delete: pubspec_entry::is_simple_scalar_dependency(
                source,
                section,
                name.trim(),
                location,
            ),
        });
    }

    dependencies
}

fn owning_package<'package>(
    packages: &'package [PubPackage],
    path: &Path,
) -> Option<&'package PubPackage> {
    packages
        .iter()
        .filter(|package| path.starts_with(&package.root))
        .max_by_key(|package| package.root.components().count())
}

fn package_name(specifier: &str) -> Option<String> {
    specifier
        .strip_prefix("package:")
        .and_then(|rest| rest.split_once('/'))
        .map(|(package, _)| package.to_owned())
}

pub(crate) fn valid_dart_package_name(name: &str) -> bool {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    (first.is_ascii_lowercase() || first == '_')
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

fn mapping_field<'value>(value: &'value Value, field: &str) -> Option<&'value Mapping> {
    value
        .as_mapping()?
        .get(Value::String(field.to_owned()))?
        .as_mapping()
}

fn string_field<'value>(value: &'value Value, field: &str) -> Option<&'value str> {
    value
        .as_mapping()?
        .get(Value::String(field.to_owned()))?
        .as_str()
}

fn dependency_location(source: &str, section: DependencySection, dependency: &str) -> Location {
    let mut in_section = false;
    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if indent == 0 {
            in_section = trimmed.trim_end() == format!("{}:", section.as_pubspec_key());
            continue;
        }
        if in_section && trimmed.starts_with(&format!("{dependency}:")) {
            return Location {
                line: index + 1,
                column: indent,
            };
        }
    }

    Location { line: 1, column: 0 }
}

#[cfg(test)]
mod tests;
