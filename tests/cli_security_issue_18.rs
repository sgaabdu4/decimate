use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn check_command_points_representative_security_findings_to_surface()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "const accessToken = '0123456789abcdef0123456789abcdef';
const refreshToken = 'fedcba9876543210fedcba9876543210';
",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["security_candidates"], 1);
    assert_eq!(json["summary"]["security_candidate_occurrences"], 2);
    assert_eq!(json["summary"]["findings"], 1);
    assert!(json["next_steps"].as_array().is_some_and(|steps| {
        steps.iter().any(|step| {
            step["id"] == "review-security-surface"
                && step["command"] == "dart-decimate security . --format json --surface"
        })
    }));

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
