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
        assert_eq!(tool["annotations"]["readOnlyHint"], true);
        assert_eq!(tool["annotations"]["destructiveHint"], false);
        assert_eq!(tool["inputSchema"]["additionalProperties"], false);
    }

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
