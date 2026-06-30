use serde_json::{Map, Value};

const GLOBAL_KEYS: &[&str] = &["root", "config"];
const REPORT_SCOPE_KEYS: &[&str] = &[
    "entry",
    "file",
    "workspace",
    "changed_workspaces",
    "changed_since",
    "production",
];
const LIST_SCOPE_KEYS: &[&str] = &[
    "entry",
    "file",
    "workspace",
    "changed_workspaces",
    "production",
];
const BASELINE_KEYS: &[&str] = &[
    "baseline",
    "regression_baseline",
    "fail_on_regression",
    "tolerance",
];
const SYMBOL_KEYS: &[&str] = &["include_entry_exports", "private_type_leaks"];
const BOUNDARY_KEYS: &[&str] = &[
    "boundary",
    "boundary_coverage",
    "boundary_call",
    "policy_pack",
    "policy_violations",
];
const DUPLICATE_KEYS: &[&str] = &[
    "mode",
    "min_tokens",
    "min_lines",
    "min_occurrences",
    "top",
    "skip_local",
    "ignore_imports",
    "no_ignore_imports",
];
const HEALTH_KEYS: &[&str] = &[
    "max_cyclomatic",
    "max_cognitive",
    "max_crap",
    "coverage",
    "runtime_coverage",
    "min_invocations_hot",
    "min_observation_volume",
    "low_traffic_threshold",
    "coverage_gaps",
    "file_scores",
    "hotspots",
    "targets",
    "ownership",
    "complexity_breakdown",
    "min_score",
    "top",
];

pub(super) fn cli_args_for_tool(
    name: &str,
    args: &Map<String, Value>,
) -> Result<Vec<String>, String> {
    reject_unknown_args(name, args)?;
    match name {
        "analyze" => report_args("check", args, analyze_args),
        "project_info" => report_args("list", args, project_info_args),
        "inspect_target" => report_args("inspect", args, inspect_args),
        "trace_file" => report_args("trace-file", args, trace_file_args),
        "trace_export" => report_args("trace-symbol", args, trace_export_args),
        "trace_dependency" => report_args("trace-dependency", args, trace_dependency_args),
        "trace_clone" => report_args("trace-clone", args, trace_clone_args),
        "find_dupes" => report_args("dupes", args, dupes_args),
        "check_health" => report_args("health", args, health_args),
        "check_runtime_coverage"
        | "get_hot_paths"
        | "get_blast_radius"
        | "get_importance"
        | "get_cleanup_candidates" => coverage_analyze_args(args),
        "security_candidates" => report_args("security", args, security_args),
        "feature_flags" => report_args("flags", args, flags_args),
        "impact" => impact_args(args),
        "impact_all" => impact_all_args(args),
        "audit" => report_args("audit", args, audit_args),
        "decision_surface" => report_args("decision-surface", args, decision_surface_args),
        "decimate_explain" => explain_args(args),
        _ => Err(format!("unknown tool {name}")),
    }
}

fn reject_unknown_args(name: &str, args: &Map<String, Value>) -> Result<(), String> {
    let allowed = allowed_args(name)?;
    for key in args.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("{name} does not accept argument {key}"));
        }
    }
    Ok(())
}

