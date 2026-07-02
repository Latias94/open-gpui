use std::{
    fs,
    path::{Path, PathBuf},
};

pub(crate) fn tracked_text_files(root: &Path, include: fn(&Path) -> bool) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_files(root, root, include, &mut files);
    files
}

fn collect_files(
    root: &Path,
    directory: &Path,
    include: fn(&Path) -> bool,
    files: &mut Vec<PathBuf>,
) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();

        if path.is_dir() {
            if should_skip_dir(root, &path, &file_name) {
                continue;
            }
            collect_files(root, &path, include, files);
        } else if include(&path) {
            files.push(path);
        }
    }
}

fn should_skip_dir(root: &Path, path: &Path, file_name: &str) -> bool {
    if matches!(file_name, ".git" | "target" | "repo-ref") {
        return true;
    }

    path.strip_prefix(root).ok().is_some_and(|relative| {
        relative
            .components()
            .any(|component| component.as_os_str() == "target")
    })
}

pub(crate) fn display_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}
