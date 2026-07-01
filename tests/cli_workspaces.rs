use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn workspaces_command_emits_workspace_inventory_json() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_workspace(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "workspaces",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let workspaces = json["workspaces"].as_array().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "workspaces should be an array",
        )
    })?;
    let workspace_names = workspaces
        .iter()
        .map(|workspace| workspace["name"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();

    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "dart-decimate.list.v1");
    assert_eq!(json["command"], "workspaces");
    assert_eq!(json["summary"]["workspaces"], 2);
    assert!(workspace_names.contains(&"app"));
    assert!(workspace_names.contains(&"shared"));
    assert_eq!(json["files"].as_array().map(Vec::len), Some(0));
    assert_eq!(json["entry_points"].as_array().map(Vec::len), Some(0));
    assert_eq!(json["plugins"].as_array().map(Vec::len), Some(0));

    Ok(())
}

#[test]
fn workspaces_command_is_listed_in_agent_manifest() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(["dart-decimate", "schema", "--format", "json"], &mut output)?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let commands = json["commands"].as_array().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "commands should be an array",
        )
    })?;
    let command = commands
        .iter()
        .find(|command| command["name"] == "workspaces")
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "workspaces command should be listed",
            )
        })?;
    let flags = command["flags"].as_array().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "workspaces command should publish flags",
        )
    })?;

    assert_eq!(code, 0);
    assert_eq!(command["schema"], "dart-decimate.list.v1");
    assert!(flags.iter().any(|flag| flag == "--workspace"));
    assert!(flags.iter().any(|flag| flag == "--changed-workspaces"));
    assert!(!flags.iter().any(|flag| flag == "--files"));
    assert!(!flags.iter().any(|flag| flag == "--entry-points"));
    assert!(!flags.iter().any(|flag| flag == "--plugins"));
    assert!(!flags.iter().any(|flag| flag == "--boundaries"));

    Ok(())
}

fn write_workspace(fixture: &TempDir) -> Result<(), std::io::Error> {
    write(
        fixture,
        "pubspec.yaml",
        "name: app
workspace:
  - packages/shared
",
    )?;
    write(
        fixture,
        "lib/main.dart",
        "import 'package:shared/shared.dart';\nvoid main() { Shared(); }\n",
    )?;
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
