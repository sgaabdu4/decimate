use std::fs;
use std::process::Command;

use decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn dupes_command_emits_json_contract() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_duplicate_pair(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "dupes",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["schema_version"], "decimate.report.v1");
    assert_eq!(json["command"], "dupes");
    assert_eq!(json["summary"]["code_duplications"], 1);
    assert_eq!(
        json["clone_groups"][0]["instances"][0]["path"],
        "lib/a.dart"
    );
    assert_eq!(
        json["clone_groups"][0]["instances"][1]["path"],
        "lib/b.dart"
    );
    assert_eq!(json["findings"][0]["rule_id"], "decimate/code-duplication");
    assert_eq!(json["findings"][0]["kind"], "code-duplication");
    assert_eq!(
        json["findings"][0]["fingerprint"],
        json["clone_groups"][0]["fingerprint"]
    );
    assert_eq!(json["findings"][0]["safe_to_delete"], false);
    assert_eq!(json["findings"][0]["actions"][0]["action"], "trace-clone");
    assert_eq!(json["findings"][0]["actions"][0]["type"], "trace-clone");
    assert_eq!(
        json["findings"][0]["actions"][0]["target_path"],
        "lib/a.dart"
    );
    assert!(
        json["findings"][0]["actions"][0]["command"]
            .as_str()
            .is_some_and(|command| command
                .starts_with("decimate trace-clone --format json --fingerprint dup:"))
    );
    assert_eq!(json["next_steps"][0]["id"], "trace-code-duplication");
    assert!(
        json["next_steps"][0]["command"]
            .as_str()
            .is_some_and(|command| command
                .starts_with("decimate trace-clone --format json --fingerprint dup:"))
    );

    Ok(())
}

#[test]
fn check_command_includes_code_duplication_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_duplicate_pair(&fixture)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let Some(findings) = json["findings"].as_array() else {
        panic!("findings array");
    };
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["code_duplications"], 1);
    assert!(
        findings
            .iter()
            .any(|finding| finding["rule_id"] == "decimate/code-duplication")
    );

    Ok(())
}

#[test]
fn workspace_scope_prunes_clone_group_instances() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: root\nworkspace:\n  - packages/*\n",
    )?;
    write(&fixture, "packages/app/pubspec.yaml", "name: app\n")?;
    write(&fixture, "packages/shared/pubspec.yaml", "name: shared\n")?;
    let source = "void shared() {\n  final items = [1, 2, 3];\n  final active = items.where((item) => item > 1);\n  print(active.length);\n}\n";
    write(&fixture, "packages/app/lib/a.dart", source)?;
    write(&fixture, "packages/shared/lib/b.dart", source)?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "dupes",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
            "--workspace",
            "app",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["code_duplications"], 1);
    assert_eq!(
        json["clone_groups"][0]["instances"][0]["path"],
        "packages/app/lib/a.dart"
    );
    assert_eq!(
        json["clone_groups"][0]["instances"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );

    Ok(())
}

#[test]
fn changed_workspaces_scope_prunes_clone_group_instances() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = git_fixture()?;
    write(
        &fixture,
        "pubspec.yaml",
        "name: root\nworkspace:\n  - packages/*\n",
    )?;
    write(&fixture, "packages/app/pubspec.yaml", "name: app\n")?;
    write(&fixture, "packages/shared/pubspec.yaml", "name: shared\n")?;
    let source = "void shared() {\n  final items = [1, 2, 3];\n  final active = items.where((item) => item > 1);\n  print(active.length);\n}\n";
    write(&fixture, "packages/app/lib/a.dart", source)?;
    write(&fixture, "packages/shared/lib/b.dart", source)?;
    commit_all(&fixture)?;
    write(&fixture, "packages/app/README.md", "changed\n")?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "dupes",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
            "--changed-workspaces",
            "HEAD",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["code_duplications"], 1);
    assert_eq!(
        json["clone_groups"][0]["instances"][0]["path"],
        "packages/app/lib/a.dart"
    );
    assert_eq!(
        json["clone_groups"][0]["instances"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );

    Ok(())
}

#[test]
fn trace_clone_command_reports_matching_group() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write_duplicate_pair(&fixture)?;
    let mut dupes_output = Vec::new();
    run_from(
        [
            "decimate",
            "dupes",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
        ],
        &mut dupes_output,
    )?;
    let dupes_json = serde_json::from_slice::<Value>(&dupes_output)?;
    let Some(fingerprint) = dupes_json["clone_groups"][0]["fingerprint"].as_str() else {
        panic!("fingerprint string");
    };
    let mut trace_output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "trace-clone",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--min-lines",
            "5",
            "--min-tokens",
            "10",
            "--fingerprint",
            fingerprint,
        ],
        &mut trace_output,
    )?;

    let json = serde_json::from_slice::<Value>(&trace_output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "decimate.trace.v1");
    assert_eq!(json["kind"], "trace-clone");
    assert_eq!(json["command"], "trace-clone");
    assert_eq!(json["found"], true);
    assert_eq!(json["clone_groups"][0]["fingerprint"], fingerprint);
    assert_eq!(
        json["clone_groups"][0]["instances"][0]["path"],
        "lib/a.dart"
    );
    assert_eq!(
        json["clone_groups"][0]["instances"][1]["path"],
        "lib/b.dart"
    );

    Ok(())
}

fn write_duplicate_pair(fixture: &TempDir) -> Result<(), std::io::Error> {
    let source = "void shared() {\n  final items = [1, 2, 3];\n  final active = items.where((item) => item > 1);\n  print(active.length);\n}\n";
    write(fixture, "lib/a.dart", source)?;
    write(fixture, "lib/b.dart", source)
}

fn git_fixture() -> Result<TempDir, Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    git(&fixture, ["init", "-q"])?;
    git(&fixture, ["config", "user.email", "decimate@example.com"])?;
    git(&fixture, ["config", "user.name", "Decimate Tests"])?;
    Ok(fixture)
}

fn commit_all(fixture: &TempDir) -> Result<(), Box<dyn std::error::Error>> {
    git(fixture, ["add", "."])?;
    git(fixture, ["commit", "-m", "initial", "-q"])
}

fn git<const N: usize>(
    fixture: &TempDir,
    args: [&str; N],
) -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(args)
        .current_dir(fixture.path())
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(format!(
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr).trim()
    )
    .into())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
