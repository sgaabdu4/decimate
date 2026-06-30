use std::fs;

use decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn check_reports_test_only_and_unused_dev_dependencies() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  test: ^1.0.0\n\
dev_dependencies:\n  mockito: ^5.0.0\n",
    )?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(
        &fixture,
        "test/app_test.dart",
        "import 'package:test/test.dart';\nvoid main() {}\n",
    )?;

    let (code, json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
    ])?;

    let rule_ids = rule_ids(&json);
    let Some(test_only) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "decimate/test-only-dependency")
    }) else {
        panic!("test-only dependency finding");
    };
    let Some(unused_dev) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "decimate/unused-dev-dependency")
    }) else {
        panic!("unused dev dependency finding");
    };
    assert_eq!(code, 1);
    assert!(rule_ids.contains(&"decimate/test-only-dependency"));
    assert!(rule_ids.contains(&"decimate/unused-dev-dependency"));
    assert_eq!(json["summary"]["unused_dependencies"], 2);
    assert_eq!(json["summary"]["test_only_dependencies"], 1);
    assert_eq!(json["summary"]["unused_dev_dependencies"], 1);
    assert_eq!(test_only["kind"], "test-only-dependency");
    assert_eq!(test_only["actions"][0]["target_path"], "pubspec.yaml");
    assert_eq!(test_only["actions"][0]["target_dependency"], "test");
    assert_eq!(test_only["actions"][0]["config_key"], "dev_dependencies");
    assert_eq!(unused_dev["kind"], "unused-dev-dependency");
    assert_eq!(unused_dev["safe_to_delete"], true);
    assert_eq!(
        unused_dev["actions"][0]["action"],
        "remove-pubspec-dependency"
    );
    assert_eq!(unused_dev["actions"][0]["auto_fixable"], true);

    Ok(())
}

#[test]
fn check_marks_only_simple_unused_pub_dependencies_auto_fixable()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  scalar: ^1.0.0\n  local:\n    path: local\n  git_dep:\n    git:\n      url: https://example.com/repo.git\n  nested:\n    hosted: https://pub.dev\n    version: ^1.0.0\n  commented: ^1.0.0 # keep\n\
dev_dependencies:\n  scalar_dev: ^2.0.0\n  sdk_dep:\n    sdk: flutter\n",
    )?;
    write(&fixture, "local/pubspec.yaml", "name: local\n")?;
    write(&fixture, "local/lib/local.dart", "void local() {}\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;

    let (code, json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 1);
    assert_unused_pub_action(&json, "scalar", true, "remove-pubspec-dependency");
    assert_unused_pub_action(&json, "scalar_dev", true, "remove-pubspec-dependency");
    for dependency in ["local", "git_dep", "nested", "commented", "sdk_dep"] {
        assert_unused_pub_action(&json, dependency, false, "review-pubspec-dependency");
    }

    Ok(())
}

#[test]
fn check_reports_dev_dependency_used_from_runtime_as_wrong_section()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dev_dependencies:\n  collection: ^1.0.0\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package:collection/collection.dart';\nvoid main() {}\n",
    )?;

    let (code, json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
    ])?;

    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "decimate/unlisted-dependency")
    }) else {
        panic!("unlisted dependency finding");
    };
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["unlisted_dependencies"], 1);
    assert!(
        finding["message"]
            .as_str()
            .is_some_and(|message| message.contains("declares it only in dev_dependencies"))
    );
    assert_eq!(
        finding["actions"][0]["action"],
        "move-pubspec-dependency-to-dependencies"
    );
    assert_eq!(
        finding["actions"][0]["type"],
        "move-pubspec-dependency-to-dependencies"
    );
    assert_eq!(finding["actions"][0]["target_path"], "pubspec.yaml");
    assert_eq!(finding["actions"][0]["target_dependency"], "collection");
    assert_eq!(finding["actions"][0]["config_key"], "dependencies");

    Ok(())
}

#[test]
fn tooling_config_usage_counts_as_dependency_usage() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dev_dependencies:\n  build_runner: ^2.0.0\n",
    )?;
    write(
        &fixture,
        "build.yaml",
        "targets:\n  $default:\n    builders:\n      build_runner|combining_builder:\n        enabled: true\n",
    )?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;

    let (code, json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
    ])?;
    let (trace_code, trace_json) = run_json([
        "decimate",
        "trace-dependency",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--dependency",
        "build_runner",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["summary"]["unused_dependencies"], 0);
    assert_eq!(json["summary"]["unused_dev_dependencies"], 0);
    assert_eq!(json["summary"]["findings"], 0);
    assert_eq!(trace_code, 0);
    assert_eq!(trace_json["total_import_count"], 0);
    assert_eq!(trace_json["used_in_scripts"], true);
    assert_eq!(trace_json["is_used"], true);

    Ok(())
}

