use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_yaml_ng::{Mapping, Value};

use crate::graph::{GraphError, normalize_path};

#[derive(Debug, Clone, Default)]
pub(crate) struct PackageMap {
    by_name: BTreeMap<String, PackageRoot>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PackageRoot {
    root: PathBuf,
    package_path: PathBuf,
    local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PackageResolution {
    pub(crate) path: PathBuf,
    pub(crate) local: bool,
}

impl PackageMap {
    pub(crate) fn discover(root: &Path) -> Result<Self, GraphError> {
        if let Some(config) = PackageConfig::read(root)? {
            return Ok(Self::from_package_config(root, &config));
        }

        let mut packages = Self::default();
        let mut visited = BTreeSet::new();
        packages.discover_pubspec(root, &mut visited)?;
        packages.discover_nested_pubspecs(root, &mut visited)?;
        Ok(packages)
    }

    pub(crate) fn names(&self) -> Vec<&str> {
        self.by_name.keys().map(String::as_str).collect()
    }

    pub(crate) fn local_roots(&self) -> BTreeSet<PathBuf> {
        self.by_name
            .values()
            .filter(|package| package.local)
            .map(|package| package.root.clone())
            .collect()
    }

    pub(crate) fn resolve(&self, package: &str, path: &str) -> Option<PackageResolution> {
        self.by_name.get(package).map(|root| PackageResolution {
            path: normalize_path(&root.package_path.join(path)),
            local: root.local,
        })
    }

    fn from_package_config(root: &Path, config: &PackageConfig) -> Self {
        let config_dir = root.join(".dart_tool");
        let by_name = config
            .packages
            .iter()
            .filter_map(|package| {
                let root_uri = package.root_uri.as_deref()?;
                let package_uri = package.package_uri.as_deref().unwrap_or("lib/");
                let package_root = resolve_config_uri(&config_dir, root_uri)?;
                let package_path = resolve_package_uri(&package_root, package_uri)?;
                let local = is_local_package_config_root(root, root_uri, &package_root);
                Some((
                    package.name.clone(),
                    PackageRoot {
                        root: package_root,
                        package_path,
                        local,
                    },
                ))
            })
            .collect();

        Self { by_name }
    }

    fn discover_pubspec(
        &mut self,
        package_root: &Path,
        visited: &mut BTreeSet<PathBuf>,
    ) -> Result<(), GraphError> {
        let package_root = normalize_path(package_root);
        if !visited.insert(package_root.clone()) {
            return Ok(());
        }

        let Some(pubspec) = Pubspec::read(&package_root)? else {
            return Ok(());
        };

        if let Some(name) = pubspec.name {
            self.insert(name, package_root.clone(), PathBuf::from("lib"));
        }

        for dependency in pubspec.path_dependencies {
            let dependency_root = normalize_path(&package_root.join(dependency.path));
            self.insert(
                dependency.name,
                dependency_root.clone(),
                PathBuf::from("lib"),
            );
            self.discover_pubspec(&dependency_root, visited)?;
        }

        for member in pubspec.workspace_members {
            for member_root in expand_workspace_member(&package_root, &member)? {
                self.discover_pubspec(&member_root, visited)?;
            }
        }

        Ok(())
    }

    fn discover_nested_pubspecs(
        &mut self,
        dir: &Path,
        visited: &mut BTreeSet<PathBuf>,
    ) -> Result<(), GraphError> {
        let entries = fs::read_dir(dir).map_err(|source| GraphError::ReadDir {
            path: dir.to_path_buf(),
            source,
        })?;

        for entry in entries {
            let entry = entry.map_err(|source| GraphError::ReadDirEntry {
                path: dir.to_path_buf(),
                source,
            })?;
            let path = entry.path();
            let file_type = entry.file_type().map_err(|source| GraphError::FileType {
                path: path.clone(),
                source,
            })?;

            if file_type.is_dir() {
                if should_skip_dir(&path) {
                    continue;
                }
                self.discover_nested_pubspecs(&path, visited)?;
            } else if file_type.is_file()
                && path.file_name().is_some_and(|name| name == "pubspec.yaml")
                && let Some(package_root) = path.parent()
            {
                self.discover_pubspec(package_root, visited)?;
            }
        }

        Ok(())
    }

    fn insert(&mut self, name: String, root: PathBuf, package_uri: PathBuf) {
        self.by_name.insert(
            name,
            PackageRoot {
                package_path: normalize_path(&root.join(package_uri)),
                root,
                local: true,
            },
        );
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct PackageConfig {
    #[serde(default)]
    packages: Vec<PackageConfigPackage>,
}

impl PackageConfig {
    fn read(root: &Path) -> Result<Option<Self>, GraphError> {
        let path = root.join(".dart_tool/package_config.json");
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(GraphError::ReadPackageConfig { path, source }),
        };

        serde_json::from_str(&source)
            .map(Some)
            .map_err(|source| GraphError::ParsePackageConfig { path, source })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageConfigPackage {
    name: String,
    root_uri: Option<String>,
    package_uri: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PathDependency {
    name: String,
    path: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Pubspec {
    name: Option<String>,
    workspace_members: Vec<String>,
    path_dependencies: Vec<PathDependency>,
}

impl Pubspec {
    fn read(package_root: &Path) -> Result<Option<Self>, GraphError> {
        let path = package_root.join("pubspec.yaml");
        let source = match fs::read_to_string(&path) {
            Ok(source) => source,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(GraphError::ReadPubspec { path, source }),
        };
        let value = serde_yaml_ng::from_str::<Value>(&source).map_err(|source| {
            GraphError::ParsePubspec {
                path: path.clone(),
                source,
            }
        })?;

        Ok(Some(Self::from_value(&value)))
    }

    fn from_value(value: &Value) -> Self {
        Self {
            name: string_field(value, "name").map(str::to_owned),
            workspace_members: string_sequence_field(value, "workspace"),
            path_dependencies: path_dependencies(value),
        }
    }
}

fn resolve_config_uri(config_dir: &Path, uri: &str) -> Option<PathBuf> {
    if let Some(path) = uri.strip_prefix("file://") {
        let path = percent_decode(path)?;
        return Some(normalize_path(Path::new(&path)));
    }
    if uri.contains(':') {
        return None;
    }

    let uri = percent_decode(uri)?;
    Some(normalize_path(&config_dir.join(uri)))
}

fn resolve_package_uri(package_root: &Path, uri: &str) -> Option<PathBuf> {
    if let Some(path) = uri.strip_prefix("file://") {
        let path = percent_decode(path)?;
        return Some(normalize_path(Path::new(&path)));
    }
    if uri.contains(':') {
        return None;
    }

    let uri = percent_decode(uri)?;
    Some(normalize_path(&package_root.join(uri)))
}

fn percent_decode(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' {
            let high = hex_value(*bytes.get(index + 1)?)?;
            let low = hex_value(*bytes.get(index + 2)?)?;
            decoded.push(high << 4 | low);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }

    String::from_utf8(decoded).ok()
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn is_local_package_config_root(root: &Path, root_uri: &str, package_root: &Path) -> bool {
    (is_relative_uri(root_uri) || package_root.starts_with(root))
        && !is_pub_cache_path(package_root)
}

fn is_relative_uri(uri: &str) -> bool {
    !uri.contains(':')
}

fn is_pub_cache_path(path: &Path) -> bool {
    path.components()
        .any(|component| component.as_os_str() == ".pub-cache")
}

fn path_dependencies(value: &Value) -> Vec<PathDependency> {
    ["dependencies", "dev_dependencies", "dependency_overrides"]
        .into_iter()
        .filter_map(|section| mapping_field(value, section))
        .flat_map(|mapping| {
            mapping.iter().filter_map(|(name, dependency)| {
                let name = name.as_str()?.to_owned();
                let path = string_field(dependency, "path")?;
                Some(PathDependency {
                    name,
                    path: PathBuf::from(path),
                })
            })
        })
        .collect()
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

fn string_sequence_field(value: &Value, field: &str) -> Vec<String> {
    value
        .as_mapping()
        .and_then(|mapping| mapping.get(Value::String(field.to_owned())))
        .and_then(Value::as_sequence)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn expand_workspace_member(root: &Path, member: &str) -> Result<Vec<PathBuf>, GraphError> {
    if !contains_glob_pattern(member) {
        return Ok(vec![normalize_path(&root.join(member))]);
    }

    let pattern_path = root.join(member).join("pubspec.yaml");
    let pattern = pattern_path.to_string_lossy().into_owned();
    let entries = glob::glob(&pattern).map_err(|source| GraphError::WorkspacePattern {
        pattern: pattern.clone(),
        source,
    })?;

    entries
        .map(|entry| {
            let pubspec_path = entry.map_err(|source| GraphError::WorkspaceGlob {
                pattern: pattern.clone(),
                source,
            })?;
            Ok(pubspec_path
                .parent()
                .map_or_else(|| normalize_path(root), normalize_path))
        })
        .collect()
}

fn contains_glob_pattern(value: &str) -> bool {
    value.contains('*') || value.contains('?') || value.contains('[')
}

fn should_skip_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".dart_tool" | ".git" | ".idea" | ".pub-cache" | "build" | "target")
    )
}
