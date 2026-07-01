use std::path::Path;

use super::{DetectedSecurityCandidate, SecurityCategory, SecurityConfidence, SecurityOccurrence};
use crate::Location;
use crate::generated::is_generated_dart_path;

pub(super) fn detect_in_source(path: &Path, source: &str) -> Vec<DetectedSecurityCandidate> {
    let mut candidates = Vec::new();
    detect_hardcoded_secrets(path, source, &mut candidates);
    detect_insecure_transport(path, source, &mut candidates);
    detect_tls_bypass(path, source, &mut candidates);
    detect_webview_risk(path, source, &mut candidates);
    detect_process_execution(path, source, &mut candidates);
    detect_raw_sql(path, source, &mut candidates);
    detect_plain_secret_storage(path, source, &mut candidates);
    candidates
}

pub(super) fn is_ignored_path(path: &Path) -> bool {
    if path.components().any(|component| {
        let value = component.as_os_str().to_string_lossy();
        matches!(value.as_ref(), "test" | "integration_test" | "test_driver")
    }) {
        return true;
    }
    is_generated_dart_path(path)
}

fn detect_hardcoded_secrets(
    path: &Path,
    source: &str,
    candidates: &mut Vec<DetectedSecurityCandidate>,
) {
    for literal in string_literals(source) {
        if is_comment_match(source, literal.index) || is_placeholder(&literal.value) {
            continue;
        }
        let line = line_at(source, literal.index);
        let secret_value = has_secret_shape(&literal.value);
        if line.contains("FirebaseOptions")
            || line.contains("googleAppId")
            || (!secret_value && literal_looks_like_storage_key(&literal.value))
            || (!secret_value && diagnostic_context(source, literal.index))
            || (!secret_value && literal.value.contains('$'))
        {
            continue;
        }
        let secret_name = has_secret_like_name(line);
        if secret_name && literal.value.len() >= 12 || secret_value {
            candidates.push(detected(
                path,
                source,
                literal.index,
                SecurityCategory::HardcodedSecret,
                "secret-literal",
                if secret_value {
                    SecurityConfidence::High
                } else {
                    SecurityConfidence::Medium
                },
                "string-literal",
            ));
        }
    }
}

fn detect_insecure_transport(
    path: &Path,
    source: &str,
    candidates: &mut Vec<DetectedSecurityCandidate>,
) {
    for literal in string_literals(source) {
        if is_comment_match(source, literal.index)
            || !literal.value.starts_with("http://")
            || is_local_http_url(&literal.value)
        {
            continue;
        }
        let line = line_at(source, literal.index);
        if has_network_context(line) {
            candidates.push(detected(
                path,
                source,
                literal.index,
                SecurityCategory::InsecureTransport,
                "cleartext-http",
                SecurityConfidence::High,
                "http-url",
            ));
        }
    }
}

fn detect_tls_bypass(path: &Path, source: &str, candidates: &mut Vec<DetectedSecurityCandidate>) {
    for pattern in [
        "badCertificateCallback",
        "HttpOverrides.global",
        "SecurityContext(withTrustedRoots: false",
        "validateCertificate",
    ] {
        for index in match_indices(source, pattern) {
            let window = following_window(source, index, 4);
            let risky = match pattern {
                "badCertificateCallback" | "validateCertificate" => {
                    returns_true(window) && !returns_false(window)
                }
                _ => true,
            };
            if risky {
                candidates.push(detected(
                    path,
                    source,
                    index,
                    SecurityCategory::TlsBypass,
                    pattern,
                    SecurityConfidence::High,
                    pattern,
                ));
            }
        }
    }
}

