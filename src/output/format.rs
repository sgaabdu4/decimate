use std::path::Path;

pub(super) fn dependency_kind(kind: crate::DependencyKind) -> String {
    match kind {
        crate::DependencyKind::Import => "import",
        crate::DependencyKind::Export => "export",
        crate::DependencyKind::Part => "part",
        crate::DependencyKind::Augment => "augment",
    }
    .to_owned()
}

pub(super) fn declaration_kind(kind: crate::DeclarationKind) -> &'static str {
    match kind {
        crate::DeclarationKind::Class => "class",
        crate::DeclarationKind::Mixin => "mixin",
        crate::DeclarationKind::Extension => "extension",
        crate::DeclarationKind::ExtensionType => "extension type",
        crate::DeclarationKind::Enum => "enum",
        crate::DeclarationKind::TypeAlias => "typedef",
        crate::DeclarationKind::Variable => "variable",
        crate::DeclarationKind::Function => "function",
    }
}

pub(super) fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
