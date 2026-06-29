use std::fs;

use decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn list_boundaries_reports_zones_rules_and_uncovered_files()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = boundary_fixture()?;
    write(
        &fixture,
        ".decimaterc",
        "[[boundary]]\nfrom = \"lib/domain\"\ndisallow = \"lib/ui\"\n",
    )?;

    let (code, json) = run_json([
        "decimate",
        "list",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--boundaries",
    ])?;

    assert_eq!(code, 0);
    assert_eq!(json["boundaries"]["configured"], true);
    assert_eq!(json["summary"]["boundary_zones"], 2);
    assert_eq!(json["boundaries"]["zones"][0]["path"], "lib/domain");
    assert_eq!(json["boundaries"]["zones"][0]["file_count"], 1);
    assert_eq!(json["boundaries"]["rules"][0]["from"], "lib/domain");
    assert_eq!(json["boundaries"]["rules"][0]["disallow"], "lib/ui");
    assert!(
        json["boundaries"]["uncovered_files"]
            .as_array()
            .is_some_and(|files| files.iter().any(|file| file == "lib/data/repository.dart"))
    );
    assert_eq!(json["files"].as_array().map(Vec::len), Some(0));

    Ok(())
}

#[test]
fn check_boundary_coverage_is_opt_in_and_actionable() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = boundary_fixture()?;

    let (default_code, default_json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--boundary",
        "lib/domain:lib/ui",
    ])?;
    assert_eq!(default_code, 0);
    assert_eq!(default_json["summary"]["boundary_coverage"], 0);

    let (code, json) = run_json([
        "decimate",
        "check",
        fixture.path().to_str().unwrap_or("."),
        "--format",
        "json",
        "--boundary",
        "lib/domain:lib/ui",
        "--boundary-coverage",
    ])?;

    let finding = &json["findings"][0];
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["boundary_coverage"], 1);
    assert_eq!(finding["rule_id"], "decimate/boundary-violation");
    assert_eq!(finding["kind"], "boundary-coverage");
    assert_eq!(finding["path"], "lib/data/repository.dart");
    assert_eq!(finding["actions"][0]["action"], "assign-boundary");
    assert_eq!(
        finding["actions"][0]["suppression_comment"],
        "// decimate-ignore-next-line boundary-violation"
    );

    Ok(())
}

#[test]
fn boundary_coverage_can_be_enabled_by_config_and_suppressed()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/domain/model.dart", "class Model {}\n")?;
    write(&fixture, "lib/ui/page.dart", "class Page {}\n")?;
    write(
        &fixture,
        "lib/data/repository.dart",
        "// decimate-ignore-next-line boundary-violation\nclass Repository {}\n",
    )?;
    write(
        &fixture,
        ".decimaterc",
        "[cli]\nformat = \"json\"\nboundaryCoverage = true\nboundary = [\"lib/domain:lib/ui\"]\n",
    )?;

    let (code, json) = run_json(["decimate", "check", fixture.path().to_str().unwrap_or(".")])?;

    assert_eq!(code, 0);
    assert_eq!(json["summary"]["boundary_coverage"], 0);
    assert_eq!(json["summary"]["findings"], 0);

    Ok(())
}

#[test]
fn boundary_coverage_rule_can_be_disabled() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = boundary_fixture()?;
    write(
        &fixture,
        ".decimaterc",
        "\
[cli]
format = \"json\"
boundaryCoverage = true
boundary = [\"lib/domain:lib/ui\"]

[rules]
boundary-coverage = \"off\"
",
    )?;

    let (code, json) = run_json(["decimate", "check", fixture.path().to_str().unwrap_or(".")])?;

    assert_eq!(code, 0);
    assert_eq!(json["summary"]["boundary_coverage"], 0);
    assert_eq!(json["summary"]["findings"], 0);

    Ok(())
}

fn boundary_fixture() -> Result<TempDir, std::io::Error> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/domain/model.dart", "class Model {}\n")?;
    write(&fixture, "lib/ui/page.dart", "class Page {}\n")?;
    write(
        &fixture,
        "lib/data/repository.dart",
        "class Repository {}\n",
    )?;
    Ok(fixture)
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