fn detect_webview_risk(path: &Path, source: &str, candidates: &mut Vec<DetectedSecurityCandidate>) {
    for pattern in [
        "JavaScriptMode.unrestricted",
        "javaScriptEnabled: true",
        "allowFileAccess: true",
        "allowFileAccessFromFileURLs: true",
        "allowUniversalAccessFromFileURLs: true",
    ] {
        for index in match_indices(source, pattern) {
            candidates.push(detected(
                path,
                source,
                index,
                SecurityCategory::WebViewRisk,
                "webview",
                SecurityConfidence::High,
                pattern,
            ));
        }
    }
    for literal in string_literals(source) {
        if literal.value.starts_with("file://") {
            let line = line_at(source, literal.index);
            if line.contains("loadUrl") || line.contains("loadRequest") {
                candidates.push(detected(
                    path,
                    source,
                    literal.index,
                    SecurityCategory::WebViewRisk,
                    "webview-file-url",
                    SecurityConfidence::Medium,
                    "file-url",
                ));
            }
        }
    }
}

fn detect_process_execution(
    path: &Path,
    source: &str,
    candidates: &mut Vec<DetectedSecurityCandidate>,
) {
    for pattern in [
        "Process.run(",
        "Process.start(",
        "processManager.run(",
        "processManager.start(",
    ] {
        for index in match_indices(source, pattern) {
            let window = following_window(source, index, 3);
            let args = call_inside_after(source, index + pattern.len());
            let risky = window.contains("runInShell: true")
                || args.as_deref().is_some_and(|call| {
                    !first_call_arg_is_fixed_literal(call) || has_dynamic_text(call)
                });
            if risky {
                candidates.push(detected(
                    path,
                    source,
                    index,
                    SecurityCategory::ProcessExecution,
                    "process-exec",
                    SecurityConfidence::High,
                    pattern.trim_end_matches('('),
                ));
            }
        }
    }
}

fn detect_raw_sql(path: &Path, source: &str, candidates: &mut Vec<DetectedSecurityCandidate>) {
    for pattern in [
        ".rawQuery(",
        ".rawInsert(",
        ".rawUpdate(",
        ".rawDelete(",
        ".execute(",
        ".customSelect(",
        ".customStatement(",
    ] {
        for index in match_indices(source, pattern) {
            let Some(call) = call_inside_after(source, index + pattern.len()) else {
                continue;
            };
            if sql_call_is_parameterized(&call) {
                continue;
            }
            let risky = has_dynamic_text(&call) || !first_call_arg_is_fixed_literal(&call);
            let broad_execute = pattern == ".execute(";
            if risky && (!broad_execute || sql_like_text(&call)) {
                candidates.push(detected(
                    path,
                    source,
                    index,
                    SecurityCategory::RawSql,
                    "raw-sql",
                    SecurityConfidence::High,
                    pattern.trim_start_matches('.').trim_end_matches('('),
                ));
            }
        }
    }
    for index in match_indices(source, "where:") {
        let line = line_at(source, index);
        if (line.contains('$') || line.contains('+')) && !line.contains("whereArgs") {
            candidates.push(detected(
                path,
                source,
                index,
                SecurityCategory::RawSql,
                "dynamic-where",
                SecurityConfidence::Medium,
                "where",
            ));
        }
    }
}

fn detect_plain_secret_storage(
    path: &Path,
    source: &str,
    candidates: &mut Vec<DetectedSecurityCandidate>,
) {
    for pattern in [
        ".setString(",
        ".setStringList(",
        ".put(",
        ".writeAsString(",
        ".writeAsBytes(",
    ] {
        for index in match_indices(source, pattern) {
            let line = line_at(source, index);
            if line.contains("FlutterSecureStorage") || line.contains(".write(") {
                continue;
            }
            let storage_context = match pattern {
                ".setString(" | ".setStringList(" => true,
                ".put(" => line.contains("Hive") || line.contains("box."),
                _ => line.contains("File(") || line.contains(".writeAs"),
            };
            if storage_context && has_secret_like_name(line) && !line.contains("HiveAesCipher") {
                candidates.push(detected(
                    path,
                    source,
                    index,
                    SecurityCategory::PlainSecretStorage,
                    "plain-local-storage",
                    SecurityConfidence::Medium,
                    pattern.trim_start_matches('.').trim_end_matches('('),
                ));
            }
        }
    }
}

