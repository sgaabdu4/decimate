use std::fs;

use decimate::cli::run_from;
use serde_json::{Value, json};
use tempfile::TempDir;

#[test]
fn check_reports_private_widget_class() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = private_widget_fixture()?;
    write(
        &fixture,
        ".decimaterc.json",
        r#"{ "rules": { "private-widget-class": "warn" } }"#,
    )?;
    let mut output = Vec::new();

    let code = run_check(&fixture, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["private_widget_classes"], 1);

    let finding = private_widget_finding(&json);
    assert_eq!(finding["kind"], "private-widget-class");
    assert_eq!(finding["severity"], "warning");
    assert_eq!(finding["path"], "lib/widgets.dart");
    assert_eq!(finding["line"], 6);
    assert_eq!(finding["safe_to_delete"], false);
    assert_eq!(finding["files"], json!([]));
    assert_eq!(finding["edge"], Value::Null);
    assert_eq!(finding["actions"][0]["action"], "make-widget-public");
    assert_eq!(finding["actions"][0]["auto_fixable"], false);
    assert_eq!(finding["actions"][0]["target_symbol"], "_PrivateCard");
    assert_eq!(
        finding["actions"][0]["suppression_comment"],
        "// decimate-ignore-next-line private-widget-class"
    );
    assert_no_private_widget_for(&json, "_PublicStatefulState");
    assert_no_private_widget_for(&json, "_PublicConsumerState");

    Ok(())
}

#[test]
fn private_widget_class_rule_can_error_or_turn_off() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = private_widget_fixture()?;
    write(
        &fixture,
        ".decimaterc.json",
        r#"{ "rules": { "private-widget-class": "error" } }"#,
    )?;
    let mut output = Vec::new();

    let code = run_check(&fixture, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(private_widget_finding(&json)["severity"], "error");

    write(
        &fixture,
        ".decimaterc.json",
        r#"{ "rules": { "decimate/private-widget-class": "off" } }"#,
    )?;
    output.clear();

    let code = run_check(&fixture, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["private_widget_classes"], 0);
    assert_no_private_widget_for(&json, "_PrivateCard");

    Ok(())
}

#[test]
fn check_ignores_generated_private_widget_files() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'generated_widget.g.dart';\nvoid main() { _GeneratedWidget(); }\n",
    )?;
    write(
        &fixture,
        "lib/generated_widget.g.dart",
        r"class _GeneratedWidget extends StatelessWidget {
  Widget build(BuildContext context) => const SizedBox();
}
",
    )?;
    let mut output = Vec::new();

    let code = run_check(&fixture, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["private_widget_classes"], 0);
    assert_no_private_widget_for(&json, "_GeneratedWidget");

    Ok(())
}

fn private_widget_fixture() -> Result<TempDir, std::io::Error> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'widgets.dart';\nvoid main() { App(); PublicStateful(); PublicConsumer(); }\n",
    )?;
    write(
        &fixture,
        "lib/widgets.dart",
        r"class App extends StatelessWidget {
  const App({super.key});
  Widget build(BuildContext context) => const _PrivateCard();
}

class _PrivateCard extends StatelessWidget {
  const _PrivateCard({super.key});
  Widget build(BuildContext context) => const SizedBox();
}

class PublicStateful extends StatefulWidget {
  State<PublicStateful> createState() => _PublicStatefulState();
}
class _PublicStatefulState extends State<PublicStateful> {}

class PublicConsumer extends ConsumerStatefulWidget {
  ConsumerState<PublicConsumer> createState() => _PublicConsumerState();
}
class _PublicConsumerState extends ConsumerState<PublicConsumer> {}
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

fn private_widget_finding(json: &Value) -> &Value {
    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "decimate/private-widget-class")
    }) else {
        panic!("private widget class finding");
    };
    finding
}

fn assert_no_private_widget_for(json: &Value, class_name: &str) {
    assert!(json["findings"].as_array().is_some_and(|findings| {
        findings.iter().all(|finding| {
            finding["rule_id"] != "decimate/private-widget-class"
                || finding["actions"][0]["target_symbol"] != class_name
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
