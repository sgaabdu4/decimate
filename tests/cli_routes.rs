use std::fs;

use decimate::cli::run_from;
use serde_json::{Value, json};
use tempfile::TempDir;

#[test]
fn check_command_reports_typed_route_collisions() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/routes/settings_a.dart",
        "@TypedGoRoute<SettingsARoute>(path: '/settings')\nclass SettingsARoute extends GoRouteData {}\n",
    )?;
    write(
        &fixture,
        "lib/routes/settings_b.dart",
        "@TypedGoRoute<SettingsBRoute>(path: '/settings')\nclass SettingsBRoute extends GoRouteData {}\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["schema_version"], "decimate.report.v1");
    assert_eq!(json["command"], "check");
    assert_eq!(json["verdict"], "fail");
    assert_eq!(json["summary"]["route_collisions"], 1);

    let Some(finding) = json["findings"].as_array().and_then(|findings| {
        findings
            .iter()
            .find(|finding| finding["rule_id"] == "decimate/route-collision")
    }) else {
        panic!("route collision finding");
    };
    assert_eq!(finding["kind"], "route-collision");
    assert_eq!(finding["severity"], "error");
    assert_eq!(finding["path"], "lib/routes/settings_b.dart");
    assert_eq!(finding["line"], 1);
    assert_eq!(finding["column"], 0);
    assert_eq!(finding["safe_to_delete"], false);
    assert_eq!(
        finding["files"],
        json!(["lib/routes/settings_a.dart", "lib/routes/settings_b.dart"])
    );
    assert_eq!(finding["edge"], Value::Null);
    assert_eq!(finding["actions"][0]["action"], "review-route-collision");
    assert_eq!(finding["actions"][0]["auto_fixable"], false);
    assert_eq!(finding["actions"][0]["target_symbol"], "SettingsBRoute");
    assert_eq!(
        finding["actions"][0]["suppression_comment"],
        "// decimate-ignore-next-line route-collision"
    );

    Ok(())
}

#[test]
fn route_collision_rule_can_warn_or_turn_off() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        ".decimaterc.json",
        r#"{ "rules": { "route-collision": "warn" } }"#,
    )?;
    write(
        &fixture,
        "lib/a.dart",
        "@TypedGoRoute<ARoute>(path: '/a')\nclass ARoute extends GoRouteData {}\n",
    )?;
    write(
        &fixture,
        "lib/b.dart",
        "@TypedGoRoute<BRoute>(path: '/a')\nclass BRoute extends GoRouteData {}\n",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 0);
    assert_eq!(json["verdict"], "pass");
    assert_eq!(json["findings"][0]["severity"], "warning");

    Ok(())
}

#[test]
fn check_command_reports_raw_go_route_collisions() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/router.dart",
        r"
final router = GoRouter(
  routes: [
    ShellRoute(
      routes: [
        GoRoute(path: '/settings', builder: (_, _) => const SizedBox()),
      ],
      builder: (_, _, child) => child,
    ),
    GoRoute(path: '/settings', builder: (_, _) => const SizedBox()),
  ],
);
",
    )?;
    let mut output = Vec::new();

    let code = run_from(
        [
            "decimate",
            "check",
            fixture.path().to_str().unwrap_or("."),
            "--format",
            "json",
        ],
        &mut output,
    )?;

    let json = serde_json::from_slice::<Value>(&output)?;
    assert_eq!(code, 1);
    assert_eq!(json["summary"]["route_collisions"], 1);
    assert!(json["findings"].as_array().is_some_and(|findings| {
        findings.iter().any(|finding| {
            finding["rule_id"] == "decimate/route-collision"
                && finding["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("GoRouter route path /settings"))
                && finding["actions"][0]["target_symbol"] == "GoRoute"
        })
    }));

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
