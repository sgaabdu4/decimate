use decimate::cli::run_from;
use serde_json::Value;

#[test]
fn migrate_returns_structured_not_applicable_report() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        ["decimate", "migrate", "--dry-run", "--format", "json"],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 2);
    assert_eq!(json["schema_version"], "decimate.unsupported.v1");
    assert_eq!(json["command"], "migrate");
    assert_eq!(json["status"], "not-applicable");
    assert_eq!(json["supported"], false);

    Ok(())
}

#[test]
fn telemetry_status_reports_disabled_without_error() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();

    let code = run_from(
        ["decimate", "telemetry", "status", "--format", "json"],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["schema_version"], "decimate.unsupported.v1");
    assert_eq!(json["command"], "telemetry status");
    assert_eq!(json["status"], "disabled");

    Ok(())
}

#[test]
fn license_activate_returns_structured_unsupported_report() -> Result<(), Box<dyn std::error::Error>>
{
    let mut output = Vec::new();

    let code = run_from(
        ["decimate", "license", "activate", "--format", "json"],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 2);
    assert_eq!(json["schema_version"], "decimate.unsupported.v1");
    assert_eq!(json["command"], "license activate");
    assert_eq!(json["status"], "not-required");

    Ok(())
}
