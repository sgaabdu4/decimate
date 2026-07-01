use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Stable schema version for local impact reports.
pub const IMPACT_SCHEMA_VERSION: &str = "dart-decimate.impact.v1";
const IMPACT_HISTORY_PATH: &str = ".dart-decimate/impact.jsonl";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactReport {
    pub schema_version: String,
    pub kind: String,
    pub tool: String,
    pub command: String,
    pub enabled: bool,
    pub enabled_source: String,
    pub explicit_decision: bool,
    pub onboarding_declined: bool,
    pub record_count: usize,
    pub project: ImpactProject,
    pub totals: ImpactTotals,
    pub trend: ImpactTrend,
    pub gate: ImpactGate,
    pub records: Vec<ImpactRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactAllReport {
    pub schema_version: String,
    pub kind: String,
    pub tool: String,
    pub command: String,
    pub summary: ImpactAllSummary,
    pub projects: Vec<ImpactProjectSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactProject {
    pub id: String,
    pub label: String,
    pub root_hint: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactTotals {
    pub surfaced: usize,
    pub resolved: usize,
    pub suppressed: usize,
    pub contained_commits: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactTrend {
    pub surfaced_delta: isize,
    pub resolved_delta: isize,
    pub suppressed_delta: isize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactGate {
    pub contained_commits: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactRecord {
    pub timestamp: String,
    pub surfaced: usize,
    pub resolved: usize,
    pub suppressed: usize,
    pub contained_commits: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactAllSummary {
    pub projects: usize,
    pub enabled_projects: usize,
    pub record_count: usize,
    pub surfaced: usize,
    pub contained_commits: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactProjectSummary {
    pub project: ImpactProject,
    pub enabled: bool,
    pub record_count: usize,
    pub surfaced: usize,
    pub contained_commits: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpactSort {
    Label,
    Surfaced,
    ContainedCommits,
    RecordCount,
}

#[must_use]
pub fn impact_report(root: impl AsRef<Path>) -> ImpactReport {
    let root = normalize_root_hint(root.as_ref());
    let project = impact_project(&root);
    let records = load_impact_history(&root);
    let enabled = !records.is_empty();
    let totals = impact_totals(&records);
    ImpactReport {
        schema_version: IMPACT_SCHEMA_VERSION.to_owned(),
        kind: "impact".to_owned(),
        tool: "dart-decimate".to_owned(),
        command: "impact".to_owned(),
        enabled,
        enabled_source: if enabled {
            "local-history".to_owned()
        } else {
            "default".to_owned()
        },
        explicit_decision: false,
        onboarding_declined: false,
        record_count: records.len(),
        project,
        trend: impact_trend(&records),
        gate: ImpactGate {
            contained_commits: totals.contained_commits,
        },
        totals,
        records,
    }
}

#[must_use]
pub fn impact_all_report(sort: ImpactSort, limit: usize) -> ImpactAllReport {
    let mut projects = Vec::<ImpactProjectSummary>::new();
    sort_project_summaries(&mut projects, sort);
    projects.truncate(limit);
    ImpactAllReport {
        schema_version: IMPACT_SCHEMA_VERSION.to_owned(),
        kind: "impact-all".to_owned(),
        tool: "dart-decimate".to_owned(),
        command: "impact --all".to_owned(),
        summary: ImpactAllSummary {
            projects: projects.len(),
            enabled_projects: projects.iter().filter(|project| project.enabled).count(),
            record_count: projects.iter().map(|project| project.record_count).sum(),
            surfaced: projects.iter().map(|project| project.surfaced).sum(),
            contained_commits: projects
                .iter()
                .map(|project| project.contained_commits)
                .sum(),
        },
        projects,
    }
}

#[must_use]
pub fn render_impact_report(report: &ImpactReport) -> String {
    if !report.enabled {
        return format!(
            "impact enabled=false project={} records=0\n",
            report.project.label
        );
    }

    format!(
        "impact enabled=true project={} surfaced={} contained_commits={}\n",
        report.project.label, report.totals.surfaced, report.totals.contained_commits
    )
}

#[must_use]
pub fn render_impact_all_report(report: &ImpactAllReport) -> String {
    let mut rendered = String::new();
    let _ = writeln!(
        rendered,
        "impact-all projects={} enabled_projects={} records={}",
        report.summary.projects, report.summary.enabled_projects, report.summary.record_count
    );
    for project in &report.projects {
        let _ = writeln!(
            rendered,
            "- {} surfaced={} contained_commits={}",
            project.project.label, project.surfaced, project.contained_commits
        );
    }
    rendered
}

fn impact_project(root: &Path) -> ImpactProject {
    ImpactProject {
        id: stable_project_id(root),
        label: project_label(root),
        root_hint: root.to_string_lossy().into_owned(),
    }
}

fn load_impact_history(root: &Path) -> Vec<ImpactRecord> {
    let Ok(contents) = fs::read_to_string(root.join(IMPACT_HISTORY_PATH)) else {
        return Vec::new();
    };

    contents
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.is_empty() {
                None
            } else {
                serde_json::from_str::<ImpactRecord>(line).ok()
            }
        })
        .collect()
}

fn impact_totals(records: &[ImpactRecord]) -> ImpactTotals {
    ImpactTotals {
        surfaced: records.iter().map(|record| record.surfaced).sum(),
        resolved: records.iter().map(|record| record.resolved).sum(),
        suppressed: records.iter().map(|record| record.suppressed).sum(),
        contained_commits: records.iter().map(|record| record.contained_commits).sum(),
    }
}

fn impact_trend(records: &[ImpactRecord]) -> ImpactTrend {
    let (Some(first), Some(last)) = (records.first(), records.last()) else {
        return ImpactTrend {
            surfaced_delta: 0,
            resolved_delta: 0,
            suppressed_delta: 0,
        };
    };

    ImpactTrend {
        surfaced_delta: usize_delta(last.surfaced, first.surfaced),
        resolved_delta: usize_delta(last.resolved, first.resolved),
        suppressed_delta: usize_delta(last.suppressed, first.suppressed),
    }
}

fn usize_delta(current: usize, previous: usize) -> isize {
    let current = isize::try_from(current).unwrap_or(isize::MAX);
    let previous = isize::try_from(previous).unwrap_or(isize::MAX);
    current.saturating_sub(previous)
}

fn normalize_root_hint(root: &Path) -> PathBuf {
    root.canonicalize().unwrap_or_else(|_| root.to_path_buf())
}

fn project_label(root: &Path) -> String {
    root.file_name()
        .and_then(std::ffi::OsStr::to_str)
        .filter(|label| !label.is_empty())
        .unwrap_or("project")
        .to_owned()
}

fn stable_project_id(root: &Path) -> String {
    format!("dart-decimate:impact:{:016x}", stable_hash(root))
}

fn stable_hash(root: &Path) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in root.to_string_lossy().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn sort_project_summaries(projects: &mut [ImpactProjectSummary], sort: ImpactSort) {
    projects.sort_by(|left, right| match sort {
        ImpactSort::Label => left.project.label.cmp(&right.project.label),
        ImpactSort::Surfaced => right
            .surfaced
            .cmp(&left.surfaced)
            .then_with(|| left.project.label.cmp(&right.project.label)),
        ImpactSort::ContainedCommits => right
            .contained_commits
            .cmp(&left.contained_commits)
            .then_with(|| left.project.label.cmp(&right.project.label)),
        ImpactSort::RecordCount => right
            .record_count
            .cmp(&left.record_count)
            .then_with(|| left.project.label.cmp(&right.project.label)),
    });
}
