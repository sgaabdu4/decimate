use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    DependencyHygieneReport, DependencySection, MisconfiguredDependencyOverride,
    UnlistedPackageDependency, UnusedPackageDependency,
};

/// Config rule for suppressing known intentional dependency overrides.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct IgnoreDependencyOverrideRule {
    /// Override package name or raw override key.
    pub package: String,
    /// Optional source file name, such as `pubspec.yaml`.
    #[serde(default)]
    pub source: Option<String>,
}

pub(super) fn ignore_dependency_overrides_schema() -> Value {
    json!({
        "type": "array",
        "items": {
            "type": "object",
            "additionalProperties": false,
            "required": ["package"],
            "properties": {
                "package": { "type": "string" },
                "source": { "type": ["string", "null"] }
            }
        }
    })
}

pub(crate) fn filter_ignored_dependency_overrides(
    report: &mut DependencyHygieneReport,
    rules: &[IgnoreDependencyOverrideRule],
) {
    if rules.is_empty() {
        return;
    }

    report
        .unused_dependencies
        .retain(|dependency| !unused_override_matches(dependency, rules));
    report
        .misconfigured_dependency_overrides
        .retain(|dependency| !misconfigured_override_matches(dependency, rules));
}

pub(crate) fn filter_ignored_dependencies(
    report: &mut DependencyHygieneReport,
    ignored: &[String],
) {
    if ignored.is_empty() {
        return;
    }

    report
        .unused_dependencies
        .retain(|dependency| !ignored_dependency_matches(&dependency.dependency, ignored));
    report
        .unlisted_dependencies
        .retain(|dependency| !unlisted_dependency_matches(dependency, ignored));
}

fn unlisted_dependency_matches(dependency: &UnlistedPackageDependency, ignored: &[String]) -> bool {
    ignored_dependency_matches(&dependency.dependency, ignored)
}

fn ignored_dependency_matches(dependency: &str, ignored: &[String]) -> bool {
    ignored.iter().any(|ignored| ignored == dependency)
}

fn unused_override_matches(
    dependency: &UnusedPackageDependency,
    rules: &[IgnoreDependencyOverrideRule],
) -> bool {
    dependency.section == DependencySection::DependencyOverrides
        && rules
            .iter()
            .any(|rule| rule.matches(&dependency.dependency, &dependency.pubspec_path))
}

fn misconfigured_override_matches(
    dependency: &MisconfiguredDependencyOverride,
    rules: &[IgnoreDependencyOverrideRule],
) -> bool {
    let package = dependency
        .dependency
        .as_deref()
        .unwrap_or(&dependency.raw_key);
    rules
        .iter()
        .any(|rule| rule.matches(package, &dependency.pubspec_path))
}

impl IgnoreDependencyOverrideRule {
    fn matches(&self, package: &str, path: &Path) -> bool {
        self.package == package
            && self.source.as_deref().is_none_or(|source| {
                path.file_name().and_then(|name| name.to_str()) == Some(source)
            })
    }
}
