fn source_contains_public_use_token(file_name: &str, token: &str) -> bool {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(file_name);
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {path:?}: {error}"));
    let mut in_public_use = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub use ") {
            in_public_use = true;
        }
        if in_public_use && trimmed.contains(token) {
            return true;
        }
        if in_public_use && trimmed.ends_with(';') {
            in_public_use = false;
        }
    }
    false
}

#[test]
fn root_and_prelude_do_not_reexport_diagnostics() {
    let forbidden = [
        "DockTransitionExecutionState",
        "DockTransitionPlan",
        "DockViewportRuntimeStatus",
        "DockViewportPlatformCapabilityRecord",
        "DockViewportRouteStatus",
        "DockVisualAffordanceDebugLayer",
        "DockVisualAffordanceDebugSummary",
        "DockViewportTearOffCancelReason",
    ];

    for file_name in ["lib.rs", "prelude.rs"] {
        let leaked = forbidden
            .iter()
            .filter(|token| source_contains_public_use_token(file_name, token))
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(
            leaked,
            Vec::<&str>::new(),
            "{file_name} leaked advanced diagnostics through the common surface"
        );
    }
}

#[test]
fn advanced_exports_diagnostic_surface_explicitly() {
    for token in [
        "DockTransitionExecutionState",
        "DockTransitionPlan",
        "DockViewportRuntimeStatus",
        "DockViewportPlatformCapabilityRecord",
        "DockViewportRouteStatus",
        "DockVisualAffordanceDebugLayer",
        "DockVisualAffordanceDebugSummary",
        "DockViewportTearOffCancelReason",
    ] {
        assert!(
            source_contains_public_use_token("advanced.rs", token),
            "advanced.rs should explicitly export {token}"
        );
    }
}

#[test]
fn common_surfaces_do_not_use_wildcard_reexports() {
    for file_name in ["lib.rs", "prelude.rs", "advanced.rs"] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join(file_name);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {path:?}: {error}"));
        let wildcard_lines = source
            .lines()
            .enumerate()
            .filter_map(|(index, line)| {
                (line.contains("pub use ") && line.contains("::*"))
                    .then(|| format!("{file_name}:{}", index + 1))
            })
            .collect::<Vec<_>>();
        assert_eq!(
            wildcard_lines,
            Vec::<String>::new(),
            "public surfaces must use explicit re-exports"
        );
    }
}

#[test]
fn common_import_paths_compile() {
    use crate::{self as root, prelude};

    let root_policy = root::DockPolicy::new();
    let prelude_policy = prelude::DockPolicy::new();
    let root_layout = root::DockLayout::new(Vec::new(), Vec::new());
    let prelude_layout = prelude::DockLayout::new(Vec::new(), Vec::new());
    let root_placement = root::DockViewportPlacementLayout::new(Vec::new());
    let prelude_placement = prelude::DockViewportPlacementLayout::new(Vec::new());
    let root_panel_placement = root::DockPanelPlacement::center("editor");
    let prelude_panel_target = prelude::DockPanelPlacementTarget::right_rail();
    let root_descriptor = root::DockPanelDescriptor::new("Editor")
        .dirty(true)
        .with_close_veto_reason("unsaved changes")
        .with_default_placement(root::DockPanelPlacementTarget::center());
    let prelude_reopen_policy = prelude::DockPanelReopenPolicy::RestoreLastKnown;
    let prelude_open_source = prelude::DockPanelOpenPlacementSource::DescriptorDefault;

    let _ = (
        root_policy.allows_floating(),
        prelude_policy.allows_platform_viewports(),
        root_layout.layout_version,
        prelude_layout.layout_version,
        root_placement.placement_version,
        prelude_placement.placement_version,
        root_panel_placement.item(),
        prelude_panel_target,
        root_descriptor.default_placement(),
        root_descriptor.is_dirty(),
        root_descriptor.close_veto_reason(),
        prelude_reopen_policy,
        prelude_open_source,
    );
}

#[test]
fn advanced_import_paths_compile() {
    use crate::advanced;

    let state = advanced::DockTransitionExecutionState::Immediate;
    let status = advanced::DockViewportRuntimeStatus::default();

    let _ = (state, status);
}
