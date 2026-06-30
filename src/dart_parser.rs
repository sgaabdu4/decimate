use std::borrow::Cow;
use std::path::{Path, PathBuf};

use thiserror::Error;
use tree_sitter::{Parser, Tree};

/// Parsed Dart source and the source buffer used for the parse.
///
/// Tree-Sitter node byte ranges are relative to this buffer, which may be a
/// normalized compatibility copy when the upstream grammar lags new Dart syntax.
pub(crate) struct ParsedDart<'source> {
    tree: Tree,
    source: Cow<'source, str>,
}

impl ParsedDart<'_> {
    pub(crate) fn tree(&self) -> &Tree {
        &self.tree
    }

    pub(crate) fn source(&self) -> &str {
        &self.source
    }
}

/// Errors returned while parsing Dart source.
#[derive(Debug, Error)]
pub(crate) enum DartParseError {
    /// Tree-Sitter rejected the Dart grammar.
    #[error("failed to load Dart grammar: {0}")]
    Language(#[from] tree_sitter::LanguageError),
    /// Tree-Sitter did not produce a parse tree.
    #[error("tree-sitter did not return a parse tree for {path}")]
    ParseCancelled {
        /// Path being parsed.
        path: PathBuf,
    },
    /// The source parsed with syntax errors.
    #[error("Dart syntax errors found in {path}")]
    Syntax {
        /// Path being parsed.
        path: PathBuf,
    },
}

/// Parse Dart source and reject unrecoverable syntax errors.
pub(crate) fn parse_dart_source_strict<'source>(
    path: &Path,
    source: &'source str,
) -> Result<ParsedDart<'source>, DartParseError> {
    let original = parse_raw(path, source)?;
    if !original.root_node().has_error() {
        return Ok(ParsedDart {
            tree: original,
            source: Cow::Borrowed(source),
        });
    }

    if let Some(normalized) = normalize_modern_dart_compatibility(source) {
        let tree = parse_raw(path, &normalized)?;
        if !tree.root_node().has_error() {
            return Ok(ParsedDart {
                tree,
                source: Cow::Owned(normalized),
            });
        }
    }

    Err(DartParseError::Syntax {
        path: path.to_path_buf(),
    })
}

/// Parse Dart source, preferring a syntax-clean compatibility parse when possible.
///
/// Some analyzers historically tolerated partial Tree-Sitter trees. This keeps
/// that behavior while still letting them benefit from compatibility rewrites.
pub(crate) fn parse_dart_source_lossy<'source>(
    path: &Path,
    source: &'source str,
) -> Result<ParsedDart<'source>, DartParseError> {
    let original = parse_raw(path, source)?;
    if !original.root_node().has_error() {
        return Ok(ParsedDart {
            tree: original,
            source: Cow::Borrowed(source),
        });
    }

    if let Some(normalized) = normalize_modern_dart_compatibility(source) {
        let tree = parse_raw(path, &normalized)?;
        if !tree.root_node().has_error() {
            return Ok(ParsedDart {
                tree,
                source: Cow::Owned(normalized),
            });
        }
    }

    Ok(ParsedDart {
        tree: original,
        source: Cow::Borrowed(source),
    })
}

fn parse_raw(path: &Path, source: &str) -> Result<Tree, DartParseError> {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_dart::LANGUAGE.into())?;
    parser
        .parse(source, None)
        .ok_or_else(|| DartParseError::ParseCancelled {
            path: path.to_path_buf(),
        })
}

fn normalize_modern_dart_compatibility(source: &str) -> Option<String> {
    let mut normalized = normalize_primary_constructors(source);
    let mut output = normalized.take().unwrap_or_else(|| source.to_owned());
    let mut changed = output != source;
    changed |= normalize_dot_shorthands(&mut output);
    changed |= normalize_null_aware_collection_elements(&mut output);
    changed.then_some(output)
}

