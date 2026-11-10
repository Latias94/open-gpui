use super::*;
use std::{ffi::OsString, os::unix::ffi::OsStringExt};

#[test]
fn builds_terminated_uri_list_for_each_dragged_path() {
    let paths = FileDragPaths::new([
        (PathBuf::from("/tmp/first"), false),
        (PathBuf::from("/tmp/nested directory"), true),
    ]);

    assert_eq!(
        file_uri_list(&paths),
        "file:///tmp/first\r\nfile:///tmp/nested%20directory\r\n"
    );
}

#[test]
fn skips_paths_that_have_no_file_url() {
    let paths = FileDragPaths::new([
        (PathBuf::from("relative/path"), false),
        (PathBuf::from("/tmp/absolute"), false),
    ]);

    assert_eq!(file_uri_list(&paths), "file:///tmp/absolute\r\n");
}

#[test]
fn builds_empty_uri_list_without_convertible_paths() {
    assert!(file_uri_list(&FileDragPaths::default()).is_empty());
    assert!(file_uri_list(&FileDragPaths::new([(PathBuf::from("relative"), false)])).is_empty());
}

#[test]
fn escapes_uri_delimiters_and_line_endings_in_file_names() {
    let paths = FileDragPaths::new([(
        PathBuf::from("/tmp/a#b?c%20\r\nfile:///tmp/injected"),
        false,
    )]);

    assert_eq!(
        file_uri_list(&paths),
        "file:///tmp/a%23b%3Fc%2520%0D%0Afile:/tmp/injected\r\n"
    );
}

#[test]
fn preserves_non_utf8_file_name_bytes() {
    let paths = FileDragPaths::new([(
        PathBuf::from(OsString::from_vec(b"/tmp/non-utf8-\xff".to_vec())),
        false,
    )]);

    assert_eq!(file_uri_list(&paths), "file:///tmp/non-utf8-%FF\r\n");
}
