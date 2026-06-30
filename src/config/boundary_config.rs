use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{BoundaryPreset, BoundaryRule, boundary_preset_rules};

use super::ConfigError;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ParsedBoundarySettings {
    pub(crate) rules: Vec<BoundaryRule>,
    pub(crate) presets: Vec<BoundaryPreset>,
    pub(crate) allow_unmatched: Vec<String>,
    pub(crate) require_all_files: Option<bool>,
}

impl ParsedBoundarySettings {
    pub(crate) fn merge(&mut self, other: Self) {
        self.rules.extend(other.rules);
        self.presets.extend(other.presets);
        self.allow_unmatched.extend(other.allow_unmatched);
        self.require_all_files = other.require_all_files.or(self.require_all_files);
    }

    pub(crate) fn finish(mut self) -> (Vec<BoundaryRule>, Vec<BoundaryPreset>, Vec<String>) {
        self.presets = dedup_presets(self.presets);
        self.rules.extend(boundary_preset_rules(&self.presets));
        (
            dedup_rules(self.rules),
            self.presets,
            dedup_strings(self.allow_unmatched),
        )
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum RawBoundaries {
    List(Vec<RawBoundary>),
    Object(RawBoundarySettings),
}

impl Default for RawBoundaries {
    fn default() -> Self {
        Self::List(Vec::new())
    }
}

impl RawBoundaries {
    pub(crate) fn into_settings(self) -> Result<ParsedBoundarySettings, ConfigError> {
        match self {
            Self::List(rules) => Ok(ParsedBoundarySettings {
                rules: parse_boundaries(rules)?,
                ..ParsedBoundarySettings::default()
            }),
            Self::Object(settings) => settings.into_settings(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct RawBoundarySettings {
    #[serde(alias = "rule", alias = "boundary")]
    rules: Vec<RawBoundary>,
    preset: Option<BoundaryPreset>,
    presets: Vec<BoundaryPreset>,
    #[serde(alias = "allowUnmatched")]
    allow_unmatched: Vec<String>,
    coverage: RawBoundaryCoverage,
}

impl RawBoundarySettings {
    fn into_settings(self) -> Result<ParsedBoundarySettings, ConfigError> {
        let mut presets = self.presets;
        if let Some(preset) = self.preset {
            presets.push(preset);
        }
        let mut allow_unmatched = self.allow_unmatched;
        allow_unmatched.extend(self.coverage.allow_unmatched);
        Ok(ParsedBoundarySettings {
            rules: parse_boundaries(self.rules)?,
            presets,
            allow_unmatched,
            require_all_files: self.coverage.require_all_files,
        })
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct RawBoundaryCoverage {
    #[serde(alias = "requireAllFiles")]
    require_all_files: Option<bool>,
    #[serde(alias = "allowUnmatched")]
    allow_unmatched: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum RawBoundary {
    String(String),
    Object(RawBoundaryObject),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RawBoundaryObject {
    from: PathBuf,
    disallow: PathBuf,
}

fn parse_boundaries(raw: Vec<RawBoundary>) -> Result<Vec<BoundaryRule>, ConfigError> {
    raw.into_iter().map(parse_boundary).collect()
}

fn parse_boundary(raw: RawBoundary) -> Result<BoundaryRule, ConfigError> {
    match raw {
        RawBoundary::String(value) => parse_boundary_string(&value),
        RawBoundary::Object(object) => {
            if object.from.as_os_str().is_empty() || object.disallow.as_os_str().is_empty() {
                return Err(ConfigError::BoundaryRule {
                    value: format!("{object:?}"),
                });
            }
            Ok(BoundaryRule::new(object.from, object.disallow))
        }
    }
}

fn parse_boundary_string(value: &str) -> Result<BoundaryRule, ConfigError> {
    let Some((from, disallow)) = value.split_once(':') else {
        return Err(ConfigError::BoundaryRule {
            value: value.to_owned(),
        });
    };
    if from.is_empty() || disallow.is_empty() {
        return Err(ConfigError::BoundaryRule {
            value: value.to_owned(),
        });
    }
    Ok(BoundaryRule::new(from, disallow))
}

fn dedup_rules(rules: Vec<BoundaryRule>) -> Vec<BoundaryRule> {
    let mut seen = BTreeSet::new();
    rules
        .into_iter()
        .filter(|rule| seen.insert((rule.from.clone(), rule.disallow.clone())))
        .collect()
}

fn dedup_presets(presets: Vec<BoundaryPreset>) -> Vec<BoundaryPreset> {
    presets
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn dedup_strings(values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .filter(|value| !value.trim().is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn boundary_schema() -> Value {
    json!({
        "oneOf": [
            boundary_rule_list_schema(),
            {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "rule": boundary_rule_list_schema(),
                    "rules": boundary_rule_list_schema(),
                    "boundary": boundary_rule_list_schema(),
                    "preset": preset_schema(),
                    "presets": {
                        "type": "array",
                        "items": preset_schema()
                    },
                    "allow_unmatched": string_list_schema(),
                    "allowUnmatched": string_list_schema(),
                    "coverage": {
                        "type": "object",
                        "additionalProperties": false,
                        "properties": {
                            "require_all_files": { "type": "boolean" },
                            "requireAllFiles": { "type": "boolean" },
                            "allow_unmatched": string_list_schema(),
                            "allowUnmatched": string_list_schema()
                        }
                    }
                }
            }
        ]
    })
}

fn boundary_rule_list_schema() -> Value {
    json!({
        "type": "array",
        "items": {
            "oneOf": [
                { "type": "string", "pattern": "^.+:.+$" },
                {
                    "type": "object",
                    "additionalProperties": false,
                    "required": ["from", "disallow"],
                    "properties": {
                        "from": { "type": "string" },
                        "disallow": { "type": "string" }
                    }
                }
            ]
        }
    })
}

fn preset_schema() -> Value {
    json!({
        "type": "string",
        "enum": ["layered", "hexagonal", "feature-sliced", "bulletproof"]
    })
}

fn string_list_schema() -> Value {
    json!({ "type": "array", "items": { "type": "string" } })
}
