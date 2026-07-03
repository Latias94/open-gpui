use super::*;

#[test]
fn gpui_role_mapping_covers_neutral_image_and_separator_fallback() {
    assert_eq!(gpui_role_from_ui(Role::Image), open_gpui::Role::Image);
    assert_eq!(gpui_role_from_ui(Role::Link), open_gpui::Role::Link);
    assert_eq!(gpui_role_from_ui(Role::Separator), open_gpui::Role::Group);
    assert_eq!(gpui_role_from_ui(Role::Slider), open_gpui::Role::Slider);
    assert_eq!(gpui_role_from_ui(Role::Tree), open_gpui::Role::Tree);
    assert_eq!(gpui_role_from_ui(Role::TreeItem), open_gpui::Role::TreeItem);
    assert_eq!(gpui_role_from_ui(Role::Table), open_gpui::Role::Table);
    assert_eq!(gpui_role_from_ui(Role::Row), open_gpui::Role::Row);
    assert_eq!(
        gpui_role_from_ui(Role::ColumnHeader),
        open_gpui::Role::ColumnHeader
    );
    assert_eq!(gpui_role_from_ui(Role::Cell), open_gpui::Role::Cell);
}

#[test]
fn public_resolved_state_contracts_avoid_gpui_runtime_types() {
    const FORBIDDEN: &[&str] = &[
        "Window",
        "App",
        "Context<",
        "RenderOnce",
        "IntoElement",
        "ElementId",
        "Entity<",
        "FocusHandle",
        "ScrollHandle",
        "Rc<dyn",
    ];
    let mut checked = 0;
    for source_file in ui_component_source_files() {
        let source = std::fs::read_to_string(&source_file)
            .unwrap_or_else(|error| panic!("failed to read {source_file:?}: {error}"));
        let file_name = source_file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<unknown>");
        for state in public_contract_structs(&source, &["State"]) {
            checked += 1;
            let fields = uncommented_lines(state.fields);
            for forbidden in FORBIDDEN {
                assert!(
                    !fields.contains(forbidden),
                    "{file_name}::{} leaks forbidden runtime/render type `{forbidden}`",
                    state.name
                );
            }
        }
    }

    assert!(
        checked >= 40,
        "expected to scan all public resolved-state structs, scanned {checked}"
    );
}

#[test]
fn public_contract_extraction_blockers_match_allowlist() {
    const BLOCKER_TOKENS: &[&str] = &["GpuiOverlayState", "open_gpui::Pixels", "Point<Pixels>"];
    let expected: [(&str, &str, &str); 0] = [];
    let mut expected = expected
        .into_iter()
        .map(|(file, contract, token)| {
            PublicContractBlocker::new(file.to_owned(), contract.to_owned(), token.to_owned())
        })
        .collect::<Vec<_>>();
    expected.sort();

    let mut actual = public_contract_extraction_blockers(BLOCKER_TOKENS);
    actual.sort();

    assert_eq!(
        actual, expected,
        "public component contracts gained or removed extraction blockers; update this inventory as U2-U6 migrate them"
    );
}

#[test]
fn adapter_only_public_surfaces_match_allowlist() {
    let expected = [
        ("focus.rs", "BoxShadow"),
        ("gpui_adapter.rs", "GpuiCommandAction"),
        ("gpui_adapter.rs", "GpuiCommandActionMap"),
        ("gpui_adapter.rs", "command_shortcut_label"),
        ("gpui_adapter.rs", "command_shortcut_label_from_keymap"),
        ("focus.rs", "focus_ring_shadow"),
        ("focus.rs", "focus_ring_shadow_with_theme"),
        ("overlay.rs", "GpuiOverlayState"),
        ("scroll_area.rs", "ScrollHandle"),
        ("text_input.rs", "Entity<TextInputController>"),
        ("text_input.rs", "EntityInputHandler"),
        ("text_input.rs", "TextInputController"),
        ("textarea.rs", "EntityInputHandler"),
    ];
    let mut expected = expected
        .into_iter()
        .map(|(file, token)| PublicSurfaceBlocker::new(file.to_owned(), token.to_owned()))
        .collect::<Vec<_>>();
    expected.sort();

    let mut actual = public_surface_blockers(&[
        "BoxShadow",
        "Entity<TextInputController>",
        "EntityInputHandler",
        "GpuiCommandAction",
        "GpuiCommandActionMap",
        "GpuiOverlayState",
        "ScrollHandle",
        "TextInputController",
        "command_shortcut_label",
        "command_shortcut_label_from_keymap",
        "focus_ring_shadow",
        "focus_ring_shadow_with_theme",
    ]);
    actual.sort();

    assert_eq!(
        actual, expected,
        "adapter-only public surfaces changed; update this inventory as U6 classifies or narrows GPUI-specific APIs"
    );
}

