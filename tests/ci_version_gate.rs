use std::fs;

fn missing(needle: &str) -> std::io::Error {
    std::io::Error::other(format!("missing {needle}"))
}

fn index_of(contents: &str, needle: &str) -> Result<usize, Box<dyn std::error::Error>> {
    contents.find(needle).ok_or_else(|| missing(needle).into())
}

fn section_between<'a>(
    contents: &'a str,
    start: &str,
    end: &str,
) -> Result<&'a str, Box<dyn std::error::Error>> {
    let start_index = index_of(contents, start)?;
    let tail = &contents[start_index..];
    let end_index = index_of(tail, end)?;
    Ok(&tail[..end_index])
}

#[test]
fn pull_request_ci_requires_unpublished_package_version() -> Result<(), Box<dyn std::error::Error>>
{
    let ci = fs::read_to_string(".github/workflows/ci.yml")?;
    let template = fs::read_to_string(".github/pull_request_template.md")?;

    assert!(ci.contains("if: github.event_name == 'pull_request'"));
    assert!(ci.contains("run: npm run release:check"));
    assert!(!ci.contains("Package versions unchanged; skipping release version check."));
    assert!(template.contains(
        "This PR bumps both `Cargo.toml` and `package.json` to an unpublished `dart-decimate` version"
    ));
    assert!(!template.contains("Version is intentionally unchanged"));

    Ok(())
}

#[test]
fn release_workflow_checks_existing_state_before_release_version()
-> Result<(), Box<dyn std::error::Error>> {
    let release = fs::read_to_string(".github/workflows/release.yml")?;
    let state_index = index_of(&release, "      - name: Check existing release state")?;
    let release_check_index = index_of(&release, "      - name: Check release version")?;
    let release_check = section_between(
        &release,
        "      - name: Check release version",
        "      - name: Validate release candidate",
    )?;
    let validate = section_between(
        &release,
        "      - name: Validate release candidate",
        "      - name: Download release assets",
    )?;

    assert!(state_index < release_check_index);
    assert!(release_check.contains("if: steps.state.outputs.npm_exists == 'false'"));
    assert!(release_check.contains("run: npm run release:check"));

    assert!(validate.contains("npm run version:check"));
    assert!(!validate.contains("npm run release:check"));
    assert!(validate.contains("npm run migration:check"));

    Ok(())
}

#[test]
fn release_workflow_rejects_fresh_reused_versions_but_allows_repairs()
-> Result<(), Box<dyn std::error::Error>> {
    let release = fs::read_to_string(".github/workflows/release.yml")?;
    let state = section_between(
        &release,
        "      - name: Check existing release state",
        "      - name: Reject reused release tag",
    )?;
    let reused_version = section_between(
        &release,
        "      - name: Reject reused package version",
        "      - name: Check release version",
    )?;

    assert!(state.contains("tag_points_at_head=true"));
    assert!(state.contains("tag_points_at_head=false"));
    assert!(state.contains("npm_exists=true"));
    assert!(state.contains("npm_exists=false"));

    assert!(reused_version.contains("steps.state.outputs.npm_exists == 'true'"));
    assert!(reused_version.contains("github.event_name == 'push'"));
    assert!(reused_version.contains("github.run_attempt == 1"));
    assert!(reused_version.contains("steps.state.outputs.tag_points_at_head == 'false'"));
    assert!(
        reused_version.contains(
            "dart-decimate@$VERSION is already published; bump Cargo.toml and package.json"
        )
    );

    Ok(())
}
