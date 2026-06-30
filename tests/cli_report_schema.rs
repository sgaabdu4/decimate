use decimate::cli::run_from;
use serde_json::Value;

#[test]
fn report_schema_types_inventory_arrays() -> Result<(), Box<dyn std::error::Error>> {
    let json = report_schema_json()?;

    for (property, definition) in [
        ("clone_groups", "clone_group"),
        ("complexity", "complexity_finding"),
        ("file_scores", "file_health_score"),
        ("hotspots", "health_hotspot"),
        ("refactoring_targets", "refactoring_target"),
        ("feature_flags", "feature_flag"),
        ("security_candidates", "security_candidate"),
        ("attack_surface", "attack_surface"),
    ] {
        assert_eq!(
            json["properties"][property]["items"]["$ref"],
            format!("#/$defs/{definition}")
        );
        assert_eq!(
            json["$defs"][definition]["additionalProperties"], false,
            "{definition} should reject unknown properties"
        );
    }

    assert_eq!(
        json["$defs"]["feature_flag"]["properties"]["source"]["enum"],
        serde_json::json!([
            "compile-time-environment",
            "process-environment",
            "sdk-call"
        ])
    );
    assert_array_contains(
        &json["$defs"]["security_candidate"]["properties"]["category"]["enum"],
        "plain-secret-storage",
    );
    for field in ["finding_id", "cwe", "candidate", "trace"] {
        assert_array_contains(&json["$defs"]["security_candidate"]["required"], field);
    }
    assert_eq!(
        json["$defs"]["security_candidate"]["properties"]["reachability"]["properties"]["taint_confidence"]
            ["enum"][0],
        "module-level"
    );
    assert_eq!(
        json["$defs"]["security_occurrence"]["properties"]["reachability"]["properties"]["reachable_from_entrypoint"]
            ["type"],
        "boolean"
    );
    assert_array_contains(
        &json["$defs"]["complexity_finding"]["required"],
        "contributions",
    );
    assert_array_contains(&json["$defs"]["clone_group"]["required"], "instances");
    for kind in [
        "route-collision",
        "unused-widget-param",
        "private-widget-class",
        "widget-top-level-function-boundary",
        "unrendered-widget",
        "missing-context-mounted-after-await",
    ] {
        assert_array_contains(
            &json["$defs"]["finding"]["properties"]["kind"]["enum"],
            kind,
        );
    }
    for field in [
        "quality_score",
        "route_collisions",
        "private_widget_classes",
        "widget_top_level_functions",
        "unused_widget_params",
        "unrendered_widgets",
        "missing_context_mounted_after_await",
    ] {
        assert_array_contains(&json["$defs"]["summary"]["required"], field);
        assert_eq!(
            json["$defs"]["summary"]["properties"][field]["type"],
            "integer"
        );
    }

    Ok(())
}

fn report_schema_json() -> Result<Value, Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let code = run_from(
        ["decimate", "report-schema", "--format", "json"],
        &mut output,
    )?;
    assert_eq!(code, 0);
    Ok(serde_json::from_slice::<Value>(&output)?)
}

fn assert_array_contains(array: &Value, expected: &str) {
    assert!(
        array
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item == expected)),
        "expected array to contain {expected}"
    );
}
