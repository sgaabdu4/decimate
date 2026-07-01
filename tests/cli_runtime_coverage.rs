use std::fs;

use dart_decimate::cli::{CliError, run_from};
use serde_json::{Value, json};
use tempfile::TempDir;

#[test]
fn coverage_setup_emits_non_mutating_json_plan() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_flutter_project(&fixture)?;
    let mut output = Vec::new();
    let root = fixture.path().to_string_lossy().into_owned();

    let code = run_from(
        [
            "dart-decimate",
            "coverage",
            "setup",
            root.as_str(),
            "--format",
            "json",
            "--non-interactive",
        ],
        &mut output,
    )?;

    let report = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(report["schema_version"], "dart-decimate.coverage.v1");
    assert_eq!(report["kind"], "coverage-setup");
    assert_eq!(report["command"], "coverage setup");
    assert_eq!(report["applied"], false);
    assert_eq!(report["non_interactive"], true);
    assert_eq!(report["summary"]["pubspec"], true);
    assert_eq!(report["summary"]["flutter"], true);
    assert_eq!(report["summary"]["dart_files"], 1);
    assert_eq!(report["files"][0]["path"], ".dart-decimaterc");
    assert_eq!(report["files"][0]["action"], "would-create");
    assert!(
        report["capture_commands"]
            .as_array()
            .is_some_and(|commands| commands
                .iter()
                .any(|command| command == "flutter test --coverage"))
    );
    assert!(!fixture.path().join(".dart-decimaterc").exists());

    Ok(())
}

#[test]
fn coverage_setup_yes_writes_defaults_once() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_flutter_project(&fixture)?;
    let root = fixture.path().to_string_lossy().into_owned();
    let mut first_output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "coverage",
            "setup",
            root.as_str(),
            "--yes",
            "--format",
            "json",
        ],
        &mut first_output,
    )?;

    let first = serde_json::from_slice::<Value>(&first_output)?;
    let config_path = fixture.path().join(".dart-decimaterc");
    let config = fs::read_to_string(&config_path)?;
    assert_eq!(code, 0);
    assert_eq!(first["files"][0]["action"], "created");
    assert!(config.contains("\"runtime_coverage\": \"coverage/coverage-final.json\""));

    let mut second_output = Vec::new();
    run_from(
        [
            "dart-decimate",
            "coverage",
            "setup",
            root.as_str(),
            "--yes",
            "--format",
            "json",
        ],
        &mut second_output,
    )?;
    let second = serde_json::from_slice::<Value>(&second_output)?;
    let config_after_second_run = fs::read_to_string(config_path)?;
    assert_eq!(second["files"][0]["action"], "unchanged");
    assert_eq!(
        config_after_second_run.matches("runtime_coverage").count(),
        1
    );

    Ok(())
}

#[test]
fn coverage_upload_inventory_dry_run_reports_sources() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(&fixture, "test/main_test.dart", "void main() {}\n")?;
    let root = fixture.path().to_string_lossy().into_owned();
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "coverage",
            "upload-inventory",
            root.as_str(),
            "--dry-run",
            "--repo",
            "owner/repo",
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let report = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(report["kind"], "coverage-upload-inventory");
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["repo"], "owner/repo");
    assert_eq!(report["summary"]["mode"], "inventory");
    assert_eq!(report["summary"]["files"], 1);
    assert_eq!(report["files"][0]["path"], "lib/main.dart");
    assert_eq!(report["files"][0]["kind"], "dart-source");
    assert_eq!(report["files"][0]["resolution_status"], "resolved");
    assert_eq!(report["files"][0]["mapping_quality"], "high");

    Ok(())
}

#[test]
fn coverage_upload_source_maps_dry_run_reports_files() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "dist/app.js.map", "{}\n")?;
    write(&fixture, "dist/chunks/app.js.map", "{}\n")?;
    write(&fixture, "dist/app.js", "console.log('x');\n")?;
    let root = fixture.path().to_string_lossy().into_owned();
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "coverage",
            "upload-source-maps",
            root.as_str(),
            "--dir",
            "dist",
            "--git-sha",
            "0123456789abcdef",
            "--repo",
            "owner/repo",
            "--dry-run",
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let report = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(report["kind"], "coverage-upload-source-maps");
    assert_eq!(report["git_sha"], "0123456789abcdef");
    assert_eq!(report["repo"], "owner/repo");
    assert_eq!(report["strip_path"], true);
    assert_eq!(report["summary"]["mode"], "source-maps");
    assert_eq!(report["summary"]["files"], 2);
    assert_eq!(report["files"][0]["kind"], "source-map");

    Ok(())
}

