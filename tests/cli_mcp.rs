use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

use decimate::{manifest::decimate_schema, mcp::handle_message};
use serde_json::Value;
use tempfile::TempDir;

const MCP_VERSION: &str = "2025-11-25";

#[test]
fn mcp_initialize_and_tools_list_follow_json_rpc_contract() -> Result<(), Box<dyn std::error::Error>>
{
    let initialize_message = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": { "protocolVersion": MCP_VERSION }
    });
    let initialized = response(&serde_json::to_string(&initialize_message)?)?;

    assert_eq!(initialized["jsonrpc"], "2.0");
    assert_eq!(initialized["id"], 1);
    assert_eq!(initialized["result"]["protocolVersion"], MCP_VERSION);
    assert_eq!(initialized["result"]["serverInfo"]["name"], "decimate-mcp");
    assert_eq!(
        initialized["result"]["capabilities"]["tools"]["listChanged"],
        false
    );

    let tools = response(r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#)?;
    assert_eq!(tool_names(&tools), manifest_tool_names()?);
    let tool_defs = tools["result"]["tools"]
        .as_array()
        .ok_or_else(|| "tools/list result must contain tools array".to_owned())?;
    for tool in tool_defs {
        if tool["name"] == "fix_apply" {
            assert_eq!(tool["annotations"]["readOnlyHint"], false);
            assert_eq!(tool["annotations"]["destructiveHint"], true);
        } else {
            assert_eq!(tool["annotations"]["readOnlyHint"], true);
            assert_eq!(tool["annotations"]["destructiveHint"], false);
        }
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    }
    assert_tool_property(tool_defs, "analyze", "changed_workspaces")?;
    assert_tool_property(tool_defs, "code_execute", "code")?;
    assert_tool_property(tool_defs, "code_execute", "max_tool_calls")?;
    assert_tool_property(tool_defs, "analyze", "private_type_leaks")?;
    assert_tool_property(tool_defs, "analyze", "dart_platform")?;
    assert_tool_property(tool_defs, "check_changed", "since")?;
    assert_tool_property(tool_defs, "list_boundaries", "workspace")?;
    assert_tool_property(tool_defs, "inspect_target", "production")?;
    assert_tool_property(tool_defs, "analyze", "policy_pack")?;
    assert_tool_property(tool_defs, "check_health", "min_score")?;
    assert_tool_property(tool_defs, "check_runtime_coverage", "coverage")?;
    assert_tool_property(tool_defs, "get_blast_radius", "coverage")?;
    assert_tool_property(tool_defs, "impact", "root")?;
    assert_tool_property(tool_defs, "impact_all", "limit")?;
    assert_tool_property(tool_defs, "security_candidates", "gate")?;
    assert_tool_property(tool_defs, "security_candidates", "paths")?;
    assert_tool_property(tool_defs, "trace_clone", "file")?;
    assert_tool_property(tool_defs, "trace_clone", "min_tokens")?;
    assert_tool_property(tool_defs, "fix_preview", "action")?;
    assert_tool_property(tool_defs, "fix_apply", "yes")?;
    assert_tool_property(tool_defs, "audit", "gate")?;
    assert_tool_property(tool_defs, "audit", "dead_code_baseline")?;
    assert_tool_property(tool_defs, "fallow_explain", "rule_id")?;

    Ok(())
}

#[test]
fn mcp_call_code_execute_composes_read_only_tools() -> Result<(), Box<dyn std::error::Error>> {
    let message = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 15,
        "method": "tools/call",
        "params": {
            "name": "code_execute",
            "arguments": {
                "code": {
                    "steps": [
                        {
                            "id": "explain",
                            "call": "fallow_explain",
                            "arguments": { "issue_type": "unused-export" }
                        },
                        {
                            "id": "id",
                            "select": {
                                "from": "explain",
                                "pointer": "/structuredContent/id"
                            }
                        }
                    ],
                    "return": { "from": "id" }
                }
            }
        }
    });

    let output = response(&serde_json::to_string(&message)?)?;

    assert_eq!(output["result"]["isError"], false);
    assert_eq!(
        output["result"]["structuredContent"]["schema_version"],
        "decimate.mcp.code_execute.v1"
    );
    assert_eq!(output["result"]["structuredContent"]["ok"], true);
    assert_eq!(
        output["result"]["structuredContent"]["result"],
        "decimate/unused-export"
    );
    assert_eq!(
        output["result"]["structuredContent"]["calls"][0]["tool"],
        "fallow_explain"
    );

    Ok(())
}

