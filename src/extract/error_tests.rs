use super::{extract_dart_file, extract_dart_source};

#[test]
fn reports_missing_files() {
    let error = extract_dart_file("__dart_decimate_missing_file__.dart")
        .err()
        .map(|error| error.to_string());

    assert!(matches!(
        error.as_deref(),
        Some(message) if message.contains("failed to read Dart file")
    ));
}

#[test]
fn reports_syntax_errors() {
    let error = extract_dart_source("lib/bad.dart", "class {")
        .err()
        .map(|error| error.to_string());

    assert!(matches!(
        error.as_deref(),
        Some("Dart syntax errors found in lib/bad.dart")
    ));
}
