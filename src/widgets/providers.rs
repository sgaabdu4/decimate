use std::path::Path;

use tree_sitter::Node;

use crate::Location;

use super::ManualRiverpodProvider;

const RIVERPOD_IMPORTS: &[&str] = &[
    "package:flutter_riverpod/",
    "package:hooks_riverpod/",
    "package:riverpod/",
];

const MANUAL_PROVIDER_TYPES: &[&str] = &[
    "Provider",
    "FutureProvider",
    "StreamProvider",
    "StateProvider",
    "NotifierProvider",
    "AsyncNotifierProvider",
    "StateNotifierProvider",
    "ChangeNotifierProvider",
];

const VARIABLE_DECLARATION_KINDS: &[&str] = &[
    "top_level_variable_declaration",
    "initialized_identifier",
    "static_final_declaration",
];

const NESTED_SCOPE_KINDS: &[&str] = &[
    "local_variable_declaration",
    "function_body",
    "block",
    "class_body",
    "enum_body",
    "mixin_body",
    "extension_body",
    "extension_type_body",
];

pub(super) fn manual_riverpod_providers(
    path: &Path,
    root: Node<'_>,
    source: &str,
) -> Vec<ManualRiverpodProvider> {
    if !imports_riverpod(source) {
        return Vec::new();
    }

    let mut providers = Vec::new();
    collect_manual_providers(path, root, source, &mut providers);
    providers
}

fn collect_manual_providers(
    path: &Path,
    node: Node<'_>,
    source: &str,
    providers: &mut Vec<ManualRiverpodProvider>,
) {
    if VARIABLE_DECLARATION_KINDS.contains(&node.kind()) && !has_nested_scope(node) {
        if let Ok(text) = node.utf8_text(source.as_bytes()) {
            providers.extend(provider_declarations_in_text(
                path,
                source,
                node.start_byte(),
                text,
            ));
        }
        return;
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_manual_providers(path, child, source, providers);
    }
}

fn has_nested_scope(node: Node<'_>) -> bool {
    let mut parent = node.parent();
    while let Some(ancestor) = parent {
        if NESTED_SCOPE_KINDS.contains(&ancestor.kind()) {
            return true;
        }
        parent = ancestor.parent();
    }
    false
}

fn imports_riverpod(source: &str) -> bool {
    source
        .lines()
        .map(str::trim_start)
        .any(|line| line.starts_with("import ") && is_riverpod_import(line))
}

fn is_riverpod_import(text: &str) -> bool {
    RIVERPOD_IMPORTS
        .iter()
        .any(|package| text.contains(package))
}

fn provider_declarations_in_text(
    path: &Path,
    source: &str,
    source_offset: usize,
    text: &str,
) -> Vec<ManualRiverpodProvider> {
    assignment_starts(text)
        .into_iter()
        .filter_map(|assignment| {
            let expression = text.get(assignment + 1..)?;
            let start = first_non_whitespace(expression)?;
            let expression_start = assignment + 1 + start;
            let provider_type = provider_type_at(expression.get(start..)?)?;
            let left_text = text.get(..assignment)?;
            let provider_name = variable_name_before_assignment(left_text)?;
            Some(ManualRiverpodProvider {
                path: path.to_path_buf(),
                provider_name,
                provider_type,
                location: location_for_byte(source, source_offset + expression_start),
            })
        })
        .collect()
}

fn assignment_starts(text: &str) -> Vec<usize> {
    text.match_indices('=')
        .filter_map(|(index, _)| {
            let before = text.as_bytes().get(index.wrapping_sub(1)).copied();
            let after = text.as_bytes().get(index + 1).copied();
            (!matches!(before, Some(b'=' | b'!' | b'<' | b'>'))
                && !matches!(after, Some(b'=' | b'>')))
            .then_some(index)
        })
        .collect()
}

fn first_non_whitespace(text: &str) -> Option<usize> {
    text.char_indices()
        .find(|(_, character)| !character.is_whitespace())
        .map(|(index, _)| index)
}

fn provider_type_at(expression: &str) -> Option<String> {
    let chain = expression
        .char_indices()
        .take_while(|(_, character)| {
            character.is_ascii_alphanumeric() || *character == '_' || *character == '.'
        })
        .map(|(_, character)| character)
        .collect::<String>();
    if chain.is_empty() || !call_follows_chain(expression, chain.len()) {
        return None;
    }
    chain
        .split('.')
        .find(|segment| MANUAL_PROVIDER_TYPES.contains(segment))
        .map(str::to_owned)
}

fn call_follows_chain(expression: &str, chain_len: usize) -> bool {
    expression
        .get(chain_len..)
        .and_then(first_non_whitespace)
        .and_then(|offset| expression[chain_len + offset..].chars().next())
        .is_some_and(|character| matches!(character, '<' | '('))
}

fn variable_name_before_assignment(left: &str) -> Option<String> {
    let trimmed = left.trim_end();
    let end = trimmed.len();
    let start = trimmed
        .char_indices()
        .rev()
        .find(|(_, character)| !is_identifier_continue(*character))
        .map_or(0, |(index, character)| index + character.len_utf8());
    let name = trimmed.get(start..end)?;
    (!name.is_empty() && name.chars().next().is_some_and(is_identifier_start))
        .then(|| name.to_owned())
}

fn is_identifier_start(character: char) -> bool {
    character == '_' || character.is_ascii_alphabetic()
}

fn is_identifier_continue(character: char) -> bool {
    is_identifier_start(character) || character.is_ascii_digit()
}

fn location_for_byte(source: &str, byte: usize) -> Location {
    let mut line = 1;
    let mut line_start = 0;
    for (index, value) in source.bytes().enumerate().take(byte) {
        if value == b'\n' {
            line += 1;
            line_start = index + 1;
        }
    }
    Location {
        line,
        column: byte.saturating_sub(line_start),
    }
}
