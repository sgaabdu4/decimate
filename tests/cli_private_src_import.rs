use std::fs;

use decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn check_reports_cross_package_private_src_imports() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_private_src_fixture(&fixture)?;

    let (code, json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--private-src-imports",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["summary"]["findings"], 2);
    assert_eq!(json["summary"]["private_src_imports"], 2);
    assert_eq!(json["summary"]["unused_dependencies"], 0);
    assert_eq!(json["summary"]["unlisted_dependencies"], 0);

    let Some(findings) = json["findings"].as_array() else {
        panic!("findings array");
    };
    assert!(
        findings
            .iter()
            .all(|finding| finding["kind"] == "private-src-import")
    );
    assert_private_src_finding(&json, "shared", "package:shared/src/internal.dart", 1);
    assert_private_src_finding(&json, "collection", "package:collection/src/utils.dart", 2);

    Ok(())
}

#[test]
fn private_src_import_ignores_generated_mockito_mocks() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  shared:\n    path: shared\n",
    )?;
    write(&fixture, "shared/pubspec.yaml", "name: shared\n")?;
    write(
        &fixture,
        "shared/lib/src/internal.dart",
        "void internal() {}\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'repository.mocks.dart';\nvoid main() { generatedMock(); }\n",
    )?;
    write(
        &fixture,
        "lib/repository.mocks.dart",
        "import 'package:shared/src/internal.dart';\nvoid generatedMock() => internal();\n",
    )?;

    let (code, json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--private-src-imports",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["summary"]["private_src_imports"], 0);
    assert_eq!(json["summary"]["unused_dependencies"], 0);
    assert_eq!(json["summary"]["findings"], 0);

    Ok(())
}

#[test]
fn private_src_import_rule_can_warn_or_turn_off() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_private_src_fixture(&fixture)?;
    write(
        &fixture,
        ".decimaterc.json",
        r#"{ "rules": { "private-src-import": "warn" } }"#,
    )?;

    let (warn_code, warn_json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--private-src-imports",
    ])?;

    assert_eq!(warn_code, 0);
    assert_eq!(warn_json["verdict"], "pass");
    assert_eq!(warn_json["summary"]["private_src_imports"], 2);
    assert!(warn_json["findings"].as_array().is_some_and(|findings| {
        findings
            .iter()
            .all(|finding| finding["severity"] == "warning")
    }));

    write(
        &fixture,
        ".decimaterc.json",
        r#"{ "rules": { "decimate/private-src-import": "off" } }"#,
    )?;
    let (off_code, off_json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--private-src-imports",
    ])?;

    assert_eq!(off_code, 0);
    assert_eq!(off_json["summary"]["private_src_imports"], 0);
    assert!(off_json["findings"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn private_src_import_respects_inline_suppression() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_private_src_fixture(&fixture)?;
    write(
        &fixture,
        "lib/main.dart",
        "// decimate-ignore-next-line private-src-import -- legacy package boundary\n\
import 'package:shared/src/internal.dart';\n\
export 'package:collection/src/utils.dart';\n\
void main() { internal(); }\n",
    )?;

    let (code, json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--private-src-imports",
    ])?;

    assert_eq!(code, 1);
    assert_eq!(json["summary"]["private_src_imports"], 1);
    assert_private_src_finding(&json, "collection", "package:collection/src/utils.dart", 3);

    Ok(())
}

#[test]
fn private_src_imports_are_in_agent_contracts() -> Result<(), Box<dyn std::error::Error>> {
    let report_schema = json_command(["decimate", "report-schema", "--format", "json"])?;
    assert_array_contains(
        &report_schema["$defs"]["finding"]["properties"]["kind"]["enum"],
        "private-src-import",
    );
    assert_array_contains(
        &report_schema["$defs"]["summary"]["required"],
        "private_src_imports",
    );
    assert_eq!(
        report_schema["$defs"]["summary"]["properties"]["private_src_imports"]["type"],
        "integer"
    );

    let manifest = json_command(["decimate", "schema"])?;
    assert_array_contains(&manifest["issue_types"], "private-src-import");
    assert!(manifest["commands"].as_array().is_some_and(|commands| {
        commands.iter().any(|command| {
            command["name"] == "check"
                && command["flags"]
                    .as_array()
                    .is_some_and(|flags| flags.iter().any(|flag| flag == "--private-src-imports"))
        })
    }));

    Ok(())
}

fn assert_private_src_finding(json: &Value, dependency: &str, specifier: &str, line: u64) {
    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["actions"][0]["target_dependency"] == dependency)
    }) else {
        panic!("private src import finding for {dependency}");
    };
    assert_eq!(finding["rule_id"], "decimate/private-src-import");
    assert_eq!(finding["severity"], "error");
    assert_eq!(finding["path"], "lib/main.dart");
    assert_eq!(finding["line"], line);
    assert_eq!(finding["edge"]["specifier"], specifier);
    assert_eq!(
        finding["actions"][0]["action"],
        "replace-package-private-import"
    );
    assert_eq!(finding["actions"][0]["auto_fixable"], false);
    assert_eq!(
        finding["actions"][0]["suppression_comment"],
        "// decimate-ignore-next-line private-src-import"
    );
}

fn write_private_src_fixture(fixture: &TempDir) -> Result<(), Box<dyn std::error::Error>> {
    write(
        fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  shared:\n    path: shared\n  collection: ^1.0.0\n",
    )?;
    write(fixture, "shared/pubspec.yaml", "name: shared\n")?;
    write(
        fixture,
        "shared/lib/shared.dart",
        "export 'src/internal.dart';\n",
    )?;
    write(
        fixture,
        "shared/lib/src/internal.dart",
        "void internal() {}\n",
    )?;
    write(fixture, "lib/src/self.dart", "void self() {}\n")?;
    write(fixture, "lib/src/local.dart", "void local() {}\n")?;
    write(
        fixture,
        "lib/main.dart",
        "import 'package:shared/src/internal.dart';\n\
export 'package:collection/src/utils.dart';\n\
import 'package:app/src/self.dart';\n\
import 'src/local.dart';\n\
import 'package:shared/shared.dart';\n\
void main() { internal(); self(); local(); }\n",
    )
    .map_err(Into::into)
}

fn run_json<const N: usize>(args: [&str; N]) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(args, &mut output)?;
    Ok((code, serde_json::from_slice::<Value>(&output)?))
}

fn json_command<const N: usize>(args: [&str; N]) -> Result<Value, Box<dyn std::error::Error>> {
    let (code, json) = run_json(args)?;
    assert_eq!(code, 0);
    Ok(json)
}

fn assert_array_contains(array: &Value, expected: &str) {
    assert!(
        array
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item == expected)),
        "expected array to contain {expected}"
    );
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