#[test]
fn config_rules_disable_specific_dependency_placement_findings()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".decimaterc.json",
        "{\"rules\":{\"test-only-dependency\":\"off\",\"unused-dev-dependency\":\"off\"}}\n",
    )?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  test: ^1.0.0\n\
dev_dependencies:\n  mockito: ^5.0.0\n",
    )?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(
        &fixture,
        "test/app_test.dart",
        "import 'package:test/test.dart';\nvoid main() {}\n",
    )?;

    let (code, json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["unused_dependencies"], 0);
    assert_eq!(json["summary"]["test_only_dependencies"], 0);
    assert_eq!(json["summary"]["unused_dev_dependencies"], 0);
    assert_eq!(json["summary"]["findings"], 0);
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn check_reports_stale_dependency_override_from_lockfile() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  http: ^1.0.0\n\
dependency_overrides:\n  stale: ^1.0.0\n",
    )?;
    write(
        &fixture,
        "pubspec.lock",
        "packages:\n  http:\n    dependency: \"direct main\"\n    description:\n      name: http\n    source: hosted\n    version: \"1.0.0\"\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package:http/http.dart';\nvoid main() {}\n",
    )?;

    let (code, json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
    ])?;

    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "decimate/unused-dependency-override")
    }) else {
        panic!("dependency override finding");
    };
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["unused_dependencies"], 1);
    assert_eq!(json["summary"]["dependency_overrides"], 1);
    assert_eq!(json["summary"]["unused_dependency_overrides"], 1);
    assert_eq!(json["summary"]["misconfigured_dependency_overrides"], 0);
    assert_eq!(finding["path"], "pubspec.yaml");
    assert_eq!(finding["kind"], "unused-dependency-override");
    assert_eq!(finding["severity"], "warning");
    assert_eq!(finding["safe_to_delete"], false);
    assert_eq!(
        finding["actions"][0]["action"],
        "review-unused-dependency-override"
    );
    assert_eq!(finding["actions"][0]["auto_fixable"], false);
    assert!(
        finding["message"]
            .as_str()
            .is_some_and(|message| message.contains("pubspec.lock"))
    );

    Ok(())
}

#[test]
fn check_reports_pubspec_overrides_dependency_override_path()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependency_overrides:\n  stale: ^1.0.0\n",
    )?;
    write(
        &fixture,
        "pubspec_overrides.yaml",
        "dependency_overrides:\n  patched: ^1.0.0\n",
    )?;
    write(&fixture, "pubspec.lock", "packages: {}\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;

    let (code, json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
    ])?;

    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "decimate/unused-dependency-override")
    }) else {
        panic!("dependency override finding");
    };
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["unused_dependency_overrides"], 1);
    assert_eq!(finding["path"], "pubspec_overrides.yaml");
    assert_eq!(finding["line"], 2);
    assert_eq!(
        finding["actions"][0]["target_path"],
        "pubspec_overrides.yaml"
    );
    assert_eq!(finding["actions"][0]["target_dependency"], "patched");

    Ok(())
}

#[test]
fn check_reports_misconfigured_dependency_override_from_pubspec_overrides_yaml()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "pubspec_overrides.yaml",
        "dependency_overrides:\n  Bad-Name: ^1.0.0\n  stale:\n",
    )?;
    write(&fixture, "pubspec.lock", "packages: {}\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;

    let (code, json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
    ])?;

    let override_findings = json["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|finding| finding["rule_id"] == "decimate/misconfigured-dependency-override")
        .collect::<Vec<_>>();
    assert_eq!(code, 1);
    assert_eq!(override_findings.len(), 2);
    assert_eq!(json["summary"]["misconfigured_dependency_overrides"], 2);
    assert!(
        override_findings
            .iter()
            .all(|finding| finding["path"] == "pubspec_overrides.yaml")
    );

    Ok(())
}

