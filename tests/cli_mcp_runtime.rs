use std::fs;

use decimate::mcp::handle_message;
use serde_json::Value;
use tempfile::TempDir;

#[test]
fn runtime_slice_tools_return_focused_contracts() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_runtime_fixture(&fixture)?;
    let message = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 19,
        "method": "tools/call",
        "params": {
            "name": "get_hot_paths",
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
        "runtime-hot-paths"
    );
    assert_eq!(
        output["result"]["structuredContent"]["hot_paths"][0]["path"],
        "lib/main.dart"
    );
    assert!(output["result"]["structuredContent"]["runtime_coverage"].is_null());

    Ok(())
}

#[test]
fn cli_failures_return_structured_error_content() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    let message = serde_json::json!({
        "jsonrpc": "2.0",
        "id": 20,
        "method": "tools/call",
        "params": {
            "name": "check_runtime_coverage",
            "arguments": {
                "root": fixture.path(),
                "coverage": "missing-coverage.json"
            }
        }
    });

    let output = response(&serde_json::to_string(&message)?)?;

    assert_eq!(output["result"]["isError"], true);
    assert_eq!(output["result"]["_meta"]["exit_code"], 2);
    assert_eq!(output["result"]["structuredContent"]["error"], true);
    assert_eq!(output["result"]["structuredContent"]["exit_code"], 2);
    assert!(
        output["result"]["structuredContent"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("missing-coverage.json"))
    );

    Ok(())
}

fn write_runtime_fixture(fixture: &TempDir) -> Result<(), Box<dyn std::error::Error>> {
    write(fixture, "pubspec.yaml", "name: app\n")?;
    write(fixture, "lib/main.dart", "void main() { print('hot'); }\n")?;
    write(
        fixture,
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
    Ok(())
}

fn response(message: &str) -> Result<Value, Box<dyn std::error::Error>> {
    handle_message(message).ok_or_else(|| "expected JSON-RPC response".into())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
