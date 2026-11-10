use open_gpui_util::{merge_json_value_into, union_json_value_into};
use serde_json::json;

const DEFAULT_SETTINGS: &str = include_str!("../assets/utility/settings/defaults.json");

#[test]
fn workspace_settings_replace_arrays_while_extensions_union_contributions() -> anyhow::Result<()> {
    let mut settings = serde_json::from_str(DEFAULT_SETTINGS)?;
    merge_json_value_into(
        json!({
            "editor": {"tab_size": 2, "rulers": [120]},
            "languages": ["Rust"],
            "theme": "Workspace"
        }),
        &mut settings,
    );
    assert_eq!(
        settings,
        json!({
            "editor": {"tab_size": 2, "format_on_save": true, "rulers": [120]},
            "languages": ["Rust"],
            "theme": "Workspace"
        })
    );

    let mut contributions = json!({
        "languages": ["Rust"],
        "commands": [{"id": "format", "title": "Format"}],
        "provider": {"enabled": false, "scope": "workspace"}
    });
    let extension = json!({
        "languages": ["Rust", "Markdown", "Markdown"],
        "commands": [
            {"id": "format", "title": "Format"},
            {"id": "lint", "title": "Lint"}
        ],
        "provider": {"enabled": true}
    });
    union_json_value_into(extension.clone(), &mut contributions);
    let expected = json!({
        "languages": ["Rust", "Markdown"],
        "commands": [
            {"id": "format", "title": "Format"},
            {"id": "lint", "title": "Lint"}
        ],
        "provider": {"enabled": true, "scope": "workspace"}
    });
    assert_eq!(contributions, expected);
    union_json_value_into(extension, &mut contributions);
    assert_eq!(contributions, expected);
    Ok(())
}

#[test]
fn markdown_source_links_navigate_through_canonical_relative_paths() -> anyhow::Result<()> {
    use open_gpui_util::{
        PathExt as _,
        markdown::{source_position_from_fragment, split_local_url_fragment},
        paths::PathWithPosition,
        rel_path::RelPathBuf,
    };

    for destination in ["src/./main.rs#L12C4-L18C8", "src/main.rs#12,4"] {
        let (path, fragment) = split_local_url_fragment(destination);
        let (row, column) = source_position_from_fragment(fragment.unwrap())
            .expect("source links carry a zero-based editor position");
        assert_eq!((row, column), (11, 3));

        let relative: RelPathBuf = std::path::Path::new(path).to_rel_path_buf()?;
        let canonical: &open_gpui_path::rel_path::RelPath = relative.as_rel_path();
        assert_eq!(canonical.as_unix_str(), "src/main.rs");
        let navigation = PathWithPosition::parse_str(&format!(
            "{}:{}:{}",
            canonical.as_unix_str(),
            row + 1,
            column + 1
        ));
        assert_eq!(navigation.path, relative.as_std_path());
        assert_eq!((navigation.row, navigation.column), (Some(12), Some(4)));
    }

    let (path, fragment) = split_local_url_fragment("docs/setup.md#installation");
    assert_eq!(path, "docs/setup.md");
    assert_eq!(source_position_from_fragment(fragment.unwrap()), None);
    let remote = "https://example.com/src/main.rs#L12C4";
    assert_eq!(split_local_url_fragment(remote), (remote, None));
    Ok(())
}

#[test]
fn environment_overrides_accept_native_names_and_redact_diagnostics() {
    use open_gpui_util::redact::{is_valid_environment_name, redact_command};

    let accepted: Vec<_> = [
        ("WORKSPACE_TOKEN", "example-token"),
        ("build.profile", "release"),
        ("项目", "demo"),
        ("", "empty-name"),
        ("INVALID=NAME", "embedded-equals"),
        ("BROKEN\nNAME", "line-break"),
        ("NUL\0NAME", "nul-byte"),
    ]
    .into_iter()
    .filter(|(name, _)| is_valid_environment_name(name))
    .collect();
    assert_eq!(
        accepted,
        [
            ("WORKSPACE_TOKEN", "example-token"),
            ("build.profile", "release"),
            ("项目", "demo")
        ]
    );

    let diagnostic = accepted
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(
        redact_command(&diagnostic),
        "WORKSPACE_TOKEN=\"[REDACTED]\" build.profile=release 项目=demo"
    );
}

#[cfg(not(target_family = "wasm"))]
mod filesystem_assets {
    use super::DEFAULT_SETTINGS;
    use anyhow::Context as _;
    use open_gpui_util::{asset_str, dev_repo_root};
    use std::{env, fs, path::PathBuf, process::Command};

    const CHILD_CHECKOUT: &str = "OPEN_GPUI_UTILITY_ASSET_CHECKOUT";
    const CHILD_TEST: &str =
        "filesystem_assets::checkout_assets_support_live_development_and_embedded_delivery";
    const CHILD_VERIFIED: &str = "utility asset checkout verified";
    const ASSET_ROOT: &str = "tests/fixtures/dependency-consumer/assets/utility";
    const NAVIGATION: &str = include_str!("../assets/utility/docs/navigation.md");
    const LIVE_SETTINGS: &str = r#"{"theme":"Live","editor":{"tab_size":8}}"#;
    const LIVE_DOCUMENT: &str = "# Live workspace guide\n";

