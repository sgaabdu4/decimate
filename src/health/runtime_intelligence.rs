use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::graph::normalize_against;
use crate::{LowTrafficThreshold, ScannedProject};

use super::runtime_coverage::{RuntimeCoverageAccumulator, RuntimeFunctionCoverage};
use super::{
    FunctionMetrics, HealthOptions, RuntimeCoverageConfidence, RuntimeCoverageFinding,
    RuntimeCoverageFindingKind, RuntimeHotPath,
};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeCoverageIntelligence {
    pub id: String,
    pub stable_id: String,
    pub kind: RuntimeCoverageIntelligenceKind,
    pub path: PathBuf,
    pub line: Option<usize>,
    pub symbol: Option<String>,
    pub action: RuntimeCoverageAction,
    pub priority: usize,
    pub confidence: RuntimeCoverageConfidence,
    pub reason: String,
    pub signals: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeCoverageIntelligenceKind {
    HotPathTouched,
    LowTraffic,
    CoverageUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeCoverageAction {
    ReviewRuntime,
    DeleteColdCode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeBlastRadius {
    pub id: String,
    pub stable_id: String,
    pub path: PathBuf,
    pub line: Option<usize>,
    pub symbol: Option<String>,
    pub caller_count: usize,
    pub callers: Vec<PathBuf>,
    pub traffic_weighted_caller_reach: LowTrafficThreshold,
    pub risk: RuntimeBlastRisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeBlastRisk {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeImportance {
    pub id: String,
    pub stable_id: String,
    pub path: PathBuf,
    pub line: Option<usize>,
    pub symbol: Option<String>,
    pub invocations: usize,
    pub traffic_fraction: LowTrafficThreshold,
    pub cyclomatic_complexity: usize,
    pub caller_count: usize,
    pub score: usize,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RuntimeIntelligenceBundle {
    pub(super) coverage_intelligence: Vec<RuntimeCoverageIntelligence>,
    pub(super) blast_radius: Vec<RuntimeBlastRadius>,
    pub(super) importance: Vec<RuntimeImportance>,
}

pub(super) fn runtime_intelligence(
    project: &ScannedProject,
    options: &HealthOptions,
    functions: &[FunctionMetrics],
    accumulator: &RuntimeCoverageAccumulator,
    total_invocations: usize,
    hot_paths: &[RuntimeHotPath],
    findings: &[RuntimeCoverageFinding],
) -> RuntimeIntelligenceBundle {
    let callers = callers_by_target(project);
    let observations = runtime_observations(accumulator);
    let importance = runtime_importance(
        options,
        functions,
        &observations,
        &callers,
        total_invocations,
    );
    let blast_radius = runtime_blast_radius(
        options,
        &observations,
        hot_paths,
        &callers,
        total_invocations,
    );
    let coverage_intelligence =
        coverage_intelligence(project, options, hot_paths, findings, total_invocations);
    RuntimeIntelligenceBundle {
        coverage_intelligence,
        blast_radius,
        importance,
    }
}

fn runtime_observations(accumulator: &RuntimeCoverageAccumulator) -> Vec<RuntimeObservation> {
    let mut observations = BTreeMap::<(PathBuf, Option<usize>, Option<String>), usize>::new();
    for (path, file) in &accumulator.files {
        if file.functions.is_empty() {
            observations.insert((path.clone(), None, None), file.invocations);
            continue;
        }
        for function in &file.functions {
            add_observation(&mut observations, path, function);
        }
    }
    observations
        .into_iter()
        .map(|((path, line, symbol), invocations)| RuntimeObservation {
            path,
            line,
            symbol,
            invocations,
        })
        .collect()
}

fn add_observation(
    observations: &mut BTreeMap<(PathBuf, Option<usize>, Option<String>), usize>,
    path: &Path,
    function: &RuntimeFunctionCoverage,
) {
    *observations
        .entry((path.to_path_buf(), function.line, function.symbol.clone()))
        .or_default() += function.invocations;
}

fn runtime_importance(
    options: &HealthOptions,
    functions: &[FunctionMetrics],
    observations: &[RuntimeObservation],
    callers: &BTreeMap<PathBuf, Vec<PathBuf>>,
    total_invocations: usize,
) -> Vec<RuntimeImportance> {
    let mut importance = observations
        .iter()
        .map(|observation| {
            let complexity = matched_complexity(functions, observation);
            let caller_count = callers.get(&observation.path).map_or(0, std::vec::Vec::len);
            let traffic =
                LowTrafficThreshold::from_fraction(observation.invocations, total_invocations);
            let score = importance_score(observation, complexity, caller_count, total_invocations);
            RuntimeImportance {
                id: stable_prefixed("dart-decimate:importance", observation),
                stable_id: stable_prefixed("dart-decimate:fn", observation),
                path: observation.path.clone(),
                line: observation.line,
                symbol: observation.symbol.clone(),
                invocations: observation.invocations,
                traffic_fraction: traffic,
                cyclomatic_complexity: complexity,
                caller_count,
                score,
                reason: importance_reason(observation, complexity, caller_count, options),
            }
        })
        .collect::<Vec<_>>();
    importance.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.invocations.cmp(&left.invocations))
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.line.cmp(&right.line))
    });
    importance
}

fn runtime_blast_radius(
    options: &HealthOptions,
    observations: &[RuntimeObservation],
    hot_paths: &[RuntimeHotPath],
    callers: &BTreeMap<PathBuf, Vec<PathBuf>>,
    total_invocations: usize,
) -> Vec<RuntimeBlastRadius> {
    let invocation_by_path = invocations_by_path(observations);
    let mut rows = hot_paths
        .iter()
        .map(|hot_path| {
            let observation = RuntimeObservation {
                path: hot_path.path.clone(),
                line: hot_path.line,
                symbol: hot_path.symbol.clone(),
                invocations: hot_path.invocations,
            };
            let callers = callers.get(&hot_path.path).cloned().unwrap_or_default();
            let caller_reach = caller_reach(&callers, &invocation_by_path, total_invocations);
            let risk = blast_risk(hot_path.invocations, callers.len(), options);
            RuntimeBlastRadius {
                id: stable_prefixed("dart-decimate:blast", &observation),
                stable_id: stable_prefixed("dart-decimate:fn", &observation),
                path: hot_path.path.clone(),
                line: hot_path.line,
                symbol: hot_path.symbol.clone(),
                caller_count: callers.len(),
                callers,
                traffic_weighted_caller_reach: caller_reach,
                risk,
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by(|left, right| {
        risk_rank(right.risk)
            .cmp(&risk_rank(left.risk))
            .then_with(|| right.caller_count.cmp(&left.caller_count))
            .then_with(|| left.path.cmp(&right.path))
    });
    rows
}

fn coverage_intelligence(
    project: &ScannedProject,
    options: &HealthOptions,
    hot_paths: &[RuntimeHotPath],
    findings: &[RuntimeCoverageFinding],
    total_invocations: usize,
) -> Vec<RuntimeCoverageIntelligence> {
    let mut rows = hot_paths
        .iter()
        .map(|hot_path| hot_path_intelligence(hot_path, options, total_invocations))
        .collect::<Vec<_>>();
    rows.extend(
        findings
            .iter()
            .map(|finding| finding_intelligence(project, finding, total_invocations, options)),
    );
    rows.sort_by(|left, right| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.line.cmp(&right.line))
    });
    rows
}

fn callers_by_target(project: &ScannedProject) -> BTreeMap<PathBuf, Vec<PathBuf>> {
    let mut callers = BTreeMap::<PathBuf, BTreeSet<PathBuf>>::new();
    for dependency in project.graph.dependencies() {
        callers
            .entry(normalize_against(&project.root, &dependency.to_path))
            .or_default()
            .insert(normalize_against(&project.root, &dependency.from_path));
    }
    callers
        .into_iter()
        .map(|(target, sources)| (target, sources.into_iter().collect()))
        .collect()
}

fn invocations_by_path(observations: &[RuntimeObservation]) -> BTreeMap<PathBuf, usize> {
    let mut by_path = BTreeMap::new();
    for observation in observations {
        *by_path.entry(observation.path.clone()).or_default() += observation.invocations;
    }
    by_path
}

fn caller_reach(
    callers: &[PathBuf],
    invocation_by_path: &BTreeMap<PathBuf, usize>,
    total_invocations: usize,
) -> LowTrafficThreshold {
    let invocations = callers
        .iter()
        .filter_map(|caller| invocation_by_path.get(caller))
        .sum::<usize>();
    LowTrafficThreshold::from_fraction(invocations, total_invocations)
}

fn matched_complexity(functions: &[FunctionMetrics], observation: &RuntimeObservation) -> usize {
    functions
        .iter()
        .find(|function| function_matches(function, observation))
        .map_or(0, |function| function.cyclomatic)
}

fn function_matches(function: &FunctionMetrics, observation: &RuntimeObservation) -> bool {
    function.path == observation.path
        && observation
            .symbol
            .as_ref()
            .is_none_or(|symbol| symbol == &function.symbol)
        && observation
            .line
            .is_none_or(|line| function.location.line <= line && line <= function.end_line)
}

fn importance_score(
    observation: &RuntimeObservation,
    complexity: usize,
    caller_count: usize,
    total_invocations: usize,
) -> usize {
    let traffic = observation
        .invocations
        .saturating_mul(70)
        .checked_div(total_invocations)
        .unwrap_or_default();
    traffic
        .saturating_add(complexity.min(20))
        .saturating_add((caller_count * 5).min(10))
        .min(100)
}

fn importance_reason(
    observation: &RuntimeObservation,
    complexity: usize,
    caller_count: usize,
    options: &HealthOptions,
) -> String {
    if observation.invocations >= options.min_invocations_hot {
        return format!(
            "{} runtime invocations on a hot path with cyclomatic complexity {complexity} and {caller_count} callers",
            observation.invocations
        );
    }
    format!(
        "{} runtime invocations with cyclomatic complexity {complexity} and {caller_count} callers",
        observation.invocations
    )
}

fn hot_path_intelligence(
    hot_path: &RuntimeHotPath,
    options: &HealthOptions,
    total_invocations: usize,
) -> RuntimeCoverageIntelligence {
    let observation = RuntimeObservation::from_hot_path(hot_path);
    RuntimeCoverageIntelligence {
        id: stable_prefixed("dart-decimate:coverage-intel", &observation),
        stable_id: stable_prefixed("dart-decimate:fn", &observation),
        kind: RuntimeCoverageIntelligenceKind::HotPathTouched,
        path: hot_path.path.clone(),
        line: hot_path.line,
        symbol: hot_path.symbol.clone(),
        action: RuntimeCoverageAction::ReviewRuntime,
        priority: 100,
        confidence: observation_confidence(total_invocations, options),
        reason: "runtime coverage marks this function as a production hot path".to_owned(),
        signals: vec!["hot-path".to_owned(), "review-runtime".to_owned()],
    }
}

fn finding_intelligence(
    project: &ScannedProject,
    finding: &RuntimeCoverageFinding,
    total_invocations: usize,
    options: &HealthOptions,
) -> RuntimeCoverageIntelligence {
    let observation = RuntimeObservation {
        path: finding.path.clone(),
        line: Some(finding.line),
        symbol: None,
        invocations: finding.invocations,
    };
    let (kind, priority, signals) = finding_intelligence_parts(finding.kind);
    RuntimeCoverageIntelligence {
        id: stable_prefixed("dart-decimate:coverage-intel", &observation),
        stable_id: stable_prefixed("dart-decimate:fn", &observation),
        kind,
        path: finding.path.clone(),
        line: Some(finding.line),
        symbol: None,
        action: if finding.safe_to_delete {
            RuntimeCoverageAction::DeleteColdCode
        } else {
            RuntimeCoverageAction::ReviewRuntime
        },
        priority,
        confidence: observation_confidence(total_invocations, options),
        reason: runtime_reason(project, finding),
        signals,
    }
}

fn finding_intelligence_parts(
    kind: RuntimeCoverageFindingKind,
) -> (RuntimeCoverageIntelligenceKind, usize, Vec<String>) {
    match kind {
        RuntimeCoverageFindingKind::LowTraffic => (
            RuntimeCoverageIntelligenceKind::LowTraffic,
            70,
            vec!["low-traffic".to_owned(), "review-runtime".to_owned()],
        ),
        RuntimeCoverageFindingKind::CoverageUnavailable => (
            RuntimeCoverageIntelligenceKind::CoverageUnavailable,
            40,
            vec![
                "coverage-unavailable".to_owned(),
                "review-runtime".to_owned(),
            ],
        ),
    }
}

fn runtime_reason(project: &ScannedProject, finding: &RuntimeCoverageFinding) -> String {
    let path = finding
        .path
        .strip_prefix(&project.root)
        .unwrap_or(&finding.path)
        .display();
    match finding.kind {
        RuntimeCoverageFindingKind::LowTraffic => {
            format!("{path} ran below the configured low-traffic threshold")
        }
        RuntimeCoverageFindingKind::CoverageUnavailable => {
            format!("{path} was scanned but absent from runtime coverage")
        }
    }
}

fn observation_confidence(
    total_invocations: usize,
    options: &HealthOptions,
) -> RuntimeCoverageConfidence {
    if total_invocations == 0 {
        RuntimeCoverageConfidence::Low
    } else if total_invocations >= options.min_observation_volume {
        RuntimeCoverageConfidence::High
    } else {
        RuntimeCoverageConfidence::Medium
    }
}

fn blast_risk(
    invocations: usize,
    caller_count: usize,
    options: &HealthOptions,
) -> RuntimeBlastRisk {
    if caller_count >= 5 || invocations >= options.min_invocations_hot.saturating_mul(10) {
        RuntimeBlastRisk::High
    } else if caller_count >= 2 || invocations >= options.min_invocations_hot {
        RuntimeBlastRisk::Medium
    } else {
        RuntimeBlastRisk::Low
    }
}

const fn risk_rank(risk: RuntimeBlastRisk) -> usize {
    match risk {
        RuntimeBlastRisk::Low => 0,
        RuntimeBlastRisk::Medium => 1,
        RuntimeBlastRisk::High => 2,
    }
}

fn stable_prefixed(prefix: &str, observation: &RuntimeObservation) -> String {
    let mut hash = FNV_OFFSET;
    update_hash(&mut hash, observation.path.to_string_lossy().as_bytes());
    update_hash(&mut hash, &observation.line.unwrap_or(0).to_le_bytes());
    if let Some(symbol) = &observation.symbol {
        update_hash(&mut hash, symbol.as_bytes());
    }
    format!("{prefix}:{hash:016x}")
}

fn update_hash(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeObservation {
    path: PathBuf,
    line: Option<usize>,
    symbol: Option<String>,
    invocations: usize,
}

impl RuntimeObservation {
    fn from_hot_path(hot_path: &RuntimeHotPath) -> Self {
        Self {
            path: hot_path.path.clone(),
            line: hot_path.line,
            symbol: hot_path.symbol.clone(),
            invocations: hot_path.invocations,
        }
    }
}