fn normalize_primary_constructors(source: &str) -> Option<String> {
    let mut replacements = Vec::new();
    let mut cursor = 0;

    while cursor < source.len() {
        let Some((keyword_start, keyword)) = find_next_header_keyword(source, cursor) else {
            break;
        };
        cursor = keyword_start + keyword.len();

        let Some(name_start) = skip_whitespace(source, cursor) else {
            continue;
        };
        let Some(name_end) = identifier_end(source, name_start) else {
            continue;
        };
        let mut header_cursor = skip_whitespace(source, name_end).unwrap_or(name_end);

        if source.as_bytes().get(header_cursor).copied() == Some(b'<')
            && let Some(type_params_end) = matching_delimiter(source, header_cursor, b'<', b'>')
        {
            header_cursor =
                skip_whitespace(source, type_params_end + 1).unwrap_or(type_params_end + 1);
        }

        if source.as_bytes().get(header_cursor).copied() != Some(b'(') {
            continue;
        }
        let Some(params_end) = matching_delimiter(source, header_cursor, b'(', b')') else {
            continue;
        };
        let after_params = skip_whitespace(source, params_end + 1).unwrap_or(params_end + 1);
        let next = source.as_bytes().get(after_params).copied();

        let terminator = find_header_terminator(source, after_params);
        match next {
            Some(b'{') => {
                replacements.push(Replacement {
                    start: header_cursor,
                    end: params_end + 1,
                    kind: ReplacementKind::Whitespace,
                });
                cursor = params_end + 1;
            }
            Some(b';') if keyword == "class" => {
                replacements.push(Replacement {
                    start: header_cursor,
                    end: after_params + 1,
                    kind: ReplacementKind::EmptyBody,
                });
                cursor = after_params + 1;
            }
            _ if terminator.is_some_and(|(_, byte)| byte == b';') && keyword == "class" => {
                replacements.push(Replacement {
                    start: header_cursor,
                    end: params_end + 1,
                    kind: ReplacementKind::Whitespace,
                });
                if let Some((terminator_start, _)) = terminator {
                    replacements.push(Replacement {
                        start: terminator_start,
                        end: terminator_start + 1,
                        kind: ReplacementKind::Body,
                    });
                }
                cursor = params_end + 1;
            }
            _ if terminator.is_some_and(|(_, byte)| byte == b'{') => {
                replacements.push(Replacement {
                    start: header_cursor,
                    end: params_end + 1,
                    kind: ReplacementKind::Whitespace,
                });
                cursor = params_end + 1;
            }
            _ => {}
        }
    }

    if replacements.is_empty() {
        return None;
    }

    Some(apply_primary_constructor_replacements(source, replacements))
}

fn apply_primary_constructor_replacements(source: &str, replacements: Vec<Replacement>) -> String {
    let mut normalized = String::with_capacity(source.len());
    let mut copied = 0;
    for replacement in replacements {
        normalized.push_str(&source[copied..replacement.start]);
        match replacement.kind {
            ReplacementKind::Whitespace => {
                push_preserved_whitespace(
                    &mut normalized,
                    &source[replacement.start..replacement.end],
                );
            }
            ReplacementKind::EmptyBody => {
                normalized.push_str("{}");
                let span = &source[replacement.start..replacement.end];
                let skip = span
                    .char_indices()
                    .nth(2)
                    .map_or(span.len(), |(index, _)| index);
                push_preserved_whitespace(&mut normalized, &span[skip..]);
            }
            ReplacementKind::Body => normalized.push_str("{}"),
        }
        copied = replacement.end;
    }
    normalized.push_str(&source[copied..]);
    normalized
}

fn normalize_dot_shorthands(source: &mut String) -> bool {
    let bytes = source.as_bytes().to_vec();
    let mut replacements = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor] != b'.' {
            cursor += 1;
            continue;
        }
        if !is_dot_shorthand_start(&bytes, cursor) {
            cursor += 1;
            continue;
        }
        if source
            .get(cursor + 1..)
            .is_some_and(|suffix| suffix.starts_with("new"))
            && source
                .get(cursor + 4..)
                .and_then(|suffix| suffix.chars().next())
                .is_none_or(|ch| !is_identifier_char(ch))
        {
            replacements.push((cursor, cursor + 4, "New_".to_owned()));
            cursor += 4;
        } else {
            replacements.push((cursor, cursor + 1, " ".to_owned()));
            cursor += 1;
        }
    }
    apply_text_replacements(source, replacements)
}

