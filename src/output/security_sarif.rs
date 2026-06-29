use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::{Finding, JsonReport, Severity};

const SARIF_VERSION: &str = "2.1.0";
const SARIF_SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";

pub(crate) fn render_sarif_report(report: &JsonReport) -> Value {
    let findings = report.findings.iter().collect::<Vec<_>>();

    json!({
        "version": SARIF_VERSION,
        "$schema": SARIF_SCHEMA,
        "runs": [
            {
                "tool": {
                    "driver": {
                        "name": "decimate",
                        "rules": sarif_rules(&findings)
                    }
                },
                "results": sarif_results(&findings),
                "properties": {
                    "schemaVersion": &report.schema_version,
                    "command": &report.command,
                    "verdict": &report.verdict,
                    "findingCount": findings.len()
                }
            }
        ]
    })
}

fn sarif_rules(findings: &[&Finding]) -> Vec<Value> {
    let mut rules = BTreeMap::<&str, &Finding>::new();
    for finding in findings {
        rules.entry(finding.rule_id.as_str()).or_insert(*finding);
    }

    rules
        .into_values()
        .map(|finding| {
            json!({
                "id": &finding.rule_id,
                "name": &finding.rule_id,
                "shortDescription": {
                    "text": "Decimate codebase intelligence finding"
                },
                "fullDescription": {
                    "text": &finding.message
                },
                "help": {
                    "text": "Review the referenced Dart or Flutter code before editing."
                },
                "defaultConfiguration": {
                    "level": sarif_level(finding.severity)
                },
                "properties": {
                    "kind": finding.kind,
                    "tags": ["security", "dart", "flutter"]
                }
            })
        })
        .collect()
}

fn sarif_results(findings: &[&Finding]) -> Vec<Value> {
    findings
        .iter()
        .map(|finding| {
            json!({
                "ruleId": &finding.rule_id,
                "level": sarif_level(finding.severity),
                "message": {
                    "text": &finding.message
                },
                "locations": [
                    {
                        "physicalLocation": {
                            "artifactLocation": {
                                "uri": &finding.path
                            },
                            "region": {
                                "startLine": finding.line.max(1),
                                "startColumn": finding.column + 1
                            }
                        }
                    }
                ],
                "partialFingerprints": partial_fingerprints(finding),
                "properties": {
                    "kind": finding.kind,
                    "safeToDelete": finding.safe_to_delete,
                    "files": &finding.files,
                    "actions": &finding.actions
                }
            })
        })
        .collect()
}

fn partial_fingerprints(finding: &Finding) -> Value {
    match finding.fingerprint.as_deref() {
        Some(fingerprint) => json!({ "decimateFingerprint": fingerprint }),
        None => json!({}),
    }
}

const fn sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    }
}
