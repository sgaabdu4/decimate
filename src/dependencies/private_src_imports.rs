use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{DependencyKind, Location};

/// A Dart import/export that reaches into another package's private `lib/src`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrivateSrcImport {
    /// Package containing the import/export.
    pub package: String,
    /// Pubspec path for the package containing the import/export.
    pub pubspec_path: PathBuf,
    /// Dart file containing the import/export.
    pub path: PathBuf,
    /// Imported package name.
    pub dependency: String,
    /// Import/export URI.
    pub specifier: String,
    /// Whether the dependency came from an import or export directive.
    pub kind: DependencyKind,
    /// Location of the import/export directive.
    pub location: Location,
}

pub(super) fn imports_private_src(specifier: &str) -> bool {
    specifier
        .strip_prefix("package:")
        .and_then(|rest| rest.split_once('/'))
        .map(|(_, path)| path)
        .is_some_and(|path| path == "src" || path.starts_with("src/"))
}
