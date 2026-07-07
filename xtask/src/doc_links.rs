use std::{
    fs,
    path::{Component, Path, PathBuf},
};

use crate::fs_scan::display_path;

pub(crate) fn scan_doc_links(root: &Path) -> Result<(), ()> {
    println!("==> scan public doc links");
    let failures = doc_link_failures(root);
    if failures.is_empty() {
        println!("public doc link scan passed");
        Ok(())
    } else {
        eprintln!("public doc link scan failed:");
        for failure in failures {
            eprintln!("  {failure}");
        }
        Err(())
    }
}

fn doc_link_failures(root: &Path) -> Vec<String> {
    let mut failures = Vec::new();
    for path in strict_doc_files(root) {
        let label = display_path(root, &path);
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) => {
                failures.push(format!("{label}: failed to read file: {error}"));
                continue;
            }
        };
        failures.extend(markdown_link_failures(root, &path, &contents));
    }
    failures
}

fn strict_doc_files(root: &Path) -> Vec<PathBuf> {
    let mut files = vec![
        root.join("README.md"),
        root.join("CHANGELOG.md"),
        root.join("docs/adr/README.md"),
        root.join("docs/release/breaking-changes.md"),
        root.join("docs/verification.md"),
    ];

    for crate_dir in [
        "crates/canvas",
        "crates/gpui",
        "crates/gpui_docking",
        "crates/gpui_platform",
        "crates/gpui_web",
        "crates/motion",
        "crates/ui_components",
    ] {
        files.push(root.join(crate_dir).join("README.md"));
    }

    files
}

fn markdown_link_failures(root: &Path, file: &Path, contents: &str) -> Vec<String> {
    let mut failures = Vec::new();
    let mut in_fence = false;

    for (line_index, line) in contents.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }

        for target in markdown_link_targets(line) {
            if should_skip_target(&target) {
                continue;
            }
            let Some(target_path) = target.split('#').next().map(str::trim) else {
                continue;
            };
            if target_path.is_empty() {
                continue;
            }
            let target_path = target_path.trim_matches('<').trim_matches('>');
            let resolved = match resolve_markdown_link_target(root, file, target_path) {
                Ok(resolved) => resolved,
                Err(()) => {
                    failures.push(format!(
                        "{}:{}: link `{target}` escapes the repository root",
                        display_path(root, file),
                        line_index + 1
                    ));
                    continue;
                }
            };
            if !resolved.exists() {
                failures.push(format!(
                    "{}:{}: link `{target}` resolves to missing path `{}`",
                    display_path(root, file),
                    line_index + 1,
                    display_path(root, &resolved)
                ));
            }
        }
    }

    failures
}

fn resolve_markdown_link_target(
    root: &Path,
    file: &Path,
    target_path: &str,
) -> Result<PathBuf, ()> {
    let source_dir = file.parent().unwrap_or(root);
    let source_relative = source_dir.strip_prefix(root).map_err(|_| ())?;
    let mut relative = PathBuf::new();

    for component in source_relative.components() {
        push_normalized_component(&mut relative, component)?;
    }
    for component in Path::new(target_path).components() {
        push_normalized_component(&mut relative, component)?;
    }

    Ok(root.join(relative))
}

fn push_normalized_component(relative: &mut PathBuf, component: Component<'_>) -> Result<(), ()> {
    match component {
        Component::Normal(part) => {
            relative.push(part);
            Ok(())
        }
        Component::CurDir => Ok(()),
        Component::ParentDir => {
            if relative.pop() {
                Ok(())
            } else {
                Err(())
            }
        }
        Component::RootDir | Component::Prefix(_) => Err(()),
    }
}

fn markdown_link_targets(line: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let bytes = line.as_bytes();
    let mut index = 0;

    while let Some(close_offset) = line[index..].find("](") {
        let close = index + close_offset;
        let open = line[..close].rfind('[');
        if open.is_some_and(|open| open > 0 && bytes[open - 1] == b'!') {
            index = close + 2;
            continue;
        }
        let target_start = close + 2;
        let target_end = if line[target_start..].starts_with('<') {
            let Some(angle_end_offset) = line[target_start + 1..].find('>') else {
                break;
            };
            let angle_end = target_start + 1 + angle_end_offset;
            if line[angle_end + 1..].starts_with(')') {
                angle_end + 1
            } else {
                break;
            }
        } else {
            let Some(target_end_offset) = line[target_start..].find(')') else {
                break;
            };
            target_start + target_end_offset
        };
        targets.push(line[target_start..target_end].to_string());
        index = target_end + 1;
    }

    targets
}

fn should_skip_target(target: &str) -> bool {
    let target = target.trim().trim_matches('<').trim_matches('>');
    target.is_empty()
        || target.starts_with('#')
        || target.starts_with("http://")
        || target.starts_with("https://")
        || target.starts_with("mailto:")
        || target.starts_with("tel:")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_link_target_parser_skips_image_links() {
        assert_eq!(
            markdown_link_targets("See [doc](docs/verification.md) and ![alt](image.png)."),
            vec!["docs/verification.md".to_string()]
        );
    }

    #[test]
    fn skip_target_allows_external_and_anchor_links() {
        assert!(should_skip_target("https://example.com"));
        assert!(should_skip_target(
            "<https://en.wikipedia.org/wiki/Register_(sociolinguistics)>"
        ));
        assert!(should_skip_target("#local"));
        assert!(!should_skip_target("docs/verification.md"));
    }

    #[test]
    fn link_target_resolver_normalizes_parent_dirs_inside_root() {
        let root = PathBuf::from("repo");
        let file = root.join("docs/adr/README.md");

        assert_eq!(
            resolve_markdown_link_target(&root, &file, "../verification.md").unwrap(),
            root.join("docs/verification.md")
        );
    }

    #[test]
    fn link_target_resolver_rejects_parent_dirs_escaping_root() {
        let root = PathBuf::from("repo");
        let file = root.join("docs/adr/README.md");

        assert!(resolve_markdown_link_target(&root, &file, "../../../outside.md").is_err());
    }

    #[test]
    fn link_target_resolver_rejects_absolute_targets() {
        let root = PathBuf::from("repo");
        let file = root.join("README.md");

        assert!(resolve_markdown_link_target(&root, &file, "/tmp/outside.md").is_err());
    }
}
