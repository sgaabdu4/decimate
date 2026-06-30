use std::fs;

use decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn config_discovery_applies_check_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, ".decimaterc", CHECK_CONFIG)?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'domain/service.dart';\nvoid main() { route([1]); }\n",
    )?;
    write(&fixture, "lib/ui/widget.dart", "class Widget {}\n")?;
    write_complex_source(&fixture, "lib/domain/service.dart")?;
    write(&fixture, "lib/dead.dart", "class Dead {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        ["decimate", "check", fixture.path().to_str().unwrap_or(".")],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["command"], "check");
    assert_eq!(json["summary"]["dead_files"], 1);
    assert_eq!(json["summary"]["boundary_violations"], 1);
    assert_eq!(json["summary"]["complex_functions"], 1);
    assert!(has_rule(&json, "decimate/dead-file"));
    assert!(has_rule(&json, "decimate/boundary-violation"));
    assert!(has_rule(&json, "decimate/high-complexity"));

    Ok(())
}

#[test]
fn cli_flags_override_config_defaults() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".decimaterc",
        "[cli]\nformat = \"json\"\n\n[health]\nmax_cyclomatic = 3\nmax_cognitive = 3\n",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_standalone_complex_source(&fixture, "lib/main.dart")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "health",
            fixture.path().to_str().unwrap_or("."),
            "--max-cyclomatic",
            "20",
            "--max-cognitive",
            "20",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["complex_functions"], 0);
    assert_eq!(json["summary"]["findings"], 0);

    Ok(())
}

#[test]
fn config_health_file_score_aliases_apply() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".decimaterc",
        "[cli]
format = \"json\"

[health]
fileScores = true
hotspots = true
minScore = 99
max_cyclomatic = 3
max_cognitive = 3
",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_standalone_complex_source(&fixture, "lib/main.dart")?;
    let mut output = Vec::new();

    let code = run_from(
        ["decimate", "health", fixture.path().to_str().unwrap_or(".")],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["file_scores"], 1);
    assert_eq!(json["summary"]["hotspots"], 1);
    assert_eq!(json["file_scores"][0]["path"], "lib/main.dart");
    assert_eq!(json["findings"][0]["rule_id"], "decimate/health-hotspot");

    Ok(())
}

#[test]
fn config_ignore_patterns_exclude_dead_files() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".decimaterc",
        "ignore_patterns = [\"lib/ignored/**\"]\n\n[cli]\nformat = \"json\"\nentry = [\"lib/main.dart\"]\n",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'live.dart';\nvoid main() { Live(); }\n",
    )?;
    write(&fixture, "lib/live.dart", "class Live {}\n")?;
    write(&fixture, "lib/ignored/dead.dart", "class Dead {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "dead-code",
            fixture.path().to_str().unwrap_or("."),
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["files"], 2);
    assert_eq!(json["summary"]["dead_files"], 0);
    assert_eq!(json["summary"]["findings"], 0);

    Ok(())
}

#[test]
fn config_include_entry_exports_reports_entry_declarations()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".decimaterc",
        "[cli]\nformat = \"json\"\nentry = [\"lib/main.dart\"]\nincludeEntryExports = true\n",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "void main() {}\nclass EntryOnly {}\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "dead-code",
            fixture.path().to_str().unwrap_or("."),
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["unused_exports"], 1);
    assert_eq!(json["findings"][0]["rule_id"], "decimate/unused-export");
    assert_eq!(json["findings"][0]["path"], "lib/main.dart");

    Ok(())
}

#[test]
fn config_schema_command_emits_json_schema() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        ["decimate", "config-schema", "--format", "json"],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "decimate.config.v1");
    assert_eq!(json["properties"]["production"]["type"], "boolean");
    assert_eq!(json["properties"]["includeEntryExports"]["type"], "boolean");
    assert_eq!(json["properties"]["boundaryCoverage"]["type"], "boolean");
    assert_boundary_config_schema(&json);
    assert_eq!(json["properties"]["boundaryCalls"]["type"], "array");
    assert_eq!(json["properties"]["rulePacks"]["type"], "array");
    assert_eq!(
        json["properties"]["cli"]["properties"]["production"]["type"],
        "boolean"
    );
    assert_eq!(
        json["properties"]["cli"]["properties"]["includeEntryExports"]["type"],
        "boolean"
    );
    assert_eq!(
        json["properties"]["cli"]["properties"]["boundaries"]["oneOf"][1]["type"],
        "object"
    );
    assert_eq!(
        json["properties"]["cli"]["properties"]["boundaryCalls"]["type"],
        "array"
    );
    assert_eq!(
        json["properties"]["cli"]["properties"]["rulePacks"]["type"],
        "array"
    );
    assert_eq!(json["properties"]["health"]["type"], "object");
    assert_eq!(
        json["properties"]["health"]["properties"]["coverage_gaps"]["type"],
        "boolean"
    );
    assert_eq!(
        json["properties"]["health"]["properties"]["fileScores"]["type"],
        "boolean"
    );
    assert_eq!(
        json["properties"]["health"]["properties"]["max_crap"]["minimum"],
        1
    );
    assert_eq!(
        json["properties"]["health"]["properties"]["runtime_coverage"]["type"],
        "string"
    );
    assert_eq!(
        json["properties"]["health"]["properties"]["lowTrafficThreshold"]["maximum"],
        1
    );
    assert_eq!(
        json["properties"]["health"]["properties"]["minScore"]["maximum"],
        100
    );
    assert_eq!(
        json["properties"]["dupes"]["properties"]["mode"]["enum"][3],
        "semantic"
    );
    assert_eq!(
        json["properties"]["ignoreDependencies"]["items"]["type"],
        "string"
    );
    assert_eq!(
        json["properties"]["ignoreDependencyOverrides"]["items"]["required"][0],
        "package"
    );
    assert_eq!(
        json["properties"]["ignoreDependencyOverrides"]["items"]["properties"]["source"]["type"][1],
        "null"
    );
    assert_eq!(
        json["properties"]["security"]["properties"]["categories"]["items"]["enum"][0],
        "hardcoded-secret"
    );

    Ok(())
}

