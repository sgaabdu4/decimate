use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Cache configuration metadata.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CacheConfig {
    /// Whether Dart Decimate-owned caches are enabled.
    pub enabled: Option<bool>,
    /// Local cache directory, relative to the project root unless absolute.
    pub path: Option<PathBuf>,
}

pub(super) fn cache_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "enabled": { "type": "boolean" },
            "path": { "type": "string" }
        }
    })
}
