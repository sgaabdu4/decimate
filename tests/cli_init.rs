use std::fs;

use decimate::cli::run_from;
use serde_json::Value;

#[test]
fn init_writes_config_and_agents_guidance() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let root = fixture.path().to_str().unwrap_or(".");
    let mut output = Vec::new();

    let code = run_from(
        ["decimate", "init", root, "--agents", "--format", "json"],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "decimate.init.v1");
    assert_eq!(json["kind"], "init");
    assert_eq!(json["files"].as_array().map_or(0, Vec::len), 2);
    assert!(fixture.path().join(".decimaterc").is_file());
    assert!(fixture.path().join("AGENTS.md").is_file());
    assert!(
        fs::read_to_string(fixture.path().join(".decimaterc"))?.contains("\"format\": \"json\"")
    );
    assert!(
        fs::read_to_string(fixture.path().join("AGENTS.md"))?
            .contains("decimate audit --format json --base origin/main")
    );

    Ok(())
}

#[test]
fn init_refuses_overwrite_without_force() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let root = fixture.path().to_str().unwrap_or(".");
    fs::write(fixture.path().join(".decimaterc"), "{}\n")?;

    let error = match run_from(["decimate", "init", root], &mut Vec::new()) {
        Ok(code) => panic!("init should refuse overwrite, got exit code {code}"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("refusing to overwrite"));
    assert_eq!(
        fs::read_to_string(fixture.path().join(".decimaterc"))?,
        "{}\n"
    );

    Ok(())
}

#[test]
fn init_force_overwrites_existing_config() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let root = fixture.path().to_str().unwrap_or(".");
    fs::write(fixture.path().join(".decimaterc"), "{}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        ["decimate", "init", root, "--force", "--format", "json"],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["files"][0]["action"], "overwritten");
    assert!(fs::read_to_string(fixture.path().join(".decimaterc"))?.contains("ignorePatterns"));

    Ok(())
}
