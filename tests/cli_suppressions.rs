use std::fs;

use decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn check_reports_stale_inline_suppression() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "// decimate-ignore-next-line dead-file\nvoid main() {}\n",
    )?;

    let (code, json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["findings"], 1);
    assert_eq!(json["findings"][0]["rule_id"], "decimate/stale-suppression");
    assert_eq!(json["findings"][0]["kind"], "stale-suppression");
    assert_eq!(json["findings"][0]["path"], "lib/main.dart");
    assert_eq!(json["findings"][0]["line"], 1);
    assert_eq!(json["findings"][0]["safe_to_delete"], true);
    assert_eq!(
        json["findings"][0]["actions"][0]["action"],
        "remove-suppression"
    );
    assert_eq!(
        json["findings"][0]["actions"][0]["type"],
        "remove-suppression"
    );
    assert_eq!(
        json["findings"][0]["actions"][0]["target_path"],
        "lib/main.dart"
    );
    assert_eq!(
        json["findings"][0]["actions"][0]["suppression_comment"],
        "// decimate-ignore-next-line dead-file"
    );
    assert_eq!(json["findings"][0]["actions"][0]["auto_fixable"], true);

    Ok(())
}

#[test]
fn used_inline_suppression_is_not_reported_as_stale() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "// fallow-ignore-next-line feature-flag\nconst beta = bool.fromEnvironment('FEATURE_BETA');\n",
    )?;

    let (code, json) = run_json([
        "decimate",
        "flags",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["findings"], 0);
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn unused_member_findings_respect_fallow_suppression() -> Result<(), Box<dyn std::error::Error>> {
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
enum Mode {
  on,
  // fallow-ignore-next-line unused-enum-member
  off,
}
void runLive() { print(Mode.on); }
",
    )?;

    let (code, json) = run_json([
        "decimate",
        "dead-code",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["unused_enum_members"], 0);
    assert_eq!(json["summary"]["findings"], 0);

    Ok(())
}

#[test]
fn stale_suppression_rule_can_be_disabled() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        ".decimaterc.json",
        "{\"rules\":{\"stale-suppression\":\"off\"}}\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "// decimate-ignore-next-line dead-file\nvoid main() {}\n",
    )?;

    let (code, json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["findings"], 0);
    assert!(json["findings"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn missing_suppression_reason_reports_when_rule_enabled() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        ".decimaterc.json",
        "{\"rules\":{\"missing-suppression-reason\":\"warn\"}}\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "// decimate-ignore-next-line feature-flag\nconst beta = bool.fromEnvironment('FEATURE_BETA');\n",
    )?;

    let (code, json) = run_json([
        "decimate",
        "flags",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["missing_suppression_reasons"], 1);
    assert_eq!(
        json["findings"][0]["rule_id"],
        "decimate/missing-suppression-reason"
    );
    assert_eq!(json["findings"][0]["kind"], "missing-suppression-reason");
    assert_eq!(json["findings"][0]["severity"], "warning");
    assert_eq!(json["findings"][0]["safe_to_delete"], false);

    Ok(())
}

#[test]
fn documented_suppression_reason_is_accepted() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        ".decimaterc.json",
        "{\"rules\":{\"missing-suppression-reason\":\"error\"}}\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "// fallow-ignore-next-line feature-flag -- platform rollout flag\nconst beta = bool.fromEnvironment('FEATURE_BETA');\n",
    )?;

    let (code, json) = run_json([
        "decimate",
        "flags",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["missing_suppression_reasons"], 0);
    assert_eq!(json["summary"]["findings"], 0);

    Ok(())
}

fn run_json<const N: usize>(args: [&str; N]) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(args, &mut output)?;
    Ok((code, serde_json::from_slice::<Value>(&output)?))
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
