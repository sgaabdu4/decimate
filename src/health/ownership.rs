use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use glob::Pattern;

use super::{FileHealthScore, HealthHotspot, RefactoringTarget};
use crate::graph::normalize_path;

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodeownersRule {
    pattern: String,
    owners: Vec<String>,
    source: String,
    section: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OwnerMatch {
    owners: Vec<String>,
    source: String,
    section: Option<String>,
}

pub(super) fn apply_ownership(
    root: &Path,
    scores: &mut [FileHealthScore],
    hotspots: &mut [HealthHotspot],
    targets: &mut [RefactoringTarget],
) {
    let rules = codeowners_rules(root);
    if rules.is_empty() {
        return;
    }
    let matches = scores
        .iter()
        .filter_map(|score| {
            owner_match(root, &score.path, &rules).map(|owner| (score.path.clone(), owner))
        })
        .collect::<BTreeMap<_, _>>();

    for score in scores {
        apply_owner(&matches, &score.path, |owner| {
            score.owners.clone_from(&owner.owners);
            score.owner_source = Some(owner.source.clone());
            score.owner_section.clone_from(&owner.section);
        });
    }
    for hotspot in hotspots {
        apply_owner(&matches, &hotspot.path, |owner| {
            hotspot.owners.clone_from(&owner.owners);
            hotspot.owner_source = Some(owner.source.clone());
            hotspot.owner_section.clone_from(&owner.section);
        });
    }
    for target in targets {
        apply_owner(&matches, &target.path, |owner| {
            target.owners.clone_from(&owner.owners);
            target.owner_source = Some(owner.source.clone());
            target.owner_section.clone_from(&owner.section);
        });
    }
}

fn apply_owner(
    matches: &BTreeMap<PathBuf, OwnerMatch>,
    path: &Path,
    apply: impl FnOnce(&OwnerMatch),
) {
    if let Some(owner) = matches.get(path) {
        apply(owner);
    }
}

fn codeowners_rules(root: &Path) -> Vec<CodeownersRule> {
    codeowners_paths(root)
        .into_iter()
        .find_map(|path| {
            let source = display_path(root, &path);
            fs::read_to_string(&path)
                .ok()
                .map(|text| parse_codeowners(&text, &source))
        })
        .unwrap_or_default()
}

fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn codeowners_paths(root: &Path) -> Vec<PathBuf> {
    [
        ".github/CODEOWNERS",
        ".gitlab/CODEOWNERS",
        "CODEOWNERS",
        "docs/CODEOWNERS",
    ]
    .into_iter()
    .map(|path| root.join(path))
    .filter(|path| path.is_file())
    .collect()
}

fn parse_codeowners(source: &str, source_path: &str) -> Vec<CodeownersRule> {
    let mut section = None;
    source
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return None;
            }
            if let Some(header) = section_header(trimmed) {
                section = Some(header.to_owned());
                return None;
            }
            let mut parts = trimmed.split_whitespace();
            let pattern = parts.next()?.to_owned();
            let owners = parts.map(ToOwned::to_owned).collect::<Vec<_>>();
            (!owners.is_empty()).then(|| CodeownersRule {
                pattern,
                owners,
                source: source_path.to_owned(),
                section: section.clone(),
            })
        })
        .collect()
}

fn section_header(line: &str) -> Option<&str> {
    line.strip_prefix('[')
        .and_then(|rest| rest.split_once(']'))
        .map(|(section, _)| section.trim())
        .filter(|section| !section.is_empty())
}

fn owner_match(root: &Path, path: &Path, rules: &[CodeownersRule]) -> Option<OwnerMatch> {
    let relative = relative_path(root, path);
    rules
        .iter()
        .rfind(|rule| pattern_matches(&rule.pattern, &relative))
        .map(|rule| OwnerMatch {
            owners: rule.owners.clone(),
            source: rule.source.clone(),
            section: rule.section.clone(),
        })
}

fn relative_path(root: &Path, path: &Path) -> String {
    normalize_path(path)
        .strip_prefix(normalize_path(root))
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn pattern_matches(pattern: &str, relative: &str) -> bool {
    let pattern = pattern.trim();
    if pattern.is_empty() {
        return false;
    }
    let pattern = pattern.strip_prefix('/').unwrap_or(pattern);
    if let Some(prefix) = pattern.strip_suffix('/') {
        return relative.starts_with(prefix);
    }
    glob_matches(pattern, relative)
        || (!pattern.contains('/') && glob_matches(&format!("**/{pattern}"), relative))
}

fn glob_matches(pattern: &str, relative: &str) -> bool {
    Pattern::new(pattern).is_ok_and(|pattern| pattern.matches(relative))
}
