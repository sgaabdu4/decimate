use std::fs;

use decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn check_reports_part_of_violations() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/model.dart",
        "part 'src/model.g.dart';\nvoid main() {}\n",
    )?;
    write(
        &fixture,
        "lib/src/model.g.dart",
        "part of '../other.dart';\n",
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
            "lib/model.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "decimate/part-of-violation")
    }) else {
        panic!("part-of-violation finding");
    };
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["part_of_violations"], 1);
    assert_eq!(finding["kind"], "part-of-violation");
    assert_eq!(finding["path"], "lib/src/model.g.dart");
    assert_eq!(finding["edge"]["kind"], "part");
    assert_eq!(finding["safe_to_delete"], false);

    Ok(())
}

#[test]
fn check_reports_duplicate_part_owner() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/a.dart",
        "part 'src/shared.dart';\nvoid main() {}\n",
    )?;
    write(&fixture, "lib/b.dart", "part 'src/shared.dart';\n")?;
    write(&fixture, "lib/src/shared.dart", "part of '../a.dart';\n")?;

    let mut output = Vec::new();
    let code = run_from(
        [
            "decimate",
            "check",
            fixture.path().to_str().unwrap_or_default(),
            "--format",
            "json",
            "--entry",
            "lib/a.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "decimate/part-of-violation")
    }) else {
        panic!("duplicate part owner finding");
    };
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["part_of_violations"], 1);
    assert_eq!(finding["kind"], "part-of-violation");
    assert_eq!(finding["path"], "lib/src/shared.dart");
    assert_eq!(finding["edge"]["from"], "lib/b.dart");
    assert_eq!(finding["edge"]["to"], "lib/src/shared.dart");
    assert_eq!(finding["edge"]["kind"], "part");
    assert_eq!(finding["safe_to_delete"], false);
    assert!(
        finding["message"]
            .as_str()
            .unwrap_or_default()
            .contains("already owned")
    );

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