    open_gpui_util::fs_embed! {
        pub struct WorkspaceAssets,
        crate_relative = "assets/utility",
        root_relative = "tests/fixtures/dependency-consumer/assets/utility",
        include = ["settings/*.json", "docs/**/*.md"],
        exclude = ["**/*.private.*"],
    }

    #[test]
    fn checkout_assets_support_live_development_and_embedded_delivery() -> anyhow::Result<()> {
        if let Some(checkout) = env::var_os(CHILD_CHECKOUT) {
            let checkout = PathBuf::from(checkout);
            assert!(checkout.join(".git").is_file());
            assert_eq!(
                dev_repo_root()
                    .context("the launched executable belongs to a checkout")?
                    .canonicalize()?,
                checkout.canonicalize()?
            );
            #[cfg(unix)]
            open_gpui_util::increase_open_file_limit()?;

            let assets = checkout.join(ASSET_ROOT);
            assert!(!assets.exists());
            let embedded = cfg!(any(feature = "debug-embed", not(debug_assertions)));
            if embedded {
                assert_bundle(DEFAULT_SETTINGS, ("docs/navigation.md", NAVIGATION));
            } else {
                assert!(WorkspaceAssets::get("settings/defaults.json").is_none());
                assert!(WorkspaceAssets::iter().next().is_none());
            }

            for (path, contents) in [
                ("settings/defaults.json", DEFAULT_SETTINGS),
                ("docs/navigation.md", NAVIGATION),
                (
                    "settings/local.private.json",
                    include_str!("../assets/utility/settings/local.private.json"),
                ),
                ("notes.txt", include_str!("../assets/utility/notes.txt")),
            ] {
                let path = assets.join(path);
                fs::create_dir_all(path.parent().unwrap())?;
                fs::write(path, contents)?;
            }
            assert_bundle(DEFAULT_SETTINGS, ("docs/navigation.md", NAVIGATION));

            fs::write(assets.join("settings/defaults.json"), LIVE_SETTINGS)?;
            fs::remove_file(assets.join("docs/navigation.md"))?;
            fs::write(assets.join("docs/live.md"), LIVE_DOCUMENT)?;
            fs::write(
                assets.join("settings/live.private.json"),
                "private live edit",
            )?;
            if embedded {
                assert_bundle(DEFAULT_SETTINGS, ("docs/navigation.md", NAVIGATION));
                assert!(WorkspaceAssets::get("docs/live.md").is_none());
            } else {
                assert_bundle(LIVE_SETTINGS, ("docs/live.md", LIVE_DOCUMENT));
                assert!(WorkspaceAssets::get("docs/navigation.md").is_none());
            }
            let settings: serde_json::Value =
                serde_json::from_str(&asset_str::<WorkspaceAssets>("settings/defaults.json"))?;
            assert_eq!(settings["theme"], if embedded { "Default" } else { "Live" });
            println!("{CHILD_VERIFIED}");
            return Ok(());
        }

        let temporary = tempfile::Builder::new()
            .prefix("open-gpui-utility-")
            .tempdir()?;
        let checkout = temporary.path().join("launch-checkout");
        let working_checkout = temporary.path().join("working-checkout");
        for root in [&checkout, &working_checkout] {
            fs::create_dir_all(root)?;
            fs::write(root.join(".git"), "gitdir: ../worktree-metadata\n")?;
        }
        let binary_directory = checkout.join("target/debug/deps");
        fs::create_dir_all(&binary_directory)?;
        let executable = env::current_exe()?;
        let child = binary_directory.join(executable.file_name().context("test executable name")?);
        fs::copy(executable, &child)?;
        let output = Command::new(child)
            .args(["--exact", CHILD_TEST, "--nocapture"])
            .env(CHILD_CHECKOUT, &checkout)
            .current_dir(working_checkout)
            .output()
            .context("launch the consumer from a relocated checkout")?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains(CHILD_VERIFIED),
            "relocated consumer failed: {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            stdout,
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    }

    fn assert_bundle(settings: &str, document: (&str, &str)) {
        let compressed = <WorkspaceAssets as open_gpui_util::__rust_embed::RustEmbed>::compressed(
            "settings/defaults.json",
        );
        assert_eq!(
            compressed.is_some(),
            cfg!(any(feature = "debug-embed", not(debug_assertions)))
        );
        let mut names: Vec<_> = WorkspaceAssets::iter()
            .map(|name| name.into_owned())
            .collect();
        names.sort_unstable();
        let mut trait_names: Vec<_> =
            <WorkspaceAssets as open_gpui_util::__rust_embed::RustEmbed>::iter()
                .map(|name| name.into_owned())
                .collect();
        trait_names.sort_unstable();
        assert_eq!(trait_names, names);
        let mut expected = ["settings/defaults.json", document.0];
        expected.sort_unstable();
        assert_eq!(names, expected);

        for (path, contents) in [("settings/defaults.json", settings), document] {
            let asset = WorkspaceAssets::get(path).expect(path);
            assert_eq!(asset.data.as_ref(), contents.as_bytes(), "{path}");
            assert_eq!(
                asset_str::<WorkspaceAssets>(path).as_ref(),
                contents,
                "{path}"
            );
        }
        for path in [
            "settings/local.private.json",
            "settings/live.private.json",
            "notes.txt",
            "settings/missing.json",
        ] {
            assert!(WorkspaceAssets::get(path).is_none(), "{path}");
        }
    }
}
