use std::collections::BTreeSet;

mod dependency_findings;
mod duplication_findings;
mod feature_flag_findings;
mod format;
mod graph_findings;
mod health_findings;
mod human;
mod next_steps;
mod runtime_coverage;
mod security_findings;
mod security_sarif;
mod suppressions;
mod symbol_findings;
mod types;

use dependency_findings::add_dependency_hygiene_findings;
pub use duplication_findings::{JsonCloneGroup, JsonCloneInstance};
use duplication_findings::{add_duplication_findings, json_clone_groups};
pub use feature_flag_findings::{JsonFeatureFlag, JsonFeatureFlagOccurrence};
use feature_flag_findings::{add_feature_flag_findings, json_feature_flags};
use graph_findings::{
    add_boundary_call_findings, add_boundary_findings, add_cycle_findings, add_dead_code_findings,
    add_part_of_findings, add_policy_findings, add_re_export_cycle_findings,
    add_unresolved_findings, project_part_of_violation_count, project_unresolved_count,
};
pub use health_findings::{
    JsonComplexityContribution, JsonComplexityFinding, JsonEffectiveThresholds,
    JsonFileHealthScore, JsonHealthHotspot, JsonRefactoringTarget, JsonThresholdOverride,
};
use health_findings::{
    add_health_findings, json_complexity, json_file_scores, json_hotspots,
    json_refactoring_targets, json_threshold_overrides,
};
pub use human::render_human_report;
use next_steps::next_steps;
pub use runtime_coverage::{
    JsonRuntimeBlastRadius, JsonRuntimeCoverage, JsonRuntimeCoverageActionable,
    JsonRuntimeCoverageFinding, JsonRuntimeCoverageIntelligence, JsonRuntimeCoverageProvenance,
    JsonRuntimeCoverageSummary, JsonRuntimeCoverageWatermark, JsonRuntimeHotPath,
    JsonRuntimeImportance, json_runtime_coverage,
};
pub use security_findings::{
    JsonAttackSurfaceEntry, JsonSecurityCandidate, JsonSecurityOccurrence,
};
use security_findings::{add_security_findings, json_attack_surface, json_security_candidates};
pub(crate) use security_sarif::render_sarif_report;
use suppressions::filter_suppressed_findings;
use symbol_findings::add_symbol_findings;
pub use types::{
    Finding, FindingAction, FindingEdge, FindingKind, JsonReport, NextStep, ReportCommand,
    ReportSummary, Severity, Verdict,
};

use crate::{
    BoundaryCallViolation, BoundaryCoverageGap, BoundaryViolation, DeadCodeReport, DependencyCycle,
    DependencyHygieneReport, DuplicateCodeReport, FeatureFlagReport, HealthReport, PolicyViolation,
    ReExportCycle, SecurityReport, SymbolReport, scan::ScannedProject,
};

/// Stable JSON schema version for agent consumers.
pub const SCHEMA_VERSION: &str = "decimate.report.v1";
/// Stable trace schema version for read-only evidence commands.
pub const TRACE_SCHEMA_VERSION: &str = "decimate.trace.v1";

/// Analysis values to serialize.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisResults {
    /// Command that produced these values.
    pub command: ReportCommand,
    /// Dead-code reachability report, when run.
    pub dead_code: Option<DeadCodeReport>,
    /// Symbol-level dead-code report, when run.
    pub symbols: Option<SymbolReport>,
    /// Circular dependencies.
    pub cycles: Vec<DependencyCycle>,
    /// Export-only circular dependencies.
    pub re_export_cycles: Vec<ReExportCycle>,
    /// Architecture boundary violations.
    pub boundary_violations: Vec<BoundaryViolation>,
    /// Files not covered by any configured architecture boundary zone.
    pub boundary_coverage: Vec<BoundaryCoverageGap>,
    /// Forbidden calls from configured architecture zones.
    pub boundary_call_violations: Vec<BoundaryCallViolation>,
    /// Declarative policy pack violations.
    pub policy_violations: Vec<PolicyViolation>,
    /// Pub dependency hygiene report, when run.
    pub dependency_hygiene: Option<DependencyHygieneReport>,
    /// Code duplication report, when run.
    pub duplicates: Option<DuplicateCodeReport>,
    /// Code health report, when run.
    pub health: Option<HealthReport>,
    /// Feature flag report, when run.
    pub feature_flags: Option<FeatureFlagReport>,
    /// Security candidate report, when run.
    pub security: Option<SecurityReport>,
    /// Root-normalized files used to scope report findings, when any.
    pub file_scope: Option<Vec<std::path::PathBuf>>,
    /// Whether suppression comments must include justification text.
    pub require_suppression_reasons: bool,
}

