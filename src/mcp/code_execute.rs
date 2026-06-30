use std::collections::{BTreeMap, BTreeSet};

use serde::Deserialize;
use serde_json::{Map, Value, json};

use super::{CliToolOutput, run_cli_json, tool_result};
use crate::mcp::cli_args::cli_args_for_tool;

pub(crate) const CODE_EXECUTE_SCHEMA_VERSION: &str = "decimate.mcp.code_execute.v1";

const DEFAULT_MAX_STEPS: usize = 8;
const HARD_MAX_STEPS: usize = 16;
const DEFAULT_MAX_TOOL_CALLS: usize = 4;
const HARD_MAX_TOOL_CALLS: usize = 8;
const DEFAULT_MAX_RESULT_BYTES: usize = 256 * 1024;
const HARD_MAX_RESULT_BYTES: usize = 4_000_000;
const MAX_SELECTION_ITEMS: usize = 100;

pub(super) fn execute(arguments: &Map<String, Value>) -> CliToolOutput {
    match execute_inner(arguments) {
        Ok(value) => CliToolOutput::json(0, value, false),
        Err(message) => CliToolOutput::json(2, error_output(&message), true),
    }
}

fn execute_inner(arguments: &Map<String, Value>) -> Result<Value, String> {
    let request = CodeExecuteRequest::from_arguments(arguments)?;
    let limits = Limits::from_request(&request)?;
    let mut executor = Executor::new(limits);
    executor.execute(&request.program)
}

fn error_output(message: &str) -> Value {
    json!({
        "schema_version": CODE_EXECUTE_SCHEMA_VERSION,
        "ok": false,
        "error": message,
        "result": Value::Null,
        "calls": [],
    })
}

#[derive(Debug)]
struct CodeExecuteRequest {
    program: Program,
    max_steps: Option<usize>,
    max_tool_calls: Option<usize>,
    max_result_bytes: Option<usize>,
}