fn allowed_args(name: &str) -> Result<Vec<&'static str>, String> {
    let mut allowed = GLOBAL_KEYS.to_vec();
    match name {
        "analyze" => {
            allowed.extend(["issue_types"]);
            allowed.extend(REPORT_SCOPE_KEYS);
            allowed.extend(BASELINE_KEYS);
            allowed.extend(SYMBOL_KEYS);
            allowed.extend(BOUNDARY_KEYS);
            allowed.extend(DUPLICATE_KEYS);
            allowed.extend(HEALTH_KEYS);
        }
        "project_info" => {
            allowed.extend([
                "files",
                "entry_points",
                "plugins",
                "boundaries",
                "workspaces",
            ]);
            allowed.extend(LIST_SCOPE_KEYS);
        }
        "inspect_target" => allowed.extend(["target", "file", "symbol"]),
        "trace_file" => allowed.extend(["file"]),
        "trace_export" => allowed.extend(["file", "symbol", "export_name"]),
        "trace_dependency" => allowed.extend(["dependency", "package_name"]),
        "trace_clone" => allowed.extend(["fingerprint"]),
        "find_dupes" => {
            allowed.extend(REPORT_SCOPE_KEYS);
            allowed.extend(BASELINE_KEYS);
            allowed.extend(DUPLICATE_KEYS);
        }
        "check_health" => {
            allowed.extend(REPORT_SCOPE_KEYS);
            allowed.extend(BASELINE_KEYS);
            allowed.extend(HEALTH_KEYS);
        }
        "check_runtime_coverage"
        | "get_hot_paths"
        | "get_blast_radius"
        | "get_importance"
        | "get_cleanup_candidates" => allowed.extend([
            "coverage",
            "min_invocations_hot",
            "min_observation_volume",
            "low_traffic_threshold",
            "top",
            "repo",
        ]),
        "security_candidates" => {
            allowed.extend(REPORT_SCOPE_KEYS);
            allowed.extend(BASELINE_KEYS);
            allowed.extend([
                "top",
                "surface",
                "gate",
                "diff_file",
                "ci",
                "fail_on_issues",
                "summary",
            ]);
        }
        "feature_flags" => {
            allowed.extend(REPORT_SCOPE_KEYS);
            allowed.extend(BASELINE_KEYS);
            allowed.extend(["top"]);
        }
        "impact" => return Ok(vec!["root"]),
        "impact_all" => return Ok(vec!["sort", "limit"]),
        "audit" => {
            allowed.extend(REPORT_SCOPE_KEYS);
            allowed.extend(SYMBOL_KEYS);
            allowed.extend(BOUNDARY_KEYS);
            allowed.extend(DUPLICATE_KEYS);
            allowed.extend(HEALTH_KEYS);
            allowed.extend([
                "base",
                "brief",
                "dead_code_baseline",
                "health_baseline",
                "dupes_baseline",
                "max_decisions",
            ]);
        }
        "decision_surface" => allowed.extend(["base", "max_decisions"]),
        "decimate_explain" => return Ok(vec!["issue_type", "rule_id"]),
        _ => return Err(format!("unknown tool {name}")),
    }
    allowed.sort_unstable();
    allowed.dedup();
    Ok(allowed)
}

fn report_args<F>(
    command: &str,
    args: &Map<String, Value>,
    append: F,
) -> Result<Vec<String>, String>
where
    F: FnOnce(&mut Vec<String>, &Map<String, Value>) -> Result<(), String>,
{
    let mut cli = json_command_args(&[command]);
    push_string_flag(&mut cli, args, "root", "--root")?;
    push_string_flag(&mut cli, args, "config", "--config")?;
    append(&mut cli, args)?;
    Ok(cli)
}

fn json_command_args(command: &[&str]) -> Vec<String> {
    let mut cli = vec!["decimate".to_owned()];
    cli.extend(command.iter().map(|part| (*part).to_owned()));
    cli.extend(["--format".to_owned(), "json".to_owned()]);
    cli
}

fn analyze_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_report_scope_args(cli, args)?;
    push_baseline_args(cli, args)?;
    push_symbol_args(cli, args)?;
    push_boundary_args(cli, args)?;
    push_duplicate_args(cli, args)?;
    push_health_args(cli, args)?;
    if let Some(issue_types) = args.get("issue_types") {
        for issue_type in array_strings(issue_types, "issue_types")? {
            cli.push(issue_filter_flag(issue_type)?);
        }
    }
    Ok(())
}

