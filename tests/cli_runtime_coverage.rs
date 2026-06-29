use std::fs;

use decimate::cli::{CliError, run_from};
use serde_json::{Value, json};
use tempfile::TempDir;

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
            "decimate",
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
            "decimate",
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
    assert_eq!(report["schema_version"], "decimate.coverage.v1");
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
        ["decimate", "coverage", "analyze", "--format", "json"],
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
            "decimate",
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
