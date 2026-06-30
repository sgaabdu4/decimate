use std::fs;

use decimate::cli::run_from;
use serde_json::{Value, json};
use tempfile::TempDir;

#[test]
fn check_reports_widget_lifecycle_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = lifecycle_fixture()?;
    let mut output = Vec::new();

    let code = run_check(&fixture, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["missing_context_mounted_after_await"], 1);
    assert_eq!(json["summary"]["missing_ref_mounted_after_await"], 1);
    assert_eq!(json["summary"]["riverpod_watch_in_notifier_methods"], 1);

    let context = finding(&json, "missing-context-mounted-after-await")?;
    assert_eq!(
        context["rule_id"],
        "decimate/missing-context-mounted-after-await"
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
        "// decimate-ignore-next-line missing-context-mounted-after-await"
    );

    let ref_guard = finding(&json, "missing-ref-mounted-after-await")?;
    assert_eq!(
        ref_guard["rule_id"],
        "decimate/missing-ref-mounted-after-await"
    );
    assert_eq!(ref_guard["line"], 13);
    assert_eq!(
        ref_guard["actions"][0]["target_symbol"],
        "CounterNotifier.save"
    );

    let watch = finding(&json, "riverpod-watch-in-notifier-method")?;
    assert_eq!(
        watch["rule_id"],
        "decimate/riverpod-watch-in-notifier-method"
    );
    assert_eq!(watch["line"], 14);
    assert_eq!(watch["actions"][0]["target_symbol"], "CounterNotifier.save");

    Ok(())
}

#[test]
fn widget_lifecycle_rules_can_error_or_turn_off() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = lifecycle_fixture()?;
    write(
        &fixture,
        ".decimaterc.json",
        r#"{ "rules": { "use-build-context-synchronously": "error", "ref-mounted-after-await": "error", "notifier-ref-watch": "error" } }"#,
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
    assert_eq!(
        finding(&json, "missing-ref-mounted-after-await")?["severity"],
        "error"
    );
    assert_eq!(
        finding(&json, "riverpod-watch-in-notifier-method")?["severity"],
        "error"
    );

    write(
        &fixture,
        ".decimaterc.json",
        r#"{ "rules": { "missing-context-mounted-after-await": "off", "missing-ref-mounted-after-await": "off", "riverpod-watch-in-notifier-method": "off" } }"#,
    )?;
    output.clear();

    let code = run_check(&fixture, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["missing_context_mounted_after_await"], 0);
    assert_eq!(json["summary"]["missing_ref_mounted_after_await"], 0);
    assert_eq!(json["summary"]["riverpod_watch_in_notifier_methods"], 0);
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

fn run_check(fixture: &TempDir, output: &mut Vec<u8>) -> Result<i32, Box<decimate::cli::CliError>> {
    run_from(
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
