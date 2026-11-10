use open_gpui_path::{PathExt as _, PathStyle, rel_path::RelPath};
use open_gpui_util::paths::{PathExt as _, PathMatcher, compare_rel_paths, normalize_path};
use std::{
    cmp::Ordering,
    path::{Path, PathBuf},
};

#[test]
fn portable_paths_compose_with_util_matching_and_sorting() {
    let first = RelPath::from_unix_str("src/file2.rs").unwrap();
    let second = RelPath::from_unix_str("src/file10.rs").unwrap();
    let other = RelPath::from_unix_str("docs/guide.md").unwrap();

    for style in [PathStyle::Unix, PathStyle::Windows] {
        let matcher = PathMatcher::new(["src"], style).unwrap();
        assert!(matcher.is_match(first));
        assert!(matcher.is_match(second));
        assert!(!matcher.is_match(other));
    }

    assert_eq!(
        compare_rel_paths((first, true), (second, true)),
        Ordering::Less
    );
}

#[test]
fn util_path_bytes_and_normalization_compose_with_portable_paths() {
    let decoded = PathBuf::try_from_bytes(b"src/../docs/./guide.md").unwrap();
    let normalized = normalize_path(&decoded);
    let portable = normalized.to_rel_path_buf().unwrap();

    assert_eq!(normalized, Path::new("docs").join("guide.md"));
    assert_eq!(
        portable.as_rel_path(),
        RelPath::from_unix_str("docs/guide.md").unwrap()
    );
}