#[test]
fn production_render_paths_do_not_use_default_light_focus_ring_helper() {
    let mut offenders = Vec::new();

    for source_file in ui_component_source_files() {
        let file_name = source_file
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("<unknown>");
        if file_name == "focus.rs" {
            continue;
        }

        let source = std::fs::read_to_string(&source_file)
            .unwrap_or_else(|error| panic!("failed to read {source_file:?}: {error}"));
        let source = uncommented_lines(&source);

        for token in ["focus_ring_shadow(", "ThemeContext::light()"] {
            if source.contains(token) {
                offenders.push(format!("{file_name}: {token}"));
            }
        }
    }

    let gallery_shell = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/ui-foundation-gallery/src/shell.rs");
    let source = std::fs::read_to_string(&gallery_shell)
        .unwrap_or_else(|error| panic!("failed to read {gallery_shell:?}: {error}"));
    let source = uncommented_lines(&source);
    for token in ["focus_ring_shadow(", "ThemeContext::light()"] {
        if source.contains(token) {
            offenders.push(format!(
                "examples/ui-foundation-gallery/src/shell.rs: {token}"
            ));
        }
    }

    assert!(
        offenders.is_empty(),
        "production render paths must resolve focus rings from ThemeContext; offenders: {offenders:?}"
    );
}

#[test]
fn gpui_adapter_exports_group_runtime_specific_surfaces() {
    use open_gpui_ui_components::{self as root, prelude};

    let module_text_input = root::text_input::TextInput::new("module-text-input", "Module input");
    let _module_state: root::text_input::TextInputState = module_text_input.state();
    let _module_colors: Option<root::text_input::TextInputColors> = None;
    let _module_metrics: Option<root::text_input::TextInputMetrics> = None;
    let _module_display_mode: root::text_input::TextInputDisplayMode =
        root::text_input::TextInputDisplayMode::Plain;
    let module_textarea = root::textarea::Textarea::new("module-textarea", "Module textarea");
    let _module_textarea_state: root::textarea::TextareaState = module_textarea.state();
    let _module_textarea_colors: Option<root::textarea::TextareaColors> = None;
    let _module_textarea_metrics: Option<root::textarea::TextareaMetrics> = None;

    let root_overlay = root::gpui_adapter::GpuiOverlayAdapterConfig::new(
        OverlayLayerKind::Tooltip,
        OverlayPresence::open(),
    )
    .state();

    let _root_init: fn(&mut open_gpui::App) = root::gpui_adapter::init_text_input;
    let _root_controller: Option<root::gpui_adapter::TextInputController> = None;
    let _root_px: fn(UiPx) -> open_gpui::Pixels = root::gpui_adapter::gpui_px_from_ui;
    let _root_point: fn(UiPoint) -> open_gpui::Point<open_gpui::Pixels> =
        root::gpui_adapter::gpui_point_from_ui;
    let _root_size: fn(UiSize) -> open_gpui::Size<open_gpui::Pixels> =
        root::gpui_adapter::gpui_size_from_ui;
    let _prelude_button: prelude::Button = prelude::Button::new("save", "Save");
    let _prelude_textarea: prelude::Textarea = prelude::Textarea::new("notes", "Notes");
    let _prelude_display_mode: prelude::TextInputDisplayMode = prelude::TextInputDisplayMode::Plain;

    assert_eq!(
        root_overlay.deferred_priority(),
        root::gpui_adapter::default_deferred_priority(OverlayLayerKind::Tooltip)
    );
    assert_eq!(
        root_overlay.snap_margin(),
        root::gpui_adapter::DEFAULT_OVERLAY_SAFE_MARGIN
    );
    assert_eq!(
        root::gpui_adapter::focus_ring_shadow(FocusRing::from_color(ColorIntent::new(
            semantic::FOCUS_RING,
            0x2f80ed,
        )))[0]
            .spread_radius,
        px(2.0)
    );
    assert_eq!(
        root::gpui_adapter::focus_ring_shadow_with_theme(
            FocusRing::from_color(ColorIntent::new(semantic::FOCUS_RING, 0x2f80ed)),
            &root::ThemeContext::light(),
        )[0]
        .spread_radius,
        px(2.0)
    );
}

#[test]
fn adapter_only_helpers_do_not_leak_from_default_exports() {
    let adapter_only_tokens = PUBLIC_SURFACE_OWNER_MAP
        .iter()
        .filter(|entry| entry.owner == PublicSurfaceOwnerClass::GpuiAdapterHelper)
        .filter(|entry| !entry.name.contains("::"))
        .map(|entry| entry.name)
        .collect::<Vec<_>>();

    for file_name in ["lib.rs", "prelude.rs"] {
        let source =
            std::fs::read_to_string(format!("{}/src/{file_name}", env!("CARGO_MANIFEST_DIR")))
                .unwrap_or_else(|error| panic!("failed to read {file_name}: {error}"));
        let default_interface = if file_name == "lib.rs" {
            source_without_gpui_adapter_module(&source)
        } else {
            source
        };

        for token in &adapter_only_tokens {
            assert!(
                !default_interface.contains(*token),
                "{file_name} default interface must not expose adapter-only token `{token}`"
            );
        }
    }

    let text_input_source =
        std::fs::read_to_string(format!("{}/src/text_input.rs", env!("CARGO_MANIFEST_DIR")))
            .unwrap_or_else(|error| panic!("failed to read text_input.rs: {error}"));
    assert!(text_input_source.contains("pub(crate) mod adapter"));
    assert!(
        !text_input_source.contains("pub use adapter"),
        "text_input must not re-export its internal adapter module"
    );
}
