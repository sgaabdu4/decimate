use serde_json::{Map, Value};

use super::cli_args::cli_args_for_tool;

#[test]
fn analyze_maps_read_only_parity_flags() -> Result<(), String> {
    let args = arguments_json(
        r#"{
            "root": "/repo",
            "config": "decimate.json",
            "entry": ["lib/main.dart"],
            "file": ["lib/src/a.dart"],
            "changed_workspaces": "origin/main",
            "changed_since": "HEAD~1",
            "production": false,
            "baseline": "baseline.json",
            "regression_baseline": "regression.json",
            "fail_on_regression": true,
            "tolerance": "10%",
            "include_entry_exports": true,
            "private_type_leaks": true,
            "boundary": ["lib/domain:lib/ui"],
            "boundary_coverage": true,
            "boundary_call": ["lib/domain:FirebaseRemoteConfig.*"],
            "policy_pack": ["policy.json"],
            "policy_violations": true,
            "mode": "strict",
            "min_tokens": 25,
            "min_lines": 4,
            "min_occurrences": 3,
            "top": 5,
            "skip_local": true,
            "no_ignore_imports": true,
            "max_cyclomatic": 9,
            "max_cognitive": 8,
            "max_crap": 20,
            "coverage": "lcov.info",
            "runtime_coverage": "coverage.json",
            "min_invocations_hot": 10,
            "min_observation_volume": 20,
            "low_traffic_threshold": 0.02,
            "coverage_gaps": true,
            "file_scores": true,
            "hotspots": true,
            "targets": true,
            "ownership": true,
            "complexity_breakdown": true,
            "min_score": 70,
            "issue_types": ["unused-files", "boundary-violations"]
        }"#,
    )?;

    let cli = cli_args_for_tool("analyze", &args)?;

    assert_eq!(cli[..4], ["decimate", "check", "--format", "json"]);
    assert_pair(&cli, "--root", "/repo");
    assert_pair(&cli, "--entry", "lib/main.dart");
    assert_pair(&cli, "--file", "lib/src/a.dart");
    assert_pair(&cli, "--changed-workspaces", "origin/main");
    assert_pair(&cli, "--changed-since", "HEAD~1");
    assert_flag(&cli, "--no-production");
    assert_pair(&cli, "--baseline", "baseline.json");
    assert_pair(&cli, "--regression-baseline", "regression.json");
    assert_flag(&cli, "--fail-on-regression");
    assert_pair(&cli, "--tolerance", "10%");
    assert_flag(&cli, "--include-entry-exports");
    assert_flag(&cli, "--private-type-leaks");
    assert_pair(&cli, "--boundary", "lib/domain:lib/ui");
    assert_flag(&cli, "--boundary-coverage");
    assert_pair(&cli, "--boundary-call", "lib/domain:FirebaseRemoteConfig.*");
    assert_pair(&cli, "--policy-pack", "policy.json");
    assert_flag(&cli, "--policy-violations");
    assert_pair(&cli, "--mode", "strict");
    assert_pair(&cli, "--min-tokens", "25");
    assert_pair(&cli, "--min-lines", "4");
    assert_pair(&cli, "--min-occurrences", "3");
    assert_pair(&cli, "--top", "5");
    assert_flag(&cli, "--skip-local");
    assert_flag(&cli, "--no-ignore-imports");
    assert_pair(&cli, "--max-cyclomatic", "9");
    assert_pair(&cli, "--max-cognitive", "8");
    assert_pair(&cli, "--max-crap", "20");
    assert_pair(&cli, "--coverage", "lcov.info");
    assert_pair(&cli, "--runtime-coverage", "coverage.json");
    assert_pair(&cli, "--min-invocations-hot", "10");
    assert_pair(&cli, "--min-observation-volume", "20");
    assert_pair(&cli, "--low-traffic-threshold", "0.02");
    assert_flag(&cli, "--coverage-gaps");
    assert_flag(&cli, "--file-scores");
    assert_flag(&cli, "--hotspots");
    assert_flag(&cli, "--targets");
    assert_flag(&cli, "--ownership");
    assert_flag(&cli, "--complexity-breakdown");
    assert_pair(&cli, "--min-score", "70");
    assert_flag(&cli, "--unused-files");
    assert_flag(&cli, "--boundary-violations");

    Ok(())
}

