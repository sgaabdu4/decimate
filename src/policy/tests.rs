use std::fs;

use tempfile::TempDir;

use super::{
    BoundaryCallRule, PolicyPack, PolicyRule, PolicyRuleKind, detect_boundary_call_violations,
    detect_policy_violations, load_policy_pack,
};
use crate::scan_project;

#[test]
fn detects_boundary_forbidden_calls_in_matching_zone() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture()?;
    write(
        &fixture,
        "lib/ui/page.dart",
        "import 'package:flutter/services.dart';\nvoid render() { SystemChrome.setPreferredOrientations([]); }\n",
    )?;
    write(
        &fixture,
        "lib/domain/model.dart",
        "void model() { SystemChrome.setPreferredOrientations([]); }\n",
    )?;
    let project = scan_project(fixture.path())?;

    let violations = detect_boundary_call_violations(
        &project,
        &[BoundaryCallRule::new(
            "lib/ui",
            vec!["SystemChrome.*".to_owned()],
        )],
    )?;

    assert_eq!(violations.len(), 1);
    assert_eq!(
        violations[0]
            .path
            .strip_prefix(fixture.path())?
            .to_string_lossy(),
        "lib/ui/page.dart"
    );
    assert_eq!(
        violations[0].callee,
        "SystemChrome.setPreferredOrientations"
    );
    assert_eq!(violations[0].pattern, "SystemChrome.*");

    Ok(())
}

#[test]
fn detects_policy_banned_imports_and_calls() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture()?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'dart:io';\nvoid main() { Process.runSync('sh', []); }\n",
    )?;
    let project = scan_project(fixture.path())?;
    let pack = PolicyPack {
        name: "mobile".to_owned(),
        path: None,
        rules: vec![
            PolicyRule {
                id: "no-dart-io".to_owned(),
                message: Some("Do not import dart:io from app code".to_owned()),
                severity: None,
                kind: PolicyRuleKind::BannedImport {
                    patterns: vec!["dart:io".to_owned()],
                },
            },
            PolicyRule {
                id: "no-process".to_owned(),
                message: None,
                severity: None,
                kind: PolicyRuleKind::BannedCall {
                    patterns: vec!["Process.*".to_owned()],
                },
            },
        ],
    };

    let violations = detect_policy_violations(&project, &[pack])?;

    assert_eq!(violations.len(), 2);
    assert!(violations.iter().any(|violation| {
        violation.rule_id == "decimate/policy/mobile/no-dart-io" && violation.target == "dart:io"
    }));
    assert!(violations.iter().any(|violation| {
        violation.rule_id == "decimate/policy/mobile/no-process"
            && violation.target == "Process.runSync"
    }));

    Ok(())
}

#[test]
fn loads_jsonc_policy_pack() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = fixture()?;
    write(
        &fixture,
        "policy.jsonc",
        r#"{
  // policy owner can keep comments
  "name": "mobile",
  "rules": [
    { "id": "no-dart-io", "type": "banned-import", "pattern": "dart:io" }
  ]
}
"#,
    )?;

    let pack = load_policy_pack(fixture.path(), "policy.jsonc")?;

    assert_eq!(pack.name, "mobile");
    assert_eq!(pack.rules.len(), 1);

    Ok(())
}

fn fixture() -> Result<TempDir, std::io::Error> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    Ok(fixture)
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
