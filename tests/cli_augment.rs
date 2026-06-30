use std::fs;

use decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn check_reports_unresolved_library_augment_edges() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/base_augment.dart",
        "library augment 'base.dart';\nvoid main() {}\n",
    )?;

    let mut output = Vec::new();
    let code = run_from(
        [
            "decimate",
            "check",
            fixture.path().to_str().unwrap_or_default(),
            "--format",
            "json",
            "--entry",
            "lib/base_augment.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "decimate/unresolved-dependency")
    }) else {
        panic!("unresolved augment finding");
    };
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["unresolved_dependencies"], 1);
    assert_eq!(finding["kind"], "unresolved-dependency");
    assert_eq!(finding["path"], "lib/base_augment.dart");
    assert_eq!(finding["edge"]["from"], "lib/base_augment.dart");
    assert_eq!(finding["edge"]["to"], "lib/base.dart");
    assert_eq!(finding["edge"]["kind"], "augment");
    assert_eq!(finding["safe_to_delete"], false);

    Ok(())
}

#[test]
fn report_schema_allows_augment_finding_edges() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(
        ["decimate", "report-schema", "--format", "json"],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert!(
        json["$defs"]["finding_edge"]["properties"]["kind"]["enum"]
            .as_array()
            .is_some_and(|values| values.iter().any(|value| value == "augment"))
    );

    Ok(())
}

#[test]
fn trace_file_reports_library_augment_edges() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/base.dart", "void main() {}\n")?;
    write(
        &fixture,
        "lib/base_augment.dart",
        "library augment 'base.dart';\n",
    )?;

    let mut output = Vec::new();
    let code = run_from(
        [
            "decimate",
            "trace-file",
            fixture.path().to_str().unwrap_or_default(),
            "--format",
            "json",
            "--entry",
            "lib/base.dart",
            "--file",
            "lib/base_augment.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["imports_from"][0]["from"], "lib/base_augment.dart");
    assert_eq!(json["imports_from"][0]["to"], "lib/base.dart");
    assert_eq!(json["imports_from"][0]["kind"], "augment");

    Ok(())
}

#[test]
fn base_library_reachability_keeps_augment_file_live() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'base.dart';\nvoid main() { Base(); }\n",
    )?;
    write(&fixture, "lib/base.dart", "class Base {}\n")?;
    write(
        &fixture,
        "lib/base_augment.dart",
        "library augment 'base.dart';\naugment class Base { void extra() {} }\n",
    )?;

    let mut output = Vec::new();
    let code = run_from(
        [
            "decimate",
            "check",
            fixture.path().to_str().unwrap_or_default(),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
            "--unused-files",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["dead_files"], 0);
    assert!(!json["findings"].as_array().is_some_and(|findings| {
        findings
            .iter()
            .any(|finding| finding["path"] == "lib/base_augment.dart")
    }));

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
