use std::collections::BTreeSet;
use std::path::Path;

use super::format::display_path;
use super::{Finding, FindingAction, FindingKind, Severity};
use crate::{RouteCollision, RouteCollisionKind, RouteCollisionReport};

pub(super) fn add_route_findings(
    root: &Path,
    report: &RouteCollisionReport,
    findings: &mut Vec<Finding>,
) {
    findings.extend(
        report
            .collisions
            .iter()
            .filter_map(|collision| route_collision_finding(root, collision)),
    );
}

fn route_collision_finding(root: &Path, collision: &RouteCollision) -> Option<Finding> {
    let primary = collision
        .declarations
        .get(1)
        .or_else(|| collision.declarations.first())?;
    let path = display_path(root, &primary.path);
    let files = collision
        .declarations
        .iter()
        .map(|declaration| display_path(root, &declaration.path))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let label = match collision.kind {
        RouteCollisionKind::Path => "path",
        RouteCollisionKind::Name => "name",
    };

    Some(Finding {
        rule_id: "decimate/route-collision".to_owned(),
        fingerprint: Some(route_collision_fingerprint(collision)),
        kind: FindingKind::RouteCollision,
        severity: Severity::Error,
        message: format!(
            "GoRouter route {label} {} is declared by {} routes",
            collision.value,
            collision.declarations.len()
        ),
        path: path.clone(),
        line: primary.location.line,
        column: primary.location.column,
        safe_to_delete: false,
        files,
        edge: None,
        actions: vec![
            FindingAction::new(
                "review-route-collision",
                "Rename or move one route so GoRouter paths remain unique",
                false,
            )
            .with_target_path(path.clone())
            .with_target_symbol(primary.route_class.clone())
            .with_decimate_args(["inspect", "--format", "json", "--file", path.as_str()])
            .with_suppression_comment("// decimate-ignore-next-line route-collision"),
        ],
    })
}

fn route_collision_fingerprint(collision: &RouteCollision) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in format!("{:?}:{}", collision.kind, collision.value).as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("route:{:08x}", hash & 0xffff_ffff)
}