#[test]
fn mcp_call_code_execute_can_ref_previous_results() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    let message = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 16,
        "method": "tools/call",
        "params": {
            "name": "code_execute",
            "arguments": {
                "code": {
                    "steps": [
                        {
                            "id": "info",
                            "call": "project_info",
                            "arguments": { "root": fixture.path(), "files": true }
                        },
                        {
                            "id": "first",
                            "select": {
                                "from": "info",
                                "pointer": "/structuredContent/files",
                                "where": { "equals": { "/path": "lib/main.dart" } },
                                "fields": ["path"],
                                "limit": 1
                            }
                        },
                        {
                            "id": "trace",
                            "call": "trace_file",
                            "arguments": {
                                "root": fixture.path(),
                                "file": { "$ref": { "from": "first", "pointer": "/0/path" } }
                            }
                        }
                    ],
                    "return": { "from": "trace", "pointer": "/structuredContent/path" }
                }
            }
        }
    });

    let output = response(&serde_json::to_string(&message)?)?;

    assert_eq!(output["result"]["isError"], false);
    assert_eq!(
        output["result"]["structuredContent"]["result"],
        "lib/main.dart"
    );
    assert_eq!(
        output["result"]["structuredContent"]["calls"]
            .as_array()
            .map_or(0, Vec::len),
        2
    );

    Ok(())
}

#[test]
fn mcp_call_code_execute_rejects_mutating_tools() -> Result<(), Box<dyn std::error::Error>> {
    let message = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 17,
        "method": "tools/call",
        "params": {
            "name": "code_execute",
            "arguments": {
                "code": {
                    "steps": [
                        {
                            "id": "fix",
                            "call": "fix_apply",
                            "arguments": { "yes": true }
                        }
                    ],
                    "return": { "from": "fix" }
                }
            }
        }
    });

    let output = response(&serde_json::to_string(&message)?)?;

    assert_eq!(output["result"]["isError"], true);
    assert!(
        output["result"]["structuredContent"]["error"]
            .as_str()
            .is_some_and(|error| error.contains("fix_apply"))
    );

    Ok(())
}

#[test]
fn mcp_call_code_execute_rejects_javascript_like_input() -> Result<(), Box<dyn std::error::Error>> {
    let message = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 18,
        "method": "tools/call",
        "params": {
            "name": "code_execute",
            "arguments": {
                "code": "return { ok: true };"
            }
        }
    });

    let output = response(&serde_json::to_string(&message)?)?;

    assert_eq!(output["result"]["isError"], true);
    assert!(
        output["result"]["structuredContent"]["error"]
            .as_str()
            .is_some_and(|error| error.contains("invalid code_execute program"))
    );

    Ok(())
}