fn detected(
    path: &Path,
    source: &str,
    index: usize,
    category: SecurityCategory,
    sink: &str,
    confidence: SecurityConfidence,
    expression: &str,
) -> DetectedSecurityCandidate {
    DetectedSecurityCandidate {
        category,
        sink: sink.to_owned(),
        confidence,
        occurrence: SecurityOccurrence {
            path: path.to_path_buf(),
            location: location_for_index(source, index),
            expression: expression.to_owned(),
            evidence: redact_line(line_at(source, index)),
            reachability: None,
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct StringLiteral {
    index: usize,
    value: String,
}

fn string_literals(source: &str) -> Vec<StringLiteral> {
    let mut literals = Vec::new();
    let mut index = 0;
    let bytes = source.as_bytes();
    while index < bytes.len() {
        if matches!(bytes[index], b'\'' | b'"') && !is_comment_match(source, index) {
            if let Some((value, end)) = read_string(source, index) {
                literals.push(StringLiteral { index, value });
                index = end;
                continue;
            }
        }
        index += 1;
    }
    literals
}

fn read_string(source: &str, start: usize) -> Option<(String, usize)> {
    let bytes = source.as_bytes();
    let quote = *bytes.get(start)?;
    let mut index = start + 1;
    let value_start = index;
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index = (index + 2).min(bytes.len());
            continue;
        }
        if bytes[index] == quote {
            return Some((source[value_start..index].to_owned(), index + 1));
        }
        index += 1;
    }
    None
}

fn match_indices(source: &str, pattern: &str) -> Vec<usize> {
    let mut matches = Vec::new();
    let mut offset = 0;
    while let Some(relative) = source[offset..].find(pattern) {
        let index = offset + relative;
        offset = index + pattern.len();
        if !is_comment_match(source, index) {
            matches.push(index);
        }
    }
    matches
}

fn call_inside_after(source: &str, start: usize) -> Option<String> {
    let bytes = source.as_bytes();
    let mut depth = 1usize;
    let mut index = start;
    while index < bytes.len() {
        match bytes[index] {
            b'\'' | b'"' => {
                index = read_string(source, index)?.1;
                continue;
            }
            b'(' => depth += 1,
            b')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(source[start..index].to_owned());
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn first_call_arg_is_fixed_literal(call: &str) -> bool {
    let trimmed = call.trim_start();
    if !matches!(trimmed.as_bytes().first(), Some(b'\'' | b'"')) {
        return false;
    }
    let Some((_, end)) = read_string(trimmed, 0) else {
        return false;
    };
    let rest = trimmed[end..].trim_start();
    matches!(rest.as_bytes().first(), Some(b',' | b')'))
}

fn sql_call_is_parameterized(call: &str) -> bool {
    call.contains('?') && (call.contains('[') || call.contains("whereArgs"))
}

fn has_dynamic_text(text: &str) -> bool {
    text.contains('$') || text.contains(" + ") || text.contains("+ ") || text.contains(" +")
}

fn sql_like_text(text: &str) -> bool {
    let upper = text.to_ascii_uppercase();
    [
        "SELECT ", "INSERT ", "UPDATE ", "DELETE ", "CREATE ", "DROP ", "ALTER ", " WHERE ",
        " FROM ", " JOIN ",
    ]
    .iter()
    .any(|keyword| upper.contains(keyword))
}

fn has_network_context(line: &str) -> bool {
    [
        "Uri.parse",
        "http.",
        "Dio(",
        "BaseOptions",
        "Request(",
        "getUrl",
        "openUrl",
        "loadRequest",
    ]
    .iter()
    .any(|pattern| line.contains(pattern))
}

fn has_secret_like_name(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    [
        "secret",
        "token",
        "jwt",
        "privatekey",
        "private_key",
        "clientsecret",
        "client_secret",
        "refreshtoken",
        "refresh_token",
        "accesstoken",
        "access_token",
        "password",
        "passwd",
        "bearer",
        "authorization",
        "apikey",
        "api_key",
        "signingkey",
        "signing_key",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn has_secret_shape(value: &str) -> bool {
    if value.contains('$') {
        return false;
    }
    value.contains("-----BEGIN ") && value.contains("PRIVATE KEY-----")
        || value.starts_with("Bearer ")
        || value.starts_with("sk_")
        || value.starts_with("ghp_")
        || jwt_like(value)
        || long_hex(value)
}

fn jwt_like(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| part.len() >= 10 && is_base64ish(part))
}

fn long_hex(value: &str) -> bool {
    value.len() >= 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn is_base64ish(value: &str) -> bool {
    value.bytes().all(|byte| {
        byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'+' | b'/' | b'=')
    })
}

fn literal_looks_like_storage_key(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed.len() <= 64
        && trimmed
            .bytes()
            .all(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'-' | b'.'))
}

fn diagnostic_context(source: &str, index: usize) -> bool {
    let mut start = source[..index]
        .rfind('\n')
        .map_or(0, |position| position + 1);
    for _ in 0..2 {
        if start == 0 {
            break;
        }
        start = source[..start - 1]
            .rfind('\n')
            .map_or(0, |position| position + 1);
    }
    let end = source[index..]
        .find('\n')
        .map_or(source.len(), |position| index + position);
    line_is_diagnostic_text(&source[start..end])
}

fn line_is_diagnostic_text(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    [
        "_log.",
        "logger.",
        "log.",
        "print(",
        "debugprint(",
        "throw ",
        "exception(",
        "error(",
    ]
    .iter()
    .any(|pattern| lower.contains(pattern))
}

fn is_placeholder(value: &str) -> bool {
    let normalized = value
        .trim()
        .trim_matches(|character| matches!(character, '<' | '>' | '{' | '}'))
        .to_ascii_lowercase()
        .replace(['-', '_', ' '], "");
    normalized.is_empty()
        || normalized.chars().all(|character| character == 'x')
        || [
            "todo",
            "example",
            "dummy",
            "test",
            "changeme",
            "redacted",
            "yourapikey",
            "yourkeyhere",
            "yourtoken",
            "placeholder",
        ]
        .iter()
        .any(|placeholder| normalized.contains(placeholder))
}

fn is_local_http_url(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "http://localhost",
        "http://127.0.0.1",
        "http://[::1]",
        "http://::1",
        "http://10.0.2.2",
        "http://0.0.0.0",
    ]
    .iter()
    .any(|prefix| lower.starts_with(prefix))
}

fn returns_true(text: &str) -> bool {
    text.contains("=> true") || text.contains("return true") || text.contains(": true")
}

fn returns_false(text: &str) -> bool {
    text.contains("=> false") || text.contains("return false") || text.contains(": false")
}

fn following_window(source: &str, index: usize, lines: usize) -> &str {
    let mut end = index;
    for _ in 0..lines {
        end = source[end..]
            .find('\n')
            .map_or(source.len(), |relative| end + relative + 1);
        if end == source.len() {
            break;
        }
    }
    &source[index..end]
}

fn line_at(source: &str, index: usize) -> &str {
    let start = source[..index]
        .rfind('\n')
        .map_or(0, |position| position + 1);
    let end = source[index..]
        .find('\n')
        .map_or(source.len(), |position| index + position);
    &source[start..end]
}

fn redact_line(line: &str) -> String {
    let mut redacted = String::new();
    let mut index = 0;
    let bytes = line.as_bytes();
    while index < bytes.len() {
        if matches!(bytes[index], b'\'' | b'"') {
            let quote = bytes[index] as char;
            redacted.push(quote);
            redacted.push_str("<redacted>");
            redacted.push(quote);
            if let Some((_, end)) = read_string(line, index) {
                index = end;
                continue;
            }
        }
        redacted.push(bytes[index] as char);
        index += 1;
    }
    redacted.trim().to_owned()
}

fn is_comment_match(source: &str, index: usize) -> bool {
    let line_start = source[..index]
        .rfind('\n')
        .map_or(0, |position| position + 1);
    if source[line_start..index].contains("//") {
        return true;
    }
    let last_block_open = source[..index].rfind("/*");
    let last_block_close = source[..index].rfind("*/");
    last_block_open.is_some_and(|open| last_block_close.is_none_or(|close| open > close))
}

fn location_for_index(source: &str, index: usize) -> Location {
    let line = source[..index]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        + 1;
    let column = source[..index]
        .rfind('\n')
        .map_or(index, |position| index - position - 1);
    Location { line, column }
}
