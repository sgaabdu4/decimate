use std::fs;

use decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn dead_code_command_reports_unused_members_without_auto_fix()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/live.dart';\nvoid main() { runLive(); }\n",
    )?;
    write(
        &fixture,
        "lib/src/live.dart",
        "\
enum Mode { on, off }
class Live {
  void _unused() {}
}
void runLive() {
  final mode = Mode.on;
  print(mode);
  Live();
}
",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "dead-code",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(findings) = json["findings"].as_array() else {
        panic!("findings array");
    };
    let Some(enum_finding) = findings
        .iter()
        .find(|finding| finding["rule_id"] == "decimate/unused-enum-member")
    else {
        panic!("unused enum member finding");
    };
    let Some(class_finding) = findings
        .iter()
        .find(|finding| finding["rule_id"] == "decimate/unused-class-member")
    else {
        panic!("unused class member finding");
    };
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["unused_enum_members"], 1);
    assert_eq!(json["summary"]["unused_class_members"], 1);
    assert_eq!(enum_finding["kind"], "unused-enum-member");
    assert_eq!(enum_finding["path"], "lib/src/live.dart");
    assert_eq!(enum_finding["safe_to_delete"], false);
    assert_eq!(enum_finding["actions"][0]["action"], "review-member");
    assert_eq!(enum_finding["actions"][0]["auto_fixable"], false);
    assert_eq!(class_finding["kind"], "unused-class-member");
    assert_eq!(class_finding["path"], "lib/src/live.dart");
    assert_eq!(class_finding["safe_to_delete"], false);

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
