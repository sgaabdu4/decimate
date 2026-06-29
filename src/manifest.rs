use serde_json::{Value, json};

use crate::ci_template::CI_TEMPLATE_SCHEMA_VERSION;
use crate::config::CONFIG_SCHEMA_VERSION;
use crate::coverage::COVERAGE_ANALYSIS_SCHEMA_VERSION;
use crate::decision_surface::DECISION_SURFACE_SCHEMA_VERSION;
use crate::explain::EXPLAIN_SCHEMA_VERSION;
use crate::fix::FIX_SCHEMA_VERSION;
use crate::impact::IMPACT_SCHEMA_VERSION;
use crate::inspect::INSPECT_SCHEMA_VERSION;
use crate::output::{SCHEMA_VERSION, TRACE_SCHEMA_VERSION};
use crate::policy::RULE_PACK_SCHEMA_VERSION;
use crate::project_list::PROJECT_LIST_SCHEMA_VERSION;

/// Stable schema version for the agent capability manifest.
pub const MANIFEST_SCHEMA_VERSION: &str = "decimate.schema.v1";

/// Return Decimate's machine-readable CLI and issue manifest.
#[must_use]
pub fn decimate_schema() -> Value {
    json!({
        "schema_version": MANIFEST_SCHEMA_VERSION,
        "kind": "schema",
        "tool": "decimate",
        "schemas": {
            "report": SCHEMA_VERSION,
            "trace": TRACE_SCHEMA_VERSION,
            "inspect": INSPECT_SCHEMA_VERSION,
            "list": PROJECT_LIST_SCHEMA_VERSION,
            "fix": FIX_SCHEMA_VERSION,
            "explain": EXPLAIN_SCHEMA_VERSION,
            "impact": IMPACT_SCHEMA_VERSION,
            "ci_template": CI_TEMPLATE_SCHEMA_VERSION,
            "config": CONFIG_SCHEMA_VERSION,
            "coverage": COVERAGE_ANALYSIS_SCHEMA_VERSION,
            "decision_surface": DECISION_SURFACE_SCHEMA_VERSION,
            "rule_pack": RULE_PACK_SCHEMA_VERSION
        },
        "commands": commands(),
        "issue_types": issue_types(),
        "task_matrix": task_matrix()
    })
}

fn commands() -> Value {
    let mut commands = Vec::new();
    append_commands(&mut commands, analysis_commands());
    append_commands(&mut commands, evidence_commands());
    append_commands(&mut commands, support_commands());
    Value::Array(commands)
}

fn append_commands(commands: &mut Vec<Value>, values: Value) {
    if let Value::Array(values) = values {
        commands.extend(values);
    }
}

