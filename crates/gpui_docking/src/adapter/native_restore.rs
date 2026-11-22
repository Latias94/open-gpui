//! Fresh native hosts for a validated logical workspace, without live replacement.

use std::collections::BTreeMap;

use dockspace::model::SurfaceId;
use open_gpui::{
    AnyWindowHandle, App, Bounds, DisplayId, Pixels, PlatformDisplaySnapshot,
    WindowActivationPolicy, WindowBounds, WindowKind, WindowOptions, point, px, size,
};
use thiserror::Error;

use super::{
    DockSurface,
    native::NativeOpenFailure,
    persistence::{
        DockHostDocument, DockPersistenceError, DockSurfaceKeyBinding, DockWindowPlacementState,
    },
};

/// Failure to validate or open a fresh native workspace.
#[derive(Debug, Error)]
pub enum DockNativeRestoreError {
    /// Host hints do not describe this complete logical session.
    #[error(transparent)]
    Persistence(#[from] DockPersistenceError),
    /// A host, anchor, display or window option is unsuitable before any windows open.
    #[error("native workspace restore is invalid: {reason}")]
    Invalid {
        /// The rejected precondition.
        reason: String,
    },
    /// Window creation failed; the original error and rollback authority are retained.
    #[error("restoring native surface {surface:?} failed: {source}; {cleanup}")]
    Open {
        /// Logical surface whose native open failed.
        surface: SurfaceId,
        /// Original typed open failure, including its original native retirement ticket.
        #[source]
        source: Box<NativeOpenFailure>,
        /// Shutdown progress at return time, not a promise that native cleanup completed.
        cleanup: String,
    },
}

fn invalid(reason: impl Into<String>) -> DockNativeRestoreError {
    DockNativeRestoreError::Invalid {
        reason: reason.into(),
    }
}

impl DockSurface {
    /// Opens one ordinary root window for every surface of a freshly restored session.
    ///
    /// First restore a [`super::persistence::DockWorkspaceDocument`], create the
    /// application Views, and pass that sole session to [`Self::new_managed`].
    /// This helper rejects existing mounts, native bindings and retiring hosts.
    /// It neither replaces a running workspace nor recreates native-child roles.
    /// Contained floating panes remain inside their logical surface's window.
    ///
    /// The complete host mapping, explicit anchor and every window option are
    /// checked before opening any window. `options` supplies ordinary window
    /// styling and fallback geometry; saved logical window/display hints take
    /// precedence. Current work areas constrain restore bounds. Presentation
    /// states remain requests to GPUI, never observed native placement proof.
    /// Host-region and central-policy hints require the application's own policy;
    /// each root created here fills its window with the corresponding surface.
    ///
    /// The anchor opens first. All opens use [`Self::open_native_root`], retaining
    /// its original callbacks and rollback tickets. Any open failure starts the
    /// existing group shutdown and returns that original typed failure together
    /// with a cleanup diagnostic. Keep this workspace alive and continue normal
    /// event processing until [`Self::native_is_quiescent`] reports completion.
    pub fn open_restored_native_roots(
        &self,
        host: &DockHostDocument,
        anchor: SurfaceId,
        mut options: impl FnMut(&DockSurfaceKeyBinding, &App) -> WindowOptions,
        cx: &mut App,
    ) -> Result<BTreeMap<SurfaceId, AnyWindowHandle>, DockNativeRestoreError> {
        {
            let state = self
                .host
                .try_borrow()
                .map_err(|_| DockPersistenceError::Busy)?;
            host.validate_session(&state.session)?;
        }
        let inspection = self.inspection().map_err(|_| DockPersistenceError::Busy)?;
        let native = inspection
            .native
            .as_ref()
            .ok_or_else(|| invalid("managed native hosting is required"))?;
        if native.admission_frozen
            || !native.bridge_available
            || native.callback_window_count != 0
            || native.root_window_count != 0
            || native.pending_root_registration_count != 0
            || native.root_retirement_count != 0
            || native.retiring_binding_count != 0
            || native.provisional_opening
            || native.provisional_promotion_count != 0
            || native.provisional_retirement_count != 0
            || !inspection.mounts.is_empty()
            || inspection.native_window_binding_count != 0
            || inspection.pending_native_create
            || inspection.update_pending
            || inspection.active_gesture
            || inspection.last_error.is_some()
        {
            return Err(invalid(
                "restore requires a fresh, unbound managed workspace",
            ));
        }
        if !host
            .surfaces
            .iter()
            .any(|binding| binding.surface == anchor)
        {
            return Err(invalid("the anchor must name a restored logical surface"));
        }
        if host.surfaces.len() > 1 && !cx.viewport_capabilities().platform_viewport_windows {
            return Err(invalid(
                "the backend cannot open multiple native workspace roots",
            ));
        }

        let displays = cx.display_snapshot();
        let mut roots = host
            .surfaces
            .iter()
            .map(|binding| {
                let options = resolve_options(binding, options(binding, cx), &displays, cx)?;
                Ok((binding.surface, options))
            })
            .collect::<Result<Vec<_>, DockNativeRestoreError>>()?;
        roots.sort_by_key(|(surface, _)| (*surface != anchor, *surface));

        let mut windows = BTreeMap::new();
        for (surface, options) in roots {
            match self.open_native_root(surface, surface == anchor, options, cx) {
                Ok(window) => {
                    windows.insert(surface, window);
                }
                Err(source) => {
                    let mut cleanup = match self.begin_native_shutdown(cx) {
                        Ok(()) if self.native_is_quiescent() => {
                            "group shutdown requested; native cleanup is quiescent".to_owned()
                        }
                        Ok(()) => "group shutdown requested; native cleanup is pending".to_owned(),
                        Err(error) => format!("group shutdown request failed: {error:#}"),
                    };
                    if let Some(error) = self.error() {
                        cleanup.push_str(&format!("; host diagnostic: {error}"));
                    }
                    return Err(DockNativeRestoreError::Open {
                        surface,
                        source: Box::new(source),
                        cleanup,
                    });
                }
            }
        }
        Ok(windows)
    }
}

fn resolve_options(
    binding: &DockSurfaceKeyBinding,
    mut options: WindowOptions,
    displays: &PlatformDisplaySnapshot,
    cx: &App,
) -> Result<WindowOptions, DockNativeRestoreError> {
    if !matches!(options.kind, WindowKind::Normal)
        || options.transient_for.is_some()
        || options.provisional_session.is_some()
        || options.tabbing_identifier.is_some()
    {
        return Err(invalid(
            "restored roots require ordinary, independent native windows",
        ));
    }
    if let Some(bounds) = options.window_bounds {
        validate_bounds(bounds.get_bounds())?;
    }
    if let Some(minimum) = options.window_min_size {
        validate_bounds(Bounds::new(point(px(0.0), px(0.0)), minimum))?;
    }
    if let Some(position) = options
        .titlebar
        .as_ref()
        .and_then(|bar| bar.traffic_light_position)
        && (!f32::from(position.x).is_finite() || !f32::from(position.y).is_finite())
    {
        return Err(invalid("titlebar positions must be finite"));
    }

    let placement = binding.logical_placement.as_ref();
    let preferred = placement
        .and_then(|placement| placement.display_id)
        .map(DisplayId::new)
        .or(options.display_id);
    let connected = preferred.and_then(|display| displays.find_display(display));
    let missing_display = preferred.is_some() && connected.is_none();
    let display = connected
        .or_else(|| {
            displays
                .primary_display()
                .and_then(|primary| displays.find_display(primary.id()))
        })
        .or_else(|| displays.displays().into_iter().next())
        .ok_or_else(|| invalid("no connected display is available for native restore"))?;
    let work_area = display.visible_bounds();
    validate_bounds(work_area)?;
    let capabilities = cx.window_capabilities_for(&options.kind, Some(display.id()));
    if !options.focus_on_appearing && !capabilities.creation.focus_on_appearing.is_supported() {
        return Err(invalid(
            "the backend cannot honor non-activating first appearance",
        ));
    }
    if options.activation_policy != WindowActivationPolicy::default()
        && !capabilities
            .mutations
            .activation_policy
            .is_available_at_creation()
    {
        return Err(invalid(
            "the backend cannot honor the requested activation policy",
        ));
    }

    let requested = if let Some(window) = placement
        .and_then(|placement| placement.window_bounds)
        .filter(|window| window.bounds.is_window_restore_hint())
    {
        let rect = window.bounds;
        let bounds = Bounds::new(
            point(px(rect.x), px(rect.y)),
            size(px(rect.width), px(rect.height)),
        );
        match window.state {
            DockWindowPlacementState::Windowed => WindowBounds::Windowed(bounds),
            DockWindowPlacementState::Maximized => WindowBounds::Maximized(bounds),
            DockWindowPlacementState::Fullscreen => WindowBounds::Fullscreen(bounds),
        }
    } else {
        options
            .window_bounds
            .unwrap_or_else(|| WindowBounds::Windowed(display.default_bounds()))
    };
    let mut bounds = requested.get_bounds();
    validate_bounds(bounds)?;
    if let Some(minimum) = options.window_min_size {
        if minimum.width > work_area.size.width || minimum.height > work_area.size.height {
            return Err(invalid(
                "window minimum size exceeds the current display work area",
            ));
        }
        bounds.size.width = bounds.size.width.max(minimum.width);
        bounds.size.height = bounds.size.height.max(minimum.height);
    }
    bounds.size.width = bounds.size.width.min(work_area.size.width);
    bounds.size.height = bounds.size.height.min(work_area.size.height);
    let offscreen = bounds.right() <= work_area.left()
        || bounds.left() >= work_area.right()
        || bounds.bottom() <= work_area.top()
        || bounds.top() >= work_area.bottom();
    if missing_display || offscreen {
        let center = work_area.center();
        bounds.origin = point(
            center.x - bounds.size.width / 2.0,
            center.y - bounds.size.height / 2.0,
        );
    } else {
        bounds.origin.x = bounds
            .origin
            .x
            .clamp(work_area.left(), work_area.right() - bounds.size.width);
        bounds.origin.y = bounds
            .origin
            .y
            .clamp(work_area.top(), work_area.bottom() - bounds.size.height);
    }
    options.display_id = Some(display.id());
    options.window_bounds = Some(match requested {
        WindowBounds::Windowed(_) => WindowBounds::Windowed(bounds),
        WindowBounds::Maximized(_) => WindowBounds::Maximized(bounds),
        WindowBounds::Fullscreen(_) => WindowBounds::Fullscreen(bounds),
    });
    Ok(options)
}

fn validate_bounds(bounds: Bounds<Pixels>) -> Result<(), DockNativeRestoreError> {
    if [
        bounds.origin.x,
        bounds.origin.y,
        bounds.size.width,
        bounds.size.height,
    ]
    .into_iter()
    .any(|value| !f32::from(value).is_finite())
        || bounds.size.width <= px(0.0)
        || bounds.size.height <= px(0.0)
        || !f32::from(bounds.right()).is_finite()
        || !f32::from(bounds.bottom()).is_finite()
    {
        return Err(invalid(
            "native restore bounds must be finite with positive area",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{cell::RefCell, rc::Rc};

    use dockspace::{
        model::{
            DockspaceLayout, DockspaceNode, DockspaceRootLayout, DockspaceSurfaceLayout, RootId,
        },
        policy::DockPolicy,
        runtime::{DockspaceDocumentBootstrap, DockspaceDocumentId, DockspaceSession},
    };
    use open_gpui::{
        AppContext as _, Empty, PlatformWindowPresentOutcome, TestAppContext,
        WindowOpenFailureStage,
    };

    use super::*;
    use crate::adapter::{
        DockPane, GpuiDockspaceConfig,
        native::NativeReport,
        persistence::{
            DockLogicalPlacementRect, DockLogicalViewportPlacement, DockLogicalWindowPlacement,
            DockWorkspaceDocument, RestoredDockWorkspace,
        },
    };

    const FIRST: SurfaceId = SurfaceId::new(1);
    const SECOND: SurfaceId = SurfaceId::new(2);
    const DOCUMENT: DockspaceDocumentId = DockspaceDocumentId::from_bytes([0x72; 16]);

    fn document() -> RestoredDockWorkspace {
        let mut bootstrap = DockspaceDocumentBootstrap::new(DOCUMENT);
        let first = bootstrap.ensure_item("first").unwrap();
        let second = bootstrap.ensure_item("second").unwrap();
        bootstrap.ensure_item("closed-history").unwrap();
        let layout =
            DockspaceLayout::new([(FIRST, first), (SECOND, second)].map(|(surface, item)| {
                DockspaceSurfaceLayout::new(
                    surface,
                    DockspaceRootLayout::new(
                        RootId::new(surface.get()),
                        DockspaceNode::tabs([item]),
                    ),
                )
            }))
            .unwrap();
        let mut session =
            DockspaceSession::from_persistent_layout(layout, DockPolicy::default(), bootstrap)
                .unwrap();
        let host = DockHostDocument {
            surfaces: [FIRST, SECOND]
                .map(|surface| DockSurfaceKeyBinding {
                    space_key: format!("surface-{}", surface.get()),
                    surface,
                    logical_placement: Some(DockLogicalViewportPlacement {
                        display_id: Some(1),
                        window_bounds: Some(DockLogicalWindowPlacement {
                            state: DockWindowPlacementState::Windowed,
                            bounds: DockLogicalPlacementRect {
                                x: 40.0,
                                y: 60.0,
                                width: 640.0,
                                height: 480.0,
                            },
                        }),
                        host_bounds: None,
                    }),
                    central_policy: None,
                })
                .into(),
            migration: None,
        };
        let encoded = DockWorkspaceDocument::capture(&mut session, host)
            .unwrap()
            .to_json()
            .unwrap();
        DockWorkspaceDocument::restore_json(&encoded, DockPolicy::default(), |document, key| {
            document == DOCUMENT && ["first", "second", "closed-history"].contains(&key)
        })
        .unwrap()
    }

    fn workspace(cx: &mut App) -> (DockSurface, DockHostDocument) {
        let restored = document();
        let panes = ["first", "second"].map(|key| {
            DockPane::new(
                restored.session.item_id_for_external_key(key).unwrap(),
                key,
                cx.new(|_| Empty),
            )
        });
        let dock =
            DockSurface::new_managed(restored.session, panes, GpuiDockspaceConfig::default(), cx)
                .unwrap();
        (dock, restored.host)
    }

    fn settle(cx: &mut TestAppContext, dock: &DockSurface) {
        cx.run_until_parked();
        cx.update(|cx| dock.pump_native(cx));
        cx.run_until_parked();
        assert!(dock.error().is_none(), "{:?}", dock.error());
    }

    #[open_gpui::test]
    fn degenerate_saved_window_hint_still_opens_both_roots(cx: &mut TestAppContext) {
        let RestoredDockWorkspace { mut session, host } = document();
        let encoded = DockWorkspaceDocument::capture(&mut session, host)
            .unwrap()
            .to_json()
            .unwrap();
        let mut legacy: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        legacy["host"]["surfaces"][1]["logical_placement"]["window_bounds"]["bounds"]["width"] =
            serde_json::json!(0);
        let RestoredDockWorkspace { mut session, host } = DockWorkspaceDocument::restore_json(
            &serde_json::to_vec(&legacy).unwrap(),
            DockPolicy::default(),
            |document, key| {
                document == DOCUMENT && ["first", "second", "closed-history"].contains(&key)
            },
        )
        .unwrap();

        let (dock, windows) = cx.update(|cx| {
            let panes = ["first", "second"].map(|key| {
                DockPane::new(
                    session.item_id_for_external_key(key).unwrap(),
                    key,
                    cx.new(|_| Empty),
                )
            });
            let mut save_hints = host.clone();
            save_hints.surfaces[1]
                .logical_placement
                .as_mut()
                .unwrap()
                .window_bounds = Some(DockLogicalWindowPlacement {
                state: DockWindowPlacementState::Windowed,
                bounds: DockLogicalPlacementRect {
                    x: 40.0,
                    y: 60.0,
                    width: 0.0,
                    height: 420.0,
                },
            });
            let saved = DockWorkspaceDocument::capture(&mut session, save_hints).unwrap();
            let dock = DockSurface::new_managed(session, panes, Default::default(), cx).unwrap();
            let windows = dock
                .open_restored_native_roots(&host, FIRST, |_, _| WindowOptions::default(), cx)
                .unwrap();
            assert!(
                saved.host().surfaces[1]
                    .logical_placement
                    .as_ref()
                    .unwrap()
                    .window_bounds
                    .is_none()
            );
            (dock, windows)
        });
        settle(cx, &dock);
        assert_eq!(windows.len(), 2);
        cx.update_window(windows[&SECOND], |_, window, _| {
            assert!(window.window_bounds().get_bounds().size.width > px(0.0));
        })
        .unwrap();
        cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
        settle(cx, &dock);
    }

    #[open_gpui::test]
    fn vanished_display_recenters_and_opens_actual_root_windows(cx: &mut TestAppContext) {
        let work_area = Bounds::new(point(px(-1200.0), px(40.0)), size(px(1200.0), px(800.0)));
        cx.simulate_display_bounds_change(DisplayId::new(1), work_area);
        let (dock, mut host) = cx.update(workspace);
        let placement = host.surfaces[0].logical_placement.as_mut().unwrap();
        placement.display_id = Some(999);
        placement.window_bounds.as_mut().unwrap().bounds.x = 20000.0;
        let windows = cx
            .update(|cx| {
                dock.open_restored_native_roots(&host, SECOND, |_, _| WindowOptions::default(), cx)
            })
            .unwrap();
        settle(cx, &dock);
        assert_eq!(cx.windows().len(), 2);
        assert_ne!(windows[&FIRST], windows[&SECOND]);
        assert!(
            windows[&SECOND].window_id().as_u64() < windows[&FIRST].window_id().as_u64(),
            "the explicit anchor must open first"
        );
        cx.update_window(windows[&FIRST], |_, window, cx| {
            assert_eq!(window.display(cx).unwrap().id(), DisplayId::new(1));
            assert_eq!(
                window.window_bounds(),
                WindowBounds::Windowed(Bounds::new(
                    point(px(-920.0), px(200.0)),
                    size(px(640.0), px(480.0))
                ))
            );
        })
        .unwrap();
        for window in windows.values() {
            let callbacks = dock
                .native_callbacks_for_window(window.window_id())
                .unwrap();
            assert_eq!(callbacks.window_id(), Some(window.window_id()));
            assert_eq!(
                dock.native_binding_for_window(window.window_id()),
                Some(callbacks.binding())
            );
        }
        assert_eq!(dock.inspection().unwrap().native_window_binding_count, 2);
        cx.update(|cx| dock.begin_native_shutdown(cx)).unwrap();
        settle(cx, &dock);
        assert!(dock.native_is_quiescent());
    }

    #[open_gpui::test]
    fn restore_bounds_clamp_to_a_connected_display_and_states_remain_requests(
        cx: &mut TestAppContext,
    ) {
        let display = DisplayId::new(2);
        let work_area = Bounds::new(point(px(-1200.0), px(40.0)), size(px(1200.0), px(800.0)));
        cx.simulate_display_added(display, None);
        cx.simulate_display_bounds_change(display, work_area);
        cx.update(|cx| {
            let mut binding = document().host.surfaces.remove(0);
            let cases = [
                (
                    DockWindowPlacementState::Windowed,
                    [-1300.0, 100.0, 600.0, 300.0],
                    [-1200.0, 100.0, 600.0, 300.0],
                ),
                (
                    DockWindowPlacementState::Maximized,
                    [-1000.0, 700.0, 800.0, 500.0],
                    [-1000.0, 340.0, 800.0, 500.0],
                ),
                (
                    DockWindowPlacementState::Fullscreen,
                    [20000.0, 20000.0, 3000.0, 1500.0],
                    [-1200.0, 40.0, 1200.0, 800.0],
                ),
            ];
            for (
                state,
                [x, y, width, height],
                [expected_x, expected_y, expected_width, expected_height],
            ) in cases
            {
                binding.logical_placement = Some(DockLogicalViewportPlacement {
                    display_id: Some(2),
                    window_bounds: Some(DockLogicalWindowPlacement {
                        state,
                        bounds: DockLogicalPlacementRect {
                            x,
                            y,
                            width,
                            height,
                        },
                    }),
                    host_bounds: None,
                });
                let options = resolve_options(
                    &binding,
                    WindowOptions::default(),
                    &cx.display_snapshot(),
                    cx,
                )
                .unwrap();
                let bounds = Bounds::new(
                    point(px(expected_x), px(expected_y)),
                    size(px(expected_width), px(expected_height)),
                );
                assert_eq!(options.display_id, Some(display));
                assert_eq!(
                    options.window_bounds,
                    Some(match state {
                        DockWindowPlacementState::Windowed => WindowBounds::Windowed(bounds),
                        DockWindowPlacementState::Maximized => WindowBounds::Maximized(bounds),
                        DockWindowPlacementState::Fullscreen => WindowBounds::Fullscreen(bounds),
                    })
                );
            }
            assert!(
                cx.windows().is_empty(),
                "resolving requests supplies no native observation"
            );
        });
    }

    #[open_gpui::test]
    fn malformed_late_hint_mapping_and_anchor_validate_before_opening(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let (dock, host) = workspace(cx);
            let before = dock.inspection().unwrap();
            let mutations: [fn(&mut DockHostDocument); 4] = [
                |host| host.surfaces[1].surface = SurfaceId::new(99),
                |host| host.surfaces[1].space_key = host.surfaces[0].space_key.clone(),
                |host| {
                    host.surfaces[1]
                        .logical_placement
                        .as_mut()
                        .unwrap()
                        .window_bounds
                        .as_mut()
                        .unwrap()
                        .bounds
                        .x = f32::NAN
                },
                |host| {
                    host.surfaces[1]
                        .logical_placement
                        .as_mut()
                        .unwrap()
                        .host_bounds = Some(DockLogicalPlacementRect {
                        x: 0.0,
                        y: 0.0,
                        width: -1.0,
                        height: 20.0,
                    })
                },
            ];
            for mutate in mutations {
                let mut malformed = host.clone();
                mutate(&mut malformed);
                let error = dock
                    .open_restored_native_roots(
                        &malformed,
                        FIRST,
                        |_, _| panic!("invalid host hints cannot reach options"),
                        cx,
                    )
                    .unwrap_err();
                assert!(matches!(error, DockNativeRestoreError::Persistence(_)));
            }
            let error = dock
                .open_restored_native_roots(
                    &host,
                    SurfaceId::new(99),
                    |_, _| panic!("an invalid anchor cannot reach options"),
                    cx,
                )
                .unwrap_err();
            assert!(matches!(error, DockNativeRestoreError::Invalid { .. }));
            assert!(cx.windows().is_empty());
            assert_eq!(dock.inspection().unwrap(), before);
        });
    }

    #[open_gpui::test]
    fn invalid_options_on_the_last_surface_cannot_open_the_anchor(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let (dock, host) = workspace(cx);
            let before = dock.inspection().unwrap();
            let mut visited = Vec::new();
            let error = dock
                .open_restored_native_roots(
                    &host,
                    FIRST,
                    |binding, _| {
                        visited.push(binding.surface);
                        WindowOptions {
                            window_min_size: (binding.surface == SECOND)
                                .then(|| size(px(f32::NAN), px(100.0))),
                            ..Default::default()
                        }
                    },
                    cx,
                )
                .unwrap_err();
            assert_eq!(visited, [FIRST, SECOND]);
            assert!(matches!(error, DockNativeRestoreError::Invalid { .. }));
            assert!(cx.windows().is_empty());
            assert_eq!(dock.inspection().unwrap(), before);
        });
    }

