use std::fs;

use decimate::cli::run_from;
use serde_json::{Value, json};
use tempfile::TempDir;

#[test]
fn check_reports_unused_widget_field_formal() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = widget_fixture()?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "check",
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
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["unused_widget_params"], 2);

    let finding = unused_widget_param_finding(&json);
    assert_eq!(finding["kind"], "unused-widget-param");
    assert_eq!(finding["severity"], "warning");
    assert_eq!(finding["path"], "lib/widgets.dart");
    assert_eq!(finding["line"], 3);
    assert_eq!(finding["safe_to_delete"], false);
    assert_eq!(finding["files"], json!([]));
    assert_eq!(finding["edge"], Value::Null);
    assert_eq!(finding["actions"][0]["action"], "review-widget-param");
    assert_eq!(finding["actions"][0]["auto_fixable"], false);
    assert_eq!(
        finding["actions"][0]["target_symbol"],
        "UnusedFieldFormal.unused"
    );
    assert_eq!(
        finding["actions"][0]["suppression_comment"],
        "// decimate-ignore-next-line unused-widget-param"
    );
    assert_no_widget_param_for(&json, "title");
    assert_no_widget_param_for(&json, "count");
    assert_no_widget_param_for(&json, "label");
    assert_no_widget_param_for(&json, "key");
    assert_widget_param_for(&json, "UnusedExplicit.unused");
    assert_no_widget_param_for(&json, "usedExplicit");

    Ok(())
}

#[test]
fn check_ignores_generated_widget_files() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'generated_widget.g.dart';\nvoid main() { GeneratedWidget(unused: 'x'); }\n",
    )?;
    write(
        &fixture,
        "lib/generated_widget.g.dart",
        r"
class GeneratedWidget extends StatelessWidget {
  const GeneratedWidget({super.key, required this.unused});
  final String unused;
  Widget build(BuildContext context) => const SizedBox();
}
",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "check",
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
    assert_eq!(json["summary"]["unused_widget_params"], 0);
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn unused_widget_param_rule_can_error_or_turn_off() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = widget_fixture()?;
    write(
        &fixture,
        ".decimaterc.json",
        r#"{ "rules": { "unused-component-prop": "error" } }"#,
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(unused_widget_param_finding(&json)["severity"], "error");

    write(
        &fixture,
        ".decimaterc.json",
        r#"{ "rules": { "unused-widget-param": "off" } }"#,
    )?;
    output.clear();

    let code = run_from(
        [
            "decimate",
            "check",
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
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["unused_widget_params"], 0);
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

fn widget_fixture() -> Result<TempDir, std::io::Error> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        r"
import 'widgets.dart';

void main() {
  UsedInBuild(title: 'ok');
  UsedViaState(count: 1);
  Parent(label: 'child');
  UnusedFieldFormal(unused: 'x', used: 'y');
  UnusedExplicit(unused: 'x', usedExplicit: 'y');
}
",
    )?;
    write(
        &fixture,
        "lib/widgets.dart",
        r"
class UnusedFieldFormal extends StatelessWidget {
  const UnusedFieldFormal({super.key, required this.unused, required this.used});
  final String unused;
  final String used;
  Widget build(BuildContext context) => Text(used);
}

class UsedInBuild extends StatelessWidget {
  const UsedInBuild({super.key, required this.title});
  final String title;
  Widget build(BuildContext context) => Text('hello $title');
}

class UsedViaState extends StatefulWidget {
  const UsedViaState({super.key, required this.count});
  final int count;
  State<UsedViaState> createState() => _UsedViaStateState();
}

class _UsedViaStateState extends State<UsedViaState> {
  Widget build(BuildContext context) => Text('${widget.count}');
}

class Parent extends StatelessWidget {
  const Parent({super.key, required this.label});
  final String label;
  Widget build(BuildContext context) => Child(label: label);
}

class Child extends StatelessWidget {
  const Child({super.key, required this.label});
  final String label;
  Widget build(BuildContext context) => Text(label);
}

class UnusedExplicit extends StatelessWidget {
  const UnusedExplicit({
    super.key,
    required String unused,
    required String usedExplicit,
  })  : unused = unused,
        usedExplicit = usedExplicit;
  final String unused;
  final String usedExplicit;
  Widget build(BuildContext context) => Text(usedExplicit);
}
",
    )?;
    Ok(fixture)
}

fn unused_widget_param_finding(json: &Value) -> &Value {
    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "decimate/unused-widget-param")
    }) else {
        panic!("unused widget param finding");
    };
    finding
}

fn assert_no_widget_param_for(json: &Value, param: &str) {
    assert!(json["findings"].as_array().is_some_and(|findings| {
        findings.iter().all(|finding| {
            finding["rule_id"] != "decimate/unused-widget-param"
                || finding["actions"][0]["target_symbol"]
                    .as_str()
                    .is_none_or(|symbol| !symbol.ends_with(&format!(".{param}")))
        })
    }));
}

fn assert_widget_param_for(json: &Value, target_symbol: &str) {
    assert!(json["findings"].as_array().is_some_and(|findings| {
        findings.iter().any(|finding| {
            finding["rule_id"] == "decimate/unused-widget-param"
                && finding["actions"][0]["target_symbol"] == target_symbol
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