fn analysis_commands() -> Value {
    json!([
        {
            "name": "check",
            "kind": "combined",
            "description": "Run all enabled graph, symbol, dependency, duplicate, health, flag, and security checks.",
            "schema": SCHEMA_VERSION,
            "flags": ["--format", "--config", "--entry", "--production", "--no-production", "--file", "--workspace", "--changed-workspaces", "--changed-since", "--regression-baseline", "--save-regression-baseline", "--fail-on-regression", "--tolerance", "--baseline", "--save-baseline", "--boundary", "--boundary-coverage", "--boundary-call", "--policy-pack", "--policy-violations", "--max-cyclomatic", "--max-cognitive", "--complexity-breakdown", "--coverage", "--coverage-gaps", "--max-crap", "--runtime-coverage", "--min-invocations-hot", "--min-observation-volume", "--low-traffic-threshold", "--file-scores", "--hotspots", "--targets", "--ownership", "--min-score", "--mode", "--min-tokens", "--min-lines", "--min-occurrences", "--top", "--skip-local", "--no-ignore-imports", "--include-entry-exports", "--private-type-leaks"]
        },
        {
            "name": "audit",
            "kind": "audit",
            "description": "Run changed-code graph checks scoped from a Git base ref.",
            "schema": SCHEMA_VERSION,
            "flags": ["--brief", "--base", "--dead-code-baseline", "--health-baseline", "--dupes-baseline", "--max-decisions", "--format", "--config", "--entry", "--production", "--no-production", "--file", "--workspace", "--changed-workspaces", "--changed-since", "--boundary", "--boundary-coverage", "--boundary-call", "--policy-pack", "--policy-violations", "--max-cyclomatic", "--max-cognitive", "--complexity-breakdown", "--coverage", "--coverage-gaps", "--max-crap", "--runtime-coverage", "--min-invocations-hot", "--min-observation-volume", "--low-traffic-threshold", "--file-scores", "--hotspots", "--targets", "--ownership", "--min-score", "--mode", "--min-tokens", "--min-lines", "--min-occurrences", "--top", "--skip-local", "--no-ignore-imports", "--include-entry-exports", "--private-type-leaks"]
        },
        {
            "name": "review",
            "kind": "decision-surface",
            "description": "Review changed-code structural decisions without failing CI.",
            "schema": DECISION_SURFACE_SCHEMA_VERSION,
            "flags": ["--base", "--format", "--config", "--max-decisions"]
        },
        {
            "name": "decision-surface",
            "kind": "decision-surface",
            "description": "Surface changed-code structural decisions for reviewer judgment.",
            "schema": DECISION_SURFACE_SCHEMA_VERSION,
            "flags": ["--base", "--format", "--config", "--max-decisions"]
        },
        {
            "name": "dead-code",
            "kind": "dead-code",
            "description": "Find unreachable Dart files and conservative symbol-level dead code.",
            "schema": SCHEMA_VERSION,
            "flags": ["--format", "--config", "--entry", "--production", "--no-production", "--file", "--workspace", "--changed-workspaces", "--changed-since", "--regression-baseline", "--save-regression-baseline", "--fail-on-regression", "--tolerance", "--baseline", "--save-baseline", "--include-entry-exports", "--private-type-leaks"]
        },
        {
            "name": "cycles",
            "kind": "cycles",
            "description": "Find import/export/part/augment dependency cycles.",
            "schema": SCHEMA_VERSION,
            "flags": ["--format", "--config", "--entry", "--production", "--no-production", "--file", "--workspace", "--changed-workspaces", "--changed-since", "--regression-baseline", "--save-regression-baseline", "--fail-on-regression", "--tolerance", "--baseline", "--save-baseline"]
        },
        {
            "name": "dupes",
            "kind": "dupes",
            "description": "Find duplicated Dart code blocks.",
            "schema": SCHEMA_VERSION,
            "flags": ["--format", "--config", "--entry", "--production", "--no-production", "--file", "--workspace", "--changed-workspaces", "--changed-since", "--regression-baseline", "--save-regression-baseline", "--fail-on-regression", "--tolerance", "--baseline", "--save-baseline", "--mode", "--min-tokens", "--min-lines", "--min-occurrences", "--top", "--skip-local", "--no-ignore-imports"]
        },
        {
            "name": "health",
            "kind": "health",
            "description": "Find complex functions, coverage gaps, hotspots, and refactoring targets.",
            "schema": SCHEMA_VERSION,
            "flags": ["--format", "--config", "--entry", "--production", "--no-production", "--file", "--workspace", "--changed-workspaces", "--changed-since", "--regression-baseline", "--save-regression-baseline", "--fail-on-regression", "--tolerance", "--baseline", "--save-baseline", "--max-cyclomatic", "--max-cognitive", "--complexity-breakdown", "--coverage", "--coverage-gaps", "--max-crap", "--runtime-coverage", "--min-invocations-hot", "--min-observation-volume", "--low-traffic-threshold", "--file-scores", "--hotspots", "--targets", "--ownership", "--min-score", "--top"]
        },
        {
            "name": "coverage analyze",
            "kind": "runtime-coverage",
            "description": "Analyze local V8 or Istanbul runtime coverage.",
            "schema": COVERAGE_ANALYSIS_SCHEMA_VERSION,
            "flags": ["--format", "--config", "--runtime-coverage", "--min-invocations-hot", "--min-observation-volume", "--low-traffic-threshold", "--top"]
        },
        {
            "name": "flags",
            "kind": "flags",
            "description": "Inventory Dart and Flutter feature flag patterns.",
            "schema": SCHEMA_VERSION,
            "flags": ["--format", "--config", "--entry", "--production", "--no-production", "--file", "--workspace", "--changed-workspaces", "--changed-since", "--regression-baseline", "--save-regression-baseline", "--fail-on-regression", "--tolerance", "--baseline", "--save-baseline", "--top"]
        },
        {
            "name": "security",
            "kind": "security",
            "description": "Surface local deterministic security review candidates.",
            "schema": SCHEMA_VERSION,
            "flags": ["--format", "--config", "--entry", "--production", "--no-production", "--file", "--workspace", "--changed-workspaces", "--regression-baseline", "--save-regression-baseline", "--fail-on-regression", "--tolerance", "--baseline", "--save-baseline", "--top", "--surface", "--sarif-file", "--ci", "--fail-on-issues", "--summary", "--gate", "--diff-file", "--diff-stdin", "--changed-since"]
        },
        {
            "name": "impact",
            "kind": "impact",
            "description": "Read the local Decimate value report without running analysis.",
            "schema": IMPACT_SCHEMA_VERSION,
            "flags": ["--format", "--quiet", "--all", "--sort", "--limit"]
        }
    ])
}

