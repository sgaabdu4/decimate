use std::fs;

use decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn list_command_emits_project_metadata_json() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_workspace(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "list",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "decimate.list.v1");
    assert_eq!(json["tool"], "decimate");
    assert_eq!(json["command"], "list");
    assert_eq!(json["summary"]["files"], 3);
    assert_eq!(json["summary"]["entry_points"], 1);
    assert_eq!(json["summary"]["workspaces"], 2);
    assert_eq!(json["files"][0]["path"], "lib/main.dart");
    assert_eq!(json["entry_points"][0]["path"], "lib/main.dart");
    assert_eq!(json["entry_points"][0]["source"], "cli");
    assert!(
        json["workspaces"]
            .as_array()
            .is_some_and(|workspaces| workspaces.iter().any(|item| item["name"] == "shared"))
    );
    assert!(json["plugins"].as_array().is_some_and(|plugins| {
        plugins
            .iter()
            .any(|plugin| plugin["name"] == "flutter" && plugin["active"] == true)
    }));

    Ok(())
}

#[test]
fn list_command_filters_requested_sections() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_workspace(&fixture)?;
    write(
        &fixture,
        ".decimaterc",
        "[cli]\nformat = \"json\"\nentry = [\"lib/main.dart\"]\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "list",
            fixture.path().to_str().unwrap_or("."),
            "--entry-points",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["files"], 3);
    assert_eq!(json["entry_points"][0]["source"], "config");
    assert_eq!(json["files"].as_array().map(Vec::len), Some(0));
    assert_eq!(json["workspaces"].as_array().map(Vec::len), Some(0));
    assert_eq!(json["plugins"].as_array().map(Vec::len), Some(0));

    Ok(())
}

fn write_workspace(fixture: &TempDir) -> Result<(), std::io::Error> {
    write(
        fixture,
        "pubspec.yaml",
        "name: app
dependencies:
  flutter:
    sdk: flutter
workspace:
  - packages/shared
",
    )?;
    write(
        fixture,
        "lib/main.dart",
        "import 'src/live.dart';\nvoid main() { live(); }\n",
    )?;
    write(fixture, "lib/src/live.dart", "void live() {}\n")?;
    write(fixture, "packages/shared/pubspec.yaml", "name: shared\n")?;
    write(
        fixture,
        "packages/shared/lib/shared.dart",
        "class Shared {}\n",
    )
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
