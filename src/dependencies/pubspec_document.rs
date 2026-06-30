use std::path::Path;

use crate::Location;

use super::{DeclaredDependency, DependencySection, pubspec_entry};

pub(super) fn declared_dependencies_from_source(
    source: &str,
    source_path: &Path,
) -> Vec<DeclaredDependency> {
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
        let name = name.trim();
        if name.is_empty() {
            continue;
        }
        let location = Location {
            line: index + 1,
            column: indent,
        };
        dependencies.push(DeclaredDependency {
            name: name.to_owned(),
            source_path: source_path.to_path_buf(),
            section,
            location,
            safe_to_delete: pubspec_entry::is_simple_scalar_dependency(
                source, section, name, location,
            ),
        });
    }

    dependencies
}

pub(super) fn dependency_location(
    source: &str,
    section: DependencySection,
    dependency: &str,
) -> Location {
    let mut in_section = false;
    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if indent == 0 {
            in_section = trimmed.trim_end() == format!("{}:", section.as_pubspec_key());
            continue;
        }
        if in_section
            && indent == 2
            && trimmed
                .split_once(':')
                .is_some_and(|(key, _)| key.trim() == dependency)
        {
            return Location {
                line: index + 1,
                column: indent,
            };
        }
    }

    Location { line: 1, column: 0 }
}
