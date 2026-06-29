use std::fs;

use serde_json::Value;
use tempfile::TempDir;

use super::*;

#[test]
fn dead_code_command_emits_json_contract() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'live.dart';\nvoid main() {}\n",
    )?;
    write(&fixture, "lib/live.dart", "class Live {}\n")?;
    write(&fixture, "lib/dead.dart", "class Dead {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "dead-code",
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
    assert_eq!(json["schema_version"], "decimate.report.v1");
    assert_eq!(json["kind"], "dead-code");
    assert_eq!(json["command"], "dead-code");
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["dead_files"], 1);
    assert_eq!(json["findings"][0]["rule_id"], "decimate/dead-file");
    assert_eq!(json["findings"][0]["path"], "lib/dead.dart");
    assert_eq!(json["findings"][0]["safe_to_delete"], true);
    assert_eq!(json["findings"][0]["actions"][0]["type"], "delete-file");
    assert_eq!(
        json["findings"][0]["actions"][0]["target_path"],
        "lib/dead.dart"
    );
    assert_eq!(
        json["findings"][0]["actions"][0]["command"],
        "decimate inspect --format json --file lib/dead.dart"
    );
    assert_eq!(json["findings"][0]["actions"][0]["auto_fixable"], true);

    Ok(())
}

#[test]
fn cycles_command_reports_circular_dependencies() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/a.dart", "import 'b.dart';\nclass A {}\n")?;
    write(&fixture, "lib/b.dart", "import 'a.dart';\nclass B {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "cycles",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["command"], "cycles");
    assert_eq!(json["summary"]["cycles"], 1);
    assert_eq!(
        json["findings"][0]["rule_id"],
        "decimate/circular-dependency"
    );
    assert_eq!(json["findings"][0]["files"][0], "lib/a.dart");
    assert_eq!(json["findings"][0]["files"][1], "lib/b.dart");

    Ok(())
}

#[test]
fn cycles_command_reports_re_export_cycles_separately() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/a.dart", "export 'b.dart';\n")?;
    write(&fixture, "lib/b.dart", "export 'a.dart';\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "cycles",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(findings) = json["findings"].as_array() else {
        panic!("findings array");
    };
    let rule_ids = findings
        .iter()
        .map(|finding| finding["rule_id"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["re_export_cycles"], 1);
    assert!(rule_ids.contains(&"decimate/circular-dependency"));
    assert!(rule_ids.contains(&"decimate/re-export-cycle"));

    Ok(())
}

#[test]
fn dead_code_command_reports_unused_exports_with_safe_fix_action()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/live.dart';\nvoid main() { Used(); }\n",
    )?;
    write(
        &fixture,
        "lib/src/live.dart",
        "class Used {}\nclass Unused {}\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "dead-code",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "decimate/unused-export")
    }) else {
        panic!("unused export finding");
    };
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["unused_exports"], 1);
    assert_eq!(finding["path"], "lib/src/live.dart");
    assert_eq!(finding["line"], 2);
    assert_eq!(finding["safe_to_delete"], true);
    assert_eq!(finding["actions"][0]["action"], "remove-declaration");
    assert_eq!(finding["actions"][0]["type"], "remove-declaration");
    assert_eq!(finding["actions"][0]["target_path"], "lib/src/live.dart");
    assert_eq!(finding["actions"][0]["target_symbol"], "Unused");
    assert_eq!(finding["actions"][0]["target_end_line"], 2);
    assert_eq!(finding["actions"][0]["auto_fixable"], true);
    assert_eq!(finding["actions"][1]["action"], "trace-symbol");
    assert_eq!(finding["actions"][1]["type"], "trace-symbol");
    assert_eq!(finding["actions"][1]["target_path"], "lib/src/live.dart");
    assert_eq!(finding["actions"][1]["target_symbol"], "Unused");
    assert_eq!(
        finding["actions"][1]["command"],
        "decimate inspect --format json --symbol lib/src/live.dart:Unused"
    );
    assert_eq!(
        finding["actions"][1]["suppression_comment"],
        "// decimate-ignore-next-line unused-export"
    );
    assert_eq!(finding["actions"][1]["auto_fixable"], false);
    assert_eq!(json["next_steps"][0]["id"], "trace-unused-export");
    assert_eq!(
        json["next_steps"][0]["command"],
        "decimate trace-symbol --format json --symbol lib/src/live.dart:Unused"
    );

    Ok(())
}

