use std::fs;

use tempfile::TempDir;

use super::*;

#[test]
fn scans_dart_files_in_parallel_and_skips_build_outputs() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'live.dart';\nvoid main() {}\n",
    )?;
    write(&fixture, "lib/live.dart", "class Live {}\n")?;
    write(&fixture, "build/generated.dart", "class Generated {}\n")?;

    let project = scan_project(fixture.path())?;

    assert_eq!(project.files.len(), 2);
    assert_eq!(project.graph.node_count(), 2);
    assert_eq!(project.graph.edge_count(), 1);

    Ok(())
}

#[test]
fn scans_local_path_dependencies_for_graph_resolution() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "app/pubspec.yaml",
        "name: app\ndependencies:\n  shared:\n    path: ../shared\n",
    )?;
    write(
        &fixture,
        "app/lib/main.dart",
        "import 'package:shared/shared.dart';\nvoid main() {}\n",
    )?;
    write(&fixture, "shared/pubspec.yaml", "name: shared\n")?;
    write(&fixture, "shared/lib/shared.dart", "class Shared {}\n")?;

    let project = scan_project(fixture.path().join("app"))?;

    assert_eq!(project.files.len(), 2);
    assert_eq!(project.graph.edge_count(), 1);
    assert!(project.graph.unresolved().is_empty());

    Ok(())
}

#[test]
fn scans_local_package_config_roots_for_graph_resolution() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "app/.dart_tool/package_config.json",
        r#"{
  "configVersion": 2,
  "packages": [
    {"name": "app", "rootUri": "../", "packageUri": "lib/"},
    {"name": "shared", "rootUri": "../../shared", "packageUri": "lib/"}
  ]
}
"#,
    )?;
    write(
        &fixture,
        "app/lib/main.dart",
        "import 'package:shared/shared.dart';\nvoid main() {}\n",
    )?;
    write(&fixture, "shared/lib/shared.dart", "class Shared {}\n")?;

    let project = scan_project(fixture.path().join("app"))?;

    assert_eq!(project.files.len(), 2);
    assert_eq!(project.graph.edge_count(), 1);
    assert!(project.graph.unresolved().is_empty());

    Ok(())
}

#[test]
fn does_not_scan_pub_cache_package_config_roots() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "app/.dart_tool/package_config.json",
        r#"{
  "configVersion": 2,
  "packages": [
    {"name": "app", "rootUri": "../", "packageUri": "lib/"},
    {"name": "hosted", "rootUri": "../../.pub-cache/hosted/hosted-1.0.0", "packageUri": "lib/"}
  ]
}
"#,
    )?;
    write(
        &fixture,
        "app/lib/main.dart",
        "import 'package:hosted/hosted.dart';\nvoid main() {}\n",
    )?;
    write(
        &fixture,
        ".pub-cache/hosted/hosted-1.0.0/lib/hosted.dart",
        "class Hosted {}\n",
    )?;

    let project = scan_project(fixture.path().join("app"))?;

    assert_eq!(project.files.len(), 1);
    assert_eq!(project.graph.edge_count(), 0);
    assert!(project.graph.unresolved().is_empty());

    Ok(())
}

#[test]
fn scans_pub_workspace_member_files() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\nworkspace:\n  - packages/*\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package:shared/shared.dart';\nvoid main() {}\n",
    )?;
    write(&fixture, "packages/shared/pubspec.yaml", "name: shared\n")?;
    write(
        &fixture,
        "packages/shared/lib/shared.dart",
        "class Shared {}\n",
    )?;

    let project = scan_project(fixture.path())?;

    assert_eq!(project.files.len(), 2);
    assert_eq!(project.graph.edge_count(), 1);
    assert!(project.graph.unresolved().is_empty());

    Ok(())
}

#[test]
fn scans_all_conditional_import_targets() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'io.dart' if (dart.library.html) 'web.dart';\nvoid main() {}\n",
    )?;
    write(&fixture, "lib/io.dart", "class Io {}\n")?;
    write(&fixture, "lib/web.dart", "class Web {}\n")?;

    let project = scan_project(fixture.path())?;

    assert_eq!(project.graph.edge_count(), 2);
    assert!(project.graph.unresolved().is_empty());

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
