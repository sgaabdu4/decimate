use std::fs;

use tempfile::TempDir;

use crate::{SecurityOptions, analyze_security, scan_project};

#[test]
fn detects_dart_and_flutter_security_candidate_patterns() -> Result<(), Box<dyn std::error::Error>>
{
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "import 'dart:io';

const accessToken = 'dart_decimate_fixture_value_1234567890';

Future<void> main(dynamic db, dynamic prefs, dynamic controller, String command, String id, String token) async {
  final uri = Uri.parse('http://api.example.com/login');
  final client = HttpClient();
  client.badCertificateCallback = (cert, host, port) => true;
  controller.setJavaScriptMode(JavaScriptMode.unrestricted);
  await Process.run(command, ['-c', id]);
  await db.rawQuery('SELECT * FROM users WHERE id = $id');
  await prefs.setString('access_token', token);
  print(uri);
}
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(
        &project,
        &SecurityOptions {
            top: None,
            surface: true,
            ..SecurityOptions::default()
        },
        None,
    )?;
    let rules = report
        .candidates
        .iter()
        .map(|candidate| candidate.rule_id.as_str())
        .collect::<Vec<_>>();

    assert_eq!(report.analyzed_files, 1);
    assert_eq!(report.total_occurrences, 7);
    assert_eq!(report.attack_surface.len(), 7);
    assert!(rules.contains(&"dart-decimate/security-hardcoded-secret"));
    assert!(rules.contains(&"dart-decimate/security-insecure-transport"));
    assert!(rules.contains(&"dart-decimate/security-tls-bypass"));
    assert!(rules.contains(&"dart-decimate/security-webview-risk"));
    assert!(rules.contains(&"dart-decimate/security-process-execution"));
    assert!(rules.contains(&"dart-decimate/security-raw-sql"));
    assert!(rules.contains(&"dart-decimate/security-plain-secret-storage"));
    assert!(
        report
            .candidates
            .iter()
            .flat_map(|candidate| &candidate.occurrences)
            .all(|occurrence| !occurrence
                .evidence
                .contains("dart_decimate_fixture_value_1234567890"))
    );

    Ok(())
}

#[test]
fn skips_comments_generated_tests_and_placeholders() -> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "// const accessToken = 'dart_decimate_fixture_value_1234567890';
const apiKey = 'YOUR_API_KEY';
const firebase = FirebaseOptions(apiKey: 'AIzaPublicMobileConfigValue');
final uri = Uri.parse('http://localhost:8080');
Future<void> run() => Process.run('git', ['status']);
Future<void> query(dynamic db, String id) => db.rawQuery('SELECT * FROM users WHERE id = ?', [id]);
",
    )?;
    write(
        &fixture,
        "lib/generated.g.dart",
        "const accessToken = 'dart_decimate_fixture_value_1234567890';\n",
    )?;
    write(
        &fixture,
        "test/security_test.dart",
        "const accessToken = 'dart_decimate_fixture_value_1234567890';\n",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;

    assert!(report.candidates.is_empty());
    assert_eq!(report.total_occurrences, 0);

    Ok(())
}

#[test]
fn skips_flutter_commands_logs_and_interpolated_bearer_headers()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "/// This shouldn't shift later string positions.
Future<void> main(dynamic viewModel, dynamic prefs, String token) async {
  viewModel.load.execute();
  viewModel.login.execute((email: 'user@example.com', password: 'not-a-secret'));
  _log.severe(
    'Failed to fetch Token from SharedPreferences',
  );
  _log.warning('Failed to set token');
  if (request.headers['Authorization'] != 'Bearer $token') {}
  final header = 'Bearer $token';
  await prefs.setString('access_token', token);
  print(header);
}
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(&project, &SecurityOptions::default(), None)?;

    assert_eq!(report.total_occurrences, 1);
    assert_eq!(
        report.candidates[0].rule_id,
        "dart-decimate/security-plain-secret-storage"
    );

    Ok(())
}

#[test]
fn top_limits_grouped_candidates_but_preserves_total_occurrences()
-> Result<(), Box<dyn std::error::Error>> {
    let fixture = tempfile::tempdir()?;
    write(&fixture, "pubspec.yaml", "name: app\n")?;
    write(
        &fixture,
        "lib/main.dart",
        "const accessToken = 'dart_decimate_fixture_value_1234567890';
final uri = Uri.parse('http://api.example.com/login');
",
    )?;

    let project = scan_project(fixture.path())?;
    let report = analyze_security(
        &project,
        &SecurityOptions {
            top: Some(1),
            surface: false,
            ..SecurityOptions::default()
        },
        None,
    )?;

    assert_eq!(report.candidates.len(), 1);
    assert_eq!(report.total_occurrences, 2);

    Ok(())
}

fn write(fixture: &TempDir, path: &str, source: &str) -> Result<(), std::io::Error> {
    let path = fixture.path().join(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, source)
}
