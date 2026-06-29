use super::{DuplicateMode, DuplicateOptions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NormalizedLine {
    pub(super) line: usize,
    pub(super) column: usize,
    pub(super) text: String,
    pub(super) token_count: usize,
}

pub(super) fn normalized_lines(source: &str, options: &DuplicateOptions) -> Vec<NormalizedLine> {
    let mut lines = Vec::new();
    let mut in_block_comment = false;

    for (index, line) in source.lines().enumerate() {
        let (tokens, column) = tokenize_line(line, options.mode, &mut in_block_comment);
        if tokens.is_empty() {
            continue;
        }
        if options.ignore_imports && is_module_directive(&tokens) {
            continue;
        }
        lines.push(NormalizedLine {
            line: index + 1,
            column,
            text: tokens.join(" "),
            token_count: tokens.len(),
        });
    }

    lines
}

fn tokenize_line(
    line: &str,
    mode: DuplicateMode,
    in_block_comment: &mut bool,
) -> (Vec<String>, usize) {
    let chars = line.char_indices().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut first_column = None;
    let mut index = 0;

    while let Some((column, current)) = chars.get(index).copied() {
        if *in_block_comment {
            if current == '*' && next_char(&chars, index) == Some('/') {
                *in_block_comment = false;
                index += 2;
            } else {
                index += 1;
            }
            continue;
        }
        if current.is_whitespace() {
            index += 1;
            continue;
        }
        if current == '/' && next_char(&chars, index) == Some('/') {
            break;
        }
        if current == '/' && next_char(&chars, index) == Some('*') {
            *in_block_comment = true;
            index += 2;
            continue;
        }

        first_column.get_or_insert(column);
        if is_identifier_start(current) {
            let (identifier, next_index) = read_identifier(&chars, index, line);
            tokens.push(normalize_identifier(&identifier, mode));
            index = next_index;
        } else if current.is_ascii_digit() {
            let next_index = read_number(&chars, index);
            tokens.push(if mode == DuplicateMode::Semantic {
                "NUM".to_owned()
            } else {
                line[column..byte_end(&chars, next_index, line)].to_owned()
            });
            index = next_index;
        } else if current == '\'' || current == '"' {
            let next_index = read_string(&chars, index, current);
            tokens.push(
                if matches!(mode, DuplicateMode::Weak | DuplicateMode::Semantic) {
                    "STR".to_owned()
                } else {
                    line[column..byte_end(&chars, next_index, line)].to_owned()
                },
            );
            index = next_index;
        } else {
            tokens.push(current.to_string());
            index += 1;
        }
    }

    (tokens, first_column.unwrap_or(0))
}

fn next_char(chars: &[(usize, char)], index: usize) -> Option<char> {
    chars.get(index + 1).map(|(_, value)| *value)
}

fn read_identifier(chars: &[(usize, char)], start: usize, line: &str) -> (String, usize) {
    let mut end = start + 1;
    while chars
        .get(end)
        .is_some_and(|(_, value)| is_identifier_continue(*value))
    {
        end += 1;
    }
    let start_byte = chars[start].0;
    let end_byte = byte_end(chars, end, line);
    (line[start_byte..end_byte].to_owned(), end)
}

fn read_number(chars: &[(usize, char)], start: usize) -> usize {
    let mut end = start + 1;
    while chars.get(end).is_some_and(|(_, value)| {
        value.is_ascii_alphanumeric() || matches!(value, '.' | '_' | '+' | '-')
    }) {
        end += 1;
    }
    end
}

fn read_string(chars: &[(usize, char)], start: usize, quote: char) -> usize {
    let mut end = start + 1;
    let mut escaped = false;
    while let Some((_, value)) = chars.get(end) {
        if escaped {
            escaped = false;
        } else if *value == '\\' {
            escaped = true;
        } else if *value == quote {
            return end + 1;
        }
        end += 1;
    }
    end
}

fn byte_end(chars: &[(usize, char)], end: usize, line: &str) -> usize {
    chars.get(end).map_or(line.len(), |(byte, _)| *byte)
}

fn normalize_identifier(identifier: &str, mode: DuplicateMode) -> String {
    if mode == DuplicateMode::Semantic && !is_keyword(identifier) {
        "ID".to_owned()
    } else {
        identifier.to_owned()
    }
}

fn is_identifier_start(value: char) -> bool {
    value == '_' || value == '$' || value.is_alphabetic()
}

fn is_identifier_continue(value: char) -> bool {
    is_identifier_start(value) || value.is_ascii_digit()
}

fn is_module_directive(tokens: &[String]) -> bool {
    matches!(
        tokens.first().map(String::as_str),
        Some("import" | "export" | "part" | "library")
    )
}

fn is_keyword(identifier: &str) -> bool {
    matches!(
        identifier,
        "abstract"
            | "as"
            | "assert"
            | "async"
            | "await"
            | "base"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "covariant"
            | "default"
            | "deferred"
            | "do"
            | "else"
            | "enum"
            | "export"
            | "extends"
            | "extension"
            | "external"
            | "factory"
            | "false"
            | "final"
            | "finally"
            | "for"
            | "Function"
            | "get"
            | "hide"
            | "if"
            | "implements"
            | "import"
            | "in"
            | "interface"
            | "is"
            | "library"
            | "mixin"
            | "new"
            | "null"
            | "on"
            | "operator"
            | "part"
            | "required"
            | "return"
            | "sealed"
            | "set"
            | "show"
            | "static"
            | "super"
            | "switch"
            | "sync"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typedef"
            | "var"
            | "void"
            | "when"
            | "while"
            | "with"
            | "yield"
    )
}