#[test]
fn security_candidates_map_gate_and_ci_flags() -> Result<(), String> {
    let args = arguments_json(
        r#"{
            "root": "/repo",
            "entry": ["lib/main.dart"],
            "file": ["lib/auth.dart"],
            "workspace": ["app"],
            "changed_since": "HEAD~1",
            "production": true,
            "baseline": "security-baseline.json",
            "top": 3,
            "surface": true,
            "gate": "newly-reachable",
            "diff_file": "changes.patch",
            "fail_on_issues": true,
            "summary": true
        }"#,
    )?;

    let cli = cli_args_for_tool("security_candidates", &args)?;

    assert_eq!(cli[..4], ["decimate", "security", "--format", "json"]);
    assert_pair(&cli, "--entry", "lib/main.dart");
    assert_pair(&cli, "--file", "lib/auth.dart");
    assert_pair(&cli, "--workspace", "app");
    assert_pair(&cli, "--changed-since", "HEAD~1");
    assert_flag(&cli, "--production");
    assert_pair(&cli, "--baseline", "security-baseline.json");
    assert_pair(&cli, "--top", "3");
    assert_flag(&cli, "--surface");
    assert_pair(&cli, "--gate", "newly-reachable");
    assert_pair(&cli, "--diff-file", "changes.patch");
    assert_flag(&cli, "--fail-on-issues");
    assert_flag(&cli, "--summary");

    Ok(())
}

#[test]
fn changed_and_boundary_tools_map_direct_fallow_aliases() -> Result<(), String> {
    let changed = cli_args_for_tool(
        "check_changed",
        &arguments_json(
            r#"{
                "root": "/repo",
                "since": "origin/main",
                "baseline": "baseline.json",
                "fail_on_regression": true,
                "production": false
            }"#,
        )?,
    )?;
    let boundaries = cli_args_for_tool(
        "list_boundaries",
        &arguments_json(
            r#"{
                "root": "/repo",
                "workspace": ["app"],
                "production": true
            }"#,
        )?,
    )?;

    assert_eq!(changed[..4], ["decimate", "check", "--format", "json"]);
    assert_pair(&changed, "--root", "/repo");
    assert_pair(&changed, "--changed-since", "origin/main");
    assert_pair(&changed, "--baseline", "baseline.json");
    assert_flag(&changed, "--fail-on-regression");
    assert_flag(&changed, "--no-production");

    assert_eq!(boundaries[..4], ["decimate", "list", "--format", "json"]);
    assert_pair(&boundaries, "--root", "/repo");
    assert_pair(&boundaries, "--workspace", "app");
    assert_flag(&boundaries, "--production");
    assert_flag(&boundaries, "--boundaries");

    Ok(())
}

#[test]
fn check_changed_accepts_changed_since_alias() -> Result<(), String> {
    let cli = cli_args_for_tool(
        "check_changed",
        &arguments_json(r#"{ "changed_since": "HEAD~1" }"#)?,
    )?;

    assert_pair(&cli, "--changed-since", "HEAD~1");
    Ok(())
}

#[test]
fn runtime_coverage_tools_map_fallow_coverage_param() -> Result<(), String> {
    let args = arguments_json(
        r#"{
            "root": "/repo",
            "config": "decimate.json",
            "coverage": "coverage/coverage-final.json",
            "min_invocations_hot": 25,
            "min_observation_volume": 100,
            "low_traffic_threshold": 0.05,
            "top": 7,
            "repo": "owner/repo"
        }"#,
    )?;

    let cli = cli_args_for_tool("get_hot_paths", &args)?;

    assert_eq!(
        cli[..5],
        ["decimate", "coverage", "analyze", "--format", "json"]
    );
    assert_pair(&cli, "--root", "/repo");
    assert_pair(&cli, "--config", "decimate.json");
    assert_pair(&cli, "--runtime-coverage", "coverage/coverage-final.json");
    assert_pair(&cli, "--min-invocations-hot", "25");
    assert_pair(&cli, "--min-observation-volume", "100");
    assert_pair(&cli, "--low-traffic-threshold", "0.05");
    assert_pair(&cli, "--top", "7");
    assert_pair(&cli, "--repo", "owner/repo");

    Ok(())
}

