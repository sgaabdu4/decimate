use std::path::Path;

pub(super) fn is_private(name: &str) -> bool {
    name.starts_with('_')
}

pub(super) fn is_library_source(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root).is_ok_and(|relative| {
        relative
            .components()
            .next()
            .is_some_and(|component| component.as_os_str() == "lib")
    })
}

pub(super) fn is_public_library_entry(root: &Path, path: &Path) -> bool {
    path.strip_prefix(root).is_ok_and(|relative| {
        let mut components = relative.components();
        components
            .next()
            .is_some_and(|component| component.as_os_str() == "lib")
            && components
                .next()
                .is_none_or(|component| component.as_os_str() != "src")
            && relative.extension().is_some_and(|ext| ext == "dart")
    })
}

pub(super) fn is_generated_path(path: &Path) -> bool {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    matches!(
        file_name,
        name if name.ends_with(".g.dart")
            || name.ends_with(".freezed.dart")
            || name.ends_with(".gen.dart")
            || name.ends_with(".gr.dart")
            || name.ends_with(".mocks.dart")
    )
}
