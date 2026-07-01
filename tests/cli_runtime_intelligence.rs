use std::fs;

use dart_decimate::cli::run_from;
use serde_json::{Value, json};
use tempfile::TempDir;

#[test]
fn health_runtime_coverage_emits_fallow_like_intelligence_arrays()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_runtime_project(&fixture)?;
    let first = run_runtime_health(&fixture)?;
    let second = run_runtime_health(&fixture)?;
    let runtime = &first["runtime_coverage"];

    assert_eq!(
        runtime["coverage_intelligence"][0]["id"],
        second["runtime_coverage"]["coverage_intelligence"][0]["id"]
    );
    assert_eq!(
        runtime["coverage_intelligence"][0]["kind"],
        "hot-path-touched"
    );
    assert_eq!(
        runtime["coverage_intelligence"][0]["action"],
        "review-runtime"
    );
    assert!(
        runtime["coverage_intelligence"]
            .as_array()
            .is_some_and(|rows| {
                rows.iter()
                    .any(|row| row["kind"] == "low-traffic" && row["path"] == "lib/rare.dart")
            })
    );
    assert!(
        runtime["coverage_intelligence"]
            .as_array()
            .is_some_and(|rows| {
                rows.iter().any(|row| {
                    row["kind"] == "coverage-unavailable" && row["path"] == "lib/cold.dart"
                })
            })
    );
    assert_eq!(runtime["blast_radius"][0]["path"], "lib/main.dart");
    assert_eq!(runtime["importance"][0]["path"], "lib/main.dart");
    assert!(array_contains(&runtime["signals"], "coverage-intelligence"));
    assert!(array_contains(&runtime["signals"], "blast-radius"));
    assert!(array_contains(&runtime["signals"], "importance"));

    Ok(())
}

#[test]
fn coverage_analyze_emits_same_runtime_intelligence_contract()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_runtime_project(&fixture)?;
    let root = fixture.path().to_string_lossy().into_owned();
    let mut output = Vec::new();

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
    assert!(
        runtime["coverage_intelligence"]
            .as_array()
            .is_some_and(|rows| !rows.is_empty())
    );
    assert!(
        runtime["blast_radius"]
            .as_array()
            .is_some_and(|rows| !rows.is_empty())
    );
    assert!(
        runtime["importance"]
            .as_array()
            .is_some_and(|rows| !rows.is_empty())
    );

    Ok(())
}

#[test]
fn runtime_coverage_importance_is_traffic_weighted() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(&fixture, "lib/checkout.dart", "void checkout() {}\n")?;
    write(&fixture, "lib/rare.dart", "void rare() {}\n")?;
    write_istanbul(
        &fixture,
        &[
            ("lib/main.dart", "main", 90),
            ("lib/checkout.dart", "checkout", 9),
            ("lib/rare.dart", "rare", 1),
        ],
    )?;

    let report = run_runtime_health(&fixture)?;
    let Some(importance) = report["runtime_coverage"]["importance"].as_array() else {
        panic!("runtime importance array");
    };

    assert_eq!(importance[0]["path"], "lib/main.dart");
    assert_eq!(importance[0]["traffic_fraction"], json!(0.9));
    assert_eq!(importance[1]["path"], "lib/checkout.dart");
    assert_eq!(importance[1]["traffic_fraction"], json!(0.09));
    assert_eq!(importance[2]["path"], "lib/rare.dart");
    assert_eq!(importance[2]["traffic_fraction"], json!(0.01));

    Ok(())
}

#[test]
fn runtime_coverage_blast_radius_includes_graph_callers() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'feature/widget.dart';\nvoid main() { widget(); }\n",
    )?;
    write(
        &fixture,
        "lib/feature/widget.dart",
        "import '../service.dart';\nvoid widget() { service(); }\n",
    )?;
    write(&fixture, "lib/service.dart", "void service() {}\n")?;
    write_istanbul(&fixture, &[("lib/service.dart", "service", 20)])?;

    let report = run_runtime_health(&fixture)?;
    let blast = &report["runtime_coverage"]["blast_radius"][0];

    assert_eq!(blast["path"], "lib/service.dart");
    assert_eq!(blast["caller_count"], 1);
    assert_eq!(blast["callers"][0], "lib/feature/widget.dart");
    assert_eq!(blast["risk"], "medium");

    Ok(())
}

