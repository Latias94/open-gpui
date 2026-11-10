use open_gpui_path::{
    PathExt, PathStyle,
    abs_path::AbsPath,
    rel_path::{RelPath, RelPathBuf},
};
use std::{ffi::OsStr, path::Path};

#[test]
fn remote_workspace_paths_normalize_and_rejoin_on_either_host() -> anyhow::Result<()> {
    for (style, workspace, incoming, expected) in [
        (
            PathStyle::Unix,
            "/srv/workspace",
            "/../../srv//workspace/src/../assets/./logo.svg",
            "/srv/workspace/assets/logo.svg",
        ),
        (
            PathStyle::Windows,
            r"C:\workspace",
            r"C:\..\workspace\src/../assets\.\logo.svg",
            r"C:\workspace\assets\logo.svg",
        ),
        (
            PathStyle::Windows,
            "//build-server/projects/workspace",
            "//build-server/projects/../workspace/src/../assets/logo.svg",
            r"//build-server/projects\workspace\assets\logo.svg",
        ),
        (
            PathStyle::Windows,
            r"\\?\C:\workspace",
            r"\\?\C:\..\workspace\src\..\assets\.\logo.svg",
            r"\\?\C:\workspace\assets\logo.svg",
        ),
    ] {
        let normalized = style.normalize_lexically(incoming);
        assert_eq!(normalized.as_os_str(), OsStr::new(expected));
        assert!(style.is_absolute(&normalized));

        let suffix = style
            .strip_prefix(&normalized, workspace)
            .expect("normalized asset stays within its remote workspace");
        let relative = RelPath::new(suffix, style)?;
        assert_eq!(relative.as_unix_str(), "assets/logo.svg");

        let rejoined =
            style.normalize_separators(style.join(workspace, relative.display(style).as_ref()));
        assert_eq!(rejoined.as_os_str(), normalized.as_os_str());
    }
    Ok(())
}

#[test]
fn checked_relative_paths_persist_and_resolve_against_a_local_base() -> anyhow::Result<()> {
    let directory = RelPath::from_unix_str("resources")?;
    let imported = RelPath::new(Path::new(r".\icons\..\assets\logo.svg"), PathStyle::Windows)?;
    let relative = directory.join(imported.as_ref());
    let encoded = serde_json::to_string(&relative)?;
    assert_eq!(encoded, r#""resources/assets/logo.svg""#);

    let restored: RelPathBuf = serde_json::from_str(&encoded)?;
    assert_eq!(restored, relative);
    assert_eq!(serde_json::to_string(restored.as_rel_path())?, encoded);

    let local_input = if cfg!(windows) {
        r".\resources\icons\..\assets\logo.svg"
    } else {
        "./resources/icons/../assets/logo.svg"
    };
    let normalized: RelPathBuf = serde_json::from_str(&serde_json::to_string(local_input)?)?;
    assert_eq!(normalized, restored);

    let root = if cfg!(windows) {
        Path::new(r"C:\open-gpui-consumer")
    } else {
        Path::new("/open-gpui-consumer")
    };
    let base = AbsPath::new(root)?;
    let absolute = restored.absolutize(base);
    assert_eq!(absolute, base.join_rel_path(&restored));
    assert_eq!(
        absolute.as_std_path(),
        root.join("resources/assets/logo.svg")
    );
    assert_eq!(absolute.file_name(), Some("logo.svg"));
    assert!(absolute.is_descendant_of(base));
    assert_eq!(
        absolute
            .strip_prefix(base)
            .expect("resolved asset stays within its local base")
            .as_ref(),
        restored.as_rel_path()
    );
    assert_eq!(restored.as_std_path().to_rel_path_buf()?, restored);
    Ok(())
}

#[test]
fn persisted_paths_and_component_edits_preserve_relative_boundaries() -> anyhow::Result<()> {
    for untrusted in [
        "../outside.json",
        "assets/../../outside.json",
        "/outside.json",
    ] {
        let encoded = serde_json::to_string(untrusted)?;
        assert!(serde_json::from_str::<RelPathBuf>(&encoded).is_err());
    }

    let mut path = RelPathBuf::try_from("assets")?;
    path.push_component("logo.svg")?;
    let encoded = serde_json::to_string(&path)?;
    assert!(path.push_component("../outside.json").is_err());
    assert_eq!(serde_json::to_string(&path)?, encoded);
    assert!(RelPath::from_unix_str("assets/../logo.svg").is_err());
    assert_eq!(path.as_unix_str(), "assets/logo.svg");
    Ok(())
}

#[test]
fn canonical_paths_compose_with_utility_filtering_ordering_and_positions() -> anyhow::Result<()> {
    use open_gpui_util::paths::{
        PathMatcher, PathWithPosition, SortMode, SortOrder, compare_rel_paths_by,
    };

    let matcher = PathMatcher::new(["**/*.rs"], PathStyle::Unix)?;
    let mut entries = [
        RelPathBuf::try_from("src/zeta.rs")?,
        RelPathBuf::try_from("src/alpha.txt")?,
        RelPathBuf::try_from("src/alpha.rs")?,
    ];
    entries.sort_by(|left, right| {
        compare_rel_paths_by(
            (left.as_rel_path(), true),
            (right.as_rel_path(), true),
            SortMode::Mixed,
            SortOrder::Extension,
        )
    });
    assert_eq!(
        entries.each_ref().map(|path| path.as_unix_str()),
        ["src/alpha.rs", "src/zeta.rs", "src/alpha.txt"]
    );
    let selected = entries
        .iter()
        .find(|path| matcher.is_match(path.as_rel_path()))
        .expect("a Rust source is available for navigation");
    let navigation = PathWithPosition::parse_str("src/./alpha.rs:12:4").map_path(|path| {
        path.to_rel_path_buf()
            .map(|relative| relative.as_std_path().to_path_buf())
    })?;
    assert_eq!(navigation.path, selected.as_std_path());
    assert_eq!((navigation.row, navigation.column), (Some(12), Some(4)));
    Ok(())
}

#[test]
#[cfg(any(windows, all(unix, not(target_family = "wasm"))))]
fn native_non_unicode_paths_round_trip_through_the_public_byte_api() -> anyhow::Result<()> {
    use open_gpui_path::try_from_bytes;
    use std::{ffi::OsString, path::PathBuf};

    #[cfg(windows)]
    let source = {
        use std::os::windows::ffi::OsStringExt;
        let units: Vec<_> = "assets/"
            .encode_utf16()
            .chain([0xd800])
            .chain(".bin".encode_utf16())
            .collect();
        OsString::from_wide(&units)
    };
    #[cfg(all(unix, not(target_family = "wasm")))]
    let source = {
        use std::os::unix::ffi::OsStringExt;
        OsString::from_vec(b"assets/\xff.bin".to_vec())
    };

    let bytes = source.as_encoded_bytes();
    let owned: PathBuf = try_from_bytes(bytes)?;
    let borrowed: &Path = try_from_bytes(bytes)?;
    assert_eq!(owned.as_os_str().as_encoded_bytes(), bytes);
    assert_eq!(borrowed.as_os_str(), source.as_os_str());
    assert!(owned.to_rel_path_buf().is_err());

    #[cfg(windows)]
    assert!(try_from_bytes::<PathBuf>(b"\xff").is_err());
    Ok(())
}
