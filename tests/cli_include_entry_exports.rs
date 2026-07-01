use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn dead_code_include_entry_exports_reports_entry_declarations()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_workspace(&fixture)?;

    let (_, default_json) = run_json(
        &fixture,
        [
            "dart-decimate",
            "dead-code",
            "$ROOT",
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
        ],
    )?;
    assert_eq!(default_json["summary"]["unused_exports"], 0);

    let (code, json) = run_json(
        &fixture,
        [
            "dart-decimate",
            "dead-code",
            "$ROOT",
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
            "--include-entry-exports",
        ],
    )?;

    assert_eq!(code, 1);
    assert_eq!(json["summary"]["unused_exports"], 2);
    let names = unused_export_messages(&json);
    assert!(names.iter().any(|message| message.contains("EntryOnly")));
    assert!(names.iter().any(|message| message.contains("helper")));
    assert!(!names.iter().any(|message| message.contains("main")));

    Ok(())
}

#[test]
fn check_include_entry_exports_reports_entry_declarations() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write_workspace(&fixture)?;

    let (code, json) = run_json(
        &fixture,
        [
            "dart-decimate",
            "check",
            "$ROOT",
            "--format",
            "json",
            "--include-entry-exports",
        ],
    )?;

    assert_eq!(code, 1);
    assert_eq!(json["summary"]["unused_exports"], 2);
    assert!(
        unused_export_messages(&json)
            .iter()
            .any(|message| message.contains("EntryOnly"))
    );

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

fn unused_export_messages(json: &Value) -> Vec<&str> {
    json["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|finding| finding["rule_id"] == "dart-decimate/unused-export")
        .map(|finding| finding["message"].as_str().unwrap_or_default())
        .collect()
}

fn write_workspace(fixture: &TempDir) -> Result<(), std::io::Error> {
    write(fixture, "pubspec.yaml", "name: app\n")?;
    write(
        fixture,
        "lib/main.dart",
        "void main() {}\nclass EntryOnly {}\nvoid helper() {}\n",
    )
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
