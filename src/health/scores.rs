use std::collections::BTreeMap;

use crate::Location;

use super::coverage::CoverageMap;
use super::{
    ComplexityFinding, CrapFinding, FileCoverageStatus, FileHealthScore, HealthHotspot,
    HealthOptions, RefactoringTarget,
};
use crate::health::FunctionMetrics;

pub(super) fn file_health_scores(
    functions: &[FunctionMetrics],
    complexity: &[ComplexityFinding],
    crap: &[CrapFinding],
    coverage: Option<&CoverageMap>,
    options: &HealthOptions,
) -> Vec<FileHealthScore> {
    let mut builders = BTreeMap::new();
    for function in functions {
        builders
            .entry(function.path.clone())
            .or_insert_with(|| FileScoreBuilder::new(function.location))
            .push_function(function);
    }
    for finding in complexity {
        if let Some(builder) = builders.get_mut(&finding.path) {
            builder.complex_functions += 1;
        }
    }
    for finding in crap {
        if let Some(builder) = builders.get_mut(&finding.path) {
            builder.complex_functions += 1;
            builder.max_crap_score = builder.max_crap_score.max(finding.crap_score);
        }
    }

    let mut scores = builders
        .into_iter()
        .map(|(path, builder)| builder.finish(path, coverage, options))
        .collect::<Vec<_>>();
    scores.sort_by(|left, right| (left.score, &left.path).cmp(&(right.score, &right.path)));
    scores
}

pub(super) fn project_quality_score(scores: &[FileHealthScore]) -> usize {
    if scores.is_empty() {
        return 0;
    }
    let weighted_total = scores
        .iter()
        .map(|score| score.score.saturating_mul(score.functions.max(1)))
        .sum::<usize>();
    let weight = scores
        .iter()
        .map(|score| score.functions.max(1))
        .sum::<usize>();
    weighted_total.saturating_add(weight / 2) / weight
}

pub(super) fn health_hotspots(scores: &[FileHealthScore], min_score: usize) -> Vec<HealthHotspot> {
    let mut hotspots = scores
        .iter()
        .filter(|score| score.score < min_score)
        .map(|score| HealthHotspot {
            path: score.path.clone(),
            location: Location { line: 1, column: 0 },
            score: score.score,
            reasons: score.reasons.clone(),
            owners: score.owners.clone(),
            owner_source: score.owner_source.clone(),
            owner_section: score.owner_section.clone(),
        })
        .collect::<Vec<_>>();
    hotspots.sort_by(|left, right| (left.score, &left.path).cmp(&(right.score, &right.path)));
    hotspots
}

