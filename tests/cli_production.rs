use std::fs;

use decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn production_scope_uses_production_entries_only() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_workspace(&fixture)?;

    let (default_code, default_json) =
        run_json(&fixture, ["decimate", "check", "$ROOT", "--format", "json"])?;
    assert_eq!(default_code, 0);
    assert!(!has_dead_file(&default_json, "lib/src/test_only.dart"));

    let (production_code, production_json) = run_json(
        &fixture,
        [
            "decimate",
            "check",
            "$ROOT",
            "--format",
            "json",
            "--production",
        ],
    )?;
    assert_eq!(production_code, 1);
    assert!(has_dead_file(&production_json, "lib/src/test_only.dart"));
    assert!(has_dead_file(&production_json, "test/app_test.dart"));
    assert!(production_dead_files_are_not_auto_fixable(&production_json));

    Ok(())
}

#[test]
fn dead_code_production_reports_production_unreachable_files()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_workspace(&fixture)?;

    let (code, json) = run_json(
        &fixture,
        [
            "decimate",
            "dead-code",
            "$ROOT",
            "--format",
            "json",
            "--production",
        ],
    )?;

    assert_eq!(code, 1);
    assert_eq!(json["command"], "dead-code");
    assert!(has_dead_file(&json, "lib/src/test_only.dart"));
    assert!(production_dead_files_are_not_auto_fixable(&json));

    Ok(())
}

#[test]
fn trace_file_respects_production_reachability() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_workspace(&fixture)?;

    let (_, default_json) = run_json(
        &fixture,
        [
            "decimate",
            "trace-file",
            "$ROOT",
            "--format",
            "json",
            "--file",
            "lib/src/test_only.dart",
        ],
    )?;
    assert_eq!(default_json["reachable"], true);

    let (_, production_json) = run_json(
        &fixture,
        [
            "decimate",
            "trace-file",
            "$ROOT",
            "--format",
            "json",
            "--file",
            "lib/src/test_only.dart",
            "--production",
        ],
    )?;
    assert_eq!(production_json["reachable"], false);
    assert_eq!(production_json["entry_point"], false);

    Ok(())
}

#[test]
fn trace_symbol_respects_production_reachability() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_workspace(&fixture)?;

    let (_, default_json) = run_json(
        &fixture,
        [
            "decimate",
            "trace-symbol",
            "$ROOT",
            "--format",
            "json",
            "--file",
            "lib/src/test_only.dart",
            "--symbol",
            "testOnly",
        ],
    )?;
    assert_eq!(default_json["reachable_file"], true);

    let (_, production_json) = run_json(
        &fixture,
        [
            "decimate",
            "trace-symbol",
            "$ROOT",
            "--format",
            "json",
            "--file",
            "lib/src/test_only.dart",
            "--symbol",
            "testOnly",
            "--production",
        ],
    )?;
    assert_eq!(production_json["reachable_file"], false);
    assert_eq!(production_json["entry_point"], false);

    Ok(())
}

#[test]
fn list_production_entry_points_reports_source() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_workspace(&fixture)?;

    let (_, json) = run_json(
        &fixture,
        [
            "decimate",
            "list",
            "$ROOT",
            "--format",
            "json",
            "--entry-points",
            "--production",
        ],
    )?;

    assert_eq!(json["summary"]["entry_points"], 1);
    assert_eq!(json["entry_points"][0]["path"], "lib/main.dart");
    assert_eq!(json["entry_points"][0]["source"], "production");

    Ok(())
}

#[test]
fn fix_production_does_not_plan_dead_file_deletes() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_workspace(&fixture)?;

    let (_, json) = run_json(
        &fixture,
        [
            "decimate",
            "fix",
            "$ROOT",
            "--format",
            "json",
            "--production",
            "--action",
            "delete-file",
        ],
    )?;

    assert_eq!(json["summary"]["planned"], 0);
    assert_eq!(json["fixes"].as_array().map(Vec::len), Some(0));

    Ok(())
}

#[test]
fn config_production_default_can_be_disabled_by_cli() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_workspace(&fixture)?;
    write(
        &fixture,
        ".decimaterc",
        "[cli]\nformat = \"json\"\nproduction = true\n",
    )?;

    let (production_code, production_json) = run_json(&fixture, ["decimate", "check", "$ROOT"])?;
    assert_eq!(production_code, 1);
    assert!(has_dead_file(&production_json, "lib/src/test_only.dart"));

    let (default_code, default_json) =
        run_json(&fixture, ["decimate", "check", "$ROOT", "--no-production"])?;
    assert_eq!(default_code, 0);
    assert!(!has_dead_file(&default_json, "lib/src/test_only.dart"));

    Ok(())
}

fn run_json<const N: usize>(
    fixture: &TempDir,
    args: [&str; N],
) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let root = fixture.path().to_str().unwrap_or(".");
    let args = args
        .into_iter()
        .map(|arg| {
            if arg == "$ROOT" {
                root.to_owned()
            } else {
                arg.to_owned()
            }
        })
        .collect::<Vec<_>>();
    let mut output = Vec::new();
    let code = run_from(args, &mut output)?;
    Ok((code, serde_json::from_slice::<Value>(&output)?))
}

fn has_dead_file(json: &Value, path: &str) -> bool {
    json["findings"].as_array().is_some_and(|findings| {
        findings
            .iter()
            .any(|finding| finding["kind"] == "dead-file" && finding["path"] == path)
    })
}

fn production_dead_files_are_not_auto_fixable(json: &Value) -> bool {
    json["findings"].as_array().is_some_and(|findings| {
        findings
            .iter()
            .filter(|finding| finding["kind"] == "dead-file")
            .all(|finding| {
                finding["safe_to_delete"] == false
                    && finding["actions"].as_array().is_some_and(|actions| {
                        actions.iter().all(|action| action["auto_fixable"] == false)
                    })
            })
    })
}

fn write_workspace(fixture: &TempDir) -> Result<(), std::io::Error> {
    write(fixture, "pubspec.yaml", "name: app\n")?;
    write(
        fixture,
        "lib/main.dart",
        "import 'src/live.dart';\nvoid main() { live(); }\n",
    )?;
    write(fixture, "lib/src/live.dart", "void live() {}\n")?;
    write(fixture, "lib/src/test_only.dart", "void testOnly() {}\n")?;
    write(
        fixture,
        "test/app_test.dart",
        "import '../lib/src/test_only.dart';\nvoid main() { testOnly(); }\n",
    )
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
