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

fn source_contains_public_signature_token(file_name: &str, token: &str) -> bool {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join(file_name);
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {path:?}: {error}"));
    let mut in_public_signature = false;
    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("pub fn ") {
            in_public_signature = true;
        }
        if in_public_signature && source_line_contains_identifier(trimmed, token) {
            return true;
        }
        if in_public_signature && (trimmed.ends_with('{') || trimmed.ends_with(';')) {
            in_public_signature = false;
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
        "DockController",
        "DockControllerBuilder",
        "DockEdgeDockPlan",
        "DockFloatingContainer",
        "DockGraph",
        "DockGraphMutationError",
        "DockGraphValidationError",
        "DockHost",
        "DockHostOptions",
        "DockLayoutBuilder",
        "DockLayoutCentralRegion",
        "DockLayoutFloatingContainer",
        "DockLayoutNode",
        "DockLayoutSpace",
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
fn surface_facade_methods_do_not_expose_low_level_model_or_runtime_types() {
    let forbidden = [
        "DockAction",
        "DockActionApplyError",
        "DockActionOutcome",
        "DockController",
        "DockControllerBuilder",
        "DockHost",
        "DockHostOptions",
        "DockNodeId",
        "DockViewportRuntimeHandle",
        "DockWorkspace",
    ];

    for file_name in [
        "surface.rs",
        "surface/builder.rs",
        "surface/panel.rs",
        "surface/viewport.rs",
    ] {
        let leaked = forbidden
            .iter()
            .filter(|token| source_contains_public_signature_token(file_name, token))
            .copied()
            .collect::<Vec<_>>();
        assert_eq!(
            leaked,
            Vec::<&str>::new(),
            "{file_name} leaked low-level docking types through facade method signatures"
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
    let root_layout = root::DockLayout::empty();
    let prelude_layout = prelude::DockLayout::empty();
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
    let root_visual_style = root::DockVisualStyle::built_in();
    let root_visual_style_resolver =
        root::DockVisualStyleResolver::fixed(root_visual_style.clone());
    let prelude_visual_style =
        prelude::DockVisualStyle::from_palette(prelude::DockVisualPalette::built_in());
    let read_only_visual_style_resolver =
        root::DockVisualStyleResolver::new(|_: &open_gpui::Window, _: &open_gpui::App| {
            root::DockVisualStyle::built_in()
        });
    let root_surface_builder = root::DockSurface::builder("main")
        .empty_message("No panels")
        .missing_panel_prefix("Missing")
        .split_min_size(open_gpui::px(80.0))
        .splitter_handle_size(open_gpui::px(8.0))
        .drop_guide_metrics(root::DockDropGuideMetrics::default())
        .visual_style_resolver(root_visual_style_resolver.clone())
        .motion_preference(open_gpui_motion::MotionPreference::Reduced);
    let root_surface_change = root::DockSurfaceChange::Changed;
    let root_surface_change_category = root::DockSurfaceChangeCategory::Selection;
    let prelude_surface_change_category = prelude::DockSurfaceChangeCategory::ViewportTopology;
    let root_surface_change_event: Option<root::DockSurfaceChangeEvent> = None;
    let prelude_activation_request: Option<prelude::DockSurfaceActivationRequestId> = None;
    let root_activation_outcome = root::DockSurfaceActivationOutcome::Committed;
    let prelude_activation_outcome = prelude::DockSurfaceActivationOutcome::Rejected;
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
        error: prelude::DockViewportPlacementValidationError::UnsupportedVersion {
            expected: 1,
            found: 0,
        },
    };
    let root_viewport_readiness_type: Option<root::DockSurfaceViewportReadiness> = None;
    let prelude_viewport_readiness_status = prelude::DockSurfaceViewportReadinessStatus::Openable;
    let root_viewport_platform_capabilities =
        root::DockSurfaceViewportPlatformCapabilities::default();
    let root_viewport_flag_warning =
        root::DockSurfaceViewportFlagWarning::PointerInputPassThroughUnsupported;
    let prelude_viewport_route_status = prelude::DockSurfaceViewportRouteStatus::RouteReady;
    let root_viewport_input_status = root::DockSurfaceViewportInputStatus::ReceivesInput;
    let prelude_viewport_stale_reason = prelude::DockSurfaceViewportStaleReason::WindowFactsChanged;
    let root_viewport_platform_readiness: Option<root::DockSurfaceViewportPlatformReadiness> = None;
    let prelude_viewport_lifecycle_readiness: Option<
        prelude::DockSurfaceViewportLifecycleReadiness,
    > = None;
    let root_viewport_readiness_report_len = root::DockSurfaceViewportReadinessReport::len;
    let root_viewport_report_into_outcomes = root::DockSurfaceViewportOpenReport::into_outcomes;
    let prelude_viewport_restore_report_len = prelude::DockSurfaceViewportRestoreReport::len;
    let root_viewport_restore_outcome_space = root::DockSurfaceViewportRestoreOutcome::space;
    let root_close_merge_target = root::DockSurfaceViewportCloseOutcome::merge_target_space;
    let prelude_should_close_allows = prelude::DockSurfaceViewportShouldCloseOutcome::allows_close;
    let root_viewports_type: Option<root::DockSurfaceViewports> = None;
    let prelude_viewports_type: Option<prelude::DockSurfaceViewports> = None;
    let root_surface_viewports = root::DockSurface::viewports;
    let root_window_session_phase = root::DockSurfaceWindowSessionPhase::Vacant;
    let prelude_window_session_reason: Option<prelude::DockSurfaceWindowSessionReason> = None;
    let root_window_session_shutdown_reason =
        root::DockSurfaceWindowSessionShutdownReason::AppShutdown;
    let prelude_window_session_rollback_reason =
        prelude::DockSurfaceWindowSessionOpeningRollbackReason::Cancelled;
    let root_window_session_status: fn(
        &root::DockSurface,
        &open_gpui::App,
    ) -> root::DockSurfaceWindowSessionStatus = root::DockSurface::window_session_status;
    let root_primary_open_conflict: Option<root::DockSurfacePrimaryWindowOpenConflict> = None;
    let root_snapshot_type: Option<root::DockSurfaceSnapshot> = None;
    let prelude_snapshot_type: Option<prelude::DockSurfaceSnapshot> = None;
    let root_surface_export_snapshot: fn(
        &root::DockSurface,
        &open_gpui::App,
    ) -> root::DockSurfaceSnapshot = root::DockSurface::export_snapshot;
    let root_builder_try_snapshot: fn(
        root::DockSurfaceBuilder,
        &root::DockSurfaceSnapshot,
    ) -> Result<
        root::DockSurfaceBuilder,
        root::DockLayoutValidationError,
    > = root::DockSurfaceBuilder::try_snapshot;

    let _ = (
        root_policy.allows_floating(),
        prelude_policy.allows_platform_viewports(),
        root_layout.layout_version(),
        prelude_layout.layout_version(),
        root_placement.placement_version,
        prelude_placement.placement_version,
        root_restore_readiness.matched,
        prelude_restore_readiness.missing,
        root_placement_validation_error,
        prelude_placement_validation_error,
        root_panel_placement.item(),
        root_visual_style,
        root_visual_style_resolver,
        prelude_visual_style,
        read_only_visual_style_resolver,
        root_surface_builder,
        root_surface_change.changed(),
        root_surface_change_category,
        prelude_surface_change_category,
        root_surface_change_event,
        prelude_activation_request,
        root_activation_outcome,
        prelude_activation_outcome,
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
        root_viewport_readiness_type,
        prelude_viewport_readiness_status,
        root_viewport_platform_capabilities,
        root_viewport_flag_warning,
        prelude_viewport_route_status,
        root_viewport_input_status,
        prelude_viewport_stale_reason,
        root_viewport_platform_readiness,
        prelude_viewport_lifecycle_readiness,
        root_viewport_readiness_report_len,
        root_viewport_report_into_outcomes,
        prelude_viewport_restore_report_len,
        root_viewport_restore_outcome_space,
        root_close_merge_target,
        prelude_should_close_allows,
        root_viewports_type,
        prelude_viewports_type,
        root_surface_viewports,
        root_window_session_phase,
        prelude_window_session_reason,
        root_window_session_shutdown_reason,
        prelude_window_session_rollback_reason,
        root_window_session_status,
        root_primary_open_conflict,
        root_snapshot_type,
        prelude_snapshot_type,
        root_surface_export_snapshot,
        root_builder_try_snapshot,
    );
}

#[test]
fn node_id_focus_is_not_a_public_host_command() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("presentation_commands.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {path:?}: {error}"));

    assert!(!source.contains("pub fn focus_pane"));
    assert!(source.contains("pub(crate) fn focus_pane"));
}

#[test]
fn removed_drop_guide_style_name_is_absent_from_public_authority_sources() {
    for file_name in [
        "lib.rs",
        "prelude.rs",
        "geometry.rs",
        "host.rs",
        "controller.rs",
        "surface/builder.rs",
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join(file_name);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {path:?}: {error}"));
        assert!(!source.contains("DockDropGuideStyle"));
        assert!(!source.contains("drop_guide_style"));
    }
}

#[test]
fn removed_viewport_session_facade_name_is_absent_from_public_authority_sources() {
    for file_name in [
        "src/lib.rs",
        "src/prelude.rs",
        "src/surface.rs",
        "src/surface/viewport.rs",
        "README.md",
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(file_name);
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {path:?}: {error}"));
        assert!(
            !source.contains("DockSurfaceViewportSession"),
            "{file_name} retained the removed viewport-session facade name"
        );
    }
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
    let raw_layout = model::layout_from_raw_parts(Vec::new(), Vec::new());
    let raw_parts = model::layout_into_raw_parts(raw_layout.clone());
    let action = model::DockActionOutcome::Unchanged;
    let controller_builder = model::DockController::builder("main");
    let _controller_type: Option<model::DockController> = None;
    let _controller_builder_type: Option<model::DockControllerBuilder> = None;
    let close_policy = runtime::DockViewportClosePolicy::Prevent;

    let _ = (
        graph,
        layout,
        raw_layout.is_empty(),
        raw_parts,
        action,
        controller_builder,
        close_policy,
    );
}