#[test]
fn coverage_upload_source_maps_validates_inputs() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let root = fixture.path().to_string_lossy().into_owned();

    let bad_sha = run_from(
        [
            "dart-decimate",
            "coverage",
            "upload-source-maps",
            root.as_str(),
            "--dir",
            "dist",
            "--git-sha",
            "not-a-sha",
            "--repo",
            "owner/repo",
            "--dry-run",
        ],
        &mut Vec::new(),
    );
    assert!(matches!(
        bad_sha,
        Err(CliError::CoverageUploadGitSha { .. })
    ));

    let missing_dir = run_from(
        [
            "dart-decimate",
            "coverage",
            "upload-source-maps",
            root.as_str(),
            "--dir",
            "missing",
            "--git-sha",
            "0123456789abcdef",
            "--repo",
            "owner/repo",
            "--dry-run",
        ],
        &mut Vec::new(),
    );
    assert!(matches!(
        missing_dir,
        Err(CliError::CoverageUploadDir { .. })
    ));

    Ok(())
}

#[test]
fn coverage_cloud_analyze_accepts_repo_then_errors() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    let root = fixture.path().to_string_lossy().into_owned();

    let result = run_from(
        [
            "dart-decimate",
            "coverage",
            "analyze",
            root.as_str(),
            "--cloud",
            "--repo",
            "owner/repo",
            "--format",
            "json",
        ],
        &mut Vec::new(),
    );

    assert!(matches!(result, Err(CliError::UnsupportedCoverageCloud)));

    Ok(())
}

#[test]
fn health_runtime_coverage_parses_istanbul_json() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() { print('hot'); }\n")?;
    write(
        &fixture,
        "lib/rare.dart",
        "void rare() { print('rare'); }\n",
    )?;
    write(
        &fixture,
        "lib/cold.dart",
        "void cold() { print('cold'); }\n",
    )?;
    write_istanbul_coverage(&fixture)?;
    let mut output = Vec::new();
    let root = fixture.path().to_string_lossy().into_owned();

    let code = run_from(
        [
            "dart-decimate",
            "health",
            root.as_str(),
            "--format",
            "json",
            "--runtime-coverage",
            "coverage/coverage-final.json",
            "--min-invocations-hot",
            "10",
            "--min-observation-volume",
            "10",
            "--low-traffic-threshold",
            "0.2",
        ],
        &mut output,
    )?;

    let report = serde_json::from_slice::<Value>(&output)?;
    let runtime = &report["runtime_coverage"];
    assert_eq!(code, 0);
    assert_eq!(runtime["verdict"], "pass");
    assert_eq!(runtime["summary"]["observed_files"], 2);
    assert_eq!(runtime["summary"]["total_invocations"], 101);
    assert_eq!(runtime["summary"]["hot_paths"], 1);
    assert_eq!(runtime["summary"]["low_traffic"], 1);
    assert_eq!(runtime["summary"]["coverage_unavailable"], 1);
    assert_eq!(runtime["summary"]["low_traffic_threshold"], json!(0.2));
    assert_eq!(runtime["provenance"]["source_format"], "istanbul");
    assert_eq!(runtime["provenance"]["capture_quality"], "high");
    assert_eq!(runtime["hot_paths"][0]["path"], "lib/main.dart");
    assert_eq!(runtime["hot_paths"][0]["symbol"], "main");
    assert_eq!(runtime["hot_paths"][0]["line"], 1);
    assert!(runtime["provenance"]["source_hash"].as_str().is_some());
    assert!(has_runtime_finding(runtime, "low-traffic", "lib/rare.dart"));
    assert!(has_runtime_finding(
        runtime,
        "coverage-unavailable",
        "lib/cold.dart"
    ));
    assert!(
        runtime["actionable"]["review_required"]
            .as_array()
            .is_some_and(|items| items.len() == 2)
    );

    Ok(())
}

#[test]
fn coverage_analyze_parses_istanbul_json() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() { print('hot'); }\n")?;
    write(
        &fixture,
        "lib/rare.dart",
        "void rare() { print('rare'); }\n",
    )?;
    write(
        &fixture,
        "lib/cold.dart",
        "void cold() { print('cold'); }\n",
    )?;
    write_istanbul_coverage(&fixture)?;
    let mut output = Vec::new();
    let root = fixture.path().to_string_lossy().into_owned();

    let code = run_from(
        [
            "dart-decimate",
            "coverage",
            "analyze",
            root.as_str(),
            "--format",
            "json",
            "--runtime-coverage",
            "coverage/coverage-final.json",
            "--min-invocations-hot",
            "10",
            "--min-observation-volume",
            "10",
            "--low-traffic-threshold",
            "0.2",
        ],
        &mut output,
    )?;

    let report = serde_json::from_slice::<Value>(&output)?;
    let runtime = &report["runtime_coverage"];
    assert_eq!(code, 0);
    assert_eq!(report["schema_version"], "dart-decimate.coverage.v1");
    assert_eq!(report["kind"], "runtime-coverage");
    assert_eq!(report["command"], "coverage analyze");
    assert_eq!(runtime["summary"]["observed_files"], 2);
    assert_eq!(runtime["summary"]["total_invocations"], 101);
    assert_eq!(runtime["summary"]["hot_paths"], 1);
    assert_eq!(runtime["summary"]["low_traffic"], 1);
    assert_eq!(runtime["summary"]["coverage_unavailable"], 1);
    assert_eq!(runtime["provenance"]["source_format"], "istanbul");
    assert_eq!(runtime["hot_paths"][0]["path"], "lib/main.dart");
    assert!(has_runtime_finding(runtime, "low-traffic", "lib/rare.dart"));
    assert!(has_runtime_finding(
        runtime,
        "coverage-unavailable",
        "lib/cold.dart"
    ));

    Ok(())
}

