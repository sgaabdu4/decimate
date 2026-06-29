use std::fs;

use decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn security_gate_newly_reachable_keeps_downstream_reachable_candidate()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'src/target.dart';\nvoid main() => target();\n",
    )?;
    write(
        &fixture,
        "lib/src/target.dart",
        "const accessToken = 'decimate_fixture_value_1234567890';\nvoid target() => print(accessToken);\n",
    )?;
    write(
        &fixture,
        "lib/src/dead.dart",
        "final uri = Uri.parse('http://dead.example.com');\n",
    )?;
    write_changed_import_diff(&fixture)?;

    let (code, json) = run_json([
        "decimate",
        "security",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--gate",
        "newly-reachable",
        "--diff-file",
        "security.diff",
    ])?;

    assert_eq!(code, 8);
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["security_candidates"], 1);
    assert_eq!(json["summary"]["security_candidate_occurrences"], 1);
    assert_eq!(json["summary"]["findings"], 1);
    assert_eq!(
        json["security_candidates"][0]["occurrences"][0]["path"],
        "lib/src/target.dart"
    );
    assert_eq!(json["findings"][0]["path"], "lib/src/target.dart");

    Ok(())
}

#[test]
fn security_gate_newly_reachable_passes_for_changed_unreachable_candidate()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(
        &fixture,
        "lib/src/secret.dart",
        "const token = 'decimate_fixture_value_1234567890';\n",
    )?;
    write(
        &fixture,
        "security.diff",
        "diff --git a/lib/src/secret.dart b/lib/src/secret.dart\n\
--- a/lib/src/secret.dart\n\
+++ b/lib/src/secret.dart\n\
@@ -0,0 +1,1 @@\n\
+const token = 'decimate_fixture_value_1234567890';\n",
    )?;

    let (code, json) = run_json([
        "decimate",
        "security",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--gate",
        "newly-reachable",
        "--diff-file",
        "security.diff",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["summary"]["security_candidates"], 0);
    assert_eq!(json["summary"]["security_candidate_occurrences"], 0);
    assert_eq!(json["summary"]["findings"], 0);
    assert!(
        json["security_candidates"]
            .as_array()
            .is_some_and(Vec::is_empty)
    );

    Ok(())
}

fn run_json<const N: usize>(args: [&str; N]) -> Result<(i32, Value), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(args, &mut output)?;
    Ok((code, serde_json::from_slice::<Value>(&output)?))
}

fn write_changed_import_diff(fixture: &TempDir) -> Result<(), std::io::Error> {
    write(
        fixture,
        "security.diff",
        "diff --git a/lib/main.dart b/lib/main.dart\n\
--- a/lib/main.dart\n\
+++ b/lib/main.dart\n\
@@ -0,0 +1,1 @@\n\
+import 'src/target.dart';\n",
    )
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
