use std::fs;

use decimate::cli::run_from;
use serde_json::{Value, json};
use tempfile::TempDir;

#[test]
fn check_reports_manual_riverpod_provider() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = riverpod_fixture()?;
    let mut output = Vec::new();

    let code = run_check(&fixture, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["manual_riverpod_providers"], 1);

    let finding = manual_provider_finding(&json)?;
    assert_eq!(finding["rule_id"], "decimate/manual-riverpod-provider");
    assert_eq!(finding["kind"], "manual-riverpod-provider");
    assert_eq!(finding["severity"], "warning");
    assert_eq!(finding["path"], "lib/providers.dart");
    assert_eq!(finding["line"], 3);
    assert_eq!(finding["safe_to_delete"], false);
    assert_eq!(finding["files"], json!([]));
    assert_eq!(finding["edge"], Value::Null);
    assert_eq!(finding["actions"][0]["action"], "migrate-riverpod-codegen");
    assert_eq!(finding["actions"][0]["auto_fixable"], false);
    assert_eq!(finding["actions"][0]["target_symbol"], "counterProvider");
    assert_eq!(
        finding["actions"][0]["suppression_comment"],
        "// decimate-ignore-next-line manual-riverpod-provider"
    );

    Ok(())
}

#[test]
fn manual_riverpod_provider_rule_can_error_or_turn_off() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = riverpod_fixture()?;
    write(
        &fixture,
        ".decimaterc.json",
        r#"{ "rules": { "riverpod-provider-wiring": "error" } }"#,
    )?;
    let mut output = Vec::new();

    let code = run_check(&fixture, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(manual_provider_finding(&json)?["severity"], "error");

    write(
        &fixture,
        ".decimaterc.json",
        r#"{ "rules": { "manual-riverpod-provider": "off" } }"#,
    )?;
    output.clear();

    let code = run_check(&fixture, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["manual_riverpod_providers"], 0);
    assert_no_manual_provider_for(&json, "counterProvider");

    Ok(())
}

fn riverpod_fixture() -> Result<TempDir, std::io::Error> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\ndependencies:\n  flutter_riverpod: any\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'providers.dart';\nvoid main() { counterProvider; }\n",
    )?;
    write(
        &fixture,
        "lib/providers.dart",
        "import 'package:flutter_riverpod/flutter_riverpod.dart';\n\nfinal counterProvider = StateProvider<int>((ref) => 0);\n",
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

fn manual_provider_finding(json: &Value) -> Result<&Value, std::io::Error> {
    json["findings"]
        .as_array()
        .and_then(|findings| {
            findings
                .iter()
                .find(|finding| finding["kind"] == "manual-riverpod-provider")
        })
        .ok_or_else(|| std::io::Error::other("expected manual Riverpod provider finding"))
}

fn assert_no_manual_provider_for(json: &Value, provider_name: &str) {
    assert!(
        !json["findings"].as_array().is_some_and(|findings| {
            findings.iter().any(|finding| {
                finding["kind"] == "manual-riverpod-provider"
                    && finding["message"]
                        .as_str()
                        .is_some_and(|message| message.contains(provider_name))
            })
        }),
        "unexpected manual Riverpod provider finding for {provider_name}"
    );
}

fn write(fixture: &TempDir, path: &str, contents: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, contents)
}
