use std::fs;

use decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn dead_code_command_reports_unused_type_aliases_separately()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/types.dart';\nvoid main() { run('ok'); }\n",
    )?;
    write(
        &fixture,
        "lib/src/types.dart",
        "\
typedef UsedAlias = String;
typedef UnusedAlias = int;

void run(UsedAlias value) {
  print(value);
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
    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "decimate/unused-type")
    }) else {
        panic!("unused type finding");
    };
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["unused_exports"], 0);
    assert_eq!(json["summary"]["unused_types"], 1);
    assert_eq!(finding["kind"], "unused-type");
    assert_eq!(finding["path"], "lib/src/types.dart");
    assert_eq!(finding["line"], 2);
    assert_eq!(finding["safe_to_delete"], true);
    assert_eq!(finding["actions"][0]["action"], "remove-declaration");
    assert_eq!(finding["actions"][0]["target_symbol"], "UnusedAlias");
    assert_eq!(finding["actions"][0]["target_end_line"], 2);
    assert_eq!(finding["actions"][0]["auto_fixable"], true);
    assert_eq!(finding["actions"][1]["action"], "trace-symbol");
    assert_eq!(finding["actions"][1]["target_symbol"], "UnusedAlias");
    assert_eq!(
        finding["actions"][1]["command"],
        "decimate inspect --format json --symbol lib/src/types.dart:UnusedAlias"
    );
    assert_eq!(
        finding["actions"][1]["suppression_comment"],
        "// decimate-ignore-next-line unused-type"
    );
    assert!(
        json["next_steps"]
            .as_array()
            .is_some_and(|steps| steps.iter().any(|step| step["id"] == "trace-unused-type"))
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
