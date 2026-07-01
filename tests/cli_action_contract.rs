use std::fs;

use dart_decimate::{cli::run_from, report_schema};
use serde_json::{Value, json};
use tempfile::TempDir;

#[test]
fn report_actions_validate_against_schema_and_include_argv()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(&fixture, "lib/dead file.dart", "class DeadFile {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "dead-code",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let report = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);

    let schema = report_schema();
    let action = &report["findings"][0]["actions"][0];
    assert_action_matches_schema(&schema, action);
    assert_eq!(action["action"], "delete-file");
    assert_eq!(action["type"], "delete-file");
    assert_eq!(
        action["command"],
        "dart-decimate inspect --format json --file 'lib/dead file.dart'"
    );
    assert_eq!(
        action["argv"],
        json!([
            "dart-decimate",
            "inspect",
            "--format",
            "json",
            "--file",
            "lib/dead file.dart"
        ])
    );

    Ok(())
}

fn assert_action_matches_schema(schema: &Value, action: &Value) {
    let action_schema = &schema["$defs"]["finding_action"];
    let Some(required) = action_schema["required"].as_array() else {
        panic!("required action fields");
    };
    for field in required {
        let Some(field) = field.as_str() else {
            panic!("required field string");
        };
        assert!(action.get(field).is_some(), "missing action field {field}");
    }

    let Some(properties) = action_schema["properties"].as_object() else {
        panic!("action schema properties");
    };
    let Some(action) = action.as_object() else {
        panic!("action object");
    };
    for field in action.keys() {
        assert!(
            properties.contains_key(field),
            "action field {field} is absent from report schema"
        );
    }
}

fn write(fixture: &TempDir, path: &str, contents: &str) -> std::io::Result<()> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}
