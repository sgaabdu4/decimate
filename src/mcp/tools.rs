use serde_json::{Map, Value, json};

pub(super) fn tools() -> Value {
    let mut values = Vec::new();
    values.extend(overview_tools());
    values.extend(trace_tools());
    values.extend(analysis_tools());
    values.extend(change_tools());
    Value::Array(values)
}

fn overview_tools() -> Vec<Value> {
    vec![
        tool(
            "analyze",
            "Run Decimate check and return the JSON report.",
            schema(
                &[
                    ("root", "string"),
                    ("config", "string"),
                    ("issue_types", "array"),
                    ("entry", "array"),
                    ("file", "array"),
                    ("workspace", "array"),
                    ("changed_since", "string"),
                    ("runtime_coverage", "string"),
                    ("production", "boolean"),
                ],
                &[],
            ),
        ),
        tool(
            "project_info",
            "List Decimate project metadata.",
            schema(
                &[
                    ("root", "string"),
                    ("config", "string"),
                    ("files", "boolean"),
                    ("entry_points", "boolean"),
                    ("plugins", "boolean"),
                    ("boundaries", "boolean"),
                    ("workspaces", "boolean"),
                    ("file", "array"),
                    ("workspace", "array"),
                ],
                &[],
            ),
        ),
        tool(
            "inspect_target",
            "Inspect one Dart file or symbol.",
            schema(
                &[
                    ("root", "string"),
                    ("config", "string"),
                    ("target", "target"),
                    ("file", "string"),
                    ("symbol", "string"),
                ],
                &[],
            ),
        ),
    ]
}

fn trace_tools() -> Vec<Value> {
    vec![
        tool(
            "trace_file",
            "Trace a Dart file.",
            schema(
                &[("root", "string"), ("config", "string"), ("file", "string")],
                &["file"],
            ),
        ),
        tool(
            "trace_export",
            "Trace a top-level Dart symbol.",
            schema(
                &[
                    ("root", "string"),
                    ("config", "string"),
                    ("file", "string"),
                    ("symbol", "string"),
                    ("export_name", "string"),
                ],
                &["file"],
            ),
        ),
        tool(
            "trace_dependency",
            "Trace pub dependency usage.",
            schema(
                &[
                    ("root", "string"),
                    ("config", "string"),
                    ("dependency", "string"),
                    ("package_name", "string"),
                ],
                &[],
            ),
        ),
        tool(
            "trace_clone",
            "Trace a duplicate-code clone group.",
            schema(
                &[
                    ("root", "string"),
                    ("config", "string"),
                    ("fingerprint", "string"),
                ],
                &["fingerprint"],
            ),
        ),
    ]
}

fn analysis_tools() -> Vec<Value> {
    vec![
        tool(
            "find_dupes",
            "Find duplicated Dart code blocks.",
            schema(
                &[
                    ("root", "string"),
                    ("config", "string"),
                    ("mode", "string"),
                    ("min_tokens", "integer"),
                    ("min_lines", "integer"),
                    ("min_occurrences", "integer"),
                    ("top", "integer"),
                ],
                &[],
            ),
        ),
        tool(
            "check_health",
            "Run Decimate health checks.",
            schema(
                &[
                    ("root", "string"),
                    ("config", "string"),
                    ("max_cyclomatic", "integer"),
                    ("max_cognitive", "integer"),
                    ("max_crap", "integer"),
                    ("coverage", "string"),
                    ("runtime_coverage", "string"),
                    ("coverage_gaps", "boolean"),
                    ("file_scores", "boolean"),
                    ("hotspots", "boolean"),
                    ("targets", "boolean"),
                    ("ownership", "boolean"),
                    ("complexity_breakdown", "boolean"),
                ],
                &[],
            ),
        ),
        tool(
            "security_candidates",
            "Surface security review candidates.",
            schema(
                &[
                    ("root", "string"),
                    ("config", "string"),
                    ("top", "integer"),
                    ("file", "array"),
                    ("surface", "boolean"),
                    ("production", "boolean"),
                ],
                &[],
            ),
        ),
        tool(
            "feature_flags",
            "Inventory Dart and Flutter feature flags.",
            schema(
                &[
                    ("root", "string"),
                    ("config", "string"),
                    ("top", "integer"),
                    ("changed_since", "string"),
                ],
                &[],
            ),
        ),
    ]
}

fn change_tools() -> Vec<Value> {
    vec![
        tool(
            "audit",
            "Run changed-code audit.",
            schema(
                &[
                    ("root", "string"),
                    ("config", "string"),
                    ("base", "string"),
                    ("brief", "boolean"),
                    ("max_decisions", "integer"),
                ],
                &["base"],
            ),
        ),
        tool(
            "decision_surface",
            "Surface changed-code architecture decisions.",
            schema(
                &[
                    ("root", "string"),
                    ("config", "string"),
                    ("base", "string"),
                    ("max_decisions", "integer"),
                ],
                &["base"],
            ),
        ),
        tool(
            "decimate_explain",
            "Explain a Decimate issue type.",
            schema(&[("issue_type", "string"), ("rule_id", "string")], &[]),
        ),
    ]
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    let mut tool = Map::new();
    tool.insert("name".to_owned(), Value::String(name.to_owned()));
    tool.insert(
        "description".to_owned(),
        Value::String(description.to_owned()),
    );
    tool.insert("inputSchema".to_owned(), input_schema);
    tool.insert(
        "annotations".to_owned(),
        json!({
            "readOnlyHint": true,
            "destructiveHint": false,
            "openWorldHint": false
        }),
    );
    Value::Object(tool)
}

fn schema(properties: &[(&str, &str)], required: &[&str]) -> Value {
    let properties = properties
        .iter()
        .map(|(name, kind)| ((*name).to_owned(), property_schema(kind)))
        .collect::<Map<_, _>>();
    json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

fn property_schema(kind: &str) -> Value {
    match kind {
        "array" => json!({ "type": "array", "items": { "type": "string" } }),
        "object" => json!({ "type": "object" }),
        "target" => target_schema(),
        _ => json!({ "type": kind }),
    }
}

fn target_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "type": { "type": "string", "enum": ["file", "symbol"] },
            "file": { "type": "string" },
            "symbol": { "type": "string" },
            "export_name": { "type": "string" }
        },
        "required": ["type"],
        "additionalProperties": false
    })
}
