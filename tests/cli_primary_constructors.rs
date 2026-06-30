use std::fs;

use decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn primary_constructor_private_type_leaks_are_reported() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: package\n")?;
    write(
        &fixture,
        "lib/package.dart",
        "\
class Api(final _Hidden hidden);
class _Hidden {}
",
    )?;

    let (code, json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--entry",
        "lib/package.dart",
        "--private-type-leaks",
    ])?;

    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "decimate/private-type-leak")
    }) else {
        panic!("primary constructor private type leak finding");
    };
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["private_type_leaks"], 1);
    assert_eq!(finding["kind"], "private-type-leak");
    assert_eq!(finding["path"], "lib/package.dart");
    assert_eq!(finding["line"], 1);
    assert_eq!(finding["safe_to_delete"], false);
    assert_eq!(finding["actions"][0]["action"], "review-public-api");
    assert_eq!(finding["actions"][0]["target_symbol"], "Api");

    Ok(())
}

fn run_json<I, S>(args: I) -> Result<(i32, Value), Box<dyn std::error::Error>>
where
    I: IntoIterator<Item = S>,
    S: Into<std::ffi::OsString> + Clone,
{
    let mut output = Vec::new();
    let code = run_from(args, &mut output)?;
    let json = serde_json::from_slice::<Value>(&output)?;
    Ok((code, json))
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
