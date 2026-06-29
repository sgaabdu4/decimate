use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::BoundaryCallRule;

use super::ConfigError;

pub(crate) fn rule_packs_schema() -> Value {
    json!({
        "type": "array",
        "items": { "type": "string" }
    })
}

pub(crate) fn boundary_calls_schema() -> Value {
    json!({
        "type": "array",
        "items": {
            "oneOf": [
                { "type": "string", "description": "FROM:PATTERN" },
                {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["from"],
                    "properties": {
                        "from": { "type": "string" },
                        "pattern": { "type": "string" },
                        "forbidden": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    }
                }
            ]
        }
    })
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum RawBoundaryCall {
    String(String),
    Object(RawBoundaryCallObject),
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct RawBoundaryCallObject {
    from: PathBuf,
    pattern: Option<String>,
    forbidden: Vec<String>,
}

pub(crate) fn parse_boundary_call(raw: RawBoundaryCall) -> Result<BoundaryCallRule, ConfigError> {
    match raw {
        RawBoundaryCall::String(value) => parse_boundary_call_string(&value),
        RawBoundaryCall::Object(object) => {
            if object.from.as_os_str().is_empty() {
                return Err(ConfigError::BoundaryCallRule {
                    value: format!("{object:?}"),
                });
            }
            let value = format!("{object:?}");
            let mut forbidden = object.forbidden;
            if let Some(pattern) = object.pattern {
                forbidden.push(pattern);
            }
            forbidden.retain(|pattern| !pattern.trim().is_empty());
            if forbidden.is_empty() {
                return Err(ConfigError::BoundaryCallRule { value });
            }
            Ok(BoundaryCallRule::new(object.from, forbidden))
        }
    }
}

fn parse_boundary_call_string(value: &str) -> Result<BoundaryCallRule, ConfigError> {
    let Some((from, pattern)) = value.split_once(':') else {
        return Err(ConfigError::BoundaryCallRule {
            value: value.to_owned(),
        });
    };
    if from.is_empty() || pattern.is_empty() {
        return Err(ConfigError::BoundaryCallRule {
            value: value.to_owned(),
        });
    }
    Ok(BoundaryCallRule::new(from, vec![pattern.to_owned()]))
}
