use crate::output::{Finding, FindingEdge, FindingKind};

#[must_use]
pub(crate) fn finding_identity(finding: &Finding) -> String {
    if let Some(fingerprint) = finding.fingerprint.as_deref() {
        let text = format!("{}\n{:?}\n{}", finding.rule_id, finding.kind, fingerprint);
        return format!("finding:{:016x}", fnv64(&text));
    }

    if complexity_kind(finding.kind) {
        let text = format!(
            "{}\n{:?}\n{}\n{}",
            finding.rule_id,
            finding.kind,
            finding.path,
            complexity_symbol(&finding.message)
        );
        return format!("finding:{:016x}", fnv64(&text));
    }

    let text = format!(
        "{}\n{:?}\n{}\n{}\n{}\n{}",
        finding.rule_id,
        finding.kind,
        finding.path,
        finding.edge.as_ref().map_or(String::new(), edge_identity),
        finding.files.join("\n"),
        finding.message
    );
    format!("finding:{:016x}", fnv64(&text))
}

fn edge_identity(edge: &FindingEdge) -> String {
    format!("{}:{}:{}:{}", edge.from, edge.to, edge.specifier, edge.kind)
}

fn complexity_symbol(message: &str) -> &str {
    message
        .split_once(" has cyclomatic complexity ")
        .map_or(message, |(symbol, _)| symbol)
}

const fn complexity_kind(kind: FindingKind) -> bool {
    matches!(
        kind,
        FindingKind::HighCyclomaticComplexity
            | FindingKind::HighCognitiveComplexity
            | FindingKind::HighComplexity
            | FindingKind::HighCrapScore
    )
}

fn fnv64(text: &str) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in text.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}