fn project_info_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_string_flags(cli, args, "entry", "--entry")?;
    push_string_flags(cli, args, "file", "--file")?;
    push_string_flags(cli, args, "workspace", "--workspace")?;
    push_string_flag(cli, args, "changed_workspaces", "--changed-workspaces")?;
    push_bool_mode(cli, args, "production", "--production", "--no-production")?;
    for (key, flag) in [
        ("files", "--files"),
        ("entry_points", "--entry-points"),
        ("plugins", "--plugins"),
        ("boundaries", "--boundaries"),
        ("workspaces", "--workspaces"),
    ] {
        push_bool_flag(cli, args, key, flag)?;
    }
    Ok(())
}

fn inspect_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    if let Some(target) = args.get("target") {
        let target = target
            .as_object()
            .ok_or_else(|| "target must be an object".to_owned())?;
        return match target.get("type").and_then(Value::as_str) {
            Some("file") => {
                reject_nested_unknown(target, &["type", "file"])?;
                push_required_string(cli, target, "file", "--file")
            }
            Some("symbol") => push_symbol_target(cli, target),
            _ => Err("target.type must be file or symbol".to_owned()),
        };
    }
    if args.contains_key("file") {
        push_required_string(cli, args, "file", "--file")?;
    } else if args.contains_key("symbol") {
        push_required_string(cli, args, "symbol", "--symbol")?;
    } else {
        return Err("inspect_target requires target, file, or symbol".to_owned());
    }
    Ok(())
}

fn trace_file_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_required_string(cli, args, "file", "--file")
}

fn trace_export_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_required_string(cli, args, "file", "--file")?;
    if let Some(export_name) = string_arg(args, "export_name")? {
        cli.extend(["--symbol".to_owned(), export_name]);
        return Ok(());
    }
    push_required_string(cli, args, "symbol", "--symbol")
}

fn trace_dependency_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    if let Some(package) = string_arg(args, "package_name")? {
        cli.extend(["--dependency".to_owned(), package]);
        return Ok(());
    }
    push_required_string(cli, args, "dependency", "--dependency")
}

fn trace_clone_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_required_string(cli, args, "fingerprint", "--fingerprint")
}

fn dupes_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_report_scope_args(cli, args)?;
    push_baseline_args(cli, args)?;
    push_duplicate_args(cli, args)
}

fn push_duplicate_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_string_flag(cli, args, "mode", "--mode")?;
    push_number_flag(cli, args, "min_tokens", "--min-tokens")?;
    push_number_flag(cli, args, "min_lines", "--min-lines")?;
    push_number_flag(cli, args, "min_occurrences", "--min-occurrences")?;
    push_number_flag(cli, args, "top", "--top")?;
    for (key, flag) in [
        ("skip_local", "--skip-local"),
        ("ignore_imports", "--ignore-imports"),
        ("no_ignore_imports", "--no-ignore-imports"),
    ] {
        push_bool_flag(cli, args, key, flag)?;
    }
    Ok(())
}

fn health_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_report_scope_args(cli, args)?;
    push_baseline_args(cli, args)?;
    push_health_args(cli, args)
}

fn coverage_analyze_args(args: &Map<String, Value>) -> Result<Vec<String>, String> {
    let mut cli = json_command_args(&["coverage", "analyze"]);
    push_string_flag(&mut cli, args, "root", "--root")?;
    push_string_flag(&mut cli, args, "config", "--config")?;
    push_required_string(&mut cli, args, "coverage", "--runtime-coverage")?;
    push_number_flag(
        &mut cli,
        args,
        "min_invocations_hot",
        "--min-invocations-hot",
    )?;
    push_number_flag(
        &mut cli,
        args,
        "min_observation_volume",
        "--min-observation-volume",
    )?;
    push_float_flag(
        &mut cli,
        args,
        "low_traffic_threshold",
        "--low-traffic-threshold",
    )?;
    push_number_flag(&mut cli, args, "top", "--top")?;
    push_string_flag(&mut cli, args, "repo", "--repo")?;
    Ok(cli)
}

