use std::fs;

use dart_decimate::cli::run_from;
use serde_json::{Value, json};
use tempfile::TempDir;

#[test]
fn check_reports_widget_lifecycle_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = lifecycle_fixture()?;
    write(
        &fixture,
        ".dart-decimaterc.json",
        r#"{ "rules": { "missing-context-mounted-after-await": "warn" } }"#,
    )?;
    let mut output = Vec::new();

    let code = run_check(&fixture, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["missing_context_mounted_after_await"], 1);

    let context = finding(&json, "missing-context-mounted-after-await")?;
    assert_eq!(
        context["rule_id"],
        "dart-decimate/missing-context-mounted-after-await"
    );
    assert_eq!(context["severity"], "warning");
    assert_eq!(context["path"], "lib/lifecycle.dart");
    assert_eq!(context["line"], 4);
    assert_eq!(context["safe_to_delete"], false);
    assert_eq!(context["files"], json!([]));
    assert_eq!(context["edge"], Value::Null);
    assert_eq!(context["actions"][0]["action"], "add-context-mounted-guard");
    assert_eq!(context["actions"][0]["auto_fixable"], false);
    assert_eq!(
        context["actions"][0]["target_symbol"],
        "LifecycleButton.save"
    );
    assert_eq!(
        context["actions"][0]["suppression_comment"],
        "// dart-decimate-ignore-next-line missing-context-mounted-after-await"
    );

    Ok(())
}

#[test]
fn widget_lifecycle_rules_can_error_or_turn_off() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = lifecycle_fixture()?;
    write(
        &fixture,
        ".dart-decimaterc.json",
        r#"{ "rules": { "use-build-context-synchronously": "error" } }"#,
    )?;
    let mut output = Vec::new();

    let code = run_check(&fixture, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(
        finding(&json, "missing-context-mounted-after-await")?["severity"],
        "error"
    );

    write(
        &fixture,
        ".dart-decimaterc.json",
        r#"{ "rules": { "missing-context-mounted-after-await": "off" } }"#,
    )?;
    output.clear();

    let code = run_check(&fixture, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["missing_context_mounted_after_await"], 0);
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

fn lifecycle_fixture() -> Result<TempDir, std::io::Error> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'lifecycle.dart';\nvoid main() { LifecycleButton(); CounterNotifier; }\n",
    )?;
    write(
        &fixture,
        "lib/lifecycle.dart",
        r"
class LifecycleButton extends StatelessWidget {
  Future<void> save(BuildContext context) async {
    await doWork();
    Navigator.of(context).pop();
  }
}

class CounterNotifier extends _$CounterNotifier {
  int build() => ref.watch(counterProvider);

  Future<void> save() async {
    await repo.save();
    final value = ref.watch(counterProvider);
    state = value;
  }
}
",
    )?;
    Ok(fixture)
}

fn run_check(
    fixture: &TempDir,
    output: &mut Vec<u8>,
) -> Result<i32, Box<dart_decimate::cli::CliError>> {
    run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
        ],
        output,
    )
    .map_err(Box::new)
}

fn finding<'json>(json: &'json Value, kind: &str) -> Result<&'json Value, std::io::Error> {
    json["findings"]
        .as_array()
        .and_then(|findings| findings.iter().find(|finding| finding["kind"] == kind))
        .ok_or_else(|| std::io::Error::other(format!("expected {kind} finding")))
}

fn write(fixture: &TempDir, path: &str, contents: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}
