use std::io::{self, BufRead, Write};

use serde_json::{Map, Value, json};

use crate::cli::run_from;

mod cli_args;
#[cfg(test)]
mod cli_args_tests;
pub(crate) mod code_execute;
mod tools;

use cli_args::cli_args_for_tool;
use code_execute::execute as execute_code;
use tools::tools;

const PROTOCOL_VERSION: &str = "2025-11-25";

/// Run Decimate's MCP stdio server.
///
/// # Errors
///
/// Returns an IO error if stdin or stdout cannot be read or written.
pub fn run_stdio() -> io::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout().lock();
    for line in stdin.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        if let Some(response) = handle_message(&line) {
            serde_json::to_writer(&mut stdout, &response)?;
            writeln!(stdout)?;
            stdout.flush()?;
        }
    }
    Ok(())
}

/// Handle one newline-delimited JSON-RPC MCP message.
#[must_use]
pub fn handle_message(message: &str) -> Option<Value> {
    let value = match serde_json::from_str::<Value>(message) {
        Ok(value) => value,
        Err(error) => return Some(response_error(&Value::Null, -32700, error.to_string())),
    };
    let id = value.get("id").cloned();
    let Some(method) = value.get("method").and_then(Value::as_str) else {
        return id.map(|id| response_error(&id, -32600, "missing method"));
    };
    id.as_ref()?;
    let id = id.unwrap_or(Value::Null);
    match method {
        "initialize" => Some(response_result(&id, &initialize_result(&value))),
        "ping" => Some(response_result(&id, &json!({}))),
        "tools/list" => Some(response_result(&id, &json!({ "tools": tools() }))),
        "tools/call" => Some(call_tool(&id, &value)),
        _ => Some(response_error(
            &id,
            -32601,
            format!("unknown method {method}"),
        )),
    }
}

fn initialize_result(message: &Value) -> Value {
    let requested = message
        .get("params")
        .and_then(|params| params.get("protocolVersion"))
        .and_then(Value::as_str)
        .unwrap_or(PROTOCOL_VERSION);
    let protocol_version = if requested == PROTOCOL_VERSION {
        requested
    } else {
        PROTOCOL_VERSION
    };
    json!({
        "protocolVersion": protocol_version,
        "capabilities": {
            "tools": { "listChanged": false }
        },
        "serverInfo": {
            "name": "decimate-mcp",
            "title": "Decimate MCP",
            "version": env!("CARGO_PKG_VERSION")
        },
        "instructions": "Dart and Flutter codebase intelligence. Mutating fix_apply is available only with explicit yes: true confirmation."
    })
}

fn call_tool(id: &Value, message: &Value) -> Value {
    let params = message.get("params").unwrap_or(&Value::Null);
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return response_error(id, -32602, "tools/call requires params.name");
    };
    let Some(arguments) = params.get("arguments") else {
        return match cli_args_for_tool(name, &Map::new()) {
            Ok(args) => response_result(id, &tool_result(run_cli_json(args))),
            Err(message) => response_error(id, -32602, message),
        };
    };
    let Some(arguments) = arguments.as_object() else {
        return response_error(id, -32602, "tools/call params.arguments must be an object");
    };
    if name == "code_execute" {
        return response_result(id, &tool_result(execute_code(arguments)));
    }
    match cli_args_for_tool(name, arguments) {
        Ok(args) => response_result(id, &tool_result(run_cli_json(args))),
        Err(message) => response_error(id, -32602, message),
    }
}

fn run_cli_json(args: Vec<String>) -> CliToolOutput {
    let mut output = Vec::new();
    let code = match run_from(args, &mut output) {
        Ok(code) => code,
        Err(error) => {
            return CliToolOutput::text(2, error.to_string(), true);
        }
    };
    let text = String::from_utf8_lossy(&output).into_owned();
    let structured = serde_json::from_str::<Value>(&text).ok();
    CliToolOutput::new(code, text, structured, code == 2)
}

fn tool_result(output: CliToolOutput) -> Value {
    let mut result = Map::new();
    result.insert(
        "content".to_owned(),
        json!([{ "type": "text", "text": output.text }]),
    );
    result.insert("isError".to_owned(), Value::Bool(output.is_error));
    result.insert("_meta".to_owned(), json!({ "exit_code": output.exit_code }));
    if let Some(structured) = output.structured {
        result.insert("structuredContent".to_owned(), structured);
    }
    Value::Object(result)
}

#[derive(Debug)]
struct CliToolOutput {
    exit_code: i32,
    text: String,
    structured: Option<Value>,
    is_error: bool,
}

impl CliToolOutput {
    fn new(exit_code: i32, text: String, structured: Option<Value>, is_error: bool) -> Self {
        Self {
            exit_code,
            text,
            structured,
            is_error,
        }
    }

    fn text(exit_code: i32, text: String, is_error: bool) -> Self {
        Self::new(exit_code, text, None, is_error)
    }

    fn json(exit_code: i32, structured: Value, is_error: bool) -> Self {
        Self::new(
            exit_code,
            structured.to_string(),
            Some(structured),
            is_error,
        )
    }
}

fn response_result(id: &Value, result: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn response_error(id: &Value, code: i32, message: impl Into<String>) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message.into()
        }
    })
}