#[test]
fn runtime_coverage_flutter_smoke_ignores_tests_and_resolves_package_paths()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(&fixture, "test/widget_test.dart", "void main() {}\n")?;
    write_package_istanbul(&fixture, "package:app/main.dart", "main", 50)?;

    let report = run_runtime_health(&fixture)?;
    let runtime = &report["runtime_coverage"];

    assert_eq!(runtime["hot_paths"][0]["path"], "lib/main.dart");
    assert_eq!(runtime["hot_paths"][0]["source_map_confidence"], "resolved");
    assert!(
        runtime["importance"]
            .as_array()
            .is_some_and(|rows| { rows.iter().any(|row| row["path"] == "lib/main.dart") })
    );
    assert!(!runtime["findings"].as_array().is_some_and(|rows| {
        rows.iter()
            .any(|row| row["path"] == "test/widget_test.dart")
    }));

    Ok(())
}

#[test]
fn runtime_coverage_schema_requires_intelligence_arrays() -> Result<(), Box<dyn std::error::Error>>
{
    let mut output = Vec::new();
    let code = run_from(
        ["dart-decimate", "report-schema", "--format", "json"],
        &mut output,
    )?;

    let schema = serde_json::from_slice::<Value>(&output)?;
    let runtime = &schema["$defs"]["runtime_coverage"];
    assert_eq!(code, 0);
    assert!(array_contains(
        &runtime["required"],
        "coverage_intelligence"
    ));
    assert!(array_contains(&runtime["required"], "blast_radius"));
    assert!(array_contains(&runtime["required"], "importance"));
    assert_eq!(
        runtime["properties"]["coverage_intelligence"]["type"],
        "array"
    );
    assert_eq!(runtime["properties"]["blast_radius"]["type"], "array");
    assert_eq!(runtime["properties"]["importance"]["type"], "array");

    Ok(())
}

fn run_runtime_health(fixture: &TempDir) -> Result<Value, Box<dyn std::error::Error>> {
    let root = fixture.path().to_string_lossy().into_owned();
    let mut output = Vec::new();
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
    assert_eq!(code, 0);
    Ok(report)
}

fn write_runtime_project(fixture: &TempDir) -> Result<(), Box<dyn std::error::Error>> {
    write(fixture, "pubspec.yaml", "name: app\n")?;
    write(fixture, "lib/main.dart", "void main() { print('hot'); }\n")?;
    write(fixture, "lib/rare.dart", "void rare() { print('rare'); }\n")?;
    write(fixture, "lib/cold.dart", "void cold() { print('cold'); }\n")?;
    write_istanbul(
        fixture,
        &[("lib/main.dart", "main", 100), ("lib/rare.dart", "rare", 1)],
    )
}

fn write_istanbul(
    fixture: &TempDir,
    files: &[(&str, &str, usize)],
) -> Result<(), Box<dyn std::error::Error>> {
    let mut entries = serde_json::Map::new();
    for (path, symbol, invocations) in files {
        let file = fixture.path().join(path);
        entries.insert(
            (*path).to_owned(),
            json!({
                "path": file.to_string_lossy().as_ref(),
                "statementMap": { "0": { "start": { "line": 1 }, "end": { "line": 1 } } },
                "s": { "0": invocations },
                "fnMap": { "0": { "name": symbol, "decl": { "start": { "line": 1 } } } },
                "f": { "0": invocations }
            }),
        );
    }
    write(
        fixture,
        "coverage/coverage-final.json",
        &serde_json::to_string(&Value::Object(entries))?,
    )?;
    Ok(())
}

fn write_package_istanbul(
    fixture: &TempDir,
    path: &str,
    symbol: &str,
    invocations: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let coverage = json!({
        "main.dart": {
            "path": path,
            "statementMap": { "0": { "start": { "line": 1 }, "end": { "line": 1 } } },
            "s": { "0": invocations },
            "fnMap": { "0": { "name": symbol, "decl": { "start": { "line": 1 } } } },
            "f": { "0": invocations }
        }
    });
    write(
        fixture,
        "coverage/coverage-final.json",
        &serde_json::to_string(&coverage)?,
    )?;
    Ok(())
}

fn array_contains(value: &Value, expected: &str) -> bool {
    value
        .as_array()
        .is_some_and(|items| items.iter().any(|item| item == expected))
}

fn write(fixture: &TempDir, path: &str, content: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)
}
