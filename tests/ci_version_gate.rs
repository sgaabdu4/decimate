use std::fs;

#[test]
fn pull_request_ci_requires_unpublished_package_version() -> Result<(), Box<dyn std::error::Error>>
{
    let ci = fs::read_to_string(".github/workflows/ci.yml")?;

    assert!(ci.contains("if: github.event_name == 'pull_request'"));
    assert!(ci.contains("run: npm run release:check"));
    assert!(!ci.contains("Package versions unchanged; skipping release version check."));

    Ok(())
}

#[test]
fn release_workflow_rejects_reused_package_versions() -> Result<(), Box<dyn std::error::Error>> {
    let release = fs::read_to_string(".github/workflows/release.yml")?;
    let validate = release
        .split("      - name: Validate release candidate")
        .nth(1)
        .and_then(|section| {
            section
                .split("      - name: Download release assets")
                .next()
        })
        .ok_or("missing release validation step")?;

    assert!(validate.contains("npm run version:check"));
    assert!(validate.contains("npm run release:check"));
    assert!(validate.contains("npm run migration:check"));

    Ok(())
}