/// Build the stable JSON report for a project analysis.
#[must_use]
pub fn build_json_report(project: &ScannedProject, results: &AnalysisResults) -> JsonReport {
    let scope = file_scope(project, results);
    let findings = report_findings(project, results, scope.as_ref());
    let mut summary = report_summary(project, results, &findings, scope.as_ref());
    let clone_groups = scope_clone_groups(
        results
            .duplicates
            .as_ref()
            .map_or_else(Vec::new, |report| json_clone_groups(&project.root, report)),
        scope.as_ref(),
    );
    let complexity = scope_complexity(
        results
            .health
            .as_ref()
            .map_or_else(Vec::new, |report| json_complexity(&project.root, report)),
        scope.as_ref(),
    );
    let file_scores = scope_file_scores(
        results
            .health
            .as_ref()
            .map_or_else(Vec::new, |report| json_file_scores(&project.root, report)),
        scope.as_ref(),
    );
    let hotspots = scope_hotspots(
        results
            .health
            .as_ref()
            .map_or_else(Vec::new, |report| json_hotspots(&project.root, report)),
        scope.as_ref(),
    );
    let refactoring_targets = scope_refactoring_targets(
        results.health.as_ref().map_or_else(Vec::new, |report| {
            json_refactoring_targets(&project.root, report)
        }),
        scope.as_ref(),
    );
    let threshold_overrides = json_threshold_overrides_for(results);
    let runtime_coverage = json_runtime_coverage_for(project, results);
    let feature_flags = scope_feature_flags(
        results
            .feature_flags
            .as_ref()
            .map_or_else(Vec::new, |report| json_feature_flags(&project.root, report)),
        scope.as_ref(),
    );
    let security_candidates = scope_security_candidates(
        results.security.as_ref().map_or_else(Vec::new, |report| {
            json_security_candidates(&project.root, report)
        }),
        scope.as_ref(),
    );
    let attack_surface = scope_attack_surface(
        results.security.as_ref().map_or_else(Vec::new, |report| {
            json_attack_surface(&project.root, report)
        }),
        scope.as_ref(),
    );
    let next_steps = if scope.is_some() {
        Vec::new()
    } else {
        next_steps(&project.root, results)
    };
    if scope.is_some() {
        summary.code_duplications = clone_groups.len();
        summary.file_scores = file_scores.len();
        summary.hotspots = hotspots.len();
        summary.refactoring_targets = refactoring_targets.len();
        summary.feature_flags = feature_flags.len();
        summary.feature_flag_occurrences = feature_flags
            .iter()
            .map(|flag| flag.occurrences.len())
            .sum();
        summary.security_candidates = security_candidates.len();
        summary.security_candidate_occurrences = security_candidates
            .iter()
            .map(|candidate| candidate.occurrences.len())
            .sum();
        summary.attack_surface = attack_surface.len();
    }

    JsonReport {
        schema_version: SCHEMA_VERSION.to_owned(),
        kind: results.command.kind().to_owned(),
        tool: "decimate".to_owned(),
        command: results.command,
        verdict: report_verdict(&findings),
        summary,
        findings,
        clone_groups,
        complexity,
        file_scores,
        hotspots,
        refactoring_targets,
        threshold_overrides,
        feature_flags,
        security_candidates,
        attack_surface,
        runtime_coverage,
        next_steps,
    }
}

