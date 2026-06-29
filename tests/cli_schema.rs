use decimate::cli::run_from;
use serde_json::Value;

#[test]
fn schema_command_emits_agent_manifest() -> Result<(), Box<dyn std::error::Error>> {
    let json = schema_json()?;
    assert_eq!(json["schema_version"], "decimate.schema.v1");
    assert_eq!(json["kind"], "schema");
    assert!(
        json["commands"]
            .as_array()
            .is_some_and(|commands| commands.iter().any(|command| command["name"] == "inspect"))
    );
    assert!(json["commands"].as_array().is_some_and(|commands| {
        commands.iter().any(|command| {
            command["name"] == "decision-surface"
                && command["schema"] == "decimate.decision-surface.v1"
        })
    }));
    assert_eq!(json["schemas"]["coverage"], "decimate.coverage.v1");
    assert_eq!(json["schemas"]["init"], "decimate.init.v1");
    assert_eq!(json["schemas"]["hooks"], "decimate.hooks.v1");
    assert!(json["commands"].as_array().is_some_and(|commands| {
        commands.iter().any(|command| {
            command["name"] == "coverage analyze"
                && command["kind"] == "runtime-coverage"
                && command["schema"] == "decimate.coverage.v1"
                && command["flags"]
                    .as_array()
                    .is_some_and(|flags| flags.iter().any(|flag| flag == "--runtime-coverage"))
        })
    }));
    assert!(json["commands"].as_array().is_some_and(|commands| {
        commands.iter().any(|command| {
            command["name"] == "trace-symbol" && command["schema"] == "decimate.trace.v1"
        })
    }));
    assert!(
        json["issue_types"]
            .as_array()
            .is_some_and(|issues| issues.iter().any(|issue| issue == "unused-export"))
    );
    assert!(
        json["issue_types"]
            .as_array()
            .is_some_and(|issues| issues.iter().any(|issue| issue == "unused-type"))
    );
    assert!(
        json["issue_types"]
            .as_array()
            .is_some_and(|issues| issues.iter().any(|issue| issue == "private-type-leak"))
    );
    assert!(
        json["issue_types"]
            .as_array()
            .is_some_and(|issues| issues.iter().any(|issue| issue == "unused-widget-param"))
    );
    assert!(
        json["issue_types"]
            .as_array()
            .is_some_and(|issues| { issues.iter().any(|issue| issue == "private-widget-class") })
    );
    assert!(json["issue_types"].as_array().is_some_and(|issues| {
        issues
            .iter()
            .any(|issue| issue == "widget-top-level-function-boundary")
    }));
    assert!(
        json["issue_types"]
            .as_array()
            .is_some_and(|issues| issues.iter().any(|issue| issue == "boundary-coverage"))
    );
    assert!(json["issue_types"].as_array().is_some_and(|issues| {
        issues
            .iter()
            .any(|issue| issue == "boundary-call-violation")
    }));
    assert!(
        json["issue_types"]
            .as_array()
            .is_some_and(|issues| issues.iter().any(|issue| issue == "policy-violation"))
    );
    assert!(json["issue_types"].as_array().is_some_and(|issues| {
        issues
            .iter()
            .any(|issue| issue == "missing-suppression-reason")
    }));
    assert!(json["task_matrix"].as_array().is_some_and(|tasks| {
        tasks.iter().any(|task| {
            task["intent"] == "trace a top-level symbol"
                && task["command"]
                    .as_str()
                    .is_some_and(|command| command.contains("decimate inspect"))
        })
    }));

    Ok(())
}

#[test]
fn schema_command_lists_actual_cli_flags() -> Result<(), Box<dyn std::error::Error>> {
    let json = schema_json()?;

    assert_manifest_flags(
        &json,
        "check",
        &["--baseline", "--max-crap", "--min-occurrences"],
    );
    assert_manifest_flags(
        &json,
        "audit",
        &[
            "--dead-code-baseline",
            "--no-production",
            "--complexity-breakdown",
        ],
    );
    assert_manifest_flags(
        &json,
        "dead-code",
        &["--changed-workspaces", "--save-baseline"],
    );
    assert_manifest_flags(
        &json,
        "dupes",
        &["--min-occurrences", "--no-ignore-imports"],
    );
    assert_manifest_flags(&json, "health", &["--max-crap", "--min-score", "--top"]);
    assert_manifest_flags(
        &json,
        "trace-clone",
        &["--min-occurrences", "--fingerprint"],
    );
    assert_manifest_flags(&json, "list", &["--files", "--entry-points", "--plugins"]);
    assert_manifest_flags(
        &json,
        "fix",
        &["--config", "--workspace", "--changed-workspaces"],
    );
    assert_manifest_flags(&json, "init", &["--format", "--agents", "--force"]);
    assert_manifest_flags(&json, "hooks", &["install", "--target", "--branch"]);
    assert_manifest_omits_flags(&json, "list", &["--section"]);

    Ok(())
}

fn schema_json() -> Result<Value, Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(["decimate", "schema", "--format", "json"], &mut output)?;
    assert_eq!(code, 0);
    Ok(serde_json::from_slice::<Value>(&output)?)
}

fn assert_manifest_flags(json: &Value, command_name: &str, expected: &[&str]) {
    let command = manifest_command(json, command_name);
    for flag in expected {
        assert!(
            command["flags"]
                .as_array()
                .is_some_and(|flags| flags.iter().any(|candidate| candidate == flag)),
            "{command_name} missing {flag}"
        );
    }
}

fn assert_manifest_omits_flags(json: &Value, command_name: &str, unexpected: &[&str]) {
    let command = manifest_command(json, command_name);
    for flag in unexpected {
        assert!(
            command["flags"]
                .as_array()
                .is_none_or(|flags| flags.iter().all(|candidate| candidate != flag)),
            "{command_name} unexpectedly lists {flag}"
        );
    }
}

fn manifest_command<'a>(json: &'a Value, command_name: &str) -> &'a Value {
    json["commands"]
        .as_array()
        .and_then(|commands| {
            commands
                .iter()
                .find(|command| command["name"] == command_name)
        })
        .unwrap_or_else(|| panic!("missing manifest command {command_name}"))
}