#[test]
fn impact_tools_map_read_only_reports() -> Result<(), String> {
    let impact = cli_args_for_tool("impact", &arguments_json(r#"{ "root": "/repo" }"#)?)?;
    let impact_all = cli_args_for_tool(
        "impact_all",
        &arguments_json(r#"{ "sort": "surfaced", "limit": 5 }"#)?,
    )?;

    assert_eq!(
        impact,
        [
            "decimate", "impact", "--format", "json", "--quiet", "--root", "/repo"
        ]
    );
    assert_eq!(
        impact_all,
        [
            "decimate", "impact", "--format", "json", "--quiet", "--all", "--sort", "surfaced",
            "--limit", "5"
        ]
    );

    Ok(())
}

#[test]
fn fix_tools_map_preview_and_confirmed_apply() -> Result<(), String> {
    let preview_args = arguments_json(
        r#"{
            "root": "/repo",
            "entry": ["lib/main.dart"],
            "file": ["lib/dead.dart"],
            "production": true,
            "action": ["delete-file"],
            "no_create_config": true
        }"#,
    )?;
    let apply_args = arguments_json(
        r#"{
            "root": "/repo",
            "entry": ["lib/main.dart"],
            "action": ["delete-file"],
            "yes": true
        }"#,
    )?;

    let preview = cli_args_for_tool("fix_preview", &preview_args)?;
    let apply = cli_args_for_tool("fix_apply", &apply_args)?;

    assert_eq!(preview[..4], ["decimate", "fix", "--format", "json"]);
    assert_pair(&preview, "--root", "/repo");
    assert_pair(&preview, "--entry", "lib/main.dart");
    assert_pair(&preview, "--file", "lib/dead.dart");
    assert_flag(&preview, "--production");
    assert_pair(&preview, "--action", "delete-file");
    assert_flag(&preview, "--dry-run");

    assert_eq!(apply[..4], ["decimate", "fix", "--format", "json"]);
    assert_pair(&apply, "--root", "/repo");
    assert_pair(&apply, "--entry", "lib/main.dart");
    assert_pair(&apply, "--action", "delete-file");
    assert_flag(&apply, "--yes");

    Ok(())
}

#[test]
fn fix_apply_requires_explicit_yes_true() -> Result<(), String> {
    let error = cli_args_for_tool(
        "fix_apply",
        &arguments_json(r#"{ "root": "/repo", "yes": false }"#)?,
    )
    .err()
    .ok_or_else(|| "expected yes rejection".to_owned())?;

    assert_eq!(error, "fix_apply requires yes: true");
    Ok(())
}

#[test]
fn audit_maps_baselines_and_analysis_knobs() -> Result<(), String> {
    let args = arguments_json(
        r#"{
            "base": "origin/main",
            "dead_code_baseline": "dead.json",
            "health_baseline": "health.json",
            "dupes_baseline": "dupes.json",
            "private_type_leaks": true,
            "boundary": ["lib/domain:lib/ui"],
            "min_score": 75,
            "brief": true,
            "max_decisions": 7
        }"#,
    )?;

    let cli = cli_args_for_tool("audit", &args)?;

    assert_eq!(cli[..4], ["decimate", "audit", "--format", "json"]);
    assert_pair(&cli, "--base", "origin/main");
    assert_pair(&cli, "--dead-code-baseline", "dead.json");
    assert_pair(&cli, "--health-baseline", "health.json");
    assert_pair(&cli, "--dupes-baseline", "dupes.json");
    assert_flag(&cli, "--private-type-leaks");
    assert_pair(&cli, "--boundary", "lib/domain:lib/ui");
    assert_pair(&cli, "--min-score", "75");
    assert_flag(&cli, "--brief");
    assert_pair(&cli, "--max-decisions", "7");

    Ok(())
}

#[test]
fn read_only_mcp_tools_reject_write_flags() -> Result<(), String> {
    let args = arguments_json(r#"{ "save_baseline": "baseline.json" }"#)?;

    let error = cli_args_for_tool("analyze", &args)
        .err()
        .ok_or_else(|| "expected save_baseline rejection".to_owned())?;

    assert!(error.contains("does not accept argument save_baseline"));
    Ok(())
}

fn arguments(value: &Value) -> Result<Map<String, Value>, String> {
    value
        .as_object()
        .cloned()
        .ok_or_else(|| "test arguments must be an object".to_owned())
}

fn arguments_json(source: &str) -> Result<Map<String, Value>, String> {
    let value = serde_json::from_str::<Value>(source).map_err(|error| error.to_string())?;
    arguments(&value)
}

fn assert_flag(cli: &[String], flag: &str) {
    assert!(
        cli.iter().any(|arg| arg == flag),
        "expected {flag} in {cli:?}"
    );
}

fn assert_pair(cli: &[String], flag: &str, value: &str) {
    assert!(
        cli.windows(2)
            .any(|pair| pair[0] == flag && pair[1] == value),
        "expected {flag} {value} in {cli:?}"
    );
}