    #[open_gpui::test]
    fn no_connected_displays_rejects_the_whole_restore(cx: &mut TestAppContext) {
        cx.simulate_display_removed(DisplayId::new(1));
        cx.update(|cx| {
            let (dock, host) = workspace(cx);
            let error = dock
                .open_restored_native_roots(&host, FIRST, |_, _| WindowOptions::default(), cx)
                .unwrap_err();
            assert!(matches!(error, DockNativeRestoreError::Invalid { .. }));
            assert!(cx.windows().is_empty());
            assert_eq!(dock.inspection().unwrap().native_window_binding_count, 0);
        });
    }

    #[open_gpui::test]
    fn an_existing_native_binding_rejects_restore_without_replacement(cx: &mut TestAppContext) {
        let (dock, host) = cx.update(workspace);
        let first = cx
            .update(|cx| dock.open_native_root(FIRST, true, WindowOptions::default(), cx))
            .unwrap();
        settle(cx, &dock);
        let original = dock.native_binding_for_window(first.window_id()).unwrap();
        cx.update(|cx| {
            let error = dock
                .open_restored_native_roots(
                    &host,
                    FIRST,
                    |_, _| panic!("a running host cannot reach options"),
                    cx,
                )
                .unwrap_err();
            assert!(matches!(error, DockNativeRestoreError::Invalid { .. }));
            assert_eq!(cx.windows(), vec![first]);
            assert_eq!(
                dock.native_binding_for_window(first.window_id()),
                Some(original)
            );
            dock.begin_native_shutdown(cx).unwrap();
        });
        settle(cx, &dock);
        assert!(dock.native_is_quiescent());
    }