fn push_health_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_number_flag(cli, args, "max_cyclomatic", "--max-cyclomatic")?;
    push_number_flag(cli, args, "max_cognitive", "--max-cognitive")?;
    push_number_flag(cli, args, "max_crap", "--max-crap")?;
    push_string_flag(cli, args, "coverage", "--coverage")?;
    push_string_flag(cli, args, "runtime_coverage", "--runtime-coverage")?;
    push_number_flag(cli, args, "min_invocations_hot", "--min-invocations-hot")?;
    push_number_flag(
        cli,
        args,
        "min_observation_volume",
        "--min-observation-volume",
    )?;
    push_float_flag(
        cli,
        args,
        "low_traffic_threshold",
        "--low-traffic-threshold",
    )?;
    for (key, flag) in [
        ("coverage_gaps", "--coverage-gaps"),
        ("file_scores", "--file-scores"),
        ("hotspots", "--hotspots"),
        ("targets", "--targets"),
        ("ownership", "--ownership"),
        ("complexity_breakdown", "--complexity-breakdown"),
    ] {
        push_bool_flag(cli, args, key, flag)?;
    }
    push_number_flag(cli, args, "min_score", "--min-score")?;
    push_number_flag(cli, args, "top", "--top")?;
    Ok(())
}

fn security_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_report_scope_args(cli, args)?;
    push_baseline_args(cli, args)?;
    push_number_flag(cli, args, "top", "--top")?;
    push_bool_flag(cli, args, "surface", "--surface")?;
    push_string_flag(cli, args, "gate", "--gate")?;
    push_string_flag(cli, args, "diff_file", "--diff-file")?;
    for (key, flag) in [
        ("ci", "--ci"),
        ("fail_on_issues", "--fail-on-issues"),
        ("summary", "--summary"),
    ] {
        push_bool_flag(cli, args, key, flag)?;
    }
    Ok(())
}

fn flags_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_report_scope_args(cli, args)?;
    push_baseline_args(cli, args)?;
    push_number_flag(cli, args, "top", "--top")?;
    Ok(())
}

fn impact_args(args: &Map<String, Value>) -> Result<Vec<String>, String> {
    let mut cli = json_command_args(&["impact"]);
    cli.push("--quiet".to_owned());
    push_string_flag(&mut cli, args, "root", "--root")?;
    Ok(cli)
}

fn impact_all_args(args: &Map<String, Value>) -> Result<Vec<String>, String> {
    let mut cli = json_command_args(&["impact"]);
    cli.extend(["--quiet".to_owned(), "--all".to_owned()]);
    push_string_flag(&mut cli, args, "sort", "--sort")?;
    push_number_flag(&mut cli, args, "limit", "--limit")?;
    Ok(cli)
}

fn audit_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_required_string(cli, args, "base", "--base")?;
    push_report_scope_args(cli, args)?;
    push_symbol_args(cli, args)?;
    push_boundary_args(cli, args)?;
    push_duplicate_args(cli, args)?;
    push_health_args(cli, args)?;
    for (key, flag) in [
        ("dead_code_baseline", "--dead-code-baseline"),
        ("health_baseline", "--health-baseline"),
        ("dupes_baseline", "--dupes-baseline"),
    ] {
        push_string_flag(cli, args, key, flag)?;
    }
    push_number_flag(cli, args, "max_decisions", "--max-decisions")?;
    push_bool_flag(cli, args, "brief", "--brief")?;
    Ok(())
}

fn decision_surface_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_required_string(cli, args, "base", "--base")?;
    push_number_flag(cli, args, "max_decisions", "--max-decisions")?;
    Ok(())
}

fn explain_args(args: &Map<String, Value>) -> Result<Vec<String>, String> {
    let issue_type = string_arg(args, "issue_type")?
        .or(string_arg(args, "rule_id")?)
        .ok_or_else(|| "decimate_explain requires issue_type".to_owned())?;
    Ok(vec![
        "decimate".to_owned(),
        "explain".to_owned(),
        issue_type,
        "--format".to_owned(),
        "json".to_owned(),
    ])
}