impl CodeExecuteRequest {
    fn from_arguments(arguments: &Map<String, Value>) -> Result<Self, String> {
        reject_unknown(
            arguments,
            &[
                "code",
                "program",
                "max_steps",
                "max_tool_calls",
                "max_result_bytes",
            ],
        )?;
        let program_value = match (arguments.get("program"), arguments.get("code")) {
            (Some(_), Some(_)) => {
                return Err("code_execute accepts either code or program, not both".to_owned());
            }
            (Some(program), None) | (None, Some(program)) => program.clone(),
            (None, None) => return Err("code_execute requires code or program".to_owned()),
        };
        let program = serde_json::from_value::<Program>(program_value)
            .map_err(|error| format!("invalid code_execute program: {error}"))?;
        Ok(Self {
            program,
            max_steps: optional_usize(arguments, "max_steps")?,
            max_tool_calls: optional_usize(arguments, "max_tool_calls")?,
            max_result_bytes: optional_usize(arguments, "max_result_bytes")?,
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
struct Program {
    steps: Vec<Step>,
    #[serde(rename = "return")]
    return_value: ReturnSpec,
}

#[derive(Debug, Clone, Deserialize)]
struct Step {
    id: String,
    #[serde(default)]
    call: Option<String>,
    #[serde(default)]
    arguments: Value,
    #[serde(default)]
    select: Option<SelectSpec>,
}

#[derive(Debug, Clone, Deserialize)]
struct SelectSpec {
    from: String,
    #[serde(default)]
    pointer: String,
    #[serde(default)]
    fields: Vec<String>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default, rename = "where")]
    where_clause: Option<WhereSpec>,
}

#[derive(Debug, Clone, Deserialize)]
struct WhereSpec {
    equals: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum ReturnSpec {
    Ref {
        from: String,
        pointer: Option<String>,
    },
    Value(Value),
}

#[derive(Debug, Clone, Deserialize)]
struct RefValue {
    #[serde(rename = "$ref")]
    reference: RefTarget,
}

#[derive(Debug, Clone, Deserialize)]
struct RefTarget {
    from: String,
    #[serde(default)]
    pointer: String,
}

#[derive(Debug, Clone, Copy)]
struct Limits {
    steps: usize,
    tool_calls: usize,
    result_bytes: usize,
}

impl Limits {
    fn from_request(request: &CodeExecuteRequest) -> Result<Self, String> {
        let max_steps = capped_limit(
            request.max_steps.unwrap_or(DEFAULT_MAX_STEPS),
            HARD_MAX_STEPS,
            "max_steps",
        )?;
        let max_tool_calls = capped_limit(
            request.max_tool_calls.unwrap_or(DEFAULT_MAX_TOOL_CALLS),
            HARD_MAX_TOOL_CALLS,
            "max_tool_calls",
        )?;
        let max_result_bytes = capped_limit(
            request.max_result_bytes.unwrap_or(DEFAULT_MAX_RESULT_BYTES),
            HARD_MAX_RESULT_BYTES,
            "max_result_bytes",
        )?;
        Ok(Self {
            steps: max_steps,
            tool_calls: max_tool_calls,
            result_bytes: max_result_bytes,
        })
    }
}

fn capped_limit(value: usize, hard_max: usize, name: &str) -> Result<usize, String> {
    if value == 0 {
        return Err(format!("{name} must be greater than zero"));
    }
    if value > hard_max {
        return Err(format!("{name} exceeds hard limit {hard_max}"));
    }
    Ok(value)
}

struct Executor {
    limits: Limits,
    outputs: BTreeMap<String, Value>,
    calls: Vec<CallRecord>,
    total_result_bytes: usize,
}

impl Executor {
    fn new(limits: Limits) -> Self {
        Self {
            limits,
            outputs: BTreeMap::new(),
            calls: Vec::new(),
            total_result_bytes: 0,
        }
    }

    fn execute(&mut self, program: &Program) -> Result<Value, String> {
        self.validate(program)?;
        for step in &program.steps {
            let value = self.execute_step(step)?;
            self.outputs.insert(step.id.clone(), value);
        }
        let result = self.return_value(&program.return_value)?;
        self.track_result_bytes(&result)?;
        Ok(json!({
            "schema_version": CODE_EXECUTE_SCHEMA_VERSION,
            "ok": true,
            "result": result,
            "calls": self.calls,
            "limits": {
                "max_steps": self.limits.steps,
                "max_tool_calls": self.limits.tool_calls,
                "max_result_bytes": self.limits.result_bytes,
                "max_selection_items": MAX_SELECTION_ITEMS
            }
        }))
    }

    fn validate(&self, program: &Program) -> Result<(), String> {
        if program.steps.is_empty() {
            return Err("code_execute program requires at least one step".to_owned());
        }
        if program.steps.len() > self.limits.steps {
            return Err(format!(
                "code_execute program has {} steps but max_steps is {}",
                program.steps.len(),
                self.limits.steps
            ));
        }
        let mut ids = BTreeSet::<&str>::new();
        for step in &program.steps {
            validate_step_id(&step.id)?;
            if !ids.insert(&step.id) {
                return Err(format!("duplicate code_execute step id {}", step.id));
            }
            match (&step.call, &step.select) {
                (Some(_), None) | (None, Some(_)) => {}
                (Some(_), Some(_)) => {
                    return Err(format!(
                        "step {} cannot contain both call and select",
                        step.id
                    ));
                }
                (None, None) => {
                    return Err(format!("step {} requires call or select", step.id));
                }
            }
        }
        Ok(())
    }

    fn execute_step(&mut self, step: &Step) -> Result<Value, String> {
        if let Some(tool) = &step.call {
            return self.call_tool(tool, &step.arguments);
        }
        let select = step
            .select
            .as_ref()
            .ok_or_else(|| format!("step {} requires call or select", step.id))?;
        self.select(select)
    }

    fn call_tool(&mut self, tool: &str, arguments: &Value) -> Result<Value, String> {
        if self.calls.len() >= self.limits.tool_calls {
            return Err(format!(
                "code_execute child tool call limit exceeded ({})",
                self.limits.tool_calls
            ));
        }
        if !READ_ONLY_TOOLS.contains(&tool) {
            return Err(format!("code_execute cannot call tool {tool}"));
        }
        let resolved = self.resolve_refs(arguments)?;
        let args = resolved
            .as_object()
            .ok_or_else(|| format!("arguments for {tool} must resolve to an object"))?;
        let cli_args = cli_args_for_tool(tool, args)?;
        let output = run_cli_json(cli_args);
        let result = tool_result(output);
        let structured = result
            .get("structuredContent")
            .cloned()
            .unwrap_or(Value::Null);
        let is_error = result
            .get("isError")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let exit_code = result
            .get("_meta")
            .and_then(|meta| meta.get("exit_code"))
            .and_then(Value::as_i64)
            .unwrap_or(0);
        self.calls.push(CallRecord {
            tool: tool.to_owned(),
            exit_code,
            ok: !is_error,
            output_bytes: serde_json::to_vec(&structured).map_or(0, |bytes| bytes.len()),
        });
        if is_error {
            return Err(format!(
                "code_execute child tool {tool} failed with exit code {exit_code}"
            ));
        }
        self.track_result_bytes(&structured)?;
        Ok(result)
    }

    fn select(&self, spec: &SelectSpec) -> Result<Value, String> {
        let source = self.step_output(&spec.from)?;
        let selected = if spec.pointer.is_empty() {
            source
        } else {
            source.pointer(&spec.pointer).ok_or_else(|| {
                format!("step {} pointer {} did not match", spec.from, spec.pointer)
            })?
        };
        let mut value = selected.clone();
        if let Some(where_clause) = &spec.where_clause {
            value = filter_value(&value, where_clause)?;
        }
        if !spec.fields.is_empty() {
            value = project_fields(value, &spec.fields)?;
        }
        if let Some(limit) = spec.limit {
            value = limit_items(&value, limit)?;
        }
        Ok(value)
    }

    fn return_value(&self, return_spec: &ReturnSpec) -> Result<Value, String> {
        match return_spec {
            ReturnSpec::Ref { from, pointer } => {
                let source = self.step_output(from)?;
                let pointer = pointer.as_deref().unwrap_or("");
                if pointer.is_empty() {
                    Ok(source.clone())
                } else {
                    source.pointer(pointer).cloned().ok_or_else(|| {
                        format!("return pointer {pointer} did not match step {from}")
                    })
                }
            }
            ReturnSpec::Value(value) => self.resolve_refs(value),
        }
    }

    fn resolve_refs(&self, value: &Value) -> Result<Value, String> {
        if let Some(reference) = parse_ref(value)? {
            return self.resolve_ref_target(&reference);
        }
        match value {
            Value::Array(items) => items
                .iter()
                .map(|item| self.resolve_refs(item))
                .collect::<Result<Vec<_>, _>>()
                .map(Value::Array),
            Value::Object(object) => object
                .iter()
                .map(|(key, value)| Ok((key.clone(), self.resolve_refs(value)?)))
                .collect::<Result<Map<_, _>, String>>()
                .map(Value::Object),
            _ => Ok(value.clone()),
        }
    }

    fn resolve_ref_target(&self, target: &RefTarget) -> Result<Value, String> {
        let source = self.step_output(&target.from)?;
        if target.pointer.is_empty() {
            return Ok(source.clone());
        }
        source.pointer(&target.pointer).cloned().ok_or_else(|| {
            format!(
                "reference from {} pointer {} did not match",
                target.from, target.pointer
            )
        })
    }

    fn step_output(&self, id: &str) -> Result<&Value, String> {
        self.outputs
            .get(id)
            .ok_or_else(|| format!("unknown code_execute step reference {id}"))
    }

    fn track_result_bytes(&mut self, value: &Value) -> Result<(), String> {
        let bytes = serde_json::to_vec(value)
            .map_err(|error| format!("failed to serialize code_execute result: {error}"))?
            .len();
        self.total_result_bytes = self
            .total_result_bytes
            .checked_add(bytes)
            .ok_or_else(|| "code_execute result byte counter overflowed".to_owned())?;
        if self.total_result_bytes > self.limits.result_bytes {
            return Err(format!(
                "code_execute result exceeded max_result_bytes {}",
                self.limits.result_bytes
            ));
        }
        Ok(())
    }
}

#[derive(Debug, serde::Serialize)]
struct CallRecord {
    tool: String,
    exit_code: i64,
    ok: bool,
    output_bytes: usize,
}

const READ_ONLY_TOOLS: &[&str] = &[
    "analyze",
    "check_changed",
    "project_info",
    "list_boundaries",
    "inspect_target",
    "trace_file",
    "trace_export",
    "trace_dependency",
    "trace_clone",
    "find_dupes",
    "check_health",
    "check_runtime_coverage",
    "get_hot_paths",
    "get_blast_radius",
    "get_importance",
    "get_cleanup_candidates",
    "security_candidates",
    "feature_flags",
    "impact",
    "impact_all",
    "audit",
    "decision_surface",
    "decimate_explain",
];

fn parse_ref(value: &Value) -> Result<Option<RefTarget>, String> {
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    if !object.contains_key("$ref") {
        return Ok(None);
    }
    if object.len() != 1 {
        return Err("$ref objects cannot contain sibling keys".to_owned());
    }
    serde_json::from_value::<RefValue>(value.clone())
        .map(|reference| Some(reference.reference))
        .map_err(|error| format!("invalid $ref: {error}"))
}

fn filter_value(value: &Value, where_clause: &WhereSpec) -> Result<Value, String> {
    let items = value
        .as_array()
        .ok_or_else(|| "where.equals can only filter arrays".to_owned())?;
    let filtered = items
        .iter()
        .filter(|item| {
            where_clause.equals.iter().all(|(pointer, expected)| {
                item.pointer(pointer)
                    .is_some_and(|actual| actual == expected)
            })
        })
        .take(MAX_SELECTION_ITEMS)
        .cloned()
        .collect::<Vec<_>>();
    Ok(Value::Array(filtered))
}

fn project_fields(value: Value, fields: &[String]) -> Result<Value, String> {
    match value {
        Value::Array(items) => Ok(Value::Array(
            items
                .into_iter()
                .take(MAX_SELECTION_ITEMS)
                .map(|item| project_object_fields(&item, fields))
                .collect::<Result<Vec<_>, _>>()?,
        )),
        Value::Object(_) => project_object_fields(&value, fields),
        _ => Err("fields can only project objects or arrays of objects".to_owned()),
    }
}

fn project_object_fields(value: &Value, fields: &[String]) -> Result<Value, String> {
    let object = value
        .as_object()
        .ok_or_else(|| "fields can only project objects".to_owned())?;
    let projected = fields
        .iter()
        .filter_map(|field| {
            object
                .get(field)
                .map(|value| (field.clone(), value.clone()))
        })
        .collect::<Map<_, _>>();
    Ok(Value::Object(projected))
}

fn limit_items(value: &Value, limit: usize) -> Result<Value, String> {
    if limit > MAX_SELECTION_ITEMS {
        return Err(format!(
            "select limit exceeds max selection items {MAX_SELECTION_ITEMS}"
        ));
    }
    let items = value
        .as_array()
        .ok_or_else(|| "limit can only apply to arrays".to_owned())?;
    Ok(Value::Array(items.iter().take(limit).cloned().collect()))
}

fn validate_step_id(id: &str) -> Result<(), String> {
    if id.is_empty() {
        return Err("code_execute step id cannot be empty".to_owned());
    }
    if !id
        .chars()
        .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
    {
        return Err(format!("invalid code_execute step id {id}"));
    }
    Ok(())
}

fn reject_unknown(arguments: &Map<String, Value>, allowed: &[&str]) -> Result<(), String> {
    for key in arguments.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("code_execute does not accept argument {key}"));
        }
    }
    Ok(())
}