fn is_dot_shorthand_start(bytes: &[u8], cursor: usize) -> bool {
    if cursor > 0 && bytes[cursor - 1] == b'?' {
        return false;
    }
    let Some(next) = bytes.get(cursor + 1).copied() else {
        return false;
    };
    if !(next == b'_' || next.is_ascii_alphabetic()) {
        return false;
    }
    if matches!(next, b'.' | b'?') {
        return false;
    }
    let Some(previous) = previous_non_whitespace_byte(bytes, cursor) else {
        return true;
    };
    matches!(
        previous,
        b'=' | b'(' | b'[' | b'{' | b',' | b':' | b'?' | b'!' | b'>' | b'|'
    )
}

fn normalize_null_aware_collection_elements(source: &mut String) -> bool {
    let bytes = source.as_bytes().to_vec();
    let mut replacements = Vec::new();
    for cursor in 0..bytes.len() {
        if bytes[cursor] != b'?' || !is_null_aware_collection_marker(&bytes, cursor) {
            continue;
        }
        replacements.push((cursor, cursor + 1, " ".to_owned()));
    }
    apply_text_replacements(source, replacements)
}

fn is_null_aware_collection_marker(bytes: &[u8], cursor: usize) -> bool {
    if matches!(bytes.get(cursor + 1), Some(b'?' | b'.')) {
        return false;
    }
    let Some(next) = next_non_whitespace_byte(bytes, cursor + 1) else {
        return false;
    };
    if next == b':' || next == b',' || next == b']' || next == b'}' {
        return false;
    }
    let Some(previous) = previous_non_whitespace_byte(bytes, cursor) else {
        return false;
    };
    matches!(previous, b'[' | b'{' | b',' | b':')
        || (previous == b'.'
            && cursor >= 3
            && bytes.get(cursor - 3..cursor) == Some(&[b'.', b'.', b'.'][..]))
}

fn apply_text_replacements(source: &mut String, replacements: Vec<(usize, usize, String)>) -> bool {
    if replacements.is_empty() {
        return false;
    }
    for (start, end, replacement) in replacements.into_iter().rev() {
        source.replace_range(start..end, &replacement);
    }
    true
}

fn find_header_terminator(source: &str, start: usize) -> Option<(usize, u8)> {
    let bytes = source.as_bytes();
    let mut cursor = start;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'{' | b';' => return Some((cursor, bytes[cursor])),
            b'(' => cursor = matching_delimiter(source, cursor, b'(', b')')?,
            b'<' => cursor = matching_delimiter(source, cursor, b'<', b'>')?,
            b'\'' | b'"' => cursor = skip_quoted(source, cursor)?,
            _ => {}
        }
        cursor += 1;
    }
    None
}

fn previous_non_whitespace_byte(bytes: &[u8], cursor: usize) -> Option<u8> {
    bytes
        .get(..cursor)?
        .iter()
        .rev()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
}

fn next_non_whitespace_byte(bytes: &[u8], cursor: usize) -> Option<u8> {
    bytes
        .get(cursor..)?
        .iter()
        .copied()
        .find(|byte| !byte.is_ascii_whitespace())
}

#[derive(Debug)]
struct Replacement {
    start: usize,
    end: usize,
    kind: ReplacementKind,
}

#[derive(Debug)]
enum ReplacementKind {
    Whitespace,
    EmptyBody,
    Body,
}

fn find_next_header_keyword(source: &str, start: usize) -> Option<(usize, &'static str)> {
    let mut cursor = start;
    while cursor < source.len() {
        if starts_keyword(source, cursor, "class") {
            return Some((cursor, "class"));
        }
        if starts_keyword(source, cursor, "enum") {
            return Some((cursor, "enum"));
        }
        cursor = next_char_boundary(source, cursor)?;
    }
    None
}