fn assert_boundary_config_schema(json: &Value) {
    let boundary_object = &json["properties"]["boundaries"]["oneOf"][1];
    assert_eq!(boundary_object["type"], "object");
    for preset in ["layered", "hexagonal", "feature-sliced", "bulletproof"] {
        assert_array_contains(&boundary_object["properties"]["preset"]["enum"], preset);
    }
    assert_eq!(
        boundary_object["properties"]["coverage"]["properties"]["requireAllFiles"]["type"],
        "boolean"
    );
    assert_eq!(
        boundary_object["properties"]["coverage"]["properties"]["allowUnmatched"]["items"]["type"],
        "string"
    );
}

#[test]
fn report_schema_command_emits_json_schema() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        ["decimate", "report-schema", "--format", "json"],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_report_schema_envelope(&json);
    assert_report_schema_finding_kinds(&json);
    assert_report_schema_summary_fields(&json);
    assert_report_schema_action_contract(&json);

    Ok(())
}

fn assert_report_schema_envelope(json: &Value) {
    assert_eq!(json["schema_version"], "decimate.report.v1");
    assert_eq!(json["properties"]["kind"]["type"], "string");
    assert_eq!(json["properties"]["findings"]["type"], "array");
    assert_eq!(json["properties"]["file_scores"]["type"], "array");
    assert_eq!(json["properties"]["hotspots"]["type"], "array");
    assert_eq!(json["properties"]["refactoring_targets"]["type"], "array");
    assert_eq!(
        json["properties"]["runtime_coverage"]["$ref"],
        "#/$defs/runtime_coverage"
    );
    assert!(
        json["properties"]["command"]["enum"]
            .as_array()
            .is_some_and(|commands| commands.iter().any(|command| command == "audit"))
    );
    assert!(
        json["properties"]["command"]["enum"]
            .as_array()
            .is_some_and(|commands| commands.iter().all(|command| command != "trace-file"))
    );
}

fn assert_report_schema_finding_kinds(json: &Value) {
    for kind in [
        "security-candidate",
        "unused-type",
        "private-type-leak",
        "boundary-coverage",
        "boundary-call-violation",
        "policy-violation",
        "part-of-violation",
        "missing-suppression-reason",
        "unused-enum-member",
        "unused-class-member",
        "coverage-gap",
        "unused-dev-dependency",
        "test-only-dependency",
        "unused-dependency-override",
        "misconfigured-dependency-override",
        "high-crap-score",
        "health-hotspot",
        "refactoring-target",
    ] {
        assert_array_contains(
            &json["$defs"]["finding"]["properties"]["kind"]["enum"],
            kind,
        );
    }
}

fn assert_report_schema_summary_fields(json: &Value) {
    for field in [
        "unused_types",
        "private_type_leaks",
        "boundary_coverage",
        "boundary_call_violations",
        "policy_violations",
        "missing_suppression_reasons",
        "unused_dev_dependencies",
        "test_only_dependencies",
        "dependency_overrides",
        "unused_dependency_overrides",
        "misconfigured_dependency_overrides",
        "unused_class_members",
        "coverage_gaps",
        "crap_functions",
        "file_scores",
        "hotspots",
        "refactoring_targets",
    ] {
        assert_array_contains(&json["$defs"]["summary"]["required"], field);
    }
}

fn assert_array_contains(array: &Value, expected: &str) {
    assert!(
        array
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item == expected)),
        "expected array to contain {expected}"
    );
}

fn assert_report_schema_action_contract(json: &Value) {
    assert!(
        json["$defs"]["finding_action"]["required"]
            .as_array()
            .is_some_and(|fields| fields.iter().any(|field| field == "type"))
    );
    assert_eq!(
        json["$defs"]["finding_action"]["properties"]["command"]["type"],
        "string"
    );
    assert_eq!(
        json["$defs"]["finding_action"]["properties"]["argv"]["type"],
        "array"
    );
    assert_eq!(
        json["$defs"]["finding_action"]["properties"]["target_dependency"]["type"],
        "string"
    );
    assert_eq!(
        json["$defs"]["finding_action"]["properties"]["suppression_comment"]["type"],
        "string"
    );
}