#[test]
fn mcp_notifications_do_not_emit_responses() {
    assert!(handle_message(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
}

#[test]
fn mcp_protocol_errors_follow_json_rpc_codes() -> Result<(), Box<dyn std::error::Error>> {
    let parse_error = response("{")?;
    assert_eq!(parse_error["error"]["code"], -32700);

    let unknown_method = response(r#"{"jsonrpc":"2.0","id":9,"method":"missing/method"}"#)?;
    assert_eq!(unknown_method["error"]["code"], -32601);

    let invalid_arguments = response(
        r#"{"jsonrpc":"2.0","id":10,"method":"tools/call","params":{"name":"analyze","arguments":{"apply":true}}}"#,
    )?;
    assert_eq!(invalid_arguments["error"]["code"], -32602);
    assert!(
        invalid_arguments["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("does not accept argument apply"))
    );

    let non_object_arguments = response(
        r#"{"jsonrpc":"2.0","id":11,"method":"tools/call","params":{"name":"analyze","arguments":[]}}"#,
    )?;
    assert_eq!(non_object_arguments["error"]["code"], -32602);

    let ignored_root = response(
        r#"{"jsonrpc":"2.0","id":12,"method":"tools/call","params":{"name":"decimate_explain","arguments":{"issue_type":"unused-export","root":"."}}}"#,
    )?;
    assert_eq!(ignored_root["error"]["code"], -32602);

    let nested_unknown = response(
        r#"{"jsonrpc":"2.0","id":13,"method":"tools/call","params":{"name":"inspect_target","arguments":{"target":{"type":"file","file":"lib/main.dart","dependency":"foo"}}}}"#,
    )?;
    assert_eq!(nested_unknown["error"]["code"], -32602);

    Ok(())
}

#[test]
fn mcp_call_explain_returns_structured_content() -> Result<(), Box<dyn std::error::Error>> {
    let output = response(
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"decimate_explain","arguments":{"issue_type":"unused-export"}}}"#,
    )?;

    assert_eq!(output["id"], 3);
    assert_eq!(output["result"]["isError"], false);
    assert_eq!(output["result"]["_meta"]["exit_code"], 0);
    assert_eq!(
        output["result"]["structuredContent"]["id"],
        "decimate/unused-export"
    );
    assert!(
        output["result"]["content"][0]["text"]
            .as_str()
            .is_some_and(|text| text.contains("decimate/unused-export"))
    );

    Ok(())
}

#[test]
fn mcp_call_fallow_explain_returns_structured_content() -> Result<(), Box<dyn std::error::Error>> {
    let output = response(
        r#"{"jsonrpc":"2.0","id":14,"method":"tools/call","params":{"name":"fallow_explain","arguments":{"rule_id":"fallow/code-duplication"}}}"#,
    )?;

    assert_eq!(output["id"], 14);
    assert_eq!(output["result"]["isError"], false);
    assert_eq!(
        output["result"]["structuredContent"]["id"],
        "decimate/code-duplication"
    );

    Ok(())
}

#[test]
fn mcp_call_analyze_runs_read_only_decimate_report() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(&fixture, "lib/src/dead.dart", "class Dead {}\n")?;
    let message = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "analyze",
            "arguments": {
                "root": fixture.path(),
                "issue_types": ["unused-files"]
            }
        }
    });

    let output = response(&serde_json::to_string(&message)?)?;

    assert_eq!(output["result"]["isError"], false);
    assert_eq!(output["result"]["_meta"]["exit_code"], 1);
    assert_eq!(
        output["result"]["structuredContent"]["schema_version"],
        "decimate.report.v1"
    );
    assert_eq!(output["result"]["structuredContent"]["command"], "check");
    assert_eq!(
        output["result"]["structuredContent"]["summary"]["dead_files"],
        1
    );

    Ok(())
}

#[test]
fn mcp_call_list_boundaries_returns_project_list() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(
        &fixture,
        "decimate.json",
        r#"{ "boundaries": [{ "from": "lib/domain", "disallow": "lib/ui" }] }"#,
    )?;
    let message = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 14,
        "method": "tools/call",
        "params": {
            "name": "list_boundaries",
            "arguments": {
                "root": fixture.path(),
                "config": "decimate.json"
            }
        }
    });

    let output = response(&serde_json::to_string(&message)?)?;

    assert_eq!(output["result"]["isError"], false);
    assert_eq!(
        output["result"]["structuredContent"]["schema_version"],
        "decimate.list.v1"
    );
    assert_eq!(output["result"]["structuredContent"]["command"], "list");
    assert_eq!(
        output["result"]["structuredContent"]["boundaries"]["rules"][0]["from"],
        "lib/domain"
    );

    Ok(())
}

#[test]
fn mcp_call_runtime_coverage_uses_coverage_analyze() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() { print('hot'); }\n")?;
    write(
        &fixture,
        "coverage/coverage-final.json",
        &serde_json::json!({
            "main.dart": {
                "path": fixture.path().join("lib/main.dart"),
                "statementMap": { "0": { "start": { "line": 1 }, "end": { "line": 1 } } },
                "s": { "0": 20 },
                "fnMap": { "0": { "name": "main", "decl": { "start": { "line": 1 } } } },
                "f": { "0": 20 }
            }
        })
        .to_string(),
    )?;
    let message = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "check_runtime_coverage",
            "arguments": {
                "root": fixture.path(),
                "coverage": "coverage/coverage-final.json",
                "min_invocations_hot": 10
            }
        }
    });

    let output = response(&serde_json::to_string(&message)?)?;

    assert_eq!(output["result"]["isError"], false);
    assert_eq!(
        output["result"]["structuredContent"]["kind"],
        "runtime-coverage"
    );
    assert_eq!(
        output["result"]["structuredContent"]["runtime_coverage"]["hot_paths"][0]["path"],
        "lib/main.dart"
    );

    Ok(())
}

#[test]
fn mcp_call_impact_returns_read_only_report() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    let message = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "tools/call",
        "params": {
            "name": "impact",
            "arguments": { "root": fixture.path() }
        }
    });

    let output = response(&serde_json::to_string(&message)?)?;

    assert_eq!(output["result"]["isError"], false);
    assert_eq!(
        output["result"]["structuredContent"]["schema_version"],
        "decimate.impact.v1"
    );
    assert_eq!(output["result"]["structuredContent"]["kind"], "impact");
    assert_eq!(output["result"]["structuredContent"]["enabled"], false);

    Ok(())
}

