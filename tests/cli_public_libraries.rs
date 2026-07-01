use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn check_uses_nested_non_src_library_files_as_default_entries()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/widgets/button.dart",
        "import '../src/theme.dart';\nclass Button {}\n",
    )?;
    write(&fixture, "lib/platform/io.dart", "class IoPlatform {}\n")?;
    write(&fixture, "lib/src/theme.dart", "class ThemeToken {}\n")?;
    write(&fixture, "lib/src/internal.dart", "class InternalOnly {}\n")?;

    let (code, json) = run_json(
        &fixture,
        ["dart-decimate", "check", "$ROOT", "--format", "json"],
    )?;

    assert_eq!(code, 1);
    assert_eq!(json["summary"]["dead_files"], 1);
    assert_eq!(dead_file_paths(&json), vec!["lib/src/internal.dart"]);

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

fn dead_file_paths(json: &Value) -> Vec<&str> {
    json["findings"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|finding| finding["rule_id"] == "dart-decimate/dead-file")
        .map(|finding| finding["path"].as_str().unwrap_or_default())
        .collect()
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