fn push_report_scope_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_string_flags(cli, args, "entry", "--entry")?;
    push_string_flags(cli, args, "file", "--file")?;
    push_string_flags(cli, args, "workspace", "--workspace")?;
    push_string_flag(cli, args, "changed_workspaces", "--changed-workspaces")?;
    push_string_flag(cli, args, "changed_since", "--changed-since")?;
    push_bool_mode(cli, args, "production", "--production", "--no-production")?;
    Ok(())
}

fn push_baseline_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_string_flag(cli, args, "baseline", "--baseline")?;
    push_string_flag(cli, args, "regression_baseline", "--regression-baseline")?;
    push_bool_flag(cli, args, "fail_on_regression", "--fail-on-regression")?;
    push_string_flag(cli, args, "tolerance", "--tolerance")?;
    Ok(())
}

fn push_symbol_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_bool_flag(
        cli,
        args,
        "include_entry_exports",
        "--include-entry-exports",
    )?;
    push_bool_flag(cli, args, "private_type_leaks", "--private-type-leaks")
}

fn push_boundary_args(cli: &mut Vec<String>, args: &Map<String, Value>) -> Result<(), String> {
    push_string_flags(cli, args, "boundary", "--boundary")?;
    push_bool_flag(cli, args, "boundary_coverage", "--boundary-coverage")?;
    push_string_flags(cli, args, "boundary_call", "--boundary-call")?;
    push_string_flags(cli, args, "policy_pack", "--policy-pack")?;
    push_bool_flag(cli, args, "policy_violations", "--policy-violations")
}

fn push_symbol_target(cli: &mut Vec<String>, target: &Map<String, Value>) -> Result<(), String> {
    reject_nested_unknown(target, &["type", "file", "symbol", "export_name"])?;
    let file = target
        .get("file")
        .and_then(Value::as_str)
        .ok_or_else(|| "symbol target requires file".to_owned())?;
    let symbol = target
        .get("export_name")
        .or_else(|| target.get("symbol"))
        .and_then(Value::as_str)
        .ok_or_else(|| "symbol target requires export_name or symbol".to_owned())?;
    cli.extend(["--symbol".to_owned(), format!("{file}:{symbol}")]);
    Ok(())
}

fn reject_nested_unknown(value: &Map<String, Value>, allowed: &[&str]) -> Result<(), String> {
    for key in value.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("target does not accept argument {key}"));
        }
    }
    Ok(())
}

fn push_string_flag(
    cli: &mut Vec<String>,
    args: &Map<String, Value>,
    key: &str,
    flag: &str,
) -> Result<(), String> {
    if let Some(value) = string_arg(args, key)? {
        cli.extend([flag.to_owned(), value]);
    }
    Ok(())
}

fn push_string_flags(
    cli: &mut Vec<String>,
    args: &Map<String, Value>,
    key: &str,
    flag: &str,
) -> Result<(), String> {
    match args.get(key) {
        Some(Value::String(value)) => cli.extend([flag.to_owned(), value.clone()]),
        Some(value @ Value::Array(_)) => {
            for value in array_strings(value, key)? {
                cli.extend([flag.to_owned(), value.to_owned()]);
            }
        }
        Some(_) => return Err(format!("{key} must be a string or string array")),
        None => {}
    }
    Ok(())
}

fn push_required_string(
    cli: &mut Vec<String>,
    args: &Map<String, Value>,
    key: &str,
    flag: &str,
) -> Result<(), String> {
    let value = string_arg(args, key)?.ok_or_else(|| format!("{key} is required"))?;
    cli.extend([flag.to_owned(), value]);
    Ok(())
}

fn push_number_flag(
    cli: &mut Vec<String>,
    args: &Map<String, Value>,
    key: &str,
    flag: &str,
) -> Result<(), String> {
    let Some(value) = args.get(key) else {
        return Ok(());
    };
    let Some(number) = value.as_u64() else {
        return Err(format!("{key} must be a non-negative integer"));
    };
    cli.extend([flag.to_owned(), number.to_string()]);
    Ok(())
}

