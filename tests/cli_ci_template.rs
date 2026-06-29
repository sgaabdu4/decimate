use std::fs;

use decimate::cli::run_from;
use serde_json::Value;

#[test]
fn github_ci_template_emits_yaml_workflow() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        ["decimate", "ci-template", "github", "--format", "yaml"],
        &mut output,
    )?;

    let yaml = String::from_utf8(output)?;
    assert_eq!(code, 0);
    assert!(yaml.contains("name: Decimate"));
    assert!(yaml.contains("pull_request:"));
    assert!(yaml.contains("decimate audit --format json --base"));

    Ok(())
}

#[test]
fn gitlab_ci_template_emits_yaml_template() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        ["decimate", "ci-template", "gitlab", "--format", "yaml"],
        &mut output,
    )?;

    let yaml = String::from_utf8(output)?;
    assert_eq!(code, 0);
    assert!(yaml.contains("stages:"));
    assert!(yaml.contains("decimate:"));
    assert!(yaml.contains("decimate audit --format json --base"));

    Ok(())
}

#[test]
fn ci_template_json_envelope_lists_target_path_and_content()
-> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        ["decimate", "ci-template", "gitlab", "--format", "json"],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "decimate.ci-template.v1");
    assert_eq!(json["kind"], "ci-template");
    assert_eq!(json["platform"], "gitlab");
    assert_eq!(json["vendor"], false);
    assert_eq!(json["files"][0]["path"], ".gitlab-ci.yml");
    assert!(
        json["files"][0]["content"]
            .as_str()
            .is_some_and(|content| content.contains("decimate audit --format json --base"))
    );

    Ok(())
}

#[test]
fn gitlab_vendor_writes_scoped_files_and_refuses_overwrite()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let root = fixture.path().to_str().unwrap_or(".");
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "ci-template",
            "gitlab",
            "--format",
            "json",
            "--vendor",
            "--root",
            root,
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["vendor"], true);
    assert!(fixture.path().join("ci/gitlab-ci.yml").is_file());
    assert!(fixture.path().join("ci/scripts/review.sh").is_file());
    assert!(fixture.path().join("ci/scripts/comment.sh").is_file());
    assert!(
        fs::read_to_string(fixture.path().join("ci/scripts/review.sh"))?
            .contains("decimate audit --format json --base")
    );

    let error = match run_from(
        [
            "decimate",
            "ci-template",
            "gitlab",
            "--vendor",
            "--root",
            root,
        ],
        &mut Vec::new(),
    ) {
        Ok(code) => panic!("vendor should refuse overwrite, got exit code {code}"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("refusing to overwrite"));

    Ok(())
}

#[test]
fn manifest_lists_ci_template_command() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(["decimate", "schema", "--format", "json"], &mut output)?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schemas"]["ci_template"], "decimate.ci-template.v1");
    assert!(json["commands"].as_array().is_some_and(|commands| {
        commands.iter().any(|command| {
            command["name"] == "ci-template" && command["schema"] == "decimate.ci-template.v1"
        })
    }));

    Ok(())
}