fn optional_usize(arguments: &Map<String, Value>, key: &str) -> Result<Option<usize>, String> {
    arguments
        .get(key)
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| format!("{key} must be an integer"))
                .and_then(|number| {
                    usize::try_from(number).map_err(|_| format!("{key} is too large"))
                })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_top_level_argument() {
        let mut args = Map::new();
        args.insert("code".to_owned(), json!({ "steps": [], "return": null }));
        args.insert("timeout_ms".to_owned(), json!(100));

        let output = execute(&args);

        assert!(output.is_error);
        assert!(
            output
                .structured
                .as_ref()
                .and_then(|json| json["error"].as_str())
                .is_some_and(|error| error.contains("timeout_ms"))
        );
    }

    #[test]
    fn rejects_fix_tools() {
        let args = request(json!({
            "steps": [
                { "id": "fix", "call": "fix_apply", "arguments": { "yes": true } }
            ],
            "return": { "from": "fix" }
        }));

        let output = execute(&args);

        assert!(output.is_error);
        assert!(
            output
                .structured
                .as_ref()
                .and_then(|json| json["error"].as_str())
                .is_some_and(|error| error.contains("fix_apply"))
        );
    }

    #[test]
    fn rejects_duplicate_step_ids() {
        let args = request(json!({
            "steps": [
                { "id": "same", "select": { "from": "same", "pointer": "" } },
                { "id": "same", "select": { "from": "same", "pointer": "" } }
            ],
            "return": { "from": "same" }
        }));

        let output = execute(&args);

        assert!(output.is_error);
        assert!(
            output
                .structured
                .as_ref()
                .and_then(|json| json["error"].as_str())
                .is_some_and(|error| error.contains("duplicate"))
        );
    }

    #[test]
    fn selects_projects_and_limits_arrays() -> Result<(), String> {
        let mut executor = Executor::new(Limits {
            steps: 4,
            tool_calls: 1,
            result_bytes: 10_000,
        });
        executor.outputs.insert(
            "input".to_owned(),
            json!({
                "items": [
                    { "kind": "unused-file", "path": "lib/a.dart", "extra": true },
                    { "kind": "unused-export", "path": "lib/b.dart", "extra": true }
                ]
            }),
        );

        let value = executor.select(&SelectSpec {
            from: "input".to_owned(),
            pointer: "/items".to_owned(),
            fields: vec!["path".to_owned()],
            limit: Some(1),
            where_clause: Some(WhereSpec {
                equals: BTreeMap::from([(
                    "/kind".to_owned(),
                    Value::String("unused-export".to_owned()),
                )]),
            }),
        })?;

        assert_eq!(value, json!([{ "path": "lib/b.dart" }]));
        Ok(())
    }

    #[test]
    fn resolves_argument_refs() -> Result<(), String> {
        let mut executor = Executor::new(Limits {
            steps: 4,
            tool_calls: 1,
            result_bytes: 10_000,
        });
        executor
            .outputs
            .insert("pick".to_owned(), json!({ "path": "lib/main.dart" }));

        let value = executor.resolve_refs(&json!({
            "file": { "$ref": { "from": "pick", "pointer": "/path" } }
        }))?;

        assert_eq!(value, json!({ "file": "lib/main.dart" }));
        Ok(())
    }

    fn request(program: Value) -> Map<String, Value> {
        Map::from_iter([("code".to_owned(), program)])
    }
}