#[test]
fn check_command_reports_duplicate_exports() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/package.dart",
        "export 'src/a.dart';\nexport 'src/b.dart';\n",
    )?;
    write(&fixture, "lib/src/a.dart", "class Api {}\n")?;
    write(&fixture, "lib/src/b.dart", "class Api {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["duplicate_exports"], 1);
    assert_eq!(json["findings"][0]["rule_id"], "decimate/duplicate-export");
    assert_eq!(json["findings"][0]["kind"], "duplicate-export");
    assert_eq!(json["findings"][0]["path"], "lib/package.dart");
    assert_eq!(json["findings"][0]["safe_to_delete"], false);
    assert_eq!(json["findings"][0]["files"][0], "lib/src/a.dart");
    assert_eq!(json["findings"][0]["files"][1], "lib/src/b.dart");
    assert_eq!(
        json["findings"][0]["actions"][0]["action"],
        "inspect-export-surface"
    );
    assert_eq!(json["findings"][0]["actions"][0]["auto_fixable"], false);

    Ok(())
}

#[test]
fn trace_file_command_emits_json_contract() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'barrel.dart';\nvoid main() { Api(); }\n",
    )?;
    write(&fixture, "lib/barrel.dart", "export 'src/api.dart';\n")?;
    write(&fixture, "lib/src/api.dart", "class Api {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "trace-file",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
            "--file",
            "lib/barrel.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "decimate.trace.v1");
    assert_eq!(json["kind"], "trace-file");
    assert_eq!(json["command"], "trace-file");
    assert_eq!(json["path"], "lib/barrel.dart");
    assert_eq!(json["reachable"], true);
    assert_eq!(json["imported_by"][0]["from"], "lib/main.dart");
    assert_eq!(json["re_exports"][0]["to"], "lib/src/api.dart");

    Ok(())
}

#[test]
fn trace_file_command_reports_missing_file_without_failure()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "trace-file",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
            "--file",
            "lib/missing.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["found"], false);
    assert_eq!(json["reason"], "file is not in the module graph");

    Ok(())
}

#[test]
fn trace_symbol_command_emits_json_contract() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'barrel.dart';\nvoid main() { Api(); }\n",
    )?;
    write(&fixture, "lib/barrel.dart", "export 'src/api.dart';\n")?;
    write(
        &fixture,
        "lib/src/api.dart",
        "class Api {}\nclass Unused {}\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "trace-symbol",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
            "--file",
            "lib/src/api.dart",
            "--symbol",
            "Api",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["command"], "trace-symbol");
    assert_eq!(json["path"], "lib/src/api.dart");
    assert_eq!(json["symbol"], "Api");
    assert_eq!(json["found"], true);
    assert_eq!(json["direct_references"][0]["path"], "lib/main.dart");
    assert_eq!(json["re_export_chains"][0][0], "lib/barrel.dart");
    assert_eq!(json["reason"], "symbol has reachable direct references");

    Ok(())
}

#[test]
fn trace_dependency_command_emits_json_contract() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  collection: ^1.18.0\n  path: ^1.9.0\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package:collection/collection.dart' deferred as coll;\n\
export 'package:collection/equality.dart';\n\
void main() {}\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "trace-dependency",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--dependency",
            "collection",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "decimate.trace.v1");
    assert_eq!(json["kind"], "trace-dependency");
    assert_eq!(json["command"], "trace-dependency");
    assert_eq!(json["dependency"], "collection");
    assert_eq!(json["found"], true);
    assert_eq!(json["declared"], true);
    assert_eq!(json["is_used"], true);
    assert_eq!(json["used_in_scripts"], false);
    assert_eq!(json["total_import_count"], 2);
    let Some(type_only_importers) = json["type_only_importers"].as_array() else {
        panic!("type_only_importers array");
    };
    assert!(type_only_importers.is_empty());
    assert_eq!(json["declared_in"][0]["pubspec_path"], "pubspec.yaml");
    assert_eq!(json["declared_in"][0]["section"], "dependencies");
    assert_eq!(json["declared_in"][0]["line"], 3);
    assert_eq!(json["importing_files"][0]["package"], "app");
    assert_eq!(json["importing_files"][0]["kind"], "import");
    assert_eq!(
        json["importing_files"][0]["specifier"],
        "package:collection/collection.dart"
    );
    assert_eq!(json["importing_files"][0]["prefix"], "coll");
    assert_eq!(json["importing_files"][0]["deferred"], true);
    assert_eq!(json["importing_files"][1]["kind"], "export");

    Ok(())
}

