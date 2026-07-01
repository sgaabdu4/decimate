use std::fs;
use std::process::Command;

use serde_json::Value;
use tempfile::TempDir;

#[test]
fn binary_emits_json_error_for_missing_entry_points() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/src/hidden.dart", "class Hidden {}\n")?;

    let output = Command::new(env!("CARGO_BIN_EXE_dart-decimate"))
        .args([
            "dead-code",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--summary",
            "--production",
        ])
        .output()?;

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let json = serde_json::from_slice::<Value>(&output.stdout)?;
    assert_eq!(json["error"], true);
    assert_eq!(json["exit_code"], 2);
    assert!(json["message"]
        .as_str()
        .is_some_and(|message| message.contains("no entry points provided")));

    Ok(())
}

#[test]
fn binary_emits_json_error_for_missing_runtime_coverage() -> Result<(), Box<dyn std::error::Error>>
{
    let output = Command::new(env!("CARGO_BIN_EXE_dart-decimate"))
        .args(["coverage", "analyze", "--format", "json"])
        .output()?;

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let json = serde_json::from_slice::<Value>(&output.stdout)?;
    assert_eq!(json["error"], true);
    assert_eq!(
        json["message"],
        "coverage analyze requires --runtime-coverage PATH"
    );
    assert_eq!(json["exit_code"], 2);

    Ok(())
}

#[test]
fn binary_emits_json_error_for_malformed_config() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        ".dart-decimaterc",
        "[health]\nmax_cyclomatic = \"low\"\n",
    )?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;

    let output = Command::new(env!("CARGO_BIN_EXE_dart-decimate"))
        .args([
            "health",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ])
        .output()?;

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let json = serde_json::from_slice::<Value>(&output.stdout)?;
    assert_eq!(json["error"], true);
    assert_eq!(json["exit_code"], 2);
    assert!(json["message"].as_str().is_some_and(|message| {
        message.contains(".dart-decimaterc") && message.contains("max_cyclomatic")
    }));

    Ok(())
}

#[test]
fn binary_emits_json_error_for_clap_errors() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new(env!("CARGO_BIN_EXE_dart-decimate"))
        .args(["dead-code", "--format", "json", "--unknown"])
        .output()?;

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let json = serde_json::from_slice::<Value>(&output.stdout)?;
    assert_eq!(json["error"], true);
    assert_eq!(json["exit_code"], 2);
    assert!(json["message"]
        .as_str()
        .is_some_and(|message| message.contains("unexpected argument '--unknown'")));

    Ok(())
}

#[test]
fn binary_emits_json_error_for_json_shortcut_clap_errors() -> Result<(), Box<dyn std::error::Error>>
{
    let output = Command::new(env!("CARGO_BIN_EXE_dart-decimate"))
        .args(["json", "--bad-flag"])
        .output()?;

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let json = serde_json::from_slice::<Value>(&output.stdout)?;
    assert_eq!(json["error"], true);
    assert_eq!(json["exit_code"], 2);
    assert!(json["message"]
        .as_str()
        .is_some_and(|message| message.contains("unexpected argument '--bad-flag'")));

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