#[test]
fn malformed_config_reports_error_before_scan() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".decimaterc",
        "[health]\nmax_cyclomatic = \"low\"\n",
    )?;
    let mut output = Vec::new();

    let error = match run_from(
        [
            "decimate",
            "health",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    ) {
        Ok(code) => panic!("malformed config should fail, got exit code {code}"),
        Err(error) => error,
    };

    let message = error.to_string();
    assert!(message.contains(".decimaterc"));
    assert!(message.contains("max_cyclomatic"));
    assert!(output.is_empty());

    Ok(())
}

#[test]
fn unknown_config_keys_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, ".decimaterc", "[dupes]\nmin_toknes = 10\n")?;
    let mut output = Vec::new();

    let error = match run_from(
        [
            "decimate",
            "dupes",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    ) {
        Ok(code) => panic!("unknown config key should fail, got exit code {code}"),
        Err(error) => error,
    };

    let message = error.to_string();
    assert!(message.contains(".decimaterc"));
    assert!(message.contains("min_toknes"));
    assert!(message.contains("unknown"));
    assert!(output.is_empty());

    Ok(())
}

#[test]
fn config_rules_warn_keep_findings_without_failing() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".decimaterc",
        "[cli]\nformat = \"json\"\nentry = [\"lib/main.dart\"]\n\n[rules]\nall = \"warn\"\n",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(&fixture, "lib/dead.dart", "class Dead {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        ["decimate", "check", fixture.path().to_str().unwrap_or(".")],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["findings"], 1);
    assert_eq!(json["findings"][0]["rule_id"], "decimate/dead-file");
    assert_eq!(json["findings"][0]["severity"], "warning");

    Ok(())
}

#[test]
fn config_rules_off_remove_findings_from_summary() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".decimaterc",
        "[cli]\nformat = \"json\"\nentry = [\"lib/main.dart\"]\n\n[rules]\nunused-files = \"off\"\n",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(&fixture, "lib/dead.dart", "class Dead {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        ["decimate", "check", fixture.path().to_str().unwrap_or(".")],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["dead_files"], 0);
    assert_eq!(json["summary"]["findings"], 0);
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn config_rules_disable_unused_class_member_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".decimaterc",
        "[cli]\nformat = \"json\"\nentry = [\"lib/main.dart\"]\n\n[rules]\nunused-class-member = \"off\"\n",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/live.dart';\nvoid main() { runLive(); }\n",
    )?;
    write(
        &fixture,
        "lib/src/live.dart",
        "class Live { void _unused() {} }\nvoid runLive() { Live(); }\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        ["decimate", "check", fixture.path().to_str().unwrap_or(".")],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["unused_class_members"], 0);
    assert_eq!(json["summary"]["findings"], 0);
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn config_rules_disable_unused_type_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".decimaterc",
        "[cli]\nformat = \"json\"\nentry = [\"lib/main.dart\"]\n\n[rules]\nunused-types = \"off\"\n",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/types.dart';\nvoid main() { run('ok'); }\n",
    )?;
    write(
        &fixture,
        "lib/src/types.dart",
        "\
typedef UsedAlias = String;
typedef UnusedAlias = int;
void run(UsedAlias value) { print(value); }
",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        ["decimate", "check", fixture.path().to_str().unwrap_or(".")],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["unused_types"], 0);
    assert_eq!(json["summary"]["findings"], 0);
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));
    assert!(json["next_steps"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn unknown_config_rules_are_rejected() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, ".decimaterc", "[rules]\nunused-fiels = \"off\"\n")?;
    let mut output = Vec::new();

    let error = match run_from(
        [
            "decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    ) {
        Ok(code) => panic!("unknown rule should fail, got exit code {code}"),
        Err(error) => error,
    };

    let message = error.to_string();
    assert!(message.contains("unused-fiels"));
    assert!(message.contains("unknown config rule"));
    assert!(output.is_empty());

    Ok(())
}

const CHECK_CONFIG: &str = "[cli]
format = \"json\"
entry = [\"lib/main.dart\"]
boundary = [\"lib/domain:lib/ui\"]

[health]
max_cyclomatic = 3
max_cognitive = 3
";

fn has_rule(json: &Value, rule_id: &str) -> bool {
    json["findings"]
        .as_array()
        .is_some_and(|findings| findings.iter().any(|finding| finding["rule_id"] == rule_id))
}

fn write_complex_source(fixture: &TempDir, path: &str) -> Result<(), std::io::Error> {
    write(
        fixture,
        path,
        r"import '../ui/widget.dart';

String route(List<int> items) {
  final widget = Widget();
  if (items.isEmpty) return widget.toString();
  for (final item in items) {
    if (item.isEven) return 'even';
  }
  return 'odd';
}
",
    )
}

fn write_standalone_complex_source(fixture: &TempDir, path: &str) -> Result<(), std::io::Error> {
    write(
        fixture,
        path,
        r"String route(List<int> items) {
  if (items.isEmpty) return 'none';
  for (final item in items) {
    if (item.isEven) return 'even';
  }
  return 'odd';
}
",
    )
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
