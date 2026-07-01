use std::fs;

use dart_decimate::cli::run_from;
use serde_json::Value;

#[test]
fn impact_reports_disabled_local_value_contract() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "impact",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--quiet",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "dart-decimate.impact.v1");
    assert_eq!(json["kind"], "impact");
    assert_eq!(json["tool"], "dart-decimate");
    assert_eq!(json["command"], "impact");
    assert_eq!(json["enabled"], false);
    assert_eq!(json["enabled_source"], "default");
    assert_eq!(json["explicit_decision"], false);
    assert_eq!(json["onboarding_declined"], false);
    assert_eq!(json["record_count"], 0);
    assert_eq!(json["totals"]["surfaced"], 0);
    assert_eq!(json["trend"]["surfaced_delta"], 0);
    assert_eq!(json["gate"]["contained_commits"], 0);
    assert!(json["records"].as_array().is_some_and(Vec::is_empty));
    assert!(
        json["project"]["id"]
            .as_str()
            .is_some_and(|id| id.starts_with("dart-decimate:impact:"))
    );

    Ok(())
}

#[test]
fn impact_reads_local_history_jsonl() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let dart_decimate_dir = fixture.path().join(".dart-decimate");
    fs::create_dir(&dart_decimate_dir)?;
    fs::write(
        dart_decimate_dir.join("impact.jsonl"),
        concat!(
            r#"{"timestamp":"2026-06-01T00:00:00Z","surfaced":3,"resolved":1,"suppressed":0,"contained_commits":2}"#,
            "\n",
            r#"{"timestamp":"2026-06-02T00:00:00Z","surfaced":5,"resolved":3,"suppressed":1,"contained_commits":4}"#,
            "\n",
        ),
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "impact",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
            "--quiet",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "dart-decimate.impact.v1");
    assert_eq!(json["enabled"], true);
    assert_eq!(json["enabled_source"], "local-history");
    assert_eq!(json["record_count"], 2);
    assert_eq!(json["totals"]["surfaced"], 8);
    assert_eq!(json["totals"]["resolved"], 4);
    assert_eq!(json["totals"]["suppressed"], 1);
    assert_eq!(json["totals"]["contained_commits"], 6);
    assert_eq!(json["trend"]["surfaced_delta"], 2);
    assert_eq!(json["trend"]["resolved_delta"], 2);
    assert_eq!(json["trend"]["suppressed_delta"], 1);
    assert_eq!(json["gate"]["contained_commits"], 6);
    assert_eq!(json["records"][0]["timestamp"], "2026-06-01T00:00:00Z");
    assert_eq!(json["records"][1]["timestamp"], "2026-06-02T00:00:00Z");

    Ok(())
}

#[test]
fn impact_all_reports_empty_cross_repo_contract() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        [
            "dart-decimate",
            "impact",
            "--all",
            "--format",
            "json",
            "--sort",
            "surfaced",
            "--limit",
            "5",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "dart-decimate.impact.v1");
    assert_eq!(json["kind"], "impact-all");
    assert_eq!(json["command"], "impact --all");
    assert_eq!(json["summary"]["projects"], 0);
    assert_eq!(json["summary"]["record_count"], 0);
    assert!(json["projects"].as_array().is_some_and(Vec::is_empty));

    Ok(())
}

#[test]
fn schema_manifest_lists_impact_command() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(["dart-decimate", "schema", "--format", "json"], &mut output)?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schemas"]["impact"], "dart-decimate.impact.v1");
    assert!(json["commands"].as_array().is_some_and(|commands| {
        commands.iter().any(|command| {
            command["name"] == "impact" && command["schema"] == "dart-decimate.impact.v1"
        })
    }));

    Ok(())
}