#[test]
fn mcp_call_fix_preview_does_not_modify_files() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = fix_fixture()?;
    let message = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "tools/call",
        "params": {
            "name": "fix_preview",
            "arguments": {
                "root": fixture.path(),
                "entry": ["lib/main.dart"],
                "action": ["delete-file"]
            }
        }
    });

    let output = response(&serde_json::to_string(&message)?)?;

    assert_eq!(output["result"]["isError"], false);
    assert_eq!(
        output["result"]["structuredContent"]["schema_version"],
        "decimate.fix.v1"
    );
    assert_eq!(output["result"]["structuredContent"]["mode"], "dry-run");
    assert_eq!(
        output["result"]["structuredContent"]["summary"]["planned"],
        1
    );
    assert!(fixture.path().join("lib/dead.dart").exists());

    Ok(())
}

#[test]
fn mcp_call_fix_apply_requires_yes_true() -> Result<(), Box<dyn std::error::Error>> {
    let output = response(
        r#"{"jsonrpc":"2.0","id":8,"method":"tools/call","params":{"name":"fix_apply","arguments":{"root":"/tmp","yes":false}}}"#,
    )?;

    assert_eq!(output["error"]["code"], -32602);
    assert!(
        output["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("fix_apply requires yes: true"))
    );

    Ok(())
}

#[test]
fn mcp_call_fix_apply_applies_confirmed_safe_changes() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = fix_fixture()?;
    let message = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 9,
        "method": "tools/call",
        "params": {
            "name": "fix_apply",
            "arguments": {
                "root": fixture.path(),
                "entry": ["lib/main.dart"],
                "action": ["delete-file"],
                "yes": true
            }
        }
    });

    let output = response(&serde_json::to_string(&message)?)?;

    assert_eq!(output["result"]["isError"], false);
    assert_eq!(output["result"]["structuredContent"]["mode"], "apply");
    assert_eq!(
        output["result"]["structuredContent"]["summary"]["applied"],
        1
    );
    assert!(!fixture.path().join("lib/dead.dart").exists());

    Ok(())
}

#[test]
fn decimate_mcp_binary_serves_stdio_json_rpc() -> Result<(), Box<dyn std::error::Error>> {
    let mut child = Command::new(env!("CARGO_BIN_EXE_decimate-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    {
        let Some(mut stdin) = child.stdin.take() else {
            return Err("child stdin".into());
        };
        let initialize_message = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": { "protocolVersion": MCP_VERSION }
        });
        writeln!(stdin, "{}", serde_json::to_string(&initialize_message)?)?;
        writeln!(stdin, r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list"}}"#)?;
    }
    let output = child.wait_with_output()?;
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout)?;
    let mut lines = stdout.lines();
    let first = lines
        .next()
        .ok_or_else(|| "missing initialize response".to_owned())?;
    let second = lines
        .next()
        .ok_or_else(|| "missing tools/list response".to_owned())?;
    let initialize = serde_json::from_str::<Value>(first)?;
    let tools = serde_json::from_str::<Value>(second)?;

    assert_eq!(initialize["result"]["serverInfo"]["name"], "decimate-mcp");
    assert!(tool_names(&tools).iter().any(|name| name == "analyze"));

    Ok(())
}

fn response(message: &str) -> Result<Value, Box<dyn std::error::Error>> {
    handle_message(message).ok_or_else(|| "expected JSON-RPC response".into())
}

fn tool_names(response: &Value) -> Vec<String> {
    response["result"]["tools"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|tool| tool["name"].as_str())
        .map(str::to_owned)
        .collect()
}

fn assert_tool_property(
    tools: &[Value],
    name: &str,
    property: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let tool = tools
        .iter()
        .find(|tool| tool["name"] == name)
        .ok_or_else(|| format!("missing MCP tool {name}"))?;
    if tool["inputSchema"]["properties"].get(property).is_none() {
        return Err(format!("missing MCP property {name}.{property}").into());
    }
    Ok(())
}

fn manifest_tool_names() -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let schema = decimate_schema();
    let tools = schema["mcp_tools"]["tools"]
        .as_array()
        .ok_or_else(|| "manifest mcp_tools.tools must be an array".to_owned())?;
    Ok(tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .map(str::to_owned)
        .collect())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}

fn fix_fixture() -> Result<TempDir, std::io::Error> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(&fixture, "lib/main.dart", "void main() {}\n")?;
    write(&fixture, "lib/dead.dart", "class Dead {}\n")?;
    Ok(fixture)
}