fn json_runtime_coverage_for(
    project: &ScannedProject,
    results: &AnalysisResults,
) -> Option<JsonRuntimeCoverage> {
    results.health.as_ref().and_then(|report| {
        report
            .runtime_coverage
            .as_ref()
            .map(|runtime| json_runtime_coverage(&project.root, runtime))
    })
}

fn json_threshold_overrides_for(results: &AnalysisResults) -> Vec<JsonThresholdOverride> {
    results
        .health
        .as_ref()
        .map_or_else(Vec::new, json_threshold_overrides)
}

fn report_verdict(findings: &[Finding]) -> Verdict {
    if findings
        .iter()
        .any(|finding| finding.severity == Severity::Error)
    {
        Verdict::Fail
    } else {
        Verdict::Pass
    }
}

fn report_findings(
    project: &ScannedProject,
    results: &AnalysisResults,
    scope: Option<&BTreeSet<String>>,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    add_unresolved_findings(project, &mut findings);
    add_part_of_findings(project, &mut findings);

    if let Some(dead_code) = &results.dead_code {
        add_dead_code_findings(&project.root, dead_code, &mut findings);
    }
    if let Some(symbols) = &results.symbols {
        add_symbol_findings(&project.root, symbols, &mut findings);
    }

    add_cycle_findings(&project.root, &results.cycles, &mut findings);
    add_re_export_cycle_findings(&project.root, &results.re_export_cycles, &mut findings);
    add_boundary_findings(&project.root, &results.boundary_violations, &mut findings);
    graph_findings::add_boundary_coverage_findings(
        &project.root,
        &results.boundary_coverage,
        &mut findings,
    );
    add_boundary_call_findings(
        &project.root,
        &results.boundary_call_violations,
        &mut findings,
    );
    add_policy_findings(&project.root, &results.policy_violations, &mut findings);
    if let Some(dependency_hygiene) = &results.dependency_hygiene {
        add_dependency_hygiene_findings(&project.root, dependency_hygiene, &mut findings);
    }
    if let Some(duplicates) = &results.duplicates {
        add_duplication_findings(&project.root, duplicates, &mut findings);
    }
    if let Some(health) = &results.health {
        add_health_findings(&project.root, health, &mut findings);
    }
    if let Some(feature_flags) = &results.feature_flags {
        add_feature_flag_findings(&project.root, feature_flags, &mut findings);
    }
    if let Some(security) = &results.security {
        add_security_findings(&project.root, security, &mut findings);
    }

    let mut findings = filter_suppressed_findings(
        &project.root,
        &project.files,
        findings,
        results.require_suppression_reasons,
    )
    .into_iter()
    .filter(|finding| finding_in_scope(finding, scope))
    .collect::<Vec<_>>();
    findings.sort_by(|left, right| {
        (
            &left.rule_id,
            &left.path,
            left.line,
            left.column,
            &left.message,
        )
            .cmp(&(
                &right.rule_id,
                &right.path,
                right.line,
                right.column,
                &right.message,
            ))
    });

    findings
}

