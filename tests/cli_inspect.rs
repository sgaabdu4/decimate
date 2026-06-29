use std::fs;

use decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn inspect_file_emits_evidence_bundle() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'barrel.dart';\nvoid main() { Api(); }\n",
    )?;
    write(&fixture, "lib/barrel.dart", "export 'src/api.dart';\n")?;
    write(&fixture, "lib/src/api.dart", "class Api {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "inspect",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
            "--file",
            "lib/barrel.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "decimate.inspect.v1");
    assert_eq!(json["kind"], "inspect");
    assert_eq!(json["target"]["kind"], "file");
    assert_eq!(json["target"]["path"], "lib/barrel.dart");
    assert_eq!(json["file_trace"]["schema_version"], "decimate.trace.v1");
    assert_eq!(
        json["file_trace"]["re_exports"][0]["to"],
        "lib/src/api.dart"
    );
    assert_eq!(json["scoped_report"]["kind"], "combined");
    assert_eq!(json["scoped_report"]["command"], "check");

    Ok(())
}

#[test]
fn inspect_symbol_emits_trace_and_scoped_report() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'barrel.dart';\nvoid main() { Api(); }\n",
    )?;
    write(&fixture, "lib/barrel.dart", "export 'src/api.dart';\n")?;
    write(
        &fixture,
        "lib/src/api.dart",
        "class Api {}\nclass Unused {}\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "inspect",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
            "--symbol",
            "lib/src/api.dart:Api",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["target"]["kind"], "symbol");
    assert_eq!(json["target"]["path"], "lib/src/api.dart");
    assert_eq!(json["target"]["symbol"], "Api");
    assert_eq!(json["symbol_trace"]["kind"], "trace-symbol");
    assert_eq!(
        json["symbol_trace"]["direct_references"][0]["path"],
        "lib/main.dart"
    );
    assert_eq!(json["file_trace"]["path"], "lib/src/api.dart");
    assert!(
        json["warnings"]
            .as_array()
            .is_some_and(|warnings| !warnings.is_empty())
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