fn push_float_flag(
    cli: &mut Vec<String>,
    args: &Map<String, Value>,
    key: &str,
    flag: &str,
) -> Result<(), String> {
    let Some(value) = args.get(key) else {
        return Ok(());
    };
    let Some(number) = value.as_f64() else {
        return Err(format!("{key} must be a non-negative number"));
    };
    if !number.is_finite() || number.is_sign_negative() {
        return Err(format!("{key} must be a non-negative number"));
    }
    cli.extend([flag.to_owned(), number.to_string()]);
    Ok(())
}

fn push_bool_flag(
    cli: &mut Vec<String>,
    args: &Map<String, Value>,
    key: &str,
    flag: &str,
) -> Result<(), String> {
    if bool_arg(args, key)? == Some(true) {
        cli.push(flag.to_owned());
    }
    Ok(())
}

fn push_bool_mode(
    cli: &mut Vec<String>,
    args: &Map<String, Value>,
    key: &str,
    true_flag: &str,
    false_flag: &str,
) -> Result<(), String> {
    match bool_arg(args, key)? {
        Some(true) => cli.push(true_flag.to_owned()),
        Some(false) => cli.push(false_flag.to_owned()),
        None => {}
    }
    Ok(())
}

fn string_arg(args: &Map<String, Value>, key: &str) -> Result<Option<String>, String> {
    match args.get(key) {
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(format!("{key} must be a string")),
        None => Ok(None),
    }
}

fn bool_arg(args: &Map<String, Value>, key: &str) -> Result<Option<bool>, String> {
    match args.get(key) {
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(format!("{key} must be a boolean")),
        None => Ok(None),
    }
}

fn array_strings<'value>(value: &'value Value, key: &str) -> Result<Vec<&'value str>, String> {
    let Some(values) = value.as_array() else {
        return Err(format!("{key} must be a string array"));
    };
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| format!("{key} entries must be strings"))
        })
        .collect()
}

fn issue_filter_flag(issue_type: &str) -> Result<String, String> {
    match issue_type {
        "unused-files" | "unused-file" => Ok("--unused-files".to_owned()),
        "unused-exports" | "unused-export" => Ok("--unused-exports".to_owned()),
        "unused-types" | "unused-type" => Ok("--unused-types".to_owned()),
        "unused-deps" | "unused-dependency" | "unused-dependencies" => {
            Ok("--unused-deps".to_owned())
        }
        "unlisted-deps" | "unlisted-dependency" | "unlisted-dependencies" => {
            Ok("--unlisted-deps".to_owned())
        }
        "private-src-import" | "private-src-imports" => Ok("--private-src-imports".to_owned()),
        "duplicate-exports" | "duplicate-export" => Ok("--duplicate-exports".to_owned()),
        "circular-deps" | "circular-dependency" => Ok("--circular-deps".to_owned()),
        "re-export-cycles" | "re-export-cycle" => Ok("--re-export-cycles".to_owned()),
        "boundary-violations" | "boundary-violation" => Ok("--boundary-violations".to_owned()),
        "policy-violations" | "policy-violation" => Ok("--policy-violations".to_owned()),
        "unused-enum-members" | "unused-enum-member" => Ok("--unused-enum-members".to_owned()),
        "unused-class-members" | "unused-class-member" => Ok("--unused-class-members".to_owned()),
        "unresolved-imports" | "unresolved-dependency" => Ok("--unresolved-imports".to_owned()),
        "stale-suppressions" | "stale-suppression" => Ok("--stale-suppressions".to_owned()),
        "unused-dependency-overrides" | "unused-dependency-override" => {
            Ok("--unused-dependency-overrides".to_owned())
        }
        "misconfigured-dependency-overrides" | "misconfigured-dependency-override" => {
            Ok("--misconfigured-dependency-overrides".to_owned())
        }
        "private-type-leak" | "private-type-leaks" => Ok("--private-type-leaks".to_owned()),
        _ => Err(format!("unsupported issue_type {issue_type}")),
    }
}