#[test]
fn check_command_reports_boundaries_and_unresolved_imports()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'domain/service.dart';\nvoid main() {}\n",
    )?;
    write(
        &fixture,
        "lib/domain/service.dart",
        "import '../ui/widget.dart';\nimport 'missing.dart';\nclass Service {}\n",
    )?;
    write(&fixture, "lib/ui/widget.dart", "class Widget {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--boundary",
            "lib/domain:lib/ui",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(findings) = json["findings"].as_array() else {
        panic!("findings array");
    };
    let rule_ids = findings
        .iter()
        .map(|finding| finding["rule_id"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(code, 1);
    assert!(rule_ids.contains(&"decimate/boundary-violation"));
    assert!(rule_ids.contains(&"decimate/unresolved-dependency"));
    assert_eq!(json["summary"]["boundary_violations"], 1);
    assert_eq!(json["summary"]["unresolved_dependencies"], 1);

    Ok(())
}

#[test]
fn dead_code_command_includes_pub_dependency_hygiene() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: app\n\
dependencies:\n  http: ^1.0.0\n  path: ^1.0.0\n",
    )?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'package:http/http.dart';\n\
import 'package:collection/collection.dart';\n\
void main() {}\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "dead-code",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(findings) = json["findings"].as_array() else {
        panic!("findings array");
    };
    let rule_ids = findings
        .iter()
        .map(|finding| finding["rule_id"].as_str().unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(code, 1);
    assert!(rule_ids.contains(&"decimate/unused-dependency"));
    assert!(rule_ids.contains(&"decimate/unlisted-dependency"));
    assert_eq!(json["summary"]["unused_dependencies"], 1);
    assert_eq!(json["summary"]["unlisted_dependencies"], 1);
    let Some(next_steps) = json["next_steps"].as_array() else {
        panic!("next_steps array");
    };
    assert!(next_steps.iter().any(|step| {
        step["id"] == "trace-unused-dependency"
            && step["command"] == "decimate trace-dependency --format json --dependency path"
    }));

    Ok(())
}

#[test]
fn unlisted_dependency_json_edge_kind_preserves_export() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "export 'package:collection/collection.dart';\nvoid main() {}\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "dead-code",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--entry",
            "lib/main.dart",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "decimate/unlisted-dependency")
    }) else {
        panic!("unlisted dependency finding");
    };
    assert_eq!(code, 1);
    assert_eq!(finding["edge"]["kind"], "export");

    Ok(())
}

#[test]
fn check_command_uses_path_dependencies_without_reporting_their_findings()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "app/pubspec.yaml",
        "name: app\n\
dependencies:\n  shared:\n    path: ../shared\n",
    )?;
    write(
        &fixture,
        "app/lib/main.dart",
        "import 'package:shared/shared.dart';\nvoid main() {}\n",
    )?;
    write(&fixture, "shared/pubspec.yaml", "name: shared\n")?;
    write(
        &fixture,
        "shared/lib/shared.dart",
        "import 'loop.dart';\nimport 'missing.dart';\nclass Shared {}\n",
    )?;
    write(
        &fixture,
        "shared/lib/loop.dart",
        "import 'shared.dart';\nclass Loop {}\n",
    )?;
    write(&fixture, "shared/lib/dead.dart", "class SharedDead {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "check",
            fixture.path().join("app").to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["unresolved_dependencies"], 0);
    assert_eq!(json["summary"]["dead_files"], 0);
    assert_eq!(json["summary"]["cycles"], 0);
    assert_eq!(json["summary"]["findings"], 0);

    Ok(())
}

#[test]
fn check_command_treats_tests_as_default_entry_points() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(&fixture, "test/all_tests.dart", "void main() {}\n")?;
    write(&fixture, "test/home_test.dart", "void main() {}\n")?;
    write(
        &fixture,
        "integration_test/app_test.dart",
        "void main() {}\n",
    )?;
    write(
        &fixture,
        "test_driver/integration_test.dart",
        "void main() {}\n",
    )?;
    write(&fixture, "tool/grind.dart", "void main() {}\n")?;
    write(&fixture, "pigeon/schema.dart", "class Schema {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["summary"]["dead_files"], 0);
    assert_eq!(json["summary"]["findings"], 0);

    Ok(())
}

#[test]
fn check_command_treats_public_library_files_as_default_entry_points()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/package.dart",
        "import 'src/live.dart';\nclass Package {}\n",
    )?;
    write(&fixture, "lib/src/live.dart", "class Live {}\n")?;
    write(&fixture, "lib/src/dead.dart", "class Dead {}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(findings) = json["findings"].as_array() else {
        panic!("findings array");
    };
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["dead_files"], 1);
    assert_eq!(findings[0]["path"], "lib/src/dead.dart");

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
