use std::fs;

use decimate::cli::run_from;
use serde_json::{Value, json};
use tempfile::TempDir;

#[test]
fn check_reports_widget_top_level_function_boundary() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = widget_helper_fixture()?;
    let mut output = Vec::new();

    let code = run_check(&fixture, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["widget_top_level_functions"], 1);

    let finding = widget_helper_finding(&json);
    assert_eq!(finding["kind"], "widget-top-level-function-boundary");
    assert_eq!(finding["severity"], "warning");
    assert_eq!(finding["path"], "lib/screens/home_screen.dart");
    assert_eq!(finding["line"], 7);
    assert_eq!(finding["safe_to_delete"], false);
    assert_eq!(finding["files"], json!([]));
    assert_eq!(finding["edge"], Value::Null);
    assert_eq!(finding["actions"][0]["action"], "extract-widget-helper");
    assert_eq!(finding["actions"][0]["auto_fixable"], false);
    assert_eq!(finding["actions"][0]["target_symbol"], "_buildHeader");
    assert_eq!(
        finding["actions"][0]["suppression_comment"],
        "// decimate-ignore-next-line widget-top-level-function-boundary"
    );

    Ok(())
}

#[test]
fn widget_top_level_function_rule_can_error_or_turn_off() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = widget_helper_fixture()?;
    write(
        &fixture,
        ".decimaterc.json",
        r#"{ "rules": { "top-level-widget-helper": "error" } }"#,
    )?;
    let mut output = Vec::new();

    let code = run_check(&fixture, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(widget_helper_finding(&json)["severity"], "error");

    write(
        &fixture,
        ".decimaterc.json",
        r#"{ "rules": { "decimate/widget-top-level-function-boundary": "off" } }"#,
    )?;
    output.clear();

    let code = run_check(&fixture, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["widget_top_level_functions"], 0);
    assert_no_widget_helper_for(&json, "_buildHeader");

    Ok(())
}

#[test]
fn check_ignores_generated_test_and_dead_widget_helpers() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        ".decimaterc.json",
        r#"{ "rules": { "dead-file": "off" } }"#,
    )?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    for path in [
        "lib/generated_widget.g.dart",
        "test/widget_test.dart",
        "integration_test/app_test.dart",
        "test_driver/app.dart",
        "lib/dead_screen.dart",
    ] {
        write(
            &fixture,
            path,
            "Widget _buildSkipped(BuildContext context) => const SizedBox();\n",
        )?;
    }
    let mut output = Vec::new();

    let code = run_check(&fixture, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["widget_top_level_functions"], 0);
    assert_no_widget_helper_for(&json, "_buildSkipped");

    Ok(())
}

fn widget_helper_fixture() -> Result<TempDir, std::io::Error> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter:\n    sdk: flutter\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'screens/home_screen.dart';\nvoid main() { HomeScreen(); }\n",
    )?;
    write(
        &fixture,
        "lib/screens/home_screen.dart",
        r"import 'package:flutter/material.dart';

class HomeScreen extends StatelessWidget {
  Widget build(BuildContext context) => _buildHeader(context);
}

Widget _buildHeader(BuildContext context) => const SizedBox();
",
    )?;
    Ok(fixture)
}

fn run_check(fixture: &TempDir, output: &mut Vec<u8>) -> Result<i32, Box<dyn std::error::Error>> {
    Ok(run_from(
        [
            "decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
        ],
        output,
    )?)
}

fn widget_helper_finding(json: &Value) -> &Value {
    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "decimate/widget-top-level-function-boundary")
    }) else {
        panic!("widget top-level function boundary finding");
    };
    finding
}

fn assert_no_widget_helper_for(json: &Value, function_name: &str) {
    assert!(json["findings"].as_array().is_some_and(|findings| {
        findings.iter().all(|finding| {
            finding["rule_id"] != "decimate/widget-top-level-function-boundary"
                || finding["actions"][0]["target_symbol"] != function_name
        })
    }));
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