fn starts_keyword(source: &str, start: usize, keyword: &str) -> bool {
    source
        .get(start..)
        .is_some_and(|suffix| suffix.starts_with(keyword))
        && !source
            .get(..start)
            .and_then(|prefix| prefix.chars().next_back())
            .is_some_and(is_identifier_char)
        && !source
            .get(start + keyword.len()..)
            .and_then(|suffix| suffix.chars().next())
            .is_some_and(is_identifier_char)
}

fn skip_whitespace(source: &str, start: usize) -> Option<usize> {
    let mut cursor = start;
    while cursor < source.len() {
        let ch = source.get(cursor..)?.chars().next()?;
        if !ch.is_whitespace() {
            return Some(cursor);
        }
        cursor += ch.len_utf8();
    }
    Some(cursor)
}

fn identifier_end(source: &str, start: usize) -> Option<usize> {
    let mut chars = source.get(start..)?.char_indices();
    let (_, first) = chars.next()?;
    if !is_identifier_start(first) {
        return None;
    }

    for (offset, ch) in chars {
        if !is_identifier_char(ch) {
            return Some(start + offset);
        }
    }
    Some(source.len())
}

fn matching_delimiter(source: &str, start: usize, open: u8, close: u8) -> Option<usize> {
    let bytes = source.as_bytes();
    if bytes.get(start).copied() != Some(open) {
        return None;
    }

    let mut depth = 0usize;
    let mut cursor = start;
    while cursor < bytes.len() {
        match bytes[cursor] {
            byte if byte == open => depth += 1,
            byte if byte == close => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(cursor);
                }
            }
            b'\'' | b'"' => cursor = skip_quoted(source, cursor)?,
            _ => {}
        }
        cursor += 1;
    }
    None
}

fn skip_quoted(source: &str, start: usize) -> Option<usize> {
    let quote = source.as_bytes().get(start).copied()?;
    let mut cursor = start + 1;
    while cursor < source.len() {
        match source.as_bytes()[cursor] {
            b'\\' => cursor += 2,
            byte if byte == quote => return Some(cursor),
            _ => cursor += 1,
        }
    }
    None
}

fn push_preserved_whitespace(output: &mut String, span: &str) {
    for ch in span.chars() {
        if ch == '\n' || ch == '\r' {
            output.push(ch);
        } else {
            output.push(' ');
        }
    }
}

fn next_char_boundary(source: &str, start: usize) -> Option<usize> {
    source
        .get(start..)?
        .chars()
        .next()
        .map(|ch| start + ch.len_utf8())
}

fn is_identifier_start(ch: char) -> bool {
    ch == '_' || ch.is_ascii_alphabetic()
}

fn is_identifier_char(ch: char) -> bool {
    is_identifier_start(ch) || ch.is_ascii_digit()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{DartParseError, parse_dart_source_strict};

    #[test]
    fn strict_parse_normalizes_primary_constructor_headers() -> Result<(), DartParseError> {
        let source = "\
class Point(
  var int x,
  var int y
) extends Shape with Traceable implements Drawable;

enum Tone(final String label) {
  quiet('q');
}
";

        let parsed = parse_dart_source_strict(Path::new("lib/modern.dart"), source)?;

        assert!(!parsed.tree().root_node().has_error());
        assert!(parsed.source().contains("class Point"));
        assert!(parsed.source().contains("extends Shape"));
        assert!(parsed.source().contains("enum Tone"));

        Ok(())
    }

    #[test]
    fn strict_parse_normalizes_current_dart_shorthands() -> Result<(), DartParseError> {
        let source = "\
enum Color { red, blue }

Widget build(Banner? banner, List<Widget>? extras) {
  final key = banner?.key;
  final color = .red;
  return Column(children: [
    ?banner,
    ...?extras,
    Button.style(.filled),
  ]);
}
";

        let parsed = parse_dart_source_strict(Path::new("lib/current.dart"), source)?;

        assert!(!parsed.tree().root_node().has_error());

        Ok(())
    }

    #[test]
    fn strict_parse_keeps_unrecoverable_syntax_errors() {
        let error = parse_dart_source_strict(Path::new("lib/bad.dart"), "class {")
            .err()
            .map(|error| error.to_string());

        assert_eq!(
            error.as_deref(),
            Some("Dart syntax errors found in lib/bad.dart")
        );
    }
}
