use std::path::Path;

pub(super) fn should_skip_dir(path: &Path) -> bool {
    matches!(
        path.file_name().and_then(|name| name.to_str()),
        Some(".dart_tool" | ".git" | ".idea" | ".pub-cache" | "build" | "target")
    )
}