fn report_summary(
    project: &ScannedProject,
    results: &AnalysisResults,
    findings: &[Finding],
    scope: Option<&BTreeSet<String>>,
) -> ReportSummary {
    let scoped = results.file_scope.is_some();
    let findings_count = findings.len();
    let health = health_summary_counts(project, results, scope);
    let mut summary = ReportSummary {
        files: project.files.len(),
        edges: project.graph.edge_count(),
        unresolved_dependencies: project_unresolved_count(project),
        part_of_violations: project_part_of_violation_count(project),
        unused_dependencies: results
            .dependency_hygiene
            .as_ref()
            .map_or(0, |report| report.unused_dependencies.len()),
        unused_dev_dependencies: kind_count(findings, FindingKind::UnusedDevDependency),
        test_only_dependencies: kind_count(findings, FindingKind::TestOnlyDependency),
        dependency_overrides: dependency_override_count(findings),
        unused_dependency_overrides: kind_count(findings, FindingKind::UnusedDependencyOverride),
        misconfigured_dependency_overrides: kind_count(
            findings,
            FindingKind::MisconfiguredDependencyOverride,
        ),
        unlisted_dependencies: results
            .dependency_hygiene
            .as_ref()
            .map_or(0, |report| report.unlisted_dependencies.len()),
        dead_files: results
            .dead_code
            .as_ref()
            .map_or(0, |dead_code| dead_code.dead_files.len()),
        unused_exports: kind_count(findings, FindingKind::UnusedExport),
        unused_types: kind_count(findings, FindingKind::UnusedType),
        private_type_leaks: kind_count(findings, FindingKind::PrivateTypeLeak),
        unused_enum_members: kind_count(findings, FindingKind::UnusedEnumMember),
        unused_class_members: kind_count(findings, FindingKind::UnusedClassMember),
        duplicate_exports: kind_count(findings, FindingKind::DuplicateExport),
        code_duplications: results
            .duplicates
            .as_ref()
            .map_or(0, |report| report.clone_groups.len()),
        health_files: health.files,
        functions: health.functions,
        complex_functions: health.complex_functions,
        max_cyclomatic_complexity: health.max_cyclomatic_complexity,
        max_cognitive_complexity: health.max_cognitive_complexity,
        coverage_files: health.coverage_files,
        coverage_gaps: kind_count(findings, FindingKind::CoverageGap),
        crap_functions: kind_count(findings, FindingKind::HighCrapScore),
        max_crap_score: health.max_crap_score,
        file_scores: health.file_scores,
        hotspots: kind_count(findings, FindingKind::HealthHotspot),
        refactoring_targets: kind_count(findings, FindingKind::RefactoringTarget),
        feature_flags: results
            .feature_flags
            .as_ref()
            .map_or(0, |report| report.flags.len()),
        feature_flag_occurrences: results
            .feature_flags
            .as_ref()
            .map_or(0, |report| report.total_occurrences),
        security_candidates: results
            .security
            .as_ref()
            .map_or(0, |report| report.candidates.len()),
        security_candidate_occurrences: results
            .security
            .as_ref()
            .map_or(0, |report| report.total_occurrences),
        attack_surface: results
            .security
            .as_ref()
            .map_or(0, |report| report.attack_surface.len()),
        missing_entry_points: results
            .dead_code
            .as_ref()
            .map_or(0, |dead_code| dead_code.missing_entry_points.len()),
        cycles: results.cycles.len(),
        re_export_cycles: results.re_export_cycles.len(),
        boundary_violations: results.boundary_violations.len(),
        boundary_coverage: kind_count(findings, FindingKind::BoundaryCoverage),
        boundary_call_violations: kind_count(findings, FindingKind::BoundaryCallViolation),
        policy_violations: kind_count(findings, FindingKind::PolicyViolation),
        missing_suppression_reasons: kind_count(findings, FindingKind::MissingSuppressionReason),
        findings: findings_count,
    };

    if scoped {
        if let Some(scope) = scope {
            summary.files = project_file_scope_count(project, scope);
        }
        apply_scoped_counts(&mut summary, findings);
    }

    summary
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HealthSummaryCounts {
    files: usize,
    functions: usize,
    complex_functions: usize,
    max_cyclomatic_complexity: usize,
    max_cognitive_complexity: usize,
    coverage_files: usize,
    max_crap_score: usize,
    file_scores: usize,
}

fn health_summary_counts(
    project: &ScannedProject,
    results: &AnalysisResults,
    scope: Option<&BTreeSet<String>>,
) -> HealthSummaryCounts {
    results.health.as_ref().map_or(
        HealthSummaryCounts {
            files: 0,
            functions: 0,
            complex_functions: 0,
            max_cyclomatic_complexity: 0,
            max_cognitive_complexity: 0,
            coverage_files: 0,
            max_crap_score: 0,
            file_scores: 0,
        },
        |report| HealthSummaryCounts {
            files: report.analyzed_files,
            functions: report.functions,
            complex_functions: report.complexity.len() + report.crap.len(),
            max_cyclomatic_complexity: report.max_cyclomatic_complexity,
            max_cognitive_complexity: report.max_cognitive_complexity,
            coverage_files: report.coverage_files,
            max_crap_score: report.max_crap_score,
            file_scores: health_file_score_count(project, report, scope),
        },
    )
}

fn project_file_scope_count(project: &ScannedProject, scope: &BTreeSet<String>) -> usize {
    project
        .files
        .iter()
        .filter(|file| scope.contains(&format::display_path(&project.root, &file.path)))
        .count()
}

fn apply_scoped_counts(summary: &mut ReportSummary, findings: &[Finding]) {
    summary.unresolved_dependencies = kind_count(findings, FindingKind::UnresolvedDependency);
    summary.part_of_violations = kind_count(findings, FindingKind::PartOfViolation);
    summary.unused_dependencies = dependency_count(findings);
    summary.unused_dev_dependencies = kind_count(findings, FindingKind::UnusedDevDependency);
    summary.test_only_dependencies = kind_count(findings, FindingKind::TestOnlyDependency);
    summary.dependency_overrides = dependency_override_count(findings);
    summary.unused_dependency_overrides =
        kind_count(findings, FindingKind::UnusedDependencyOverride);
    summary.misconfigured_dependency_overrides =
        kind_count(findings, FindingKind::MisconfiguredDependencyOverride);
    summary.unlisted_dependencies = kind_count(findings, FindingKind::UnlistedDependency);
    summary.dead_files = kind_count(findings, FindingKind::DeadFile);
    summary.unused_exports = kind_count(findings, FindingKind::UnusedExport);
    summary.unused_types = kind_count(findings, FindingKind::UnusedType);
    summary.private_type_leaks = kind_count(findings, FindingKind::PrivateTypeLeak);
    summary.unused_enum_members = kind_count(findings, FindingKind::UnusedEnumMember);
    summary.unused_class_members = kind_count(findings, FindingKind::UnusedClassMember);
    summary.duplicate_exports = kind_count(findings, FindingKind::DuplicateExport);
    summary.code_duplications = kind_count(findings, FindingKind::CodeDuplication);
    summary.complex_functions = complexity_count(findings);
    summary.coverage_gaps = kind_count(findings, FindingKind::CoverageGap);
    summary.crap_functions = kind_count(findings, FindingKind::HighCrapScore);
    summary.hotspots = kind_count(findings, FindingKind::HealthHotspot);
    summary.refactoring_targets = kind_count(findings, FindingKind::RefactoringTarget);
    summary.feature_flags = kind_count(findings, FindingKind::FeatureFlag);
    summary.feature_flag_occurrences = summary.feature_flags;
    summary.security_candidates = kind_count(findings, FindingKind::SecurityCandidate);
    summary.security_candidate_occurrences = summary.security_candidates;
    summary.attack_surface = 0;
    summary.missing_entry_points = kind_count(findings, FindingKind::MissingEntryPoint);
    summary.cycles = kind_count(findings, FindingKind::CircularDependency);
    summary.re_export_cycles = kind_count(findings, FindingKind::ReExportCycle);
    summary.boundary_violations = kind_count(findings, FindingKind::BoundaryViolation);
    summary.boundary_coverage = kind_count(findings, FindingKind::BoundaryCoverage);
    summary.boundary_call_violations = kind_count(findings, FindingKind::BoundaryCallViolation);
    summary.policy_violations = kind_count(findings, FindingKind::PolicyViolation);
    summary.missing_suppression_reasons =
        kind_count(findings, FindingKind::MissingSuppressionReason);
}

fn file_scope(project: &ScannedProject, results: &AnalysisResults) -> Option<BTreeSet<String>> {
    results.file_scope.as_ref().map(|paths| {
        paths
            .iter()
            .map(|path| format::display_path(&project.root, path))
            .collect()
    })
}

fn finding_in_scope(finding: &Finding, scope: Option<&BTreeSet<String>>) -> bool {
    scope.is_none_or(|scope| {
        scope.contains(&finding.path)
            || finding.files.iter().any(|file| scope.contains(file))
            || finding
                .edge
                .as_ref()
                .is_some_and(|edge| scope.contains(&edge.from) || scope.contains(&edge.to))
    })
}

fn scope_clone_groups(
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

fn scope_complexity(
    findings: Vec<JsonComplexityFinding>,
    scope: Option<&BTreeSet<String>>,
) -> Vec<JsonComplexityFinding> {
    findings
        .into_iter()
        .filter(|finding| scope.is_none_or(|scope| scope.contains(&finding.path)))
        .collect()
}

fn scope_file_scores(
    scores: Vec<JsonFileHealthScore>,
    scope: Option<&BTreeSet<String>>,
) -> Vec<JsonFileHealthScore> {
    scores
        .into_iter()
        .filter(|score| scope.is_none_or(|scope| scope.contains(&score.path)))
        .collect()
}

fn scope_hotspots(
    hotspots: Vec<JsonHealthHotspot>,
    scope: Option<&BTreeSet<String>>,
) -> Vec<JsonHealthHotspot> {
    hotspots
        .into_iter()
        .filter(|hotspot| scope.is_none_or(|scope| scope.contains(&hotspot.path)))
        .collect()
}

fn scope_refactoring_targets(
    targets: Vec<JsonRefactoringTarget>,
    scope: Option<&BTreeSet<String>>,
) -> Vec<JsonRefactoringTarget> {
    targets
        .into_iter()
        .filter(|target| scope.is_none_or(|scope| scope.contains(&target.path)))
        .collect()
}

fn scope_feature_flags(
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

fn scope_security_candidates(
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

fn scope_attack_surface(
    entries: Vec<JsonAttackSurfaceEntry>,
    scope: Option<&BTreeSet<String>>,
) -> Vec<JsonAttackSurfaceEntry> {
    entries
        .into_iter()
        .filter(|entry| scope.is_none_or(|scope| scope.contains(&entry.path)))
        .collect()
}

fn kind_count(findings: &[Finding], kind: FindingKind) -> usize {
    findings
        .iter()
        .filter(|finding| finding.kind == kind)
        .count()
}

fn health_file_score_count(
    project: &ScannedProject,
    report: &HealthReport,
    scope: Option<&BTreeSet<String>>,
) -> usize {
    report
        .file_scores
        .iter()
        .filter(|score| {
            scope.is_none_or(|scope| {
                scope.contains(&format::display_path(&project.root, &score.path))
            })
        })
        .count()
}

fn dependency_count(findings: &[Finding]) -> usize {
    findings
        .iter()
        .filter(|finding| is_dependency_hygiene_kind(finding.kind))
        .count()
}

fn dependency_override_count(findings: &[Finding]) -> usize {
    kind_count(findings, FindingKind::UnusedDependencyOverride)
        + kind_count(findings, FindingKind::MisconfiguredDependencyOverride)
}

const fn is_dependency_hygiene_kind(kind: FindingKind) -> bool {
    matches!(
        kind,
        FindingKind::UnusedDependency
            | FindingKind::UnusedDevDependency
            | FindingKind::TestOnlyDependency
            | FindingKind::UnusedDependencyOverride
            | FindingKind::MisconfiguredDependencyOverride
    )
}

fn complexity_count(findings: &[Finding]) -> usize {
    findings
        .iter()
        .filter(|finding| {
            matches!(
                finding.kind,
                FindingKind::HighCyclomaticComplexity
                    | FindingKind::HighCognitiveComplexity
                    | FindingKind::HighComplexity
                    | FindingKind::HighCrapScore
            )
        })
        .count()
}

#[cfg(test)]
mod tests;
