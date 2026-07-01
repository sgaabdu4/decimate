use std::collections::BTreeMap;

use serde_json::{Map, Value, json};

use super::{Finding, JsonReport, Severity};

const SARIF_VERSION: &str = "2.1.0";
const SARIF_SCHEMA: &str = "https://json.schemastore.org/sarif-2.1.0.json";

pub(crate) fn render_sarif_report(report: &JsonReport) -> Value {
    let findings = report.findings.iter().collect::<Vec<_>>();
    let reachability_by_fingerprint = security_reachability_by_fingerprint(report);

    json!({
        "version": SARIF_VERSION,
        "$schema": SARIF_SCHEMA,
        "runs": [
            {
                "tool": {
                    "driver": {
                        "name": "dart-decimate",
                        "rules": sarif_rules(&findings)
                    }
                },
                "results": sarif_results_with_reachability(
                    &findings,
                    &reachability_by_fingerprint
                ),
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

fn security_reachability_by_fingerprint(report: &JsonReport) -> BTreeMap<String, Value> {
    report
        .security_candidates
        .iter()
        .filter_map(|candidate| {
            candidate
                .reachability
                .as_ref()
                .map(|reachability| (candidate.fingerprint.clone(), json!(reachability)))
        })
        .collect()
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
                    "text": "Dart Decimate codebase intelligence finding"
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

fn sarif_results_with_reachability(
    findings: &[&Finding],
    reachability_by_fingerprint: &BTreeMap<String, Value>,
) -> Vec<Value> {
    findings
        .iter()
        .map(|finding| {
            let properties = result_properties(finding, reachability_by_fingerprint);
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
                "properties": properties
            })
        })
        .collect()
}

fn result_properties(
    finding: &Finding,
    reachability_by_fingerprint: &BTreeMap<String, Value>,
) -> Value {
    let mut properties = Map::new();
    properties.insert("kind".to_owned(), json!(finding.kind));
    properties.insert("findingId".to_owned(), json!(&finding.fingerprint));
    properties.insert("safeToDelete".to_owned(), json!(finding.safe_to_delete));
    properties.insert("files".to_owned(), json!(&finding.files));
    properties.insert("actions".to_owned(), json!(&finding.actions));
    if let Some(fingerprint) = &finding.fingerprint
        && let Some(reachability) = reachability_by_fingerprint.get(fingerprint)
    {
        properties.insert("securityReachability".to_owned(), reachability.clone());
    }
    Value::Object(properties)
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
