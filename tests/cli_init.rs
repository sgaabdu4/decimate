use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;

#[test]
fn init_writes_config_and_agents_guidance() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let root = fixture.path().to_str().unwrap_or(".");
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "init",
            root,
            "--agents",
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "dart-decimate.init.v1");
    assert_eq!(json["kind"], "init");
    assert_eq!(json["files"].as_array().map_or(0, Vec::len), 2);
    assert!(fixture.path().join(".dart-decimaterc").is_file());
    assert!(fixture.path().join("AGENTS.md").is_file());
    let config = fs::read_to_string(fixture.path().join(".dart-decimaterc"))?;
    assert!(config.contains("\"format\": \"json\""));
    for pattern in [
        "**/*.g.dart",
        "**/*.freezed.dart",
        "**/*.gen.dart",
        "**/*.gr.dart",
        "**/*.mocks.dart",
    ] {
        assert!(config.contains(pattern), "{pattern}");
    }
    assert!(
        fs::read_to_string(fixture.path().join("AGENTS.md"))?
            .contains("dart-decimate audit --format json --base origin/main")
    );

    Ok(())
}

#[test]
fn init_refuses_overwrite_without_force() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let root = fixture.path().to_str().unwrap_or(".");
    fs::write(fixture.path().join(".dart-decimaterc"), "{}\n")?;

    let error = match run_from(["dart-decimate", "init", root], &mut Vec::new()) {
        Ok(code) => panic!("init should refuse overwrite, got exit code {code}"),
        Err(error) => error,
    };

    assert!(error.to_string().contains("refusing to overwrite"));
    assert_eq!(
        fs::read_to_string(fixture.path().join(".dart-decimaterc"))?,
        "{}\n"
    );

    Ok(())
}

#[test]
fn init_force_overwrites_existing_config() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let root = fixture.path().to_str().unwrap_or(".");
    fs::write(fixture.path().join(".dart-decimaterc"), "{}\n")?;
    let mut output = Vec::new();

    let code = run_from(
        ["dart-decimate", "init", root, "--force", "--format", "json"],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["files"][0]["action"], "overwritten");
    assert!(
        fs::read_to_string(fixture.path().join(".dart-decimaterc"))?.contains("ignorePatterns")
    );

    Ok(())
}
