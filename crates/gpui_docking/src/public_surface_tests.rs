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
        if in_public_use && source_line_contains_identifier(trimmed, token) {
            return true;
        }
        if in_public_use && trimmed.ends_with(';') {
            in_public_use = false;
        }
    }
    false
}

fn source_line_contains_identifier(line: &str, token: &str) -> bool {
    line.split(|character: char| character != '_' && !character.is_ascii_alphanumeric())
        .any(|part| part == token)
}

#[test]
fn root_and_prelude_do_not_reexport_diagnostics() {
    let forbidden = [
        "DockTransitionExecutionState",
        "DockTransitionPlan",
        "DockViewportRuntimeStatus",
        "DockViewportPlatformCapabilityRecord",
        "DockViewportRouteStatus",
        "DockViewportRuntime",
        "DockViewportAdapter",
        "DockViewportDropRoute",
        "DockViewportDropRouteRequest",
        "DockViewportResolvedDropRoute",
        "DockDropDelivery",
        "DockResolvedDropTarget",
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
fn root_and_prelude_do_not_reexport_low_level_model_or_runtime_types() {
    let forbidden = [
        "DockAction",
        "DockActionApplyError",
        "DockActionOutcome",
        "DockCentralRegion",
        "DockEdgeDockPlan",
        "DockFloatingContainer",
        "DockGraph",
        "DockGraphMutationError",
        "DockGraphValidationError",
        "DockHost",
        "DockHostOptions",
        "DockLayoutBuilder",
        "DockLayoutNode",
        "DockNode",
        "DockNodeId",
        "DockSpatialDirection",
        "DockSplitResize",
        "DockViewportCloseOutcome",
        "DockViewportCloseStatus",
        "DockViewportFocusRequest",
        "DockViewportOpenOutcome",
        "DockViewportOpenStatus",
        "DockViewportRuntimeHandle",
        "DockViewportShouldCloseOutcome",
        "DockViewportShouldCloseStatus",
        "DockViewportUnregisterOutcome",
        "DockViewportUnregisterReason",
        "DockWorkspace",
        "DropZone",
        "EditorDockLayoutSpec",
        "SplitAxis",
        "dock_bounds",
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
            "{file_name} leaked low-level docking model/runtime types"
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
    let root_restore_readiness = root::DockViewportRestoreReadiness {
        matched: 0,
        missing: 0,
    };
    let prelude_restore_readiness = prelude::DockViewportRestoreReadiness {
        matched: 0,
        missing: 0,
    };
    let root_placement_validation_error =
        root::DockViewportPlacementValidationError::UnsupportedVersion {
            expected: 1,
            found: 0,
        };
    let prelude_placement_validation_error =
        prelude::DockViewportPlacementValidationError::DuplicateSpace {
            space: "main".into(),
        };
    let root_panel_placement = root::DockPanelPlacement::center("editor");
    let root_surface_builder = root::DockSurface::builder("main");
    let root_surface_change = root::DockSurfaceChange::Changed;
    let root_close_policy = root::DockViewportClosePolicy::RetainLayout;
    let prelude_panel_target = prelude::DockPanelPlacementTarget::right_rail();
    let prelude_surface_builder = prelude::DockSurface::builder("main");
    let prelude_surface_change = prelude::DockSurfaceChange::Unchanged;
    let prelude_close_policy = prelude::DockViewportClosePolicy::Prevent;
    let root_descriptor = root::DockPanelDescriptor::new("Editor")
        .dirty(true)
        .with_close_veto_reason("unsaved changes")
        .with_default_placement(root::DockPanelPlacementTarget::center());
    let prelude_reopen_policy = prelude::DockPanelReopenPolicy::RestoreLastKnown;
    let prelude_open_source = prelude::DockPanelOpenPlacementSource::DescriptorDefault;
    let prelude_open_status = prelude::DockSurfaceViewportOpenStatus::Opened;
    let root_should_close_status = root::DockSurfaceViewportShouldCloseStatus::Allowed;
    let prelude_close_status = prelude::DockSurfaceViewportCloseStatus::MergedBack;
    let root_viewport_spec =
        root::DockSurfaceViewportSpec::new("main", open_gpui::WindowOptions::default());
    let prelude_viewport_spec_error = prelude::DockSurfaceViewportSpecError::InvalidPlacement {
        message: String::new(),
    };
    let root_viewport_report_into_outcomes = root::DockSurfaceViewportOpenReport::into_outcomes;
    let root_close_merge_target = root::DockSurfaceViewportCloseOutcome::merge_target_space;
    let prelude_should_close_allows = prelude::DockSurfaceViewportShouldCloseOutcome::allows_close;

    let _ = (
        root_policy.allows_floating(),
        prelude_policy.allows_platform_viewports(),
        root_layout.layout_version,
        prelude_layout.layout_version,
        root_placement.placement_version,
        prelude_placement.placement_version,
        root_restore_readiness.matched,
        prelude_restore_readiness.missing,
        root_placement_validation_error,
        prelude_placement_validation_error,
        root_panel_placement.item(),
        root_surface_builder,
        root_surface_change.changed(),
        root_close_policy,
        prelude_panel_target,
        prelude_surface_builder,
        prelude_surface_change.changed(),
        prelude_close_policy,
        root_descriptor.default_placement(),
        root_descriptor.is_dirty(),
        root_descriptor.close_veto_reason(),
        prelude_reopen_policy,
        prelude_open_source,
        prelude_open_status,
        root_should_close_status,
        prelude_close_status,
        root_viewport_spec.space(),
        prelude_viewport_spec_error,
        root_viewport_report_into_outcomes,
        root_close_merge_target,
        prelude_should_close_allows,
    );
}

#[test]
fn advanced_import_paths_compile() {
    use crate::advanced;

    let state = advanced::DockTransitionExecutionState::Immediate;
    let status = advanced::DockViewportRuntimeStatus::default();

    let _ = (state, status);
}

#[test]
fn explicit_low_level_import_paths_compile() {
    use crate::{model, runtime};

    let graph = model::DockGraph::new();
    let layout = model::DockLayoutBuilder::new().build();
    let action = model::DockActionOutcome::Unchanged;
    let close_policy = runtime::DockViewportClosePolicy::Prevent;

    let _ = (graph, layout, action, close_policy);
}