fn evidence_commands() -> Value {
    json!([
        {
            "name": "inspect",
            "kind": "inspect",
            "description": "Compose one evidence bundle for a Dart file or top-level symbol.",
            "schema": INSPECT_SCHEMA_VERSION,
            "flags": ["--format", "--config", "--entry", "--production", "--no-production", "--file", "--symbol"]
        },
        {
            "name": "trace-file",
            "kind": "trace-file",
            "description": "Trace one Dart file's graph evidence.",
            "schema": TRACE_SCHEMA_VERSION,
            "flags": ["--format", "--config", "--entry", "--production", "--no-production", "--file"]
        },
        {
            "name": "trace-symbol",
            "kind": "trace-symbol",
            "description": "Trace one top-level symbol's declaration, references, and re-export chains.",
            "schema": TRACE_SCHEMA_VERSION,
            "flags": ["--format", "--config", "--entry", "--production", "--no-production", "--file", "--symbol"]
        },
        {
            "name": "trace-dependency",
            "kind": "trace-dependency",
            "description": "Trace one pub dependency declaration and Dart import/export usage.",
            "schema": TRACE_SCHEMA_VERSION,
            "flags": ["--format", "--config", "--entry", "--production", "--no-production", "--dependency"]
        },
        {
            "name": "trace-clone",
            "kind": "trace-clone",
            "description": "Trace one duplicate-code group by fingerprint or file line.",
            "schema": TRACE_SCHEMA_VERSION,
            "flags": ["--format", "--config", "--entry", "--production", "--no-production", "--mode", "--min-tokens", "--min-lines", "--min-occurrences", "--top", "--skip-local", "--no-ignore-imports", "--fingerprint"]
        }
    ])
}

fn support_commands() -> Value {
    json!([
        {
            "name": "list",
            "kind": "list",
            "description": "List Decimate project structure and active Dart/Flutter adapters.",
            "schema": PROJECT_LIST_SCHEMA_VERSION,
            "flags": ["--format", "--config", "--entry", "--production", "--no-production", "--files", "--entry-points", "--workspaces", "--plugins", "--boundaries", "--file", "--workspace", "--changed-workspaces"]
        },
        {
            "name": "workspaces",
            "kind": "list",
            "description": "List discovered local pub packages.",
            "schema": PROJECT_LIST_SCHEMA_VERSION,
            "flags": ["--format", "--config", "--entry", "--production", "--no-production", "--file", "--workspace", "--changed-workspaces"]
        },
        {
            "name": "explain",
            "kind": "explain",
            "description": "Explain one Decimate issue type without running analysis.",
            "schema": EXPLAIN_SCHEMA_VERSION,
            "flags": ["--format"]
        },
        {
            "name": "fix",
            "kind": "fix",
            "description": "Plan or apply safe auto-fixes.",
            "schema": FIX_SCHEMA_VERSION,
            "flags": ["--format", "--config", "--entry", "--production", "--no-production", "--file", "--workspace", "--changed-workspaces", "--changed-since", "--action", "--apply", "--confirm"]
        },
        {
            "name": "ci-template",
            "kind": "ci-template",
            "description": "Print or vendor GitHub Actions and GitLab CI templates.",
            "schema": CI_TEMPLATE_SCHEMA_VERSION,
            "flags": ["--format", "--vendor", "--root", "--force"]
        },
        {
            "name": "schema",
            "kind": "schema",
            "description": "Print this machine-readable CLI and issue manifest.",
            "schema": MANIFEST_SCHEMA_VERSION,
            "flags": ["--format"]
        },
        {
            "name": "config-schema",
            "kind": "config-schema",
            "description": "Print the configuration JSON schema.",
            "schema": CONFIG_SCHEMA_VERSION,
            "flags": ["--format"]
        },
        {
            "name": "report-schema",
            "kind": "report-schema",
            "description": "Print the analysis report JSON schema.",
            "schema": SCHEMA_VERSION,
            "flags": ["--format"]
        },
        {
            "name": "rule-pack-schema",
            "kind": "rule-pack-schema",
            "description": "Print the policy rule-pack JSON schema.",
            "schema": RULE_PACK_SCHEMA_VERSION,
            "flags": ["--format"]
        }
    ])
}

