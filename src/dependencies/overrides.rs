use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_yaml_ng::Value;

use crate::Location;

use super::{DependencySection, dependency_location, mapping_field, valid_dart_package_name};

/// A dependency override declaration that Pub cannot honor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MisconfiguredDependencyOverride {
    /// Package declaring the override.
    pub package: String,
    /// Pubspec path that declares the override.
    pub pubspec_path: PathBuf,
    /// Raw override key from `dependency_overrides`.
    pub raw_key: String,
    /// Parsed package name when the key is syntactically valid.
    pub dependency: Option<String>,
    /// Reason the override is misconfigured.
    pub reason: DependencyOverrideMisconfigReason,
    /// Best-effort location of the override key in `pubspec.yaml`.
    pub location: Location,
}

/// Dependency override misconfiguration reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DependencyOverrideMisconfigReason {
    /// Override key is not a valid Dart package name.
    UnparsableKey,
    /// Override value is empty.
    EmptyValue,
}

pub(super) fn misconfigured_dependency_overrides(
    value: &Value,
    source: &str,
    package: &str,
    pubspec_path: &Path,
) -> Vec<MisconfiguredDependencyOverride> {
    let Some(overrides) = mapping_field(value, "dependency_overrides") else {
        return Vec::new();
    };
    overrides
        .iter()
        .filter_map(|(name, value)| {
            let raw_key = name
                .as_str()
                .map_or_else(|| format!("{name:?}"), ToOwned::to_owned);
            let reason = if !name.as_str().is_some_and(valid_dart_package_name) {
                DependencyOverrideMisconfigReason::UnparsableKey
            } else if value.is_null() {
                DependencyOverrideMisconfigReason::EmptyValue
            } else {
                return None;
            };
            Some(MisconfiguredDependencyOverride {
                package: package.to_owned(),
                pubspec_path: pubspec_path.to_path_buf(),
                dependency: name.as_str().map(ToOwned::to_owned),
                location: dependency_location(
                    source,
                    DependencySection::DependencyOverrides,
                    &raw_key,
                ),
                raw_key,
                reason,
            })
        })
        .collect()
}