#[test]
fn check_marks_real_path_package_simple_scalar_even_after_local_path_dependency()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  local:\n    path: local\n  path: ^1.9.0\n",
    )?;
    write(&fixture, "local/pubspec.yaml", "name: local\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;

    let (code, json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
    ])?;

    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings.iter().find(|finding| {
            finding["rule_id"] == "decimate/unused-dependency"
                && finding["actions"][0]["target_dependency"] == "path"
        })
    }) else {
        panic!("unused path package finding");
    };
    assert_eq!(code, 1);
    assert_eq!(finding["line"], 5);
    assert_eq!(finding["safe_to_delete"], true);
    assert_eq!(finding["actions"][0]["action"], "remove-pubspec-dependency");

    Ok(())
}

#[test]
fn check_reports_misconfigured_dependency_override() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependency_overrides:\n  Bad-Name: ^1.0.0\n  stale:\n",
    )?;
    write(&fixture, "pubspec.lock", "packages: {}\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;

    let (code, json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
    ])?;

    let Some(findings) = json["findings"].as_array() else {
        panic!("findings array");
    };
    let override_findings = findings
        .iter()
        .filter(|finding| finding["rule_id"] == "decimate/misconfigured-dependency-override")
        .collect::<Vec<_>>();
    assert_eq!(code, 1);
    assert_eq!(override_findings.len(), 2);
    assert_eq!(json["summary"]["unused_dependencies"], 0);
    assert_eq!(json["summary"]["dependency_overrides"], 2);
    assert_eq!(json["summary"]["unused_dependency_overrides"], 0);
    assert_eq!(json["summary"]["misconfigured_dependency_overrides"], 2);
    assert!(override_findings.iter().any(|finding| {
        finding["message"]
            .as_str()
            .is_some_and(|message| message.contains("invalid package name"))
    }));
    assert!(override_findings.iter().any(|finding| {
        finding["message"]
            .as_str()
            .is_some_and(|message| message.contains("empty value"))
    }));

    Ok(())
}

#[test]
fn config_rules_disable_dependency_override_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".decimaterc.json",
        "{\"rules\":{\"dependency-override\":\"off\"}}\n",
    )?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependency_overrides:\n  stale: ^1.0.0\n  Bad-Name: ^1.0.0\n",
    )?;
    write(&fixture, "pubspec.lock", "packages: {}\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;

    let (code, json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["unused_dependencies"], 0);
    assert_eq!(json["summary"]["dependency_overrides"], 0);
    assert_eq!(json["summary"]["unused_dependency_overrides"], 0);
    assert_eq!(json["summary"]["misconfigured_dependency_overrides"], 0);
    assert_eq!(json["summary"]["findings"], 0);
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn config_ignores_dependency_override_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".decimaterc.json",
        "{\"ignoreDependencyOverrides\":[{\"package\":\"stale\"},{\"package\":\"Bad-Name\",\"source\":\"pubspec.yaml\"}]}\n",
    )?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependency_overrides:\n  stale: ^1.0.0\n  Bad-Name: ^1.0.0\n",
    )?;
    write(&fixture, "pubspec.lock", "packages: {}\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;

    let (code, json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["dependency_overrides"], 0);
    assert_eq!(json["summary"]["unused_dependency_overrides"], 0);
    assert_eq!(json["summary"]["misconfigured_dependency_overrides"], 0);
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn config_ignores_dependency_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".decimaterc.json",
        "{\"ignoreDependencies\":[\"http\",\"collection\"]}\n",
    )?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  http: ^1.0.0\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package:collection/collection.dart';\nvoid main() {}\n",
    )?;

    let (code, json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["unused_dependencies"], 0);
    assert_eq!(json["summary"]["unlisted_dependencies"], 0);
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

fn run_json<const N: usize>(args: [&str; N]) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(args, &mut output)?;
    Ok((code, serde_json::from_slice::<Value>(&output)?))
}

fn assert_unused_pub_action(json: &Value, dependency: &str, safe_to_delete: bool, action: &str) {
    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings.iter().find(|finding| {
            finding["actions"][0]["target_dependency"] == dependency
                && (finding["rule_id"] == "decimate/unused-dependency"
                    || finding["rule_id"] == "decimate/unused-dev-dependency")
        })
    }) else {
        panic!("unused dependency finding for {dependency}");
    };
    assert_eq!(finding["safe_to_delete"], safe_to_delete);
    assert_eq!(finding["actions"][0]["action"], action);
    assert_eq!(finding["actions"][0]["auto_fixable"], safe_to_delete);
}

fn rule_ids(json: &Value) -> Vec<&str> {
    json["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|finding| finding["rule_id"].as_str())
        .collect()
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
