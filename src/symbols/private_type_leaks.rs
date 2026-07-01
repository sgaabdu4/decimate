use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::SymbolIndex;
use super::path_filters::{is_library_source, is_private};
use crate::generated::is_generated_dart_path;
use crate::graph::normalize_against;
use crate::{DeclarationKind, Location, ScannedProject};

/// A public API signature that exposes a Dart library-private type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateTypeLeak {
    /// File containing the public declaration.
    pub path: PathBuf,
    /// Public declaration whose signature leaks the private type.
    pub declaration: String,
    /// Kind of the public declaration.
    pub declaration_kind: DeclarationKind,
    /// Private type referenced by the public signature.
    pub private_type: String,
    /// Location of the private type token in the signature.
    pub location: Location,
    /// Whether Dart Decimate can safely delete code from graph evidence alone.
    pub safe_to_delete: bool,
}

pub(super) fn private_type_leaks(
    project: &ScannedProject,
    index: &SymbolIndex,
) -> Vec<PrivateTypeLeak> {
    let private_types_by_library = private_types_by_library(index);
    let mut leaks = Vec::new();

    for file in &project.files {
        let path = normalize_against(&project.root, &file.path);
        if !path.starts_with(&project.root)
            || is_generated_dart_path(&path)
            || !is_library_source(&project.root, &path)
        {
            continue;
        }

        let library = index.library_path(&path);
        let Some(private_types) = private_types_by_library.get(library) else {
            continue;
        };

        leaks.extend(file.signature_references.iter().filter_map(|reference| {
            if !public_signature_leaks_private_type(index, &path, private_types, reference) {
                return None;
            }

            Some(PrivateTypeLeak {
                path: path.clone(),
                declaration: reference.declaration.clone(),
                declaration_kind: reference.declaration_kind,
                private_type: reference.name.clone(),
                location: reference.location,
                safe_to_delete: false,
            })
        }));
    }

    leaks.sort_by(|left, right| {
        (
            &left.path,
            left.location.line,
            left.location.column,
            &left.declaration,
            &left.private_type,
        )
            .cmp(&(
                &right.path,
                right.location.line,
                right.location.column,
                &right.declaration,
                &right.private_type,
            ))
    });
    leaks.dedup_by(|left, right| {
        left.path == right.path
            && left.declaration == right.declaration
            && left.private_type == right.private_type
            && left.location == right.location
    });
    leaks
}

fn private_types_by_library(index: &SymbolIndex) -> BTreeMap<PathBuf, BTreeSet<String>> {
    let mut private_types = BTreeMap::<PathBuf, BTreeSet<String>>::new();
    for indexed in &index.declarations {
        if is_private(&indexed.declaration.name) && is_type_declaration(indexed.declaration.kind) {
            private_types
                .entry(index.library_path(&indexed.path).to_path_buf())
                .or_default()
                .insert(indexed.declaration.name.clone());
        }
    }
    private_types
}

fn public_signature_leaks_private_type(
    index: &SymbolIndex,
    path: &std::path::Path,
    private_types: &BTreeSet<String>,
    reference: &crate::SignatureReference,
) -> bool {
    !is_private(&reference.declaration)
        && is_private(&reference.name)
        && private_types.contains(&reference.name)
        && index.is_public_export(path, &reference.declaration)
}

const fn is_type_declaration(kind: DeclarationKind) -> bool {
    matches!(
        kind,
        DeclarationKind::Class
            | DeclarationKind::Mixin
            | DeclarationKind::ExtensionType
            | DeclarationKind::Enum
            | DeclarationKind::TypeAlias
    )
}
