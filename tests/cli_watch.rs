use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn watch_once_runs_check_and_exits() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'missing.dart';\nvoid main() {}\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "watch",
            fixture.path().to_str().unwrap_or("."),
            "--once",
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["schema_version"], "dart-decimate.report.v1");
    assert_eq!(json["command"], "check");
    assert_eq!(json["summary"]["unresolved_dependencies"], 1);

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
