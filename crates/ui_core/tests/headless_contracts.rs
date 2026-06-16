#[test]
fn ui_core_extraction_blockers_match_allowlist() {
    let expected = [
        ("adaptive.rs", "Pixels as Px"),
        ("a11y.rs", "pub use open_gpui::{AccessibleAction"),
        ("focus.rs", "pub use open_gpui::{FocusHandle"),
        ("overlay.rs", "Bounds<Px>"),
        ("overlay.rs", "Edges<Px>"),
        ("overlay.rs", "Pixels as Px"),
        ("overlay.rs", "Point<Px>"),
        ("overlay.rs", "Size<Px>"),
        ("prelude.rs", "focus::{FocusHandle"),
        ("sizing.rs", "Pixels as Px"),
    ];
    let mut expected = expected
        .into_iter()
        .map(|(file, token)| SourceBlocker::new(file.to_owned(), token.to_owned()))
        .collect::<Vec<_>>();
    expected.sort();

    let mut actual = source_blockers(&[
        "pub use open_gpui::{AccessibleAction",
        "pub use open_gpui::{FocusHandle",
        "Bounds<Px>",
        "Edges<Px>",
        "Pixels as Px",
        "Point<Px>",
        "Size<Px>",
        "focus::{FocusHandle",
    ]);
    actual.sort();

    assert_eq!(
        actual, expected,
        "ui_core public contracts gained or removed headless extraction blockers; update this inventory as neutral facades land"
    );
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct SourceBlocker {
    file: String,
    token: String,
}

impl SourceBlocker {
    fn new(file: String, token: String) -> Self {
        Self { file, token }
    }
}

fn source_blockers(tokens: &[&str]) -> Vec<SourceBlocker> {
    let mut source_files = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/src"))
        .expect("ui_core src directory should be readable")
        .map(|entry| {
            entry
                .expect("source directory entry should be readable")
                .path()
        })
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .collect::<Vec<_>>();
    source_files.sort();

    let mut blockers = Vec::new();
    for source_file in source_files {
        let source = std::fs::read_to_string(&source_file)
            .unwrap_or_else(|error| panic!("failed to read {source_file:?}: {error}"));
        let source = uncommented_lines(&source);
        let file_name = source_file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<unknown>");
        for token in tokens {
            if source.contains(token) {
                blockers.push(SourceBlocker::new(
                    file_name.to_owned(),
                    (*token).to_owned(),
                ));
            }
        }
    }

    blockers
}

fn uncommented_lines(source: &str) -> String {
    source
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//") && !trimmed.starts_with("///")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
