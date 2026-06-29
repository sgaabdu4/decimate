use std::fs;

use tempfile::TempDir;

use crate::{HealthOptions, analyze_health, scan_project};

#[test]
fn health_counts_branch_constructs() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "lib/main.dart",
        "void main() {
  if (a && b || c) {}
  for (final item in items) {}
  while (ready) {}
  do {} while (again);
  switch (value) {
    case 1:
      break;
    default:
      break;
  }
  try {} catch (error) {}
  final next = a ? b : c;
}
",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_health(
        &project,
        &HealthOptions {
            max_cyclomatic: 1,
            max_cognitive: 99,
            top: None,
            complexity_breakdown: true.into(),
            ..HealthOptions::default()
        },
    )?;

    assert_eq!(report.functions, 1);
    assert_eq!(report.max_cyclomatic_complexity, 10);
    assert_eq!(report.complexity[0].cyclomatic_complexity, 10);
    assert_eq!(report.complexity[0].contributions.len(), 9);

    Ok(())
}

#[test]
fn cognitive_complexity_penalizes_nesting() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "lib/main.dart",
        "void flat() {
  if (a) {}
  if (b) {}
}

void nested() {
  if (a) {
    if (b) {}
  }
}
",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_health(
        &project,
        &HealthOptions {
            max_cyclomatic: 1,
            max_cognitive: 1,
            top: None,
            complexity_breakdown: false.into(),
            ..HealthOptions::default()
        },
    )?;

    let Some(flat) = report
        .complexity
        .iter()
        .find(|finding| finding.symbol == "flat")
    else {
        panic!("flat finding");
    };
    let Some(nested) = report
        .complexity
        .iter()
        .find(|finding| finding.symbol == "nested")
    else {
        panic!("nested finding");
    };
    assert_eq!(flat.cyclomatic_complexity, nested.cyclomatic_complexity);
    assert!(nested.cognitive_complexity > flat.cognitive_complexity);

    Ok(())
}

#[test]
fn nested_closures_are_scored_separately() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(
        &fixture,
        "lib/main.dart",
        "void outer() {
  final inner = () {
    if (ready) {}
  };
  inner();
}
",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_health(
        &project,
        &HealthOptions {
            max_cyclomatic: 0,
            max_cognitive: 99,
            top: None,
            complexity_breakdown: false.into(),
            ..HealthOptions::default()
        },
    )?;

    let outer = report
        .complexity
        .iter()
        .find(|finding| finding.symbol == "outer");
    let closure = report
        .complexity
        .iter()
        .find(|finding| finding.symbol == "<closure>");
    let (Some(outer), Some(closure)) = (outer, closure) else {
        panic!("outer function and closure findings");
    };
    assert_eq!(outer.cyclomatic_complexity, 1);
    assert_eq!(closure.cyclomatic_complexity, 2);

    Ok(())
}

#[test]
fn lcov_drives_coverage_gaps_and_crap_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_coverage_source(&fixture)?;
    write(
        &fixture,
        "coverage/lcov.info",
        "SF:lib/main.dart
DA:2,0
DA:3,0
DA:4,0
DA:5,0
end_of_record
",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_health(
        &project,
        &HealthOptions {
            coverage_path: Some("coverage/lcov.info".into()),
            coverage_gaps: true.into(),
            max_crap: Some(10),
            ..HealthOptions::default()
        },
    )?;

    assert_eq!(report.coverage_files, 1);
    assert_eq!(report.coverage_gaps.len(), 1);
    assert_eq!(report.coverage_gaps[0].covered_lines, 0);
    assert_eq!(report.coverage_gaps[0].executable_lines, 4);
    assert_eq!(report.crap.len(), 1);
    assert_eq!(report.crap[0].symbol, "uncovered");
    assert_eq!(report.crap[0].cyclomatic_complexity, 4);
    assert_eq!(report.crap[0].line_coverage_percent, 0);
    assert_eq!(report.crap[0].crap_score, 20);
    assert_eq!(report.max_crap_score, 20);

    Ok(())
}

#[test]
fn covered_lcov_lines_do_not_emit_coverage_findings() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write_coverage_source(&fixture)?;
    write(
        &fixture,
        "coverage/lcov.info",
        "SF:lib/main.dart
DA:2,1
DA:3,1
DA:4,1
DA:5,1
end_of_record
",
    )?;
    let project = scan_project(fixture.path())?;

    let report = analyze_health(
        &project,
        &HealthOptions {
            coverage_path: Some("coverage/lcov.info".into()),
            coverage_gaps: true.into(),
            max_crap: Some(10),
            ..HealthOptions::default()
        },
    )?;

    assert!(report.coverage_gaps.is_empty());
    assert!(report.crap.is_empty());
    assert_eq!(report.max_crap_score, 0);

    Ok(())
}

fn write_coverage_source(fixture: &TempDir) -> Result<(), std::io::Error> {
    write(
        fixture,
        "lib/main.dart",
        "void uncovered(List<int> items) {
  if (items.isEmpty) return;
  for (final item in items) {
    if (item.isEven) return;
  }
}
",
    )
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