fn issue_types() -> Value {
    json!([
        "dead-file",
        "unused-export",
        "unused-type",
        "private-type-leak",
        "unused-enum-member",
        "unused-class-member",
        "duplicate-export",
        "missing-entry-point",
        "circular-dependency",
        "re-export-cycle",
        "boundary-violation",
        "boundary-coverage",
        "boundary-call-violation",
        "policy-violation",
        "unresolved-dependency",
        "part-of-violation",
        "unused-dependency",
        "unused-dev-dependency",
        "test-only-dependency",
        "unused-dependency-override",
        "misconfigured-dependency-override",
        "unlisted-dependency",
        "code-duplication",
        "high-cyclomatic-complexity",
        "high-cognitive-complexity",
        "high-complexity",
        "coverage-gap",
        "high-crap-score",
        "health-hotspot",
        "refactoring-target",
        "feature-flag",
        "security-candidate",
        "stale-suppression",
        "missing-suppression-reason"
    ])
}

fn task_matrix() -> Value {
    json!([
        {
            "intent": "find cleanup opportunities",
            "command": "decimate check --format json",
            "reason": "Combined graph, symbol, dependency, duplication, health, flag, and security report."
        },
        {
            "intent": "review changed code",
            "command": "decimate audit --format json --base <ref>",
            "reason": "Changed-file gate with related graph findings."
        },
        {
            "intent": "review structural decisions",
            "command": "decimate decision-surface --format json --base <ref>",
            "reason": "Ranked changed-code questions for architecture, API, and dependency reviewers."
        },
        {
            "intent": "delete an unused file or export",
            "command": "decimate inspect --format json --file <path>",
            "reason": "Evidence bundle before any deletion or suppression."
        },
        {
            "intent": "trace a top-level symbol",
            "command": "decimate inspect --format json --symbol <file>:<symbol>",
            "reason": "Declaration, references, re-export chains, and file-scoped findings."
        },
        {
            "intent": "verify an unused dependency",
            "command": "decimate trace-dependency --format json --dependency <package>",
            "reason": "Pubspec declarations and Dart import/export usage."
        },
        {
            "intent": "consolidate duplicated code",
            "command": "decimate trace-clone --format json --fingerprint <fingerprint>",
            "reason": "Duplicate group instances and extraction suggestion."
        },
        {
            "intent": "scope a monorepo",
            "command": "decimate check --format json --workspace <pattern>",
            "reason": "Restrict findings to matching local pub packages."
        },
        {
            "intent": "explain an issue",
            "command": "decimate explain --format json <issue-type>",
            "reason": "Rule rationale, aliases, suppressions, and follow-up commands."
        },
        {
            "intent": "show local value report",
            "command": "decimate impact --format json --quiet",
            "reason": "Read-only local impact report; disabled projects return a populated zero-count report."
        },
        {
            "intent": "set up CI",
            "command": "decimate ci-template github --format yaml",
            "reason": "Read-only CI template output for changed-code audit gating."
        }
    ])
}