    #[open_gpui::test]
    fn a_later_open_failure_preserves_its_ticket_and_shuts_down_earlier_roots(
        cx: &mut TestAppContext,
    ) {
        let (dock, host) = cx.update(workspace);
        let closed = Rc::new(RefCell::new(Vec::new()));
        let observed = closed.clone();
        let _closed = cx
            .update(|cx| cx.on_window_closed(move |_, window| observed.borrow_mut().push(window)));
        cx.set_window_open_present_outcomes([
            PlatformWindowPresentOutcome::Submitted,
            PlatformWindowPresentOutcome::Rejected,
        ]);
        let error = cx
            .update(|cx| {
                dock.open_restored_native_roots(&host, FIRST, |_, _| WindowOptions::default(), cx)
            })
            .unwrap_err();
        let DockNativeRestoreError::Open {
            surface,
            source,
            cleanup,
        } = error
        else {
            panic!("expected an original native open failure")
        };
        assert_eq!(surface, SECOND);
        assert_eq!(
            source.stage,
            Some(WindowOpenFailureStage::BeforeVisibilityPresentation)
        );
        assert!(source.root_binding.is_none());
        assert!(cleanup.contains("group shutdown requested"), "{cleanup}");
        let ticket = source.retirement_ticket.as_ref().unwrap().clone();
        assert!(
            ticket.same_authority(source.source.as_ref().unwrap().retirement_ticket().unwrap())
        );
        settle(cx, &dock);
        assert!(ticket.snapshot().native_terminal());
        assert!(ticket.snapshot().quiesced());
        assert!(dock.native_is_quiescent());
        assert!(cx.windows().is_empty());
        assert!(
            closed
                .borrow()
                .iter()
                .any(|window| *window != ticket.snapshot().window_id()),
            "the earlier successfully created root must retire too"
        );
        assert!(
            dock.take_native_reports()
                .iter()
                .any(|report| matches!(report, NativeReport::GroupShutdownRequested))
        );
    }
}
