use std::path::Path;

pub(crate) const GENERATED_DART_SUFFIXES: &[&str] = &[
    ".g.dart",
    ".freezed.dart",
    ".gen.dart",
    ".gr.dart",
    ".mocks.dart",
];

#[must_use]
pub(crate) fn is_generated_dart_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(is_generated_dart_file_name)
}

#[must_use]
pub(crate) fn is_generated_dart_file_name(name: &str) -> bool {
    GENERATED_DART_SUFFIXES
        .iter()
        .any(|suffix| name.ends_with(suffix))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;

    #[test]
    fn recognizes_common_dart_generated_companions() {
        for file_name in [
            "model.g.dart",
            "model.freezed.dart",
            "l10n.gen.dart",
            "routes.gr.dart",
            "service.mocks.dart",
        ] {
            assert!(is_generated_dart_file_name(file_name), "{file_name}");
            assert!(is_generated_dart_path(
                Path::new("lib").join(file_name).as_path()
            ));
        }
    }

    #[test]
    fn rejects_regular_dart_sources() {
        for file_name in ["main.dart", "mock_service.dart", "generated.dart"] {
            assert!(!is_generated_dart_file_name(file_name), "{file_name}");
        }
    }
}