pub(super) fn refactoring_targets(scores: &[FileHealthScore]) -> Vec<RefactoringTarget> {
    let mut targets = scores
        .iter()
        .filter(|score| score.score < 85)
        .filter(|score| score.complex_functions > 0 || score.max_crap_score > 0)
        .map(|score| RefactoringTarget {
            path: score.path.clone(),
            location: Location { line: 1, column: 0 },
            score: score.score,
            priority: target_priority(score),
            reasons: score.reasons.clone(),
            owners: score.owners.clone(),
            owner_source: score.owner_source.clone(),
            owner_section: score.owner_section.clone(),
        })
        .collect::<Vec<_>>();
    targets.sort_by(|left, right| {
        (std::cmp::Reverse(left.priority), left.score, &left.path).cmp(&(
            std::cmp::Reverse(right.priority),
            right.score,
            &right.path,
        ))
    });
    targets
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileScoreBuilder {
    functions: usize,
    complex_functions: usize,
    max_cyclomatic_complexity: usize,
    max_cognitive_complexity: usize,
    max_crap_score: usize,
}

impl FileScoreBuilder {
    fn new(_location: Location) -> Self {
        Self {
            functions: 0,
            complex_functions: 0,
            max_cyclomatic_complexity: 0,
            max_cognitive_complexity: 0,
            max_crap_score: 0,
        }
    }

    fn push_function(&mut self, function: &FunctionMetrics) {
        self.functions += 1;
        self.max_cyclomatic_complexity = self.max_cyclomatic_complexity.max(function.cyclomatic);
        self.max_cognitive_complexity = self.max_cognitive_complexity.max(function.cognitive);
    }

    fn finish(
        self,
        path: std::path::PathBuf,
        coverage: Option<&CoverageMap>,
        options: &HealthOptions,
    ) -> FileHealthScore {
        let coverage_summary = coverage_summary(coverage, &path);
        let mut reasons = score_reasons(&self, &coverage_summary, options);
        if reasons.is_empty() {
            reasons.push("no health penalties".to_owned());
        }
        let score = health_score(&self, coverage_summary.status, options);
        FileHealthScore {
            path,
            score,
            functions: self.functions,
            complex_functions: self.complex_functions,
            max_cyclomatic_complexity: self.max_cyclomatic_complexity,
            max_cognitive_complexity: self.max_cognitive_complexity,
            max_crap_score: self.max_crap_score,
            coverage_status: coverage_summary.status,
            covered_lines: coverage_summary.covered_lines,
            executable_lines: coverage_summary.executable_lines,
            line_coverage_percent: coverage_summary.line_coverage_percent,
            reasons,
            owners: Vec::new(),
            owner_source: None,
            owner_section: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CoverageSummary {
    status: FileCoverageStatus,
    covered_lines: Option<usize>,
    executable_lines: Option<usize>,
    line_coverage_percent: Option<usize>,
}

fn coverage_summary(coverage: Option<&CoverageMap>, path: &std::path::Path) -> CoverageSummary {
    let Some(coverage) = coverage else {
        return CoverageSummary {
            status: FileCoverageStatus::NotRequested,
            covered_lines: None,
            executable_lines: None,
            line_coverage_percent: None,
        };
    };
    let Some(file) = coverage.file(path) else {
        return CoverageSummary {
            status: FileCoverageStatus::Missing,
            covered_lines: Some(0),
            executable_lines: Some(0),
            line_coverage_percent: None,
        };
    };

    let covered_lines = file.covered_lines();
    let executable_lines = file.executable_lines();
    let status = if executable_lines == 0 {
        FileCoverageStatus::NoExecutableLines
    } else if covered_lines == 0 {
        FileCoverageStatus::Uncovered
    } else {
        FileCoverageStatus::Covered
    };
    CoverageSummary {
        status,
        covered_lines: Some(covered_lines),
        executable_lines: Some(executable_lines),
        line_coverage_percent: (executable_lines > 0)
            .then(|| coverage_percent(covered_lines, executable_lines)),
    }
}

fn health_score(
    builder: &FileScoreBuilder,
    coverage_status: FileCoverageStatus,
    options: &HealthOptions,
) -> usize {
    let penalty = complexity_penalty(builder, options) + coverage_penalty(coverage_status);
    100usize.saturating_sub(penalty.min(100))
}

fn complexity_penalty(builder: &FileScoreBuilder, options: &HealthOptions) -> usize {
    builder
        .max_cyclomatic_complexity
        .saturating_sub(options.max_cyclomatic / 2)
        .saturating_mul(2)
        + builder
            .max_cognitive_complexity
            .saturating_sub(options.max_cognitive / 2)
            .saturating_mul(2)
        + builder.complex_functions.saturating_mul(8)
        + builder
            .max_crap_score
            .saturating_sub(options.max_crap.unwrap_or(30) / 2)
            .saturating_mul(2)
}

fn coverage_penalty(status: FileCoverageStatus) -> usize {
    match status {
        FileCoverageStatus::NotRequested | FileCoverageStatus::Covered => 0,
        FileCoverageStatus::NoExecutableLines => 15,
        FileCoverageStatus::Missing => 25,
        FileCoverageStatus::Uncovered => 35,
    }
}

fn score_reasons(
    builder: &FileScoreBuilder,
    coverage: &CoverageSummary,
    options: &HealthOptions,
) -> Vec<String> {
    let mut reasons = Vec::new();
    if builder.max_cyclomatic_complexity > options.max_cyclomatic / 2 {
        reasons.push(format!(
            "max cyclomatic complexity {}",
            builder.max_cyclomatic_complexity
        ));
    }
    if builder.max_cognitive_complexity > options.max_cognitive / 2 {
        reasons.push(format!(
            "max cognitive complexity {}",
            builder.max_cognitive_complexity
        ));
    }
    if builder.complex_functions > 0 {
        reasons.push(format!("{} complex functions", builder.complex_functions));
    }
    if builder.max_crap_score > 0 {
        reasons.push(format!("max CRAP score {}", builder.max_crap_score));
    }
    match coverage.status {
        FileCoverageStatus::NotRequested | FileCoverageStatus::Covered => {}
        FileCoverageStatus::Missing => reasons.push("missing coverage record".to_owned()),
        FileCoverageStatus::NoExecutableLines => {
            reasons.push("no executable LCOV lines".to_owned());
        }
        FileCoverageStatus::Uncovered => reasons.push("zero covered LCOV lines".to_owned()),
    }
    reasons
}

fn target_priority(score: &FileHealthScore) -> usize {
    100usize.saturating_sub(score.score)
        + score.complex_functions.saturating_mul(10)
        + score.max_crap_score / 2
        + uncovered_bonus(score.coverage_status)
}

fn uncovered_bonus(status: FileCoverageStatus) -> usize {
    match status {
        FileCoverageStatus::Missing | FileCoverageStatus::Uncovered => 10,
        FileCoverageStatus::NoExecutableLines => 5,
        FileCoverageStatus::NotRequested | FileCoverageStatus::Covered => 0,
    }
}

fn coverage_percent(covered_lines: usize, executable_lines: usize) -> usize {
    covered_lines
        .saturating_mul(100)
        .saturating_add(executable_lines / 2)
        / executable_lines
}
