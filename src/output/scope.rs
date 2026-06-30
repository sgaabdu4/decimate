use std::collections::BTreeSet;

use super::format;
use super::{
    Finding, JsonAttackSurfaceEntry, JsonCloneGroup, JsonComplexityFinding, JsonFeatureFlag,
    JsonFileHealthScore, JsonHealthHotspot, JsonRefactoringTarget, JsonSecurityCandidate,
};
use crate::{HealthReport, scan::ScannedProject};

pub(super) fn file_scope(
    project: &ScannedProject,
    paths: Option<&Vec<std::path::PathBuf>>,
) -> Option<BTreeSet<String>> {
    paths.map(|paths| {
        paths
            .iter()
            .map(|path| format::display_path(&project.root, path))
            .collect()
    })
}

pub(super) fn finding_in_scope(finding: &Finding, scope: Option<&BTreeSet<String>>) -> bool {
    scope.is_none_or(|scope| {
        scope.contains(&finding.path)
            || finding.files.iter().any(|file| scope.contains(file))
            || finding
                .edge
                .as_ref()
                .is_some_and(|edge| scope.contains(&edge.from) || scope.contains(&edge.to))
    })
}

pub(super) fn scope_clone_groups(
    groups: Vec<JsonCloneGroup>,
    scope: Option<&BTreeSet<String>>,
) -> Vec<JsonCloneGroup> {
    groups
        .into_iter()
        .filter_map(|mut group| {
            if let Some(scope) = scope {
                group
                    .instances
                    .retain(|instance| scope.contains(&instance.path));
            }
            (!group.instances.is_empty()).then_some(group)
        })
        .collect()
}

pub(super) fn scope_complexity(
    findings: Vec<JsonComplexityFinding>,
    scope: Option<&BTreeSet<String>>,
) -> Vec<JsonComplexityFinding> {
    findings
        .into_iter()
        .filter(|finding| scope.is_none_or(|scope| scope.contains(&finding.path)))
        .collect()
}

pub(super) fn scope_file_scores(
    scores: Vec<JsonFileHealthScore>,
    scope: Option<&BTreeSet<String>>,
) -> Vec<JsonFileHealthScore> {
    scores
        .into_iter()
        .filter(|score| scope.is_none_or(|scope| scope.contains(&score.path)))
        .collect()
}

pub(super) fn scope_hotspots(
    hotspots: Vec<JsonHealthHotspot>,
    scope: Option<&BTreeSet<String>>,
) -> Vec<JsonHealthHotspot> {
    hotspots
        .into_iter()
        .filter(|hotspot| scope.is_none_or(|scope| scope.contains(&hotspot.path)))
        .collect()
}

pub(super) fn scope_refactoring_targets(
    targets: Vec<JsonRefactoringTarget>,
    scope: Option<&BTreeSet<String>>,
) -> Vec<JsonRefactoringTarget> {
    targets
        .into_iter()
        .filter(|target| scope.is_none_or(|scope| scope.contains(&target.path)))
        .collect()
}

pub(super) fn scope_feature_flags(
    flags: Vec<JsonFeatureFlag>,
    scope: Option<&BTreeSet<String>>,
) -> Vec<JsonFeatureFlag> {
    flags
        .into_iter()
        .filter_map(|mut flag| {
            if let Some(scope) = scope {
                flag.occurrences
                    .retain(|occurrence| scope.contains(&occurrence.path));
            }
            (!flag.occurrences.is_empty()).then_some(flag)
        })
        .collect()
}

pub(super) fn scope_security_candidates(
    candidates: Vec<JsonSecurityCandidate>,
    scope: Option<&BTreeSet<String>>,
) -> Vec<JsonSecurityCandidate> {
    candidates
        .into_iter()
        .filter_map(|mut candidate| {
            if let Some(scope) = scope {
                candidate
                    .occurrences
                    .retain(|occurrence| scope.contains(&occurrence.path));
            }
            (!candidate.occurrences.is_empty()).then_some(candidate)
        })
        .collect()
}

pub(super) fn scope_attack_surface(
    entries: Vec<JsonAttackSurfaceEntry>,
    scope: Option<&BTreeSet<String>>,
) -> Vec<JsonAttackSurfaceEntry> {
    entries
        .into_iter()
        .filter(|entry| scope.is_none_or(|scope| scope.contains(&entry.path)))
        .collect()
}

pub(super) fn project_file_scope_count(
    project: &ScannedProject,
    scope: &BTreeSet<String>,
) -> usize {
    project
        .files
        .iter()
        .filter(|file| scope.contains(&format::display_path(&project.root, &file.path)))
        .count()
}

pub(super) fn health_file_score_count(
    project: &ScannedProject,
    scores: &[crate::FileHealthScore],
    scope: Option<&BTreeSet<String>>,
) -> usize {
    scores
        .iter()
        .filter(|score| {
            scope.is_none_or(|scope| {
                scope.contains(&format::display_path(&project.root, &score.path))
            })
        })
        .count()
}

pub(super) fn scoped_quality_score(
    project: &ScannedProject,
    report: &HealthReport,
    scope: Option<&BTreeSet<String>>,
) -> usize {
    let Some(scope) = scope else {
        return report.quality_score;
    };
    if report.file_scores.is_empty() {
        return report.quality_score;
    }
    let mut total = 0usize;
    let mut weight = 0usize;
    for score in &report.file_scores {
        if scope.contains(&format::display_path(&project.root, &score.path)) {
            let file_weight = score.functions.max(1);
            total = total.saturating_add(score.score.saturating_mul(file_weight));
            weight = weight.saturating_add(file_weight);
        }
    }
    if weight == 0 {
        return 0;
    }
    total.saturating_add(weight / 2) / weight
}