#[test]
fn coverage_analyze_requires_runtime_coverage_path() {
    let mut output = Vec::new();

    let result = run_from(
        ["dart-decimate", "coverage", "analyze", "--format", "json"],
        &mut output,
    );

    match result {
        Err(CliError::MissingRuntimeCoverage) => {}
        Ok(code) => panic!("expected missing runtime coverage error, got code {code}"),
        Err(error) => panic!("expected missing runtime coverage error, got {error}"),
    }
}

#[test]
fn health_runtime_coverage_parses_v8_directory() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() { print('hot'); }\n")?;
    write_v8_coverage(&fixture)?;
    let mut output = Vec::new();
    let root = fixture.path().to_string_lossy().into_owned();

    let code = run_from(
        [
            "dart-decimate",
            "health",
            root.as_str(),
            "--format",
            "json",
            "--runtime-coverage",
            "coverage/v8",
            "--min-invocations-hot",
            "10",
        ],
        &mut output,
    )?;

    let report = serde_json::from_slice::<Value>(&output)?;
    let runtime = &report["runtime_coverage"];
    assert_eq!(code, 0);
    assert_eq!(runtime["summary"]["observed_files"], 1);
    assert_eq!(runtime["summary"]["total_invocations"], 12);
    assert_eq!(runtime["summary"]["hot_paths"], 1);
    assert_eq!(runtime["provenance"]["source_format"], "v8");
    assert_eq!(runtime["provenance"]["capture_quality"], "medium");
    assert_eq!(runtime["hot_paths"][0]["path"], "lib/main.dart");
    assert_eq!(runtime["hot_paths"][0]["line"], 1);
    assert_eq!(runtime["hot_paths"][0]["source_map_confidence"], "resolved");

    Ok(())
}

fn write_flutter_project(fixture: &TempDir) -> Result<(), std::io::Error> {
    write(
        fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )?;
    write(fixture, "lib/main.dart", "void main() {}\n")
}

fn has_runtime_finding(runtime: &Value, kind: &str, path: &str) -> bool {
    runtime["findings"].as_array().is_some_and(|findings| {
        findings
            .iter()
            .any(|finding| finding["kind"] == kind && finding["path"] == path)
    })
}

fn write_istanbul_coverage(fixture: &TempDir) -> Result<(), Box<dyn std::error::Error>> {
    let main = fixture.path().join("lib/main.dart");
    let rare = fixture.path().join("lib/rare.dart");
    let coverage = json!({
        "main.dart": {
            "path": main.to_string_lossy().as_ref(),
            "statementMap": { "0": { "start": { "line": 1 }, "end": { "line": 1 } } },
            "s": { "0": 100 },
            "fnMap": { "0": { "name": "main", "decl": { "start": { "line": 1 } } } },
            "f": { "0": 100 }
        },
        "rare.dart": {
            "path": rare.to_string_lossy().as_ref(),
            "statementMap": { "0": { "start": { "line": 1 }, "end": { "line": 1 } } },
            "s": { "0": 1 },
            "fnMap": { "0": { "name": "rare", "decl": { "start": { "line": 1 } } } },
            "f": { "0": 1 }
        }
    });
    write(
        fixture,
        "coverage/coverage-final.json",
        &serde_json::to_string(&coverage)?,
    )?;
    Ok(())
}

fn write_v8_coverage(fixture: &TempDir) -> Result<(), Box<dyn std::error::Error>> {
    let main = fixture.path().join("lib/main.dart");
    let coverage = json!({
        "result": [{
            "url": format!("file://{}", main.display()),
            "functions": [{
                "functionName": "main",
                "ranges": [{ "startOffset": 0, "endOffset": 30, "count": 12 }],
                "isBlockCoverage": true
            }]
        }]
    });
    write(
        fixture,
        "coverage/v8/main.json",
        &serde_json::to_string(&coverage)?,
    )?;
    Ok(())
}

fn write(fixture: &TempDir, path: &str, content: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)
}
