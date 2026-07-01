use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn security_candidates_include_honest_module_level_reachability()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_reachability_fixture(&fixture)?;

    let (code, json) = run_json([
        "dart-decimate",
        "security",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/main.dart",
    ])?;

    let secret = candidate(&json, "hardcoded-secret");
    let insecure = candidate(&json, "insecure-transport");
    assert_eq!(code, 1);
    assert_eq!(secret["reachability"]["reachable_from_entrypoint"], true);
    assert_eq!(secret["reachability"]["taint_confidence"], "module-level");
    assert_eq!(secret["reachability"]["entry_points"][0], "lib/main.dart");
    assert_eq!(secret["reachability"]["reachable_occurrences"], 1);
    assert_eq!(
        secret["occurrences"][0]["reachability"]["taint_confidence"],
        "module-level"
    );
    assert!(secret.get("taint_flow").is_none());
    assert!(insecure.get("reachability").is_none());
    assert!(insecure["occurrences"][0].get("reachability").is_none());

    Ok(())
}

#[test]
fn security_sarif_includes_security_reachability_when_available()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_reachability_fixture(&fixture)?;

    let (code, json) = run_json([
        "dart-decimate",
        "security",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "sarif",
        "--entry",
        "lib/main.dart",
    ])?;

    let Some(results) = json["runs"][0]["results"].as_array() else {
        panic!("sarif results array");
    };
    let secret = results
        .iter()
        .find(|result| result["ruleId"] == "dart-decimate/security-hardcoded-secret")
        .unwrap_or_else(|| panic!("hardcoded secret result"));
    let insecure = results
        .iter()
        .find(|result| result["ruleId"] == "dart-decimate/security-insecure-transport")
        .unwrap_or_else(|| panic!("insecure transport result"));

    assert_eq!(code, 1);
    assert_eq!(
        secret["properties"]["securityReachability"]["taint_confidence"],
        "module-level"
    );
    assert!(insecure["properties"].get("securityReachability").is_none());

    Ok(())
}

fn write_reachability_fixture(fixture: &TempDir) -> Result<(), std::io::Error> {
    write(fixture, "pubspec.yaml", "name: app\n")?;
    write(
        fixture,
        "lib/main.dart",
        "import 'live.dart';\nvoid main() { print(accessToken); }\n",
    )?;
    write(
        fixture,
        "lib/live.dart",
        "const accessToken = 'dart_decimate_fixture_value_1234567890';\n",
    )?;
    write(
        fixture,
        "lib/dead.dart",
        "final uri = Uri.parse('http://api.example.com/login');\n",
    )
}

fn run_json<const N: usize>(args: [&str; N]) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(args, &mut output)?;
    Ok((code, serde_json::from_slice::<Value>(&output)?))
}

fn candidate<'a>(json: &'a Value, category: &str) -> &'a Value {
    json["security_candidates"]
        .as_array()
        .and_then(|candidates| {
            candidates
                .iter()
                .find(|candidate| candidate["category"] == category)
        })
        .unwrap_or_else(|| panic!("missing candidate {category}"))
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
