use std::fs;

use decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn hooks_status_reports_missing_git_hook() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "hooks",
            "status",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "decimate.hooks.v1");
    assert_eq!(json["command"], "hooks status");
    assert_eq!(json["files"][0]["path"], ".git/hooks/pre-commit");
    assert_eq!(json["files"][0]["installed"], false);
    assert_eq!(json["files"][0]["managed"], false);

    Ok(())
}

#[test]
fn hooks_install_writes_managed_executable_git_hook() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "hooks",
            "install",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--branch",
            "origin/dev",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    let hook = fixture.path().join(".git/hooks/pre-commit");
    let source = fs::read_to_string(&hook)?;
    assert_eq!(code, 0);
    assert_eq!(json["command"], "hooks install");
    assert_eq!(json["files"][0]["action"], "created");
    assert_eq!(json["files"][0]["managed"], true);
    assert!(source.contains("decimate-managed-hook"));
    assert!(source.contains("origin/dev"));
    assert!(source.contains("decimate audit . --base \"$BASE\" --format json --summary"));

    Ok(())
}

#[test]
fn hooks_install_refuses_unmanaged_hook_without_force() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    let hook = fixture.path().join(".git/hooks/pre-commit");
    fs::write(&hook, "#!/bin/sh\necho custom\n")?;

    let error = match run_from(
        [
            "decimate",
            "hooks",
            "install",
            fixture.path().to_str().unwrap_or("."),
        ],
        &mut Vec::new(),
    ) {
        Ok(code) => panic!("install should refuse unmanaged hook, got {code}"),
        Err(error) => error,
    };

    assert!(
        error
            .to_string()
            .contains("refusing to overwrite unmanaged hook")
    );
    assert!(fs::read_to_string(hook)?.contains("echo custom"));

    Ok(())
}

#[test]
fn hooks_uninstall_removes_only_managed_hook() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = git_fixture()?;
    run_from(
        [
            "decimate",
            "hooks",
            "install",
            fixture.path().to_str().unwrap_or("."),
        ],
        &mut Vec::new(),
    )?;
    let hook = fixture.path().join(".git/hooks/pre-commit");
    assert!(hook.is_file());
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "hooks",
            "uninstall",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["files"][0]["action"], "removed");
    assert_eq!(json["files"][0]["installed"], false);
    assert!(!hook.exists());

    Ok(())
}

fn git_fixture() -> Result<TempDir, std::io::Error> {
    let fixture = tempfile::tempdir()?;
    fs::create_dir_all(fixture.path().join(".git/hooks"))?;
    Ok(fixture)
}
