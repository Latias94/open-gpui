use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

use open_gpui::{
    App, Bounds, ClickEvent, Context, Entity, InteractiveElement, IntoElement, ParentElement,
    Pixels, PlatformWindowCapabilities, PlatformWindowCreationCapabilities,
    PlatformWindowMutationCapabilities, Render, Rgba, Styled, TargetedEvent, Window, WindowBounds,
    WindowCoordinateSpace, WindowCreationSupport, WindowInitialPresentationOrder,
    WindowMutationRequest, WindowMutationSupport, WindowOptions, WindowPlatformFacts, div, point,
    prelude::*, px, rgb, size,
};
use open_gpui_devtools::{
    DevtoolsArtifact, DevtoolsArtifactMetadata, DevtoolsArtifactRecord,
    DevtoolsInspectorController, DevtoolsInspectorState, DevtoolsRegistry, DevtoolsReport,
    DevtoolsSession, DevtoolsSessionError, DevtoolsSessionExport, DevtoolsSessionFrame,
    DevtoolsWorkbench, ProbeSnapshotError,
};
use open_gpui_docking::{
    DockItemId, DockLayout, DockPanel, DockPanelDescriptor, DockPanelPlacement, DockSpaceId,
    DockSurface, DockSurfacePrimaryWindowOpenOutcome, DockSurfaceViewportOpenOutcome,
    DockSurfaceWindowSessionStatus, DockViewportPlacement, DockViewportPlacementLayout,
    DockViewportWindowBounds, DockVisualPalette, DockVisualStyle, DockVisualStyleResolver,
    advanced::{
        DockViewportCoordinateSpaceRecord, DockViewportCoordinateStatusRecord,
        DockViewportDropOutcomeKind, DockViewportDropOutcomeRecord, DockViewportInputStatus,
        DockViewportLifecycleRecord, DockViewportPayloadRecord,
        DockViewportPlatformCapabilityRecord, DockViewportPlatformRequestStatus,
        DockViewportPlatformSyncDispatch, DockViewportPlatformSyncDomain,
        DockViewportPlatformSyncObservation, DockViewportPlatformSyncObservationOutcome,
        DockViewportPlatformSyncObservedRecord, DockViewportPlatformSyncRecord,
        DockViewportPlatformSyncRequest, DockViewportReleaseUnavailableRecord,
        DockViewportRestoreReadinessRecord, DockViewportRouteStatus, DockViewportRuntimeStatus,
        DockViewportStaleStatusReason, DockViewportTearOffOutcomeKind,
        DockViewportTearOffPlacementRecord, DockViewportTearOffRecord,
        DockViewportVisualAffordanceRecord, DockViewportWindowProfileRecord,
        DockVisualAffordanceDebugLayer, DockVisualAffordanceDebugSummary,
    },
    model::{
        DockController, DockLayoutCentralRegion, DockLayoutSpace, layout_from_raw_parts,
        layout_into_raw_parts,
    },
    runtime::DockViewportClosePolicy,
};
#[cfg(test)]
use open_gpui_docking::{
    DockPanelOpenOutcome, model::DockActionApplyError, runtime::DockViewportRuntimeHandle,
};
use open_gpui_platform::application;
use open_gpui_ui_components::{
    ColorState,
    theme::{
        DARK_THEME_ID, HIGH_CONTRAST_THEME_ID, LIGHT_THEME_ID, ThemeResolver, ThemeSnapshot,
        set_window_theme,
    },
};
use open_gpui_ui_core::{TokenKey, semantic};

const SPACE: &str = "docking-demo";
const SECONDARY_SPACE: &str = "docking-preview";
const CENTRAL_SPACE: &str = "docking-empty-central";
const PRIMARY_DOCK_CLASS: &str = "primary-demo";
const SECONDARY_DOCK_CLASS: &str = "secondary-demo";
const CENTRAL_DOCK_CLASS: &str = "central-demo";
const DOCKING_DEBUG_PREFIX: &str = "[DEBUG-docking-native]";
pub const DOCKING_NATIVE_ARTIFACT_PRODUCER_ID: &str = "docking-native.devtools";
pub const DOCKING_NATIVE_ARTIFACT_SCENARIO_ID: &str = "docking-native.headless";
const DOCKING_NATIVE_ARTIFACT_TIMESTAMP_MS: u64 = 1_725_100_000_000;

struct DemoPanel {
    title: &'static str,
    subtitle: &'static str,
    accent: u32,
    lines: &'static [&'static str],
}

enum RuntimePanelAuthority {
    Managed(DockSurface),
    #[cfg(test)]
    Unmanaged {
        runtime: DockViewportRuntimeHandle,
        controller: Entity<DockController>,
    },
}

struct RuntimeStatusPanel {
    authority: RuntimePanelAuthority,
    devtools_panel: DockingDevtoolsPanel,
    placement: DockViewportPlacementLayout,
    primary_bounds: Bounds<Pixels>,
    secondary_bounds: Bounds<Pixels>,
    central_bounds: Bounds<Pixels>,
    last_operation: Option<String>,
}

#[derive(Clone)]
struct DockingDevtoolsStatus {
    window_session: Option<DockSurfaceWindowSessionStatus>,
    runtime: DockViewportRuntimeStatus,
}

struct DockingDevtoolsPanel {
    workbench: DevtoolsWorkbench,
    status: Arc<Mutex<DockingDevtoolsStatus>>,
    inspector: Entity<DevtoolsInspectorController>,
}

impl DockingDevtoolsPanel {
    fn new(
        initial_status: DockViewportRuntimeStatus,
        window_session: Option<DockSurfaceWindowSessionStatus>,
        cx: &mut Context<RuntimeStatusPanel>,
    ) -> Self {
        let status = Arc::new(Mutex::new(DockingDevtoolsStatus {
            window_session,
            runtime: initial_status,
        }));
        let mut session = docking_panel_devtools_session(Arc::clone(&status));
        let frame = session
            .refresh()
            .expect("docking devtools initial capture should succeed");
        let mut workbench = DevtoolsWorkbench::from_session(session);
        workbench.mark_idle();
        let inspector = cx.new(|cx| {
            DevtoolsInspectorController::new(
                "docking-devtools-inspector",
                DevtoolsInspectorState::from_session_frame(frame),
                cx,
            )
            .title("Docking Runtime DevTools")
        });

        Self {
            workbench,
            status,
            inspector,
        }
    }

    fn refresh(
        &mut self,
        status: DockViewportRuntimeStatus,
        window_session: Option<DockSurfaceWindowSessionStatus>,
        cx: &mut Context<RuntimeStatusPanel>,
    ) -> Result<DevtoolsSessionFrame, DevtoolsSessionError> {
        {
            let mut status_slot = self
                .status
                .lock()
                .expect("docking runtime status lock should not be poisoned");
            *status_slot = DockingDevtoolsStatus {
                window_session,
                runtime: status,
            };
        }

        match self.workbench.refresh() {
            Ok(frame) => {
                self.inspector.update(cx, |inspector, cx| {
                    inspector.update_session_frame(frame.clone(), cx);
                });
                Ok(frame)
            }
            Err(error) => Err(error),
        }
    }

    fn current_generation(&self) -> Option<u64> {
        self.workbench.current_generation()
    }

    fn previous_generation(&self) -> Option<u64> {
        self.workbench.previous_generation()
    }

    fn retained_frames(&self) -> usize {
        self.workbench.retained_frames()
    }

    fn history_limit(&self) -> usize {
        self.workbench.history_limit()
    }

    fn diff_label(&self) -> String {
        self.workbench.diff_summary_label()
    }

    fn refresh_status_label(&self) -> &'static str {
        self.workbench.refresh_status().as_label()
    }

    fn last_error(&self) -> Option<&str> {
        self.workbench.last_error()
    }
}

impl RuntimeStatusPanel {
    #[cfg(test)]
    fn new(
        runtime: DockViewportRuntimeHandle,
        controller: Entity<DockController>,
        placement: DockViewportPlacementLayout,
        primary_bounds: Bounds<Pixels>,
        secondary_bounds: Bounds<Pixels>,
        central_bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) -> Self {
        let initial_status = runtime.runtime_status_for_app(cx);
        let devtools_panel = DockingDevtoolsPanel::new(initial_status, None, cx);

        Self {
            authority: RuntimePanelAuthority::Unmanaged {
                runtime,
                controller,
            },
            devtools_panel,
            placement,
            primary_bounds,
            secondary_bounds,
            central_bounds,
            last_operation: None,
        }
    }

    fn new_managed(
        surface: DockSurface,
        placement: DockViewportPlacementLayout,
        primary_bounds: Bounds<Pixels>,
        secondary_bounds: Bounds<Pixels>,
        central_bounds: Bounds<Pixels>,
        cx: &mut Context<Self>,
    ) -> Self {
        let initial_status = surface.viewports().runtime_status(cx);
        let window_session = surface.window_session_status(cx);
        let devtools_panel = DockingDevtoolsPanel::new(initial_status, Some(window_session), cx);

        Self {
            authority: RuntimePanelAuthority::Managed(surface),
            devtools_panel,
            placement,
            primary_bounds,
            secondary_bounds,
            central_bounds,
            last_operation: None,
        }
    }

    fn set_operation_log(&mut self, message: impl Into<String>, cx: &mut Context<Self>) {
        self.last_operation = Some(message.into());
        cx.notify();
    }

    fn set_close_policy(&mut self, policy: DockViewportClosePolicy, cx: &mut Context<Self>) {
        match &self.authority {
            RuntimePanelAuthority::Managed(surface) => {
                surface.viewports().set_close_policy(policy.clone(), cx)
            }
            #[cfg(test)]
            RuntimePanelAuthority::Unmanaged { runtime, .. } => {
                runtime.set_close_policy(policy.clone())
            }
        }
        self.set_operation_log(format!("set close policy: {policy:?}"), cx);
    }

    fn open_demo_viewport(&mut self, space: &str, _window: &mut Window, cx: &mut Context<Self>) {
        let space_id = DockSpaceId::from(space);
        let fallback_bounds = self.fallback_bounds(&space_id);
        let options = restored_viewport_options(&self.placement, space_id.clone(), fallback_bounds);
        let message = match &self.authority {
            RuntimePanelAuthority::Managed(surface) => {
                match surface.viewports().open(space_id.clone(), options, cx) {
                    DockSurfaceViewportOpenOutcome::Opened(outcome) => format!(
                        "opened managed viewport {}: {:?}",
                        outcome.space().as_str(),
                        outcome.status()
                    ),
                    DockSurfaceViewportOpenOutcome::Unavailable(error) => format!(
                        "open managed viewport {} failed: {error:?}",
                        space_id.as_str()
                    ),
                }
            }
            #[cfg(test)]
            RuntimePanelAuthority::Unmanaged { runtime, .. } => {
                match runtime.open_viewport_from_window(space_id.clone(), options, _window, cx) {
                    Ok(outcome) => format!(
                        "opened viewport {}: {:?}",
                        outcome.space().as_str(),
                        outcome.status()
                    ),
                    Err(error) => {
                        format!("open viewport {} failed: {error}", space_id.as_str())
                    }
                }
            }
        };
        self.set_operation_log(message, cx);
    }

    fn check_saved_placement_restore(&mut self, cx: &mut Context<Self>) {
        let message = match &self.authority {
            RuntimePanelAuthority::Managed(surface) => {
                match surface.viewports().check_restore(&self.placement, cx) {
                    Ok(readiness) => format!("placement restore readiness: {readiness:?}"),
                    Err(error) => format!("check placement failed: {error}"),
                }
            }
            #[cfg(test)]
            RuntimePanelAuthority::Unmanaged { runtime, .. } => {
                match runtime.check_placement_restore(&self.placement) {
                    Ok(readiness) => format!("placement restore readiness: {readiness:?}"),
                    Err(error) => format!("check placement failed: {error}"),
                }
            }
        };
        self.set_operation_log(message, cx);
    }

    fn restore_secondary_panels(&mut self, cx: &mut Context<Self>) {
        let message = match &self.authority {
            RuntimePanelAuthority::Managed(surface) => {
                restore_secondary_panels_on_surface(surface, cx)
            }
            #[cfg(test)]
            RuntimePanelAuthority::Unmanaged { controller, .. } => {
                controller.update(cx, |controller, _| restore_secondary_panels(controller))
            }
        };
        self.set_operation_log(message, cx);
    }

    fn restore_outline_panel(&mut self, cx: &mut Context<Self>) {
        let message = match &self.authority {
            RuntimePanelAuthority::Managed(surface) => {
                restore_outline_panel_on_surface(surface, cx)
            }
            #[cfg(test)]
            RuntimePanelAuthority::Unmanaged { controller, .. } => {
                controller.update(cx, |controller, _| restore_outline_panel(controller))
            }
        };
        self.set_operation_log(message, cx);
    }

    fn restore_central_note_panel(&mut self, cx: &mut Context<Self>) {
        let message = match &self.authority {
            RuntimePanelAuthority::Managed(surface) => {
                restore_central_note_panel_on_surface(surface, cx)
            }
            #[cfg(test)]
            RuntimePanelAuthority::Unmanaged { controller, .. } => {
                controller.update(cx, |controller, _| restore_central_note_panel(controller))
            }
        };
        self.set_operation_log(message, cx);
    }

    fn current_runtime_status(&self, cx: &mut Context<Self>) -> DockViewportRuntimeStatus {
        match &self.authority {
            RuntimePanelAuthority::Managed(surface) => surface.viewports().runtime_status(cx),
            #[cfg(test)]
            RuntimePanelAuthority::Unmanaged { runtime, .. } => runtime.runtime_status_for_app(cx),
        }
    }

    fn current_window_session_status(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<DockSurfaceWindowSessionStatus> {
        match &self.authority {
            RuntimePanelAuthority::Managed(surface) => Some(surface.window_session_status(cx)),
            #[cfg(test)]
            RuntimePanelAuthority::Unmanaged { .. } => None,
        }
    }

    fn runtime_overview(
        &self,
        cx: &mut Context<Self>,
    ) -> (
        DockViewportClosePolicy,
        Vec<String>,
        DockViewportPlacementLayout,
    ) {
        match &self.authority {
            RuntimePanelAuthority::Managed(surface) => {
                let viewports = surface.viewports();
                let spaces = viewports
                    .registered_spaces(cx)
                    .into_iter()
                    .map(|space| {
                        let status = if viewports.is_open(&space, cx) {
                            "open"
                        } else {
                            "missing"
                        };
                        format!("{}: {status}", space.as_str())
                    })
                    .collect();
                (
                    viewports.close_policy(cx),
                    spaces,
                    viewports.export_placement(cx),
                )
            }
            #[cfg(test)]
            RuntimePanelAuthority::Unmanaged { runtime, .. } => {
                let spaces = runtime
                    .registered_viewport_spaces()
                    .into_iter()
                    .map(|space| {
                        let status = if runtime.is_viewport_open(&space) {
                            "open"
                        } else {
                            "missing"
                        };
                        format!("{}: {status}", space.as_str())
                    })
                    .collect();
                (runtime.close_policy(), spaces, runtime.export_placement())
            }
        }
    }

    fn surface_session_lines(&self, cx: &mut Context<Self>) -> Vec<String> {
        let surface = match &self.authority {
            RuntimePanelAuthority::Managed(surface) => surface,
            #[cfg(test)]
            RuntimePanelAuthority::Unmanaged { .. } => return Vec::new(),
        };
        let session = open_gpui_devtools::docking::docking_surface_inspection(
            surface.window_session_status(cx),
            &surface.viewports().runtime_status(cx),
        )
        .session;
        let anchor = session
            .anchor_window_id
            .map(|window| window.to_string())
            .unwrap_or_else(|| "none".to_owned());
        let reason = session
            .reason_kind
            .zip(session.reason_detail)
            .map(|(kind, detail)| format!("{kind}/{detail}"))
            .unwrap_or_else(|| "none".to_owned());
        vec![
            format!("surface session: {} G{}", session.phase, session.generation),
            format!("surface anchor: {anchor}"),
            format!("surface terminal reason: {reason}"),
            format!(
                "surface windows: {} total / {} opening / {} active / {} retiring",
                session.owned_window_count,
                session.opening_window_count,
                session.active_window_count,
                session.retiring_window_count
            ),
            format!(
                "surface terminal tickets: {} total / {} pending",
                session.terminal_ticket_count, session.pending_terminal_ticket_count
            ),
            format!("surface runtime empty: {:?}", session.runtime_empty),
        ]
    }

    fn refresh_devtools_inspector(&mut self, cx: &mut Context<Self>) {
        let status = self.current_runtime_status(cx);
        let window_session = self.current_window_session_status(cx);
        match self.devtools_panel.refresh(status, window_session, cx) {
            Ok(frame) => self.set_operation_log(
                format!(
                    "refreshed devtools inspector: generation {}",
                    frame.generation
                ),
                cx,
            ),
            Err(error) => self.set_operation_log(format!("refresh devtools failed: {error}"), cx),
        }
    }

    fn render_devtools_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let generation = self
            .devtools_panel
            .current_generation()
            .map(|generation| generation.to_string())
            .unwrap_or_else(|| "none".to_string());
        let previous_generation = self
            .devtools_panel
            .previous_generation()
            .map(|generation| generation.to_string())
            .unwrap_or_else(|| "none".to_string());
        let history = format!(
            "{}/{} frames",
            self.devtools_panel.retained_frames(),
            self.devtools_panel.history_limit()
        );
        let diff = self.devtools_panel.diff_label();
        let refresh_status = self.devtools_panel.refresh_status_label();
        let last_error = self.devtools_panel.last_error().map(str::to_owned);

        div()
            .id("docking-devtools:panel")
            .debug_selector(|| "docking-devtools:panel".to_string())
            .flex()
            .flex_col()
            .gap_2()
            .border_1()
            .border_color(rgb(0xd6d8ce))
            .bg(rgb(0xffffff))
            .p_3()
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(control_button_with_id(
                        "docking-devtools:refresh",
                        "Refresh DevTools",
                        cx.listener(|this, _, _, cx| {
                            this.refresh_devtools_inspector(cx);
                        }),
                    ))
                    .child(docking_devtools_status_pill(
                        "refresh-state",
                        "refresh",
                        refresh_status,
                    ))
                    .child(docking_devtools_status_pill(
                        "frame-history",
                        "history",
                        history,
                    ))
                    .child(docking_devtools_status_pill(
                        "generation",
                        "generation",
                        format!("{generation} prev {previous_generation}"),
                    ))
                    .child(docking_devtools_status_pill("diff-state", "diff", diff))
                    .when_some(last_error, |element, error| {
                        element.child(docking_devtools_status_pill(
                            "capture-error",
                            "error",
                            error,
                        ))
                    }),
            )
            .child(self.devtools_panel.inspector.clone())
    }

    fn fallback_bounds(&self, space: &DockSpaceId) -> Bounds<Pixels> {
        if space.as_str() == SECONDARY_SPACE {
            self.secondary_bounds
        } else if space.as_str() == CENTRAL_SPACE {
            self.central_bounds
        } else {
            self.primary_bounds
        }
    }
}

impl Render for RuntimeStatusPanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let lines = {
            let status = self.current_runtime_status(cx);
            let (close_policy, spaces, placement) = self.runtime_overview(cx);
            let mut lines = self.surface_session_lines(cx);
            lines.extend([
                format!("close policy: {close_policy:?}"),
                format!("registered viewports: {}", spaces.len()),
                format!("placement snapshots: {}", placement.viewports.len()),
                format!("spaces: {}", spaces.join(", ")),
                format!(
                    "devtools capture: {}",
                    docking_runtime_devtools_summary(&status)
                ),
                format!(
                    "last route: {}",
                    debug_option(status.last_route.as_ref().map(|record| &record.target))
                ),
                format!(
                    "last route source: {}",
                    debug_option(
                        status
                            .last_route
                            .as_ref()
                            .and_then(|record| record.selection_source.as_ref())
                    )
                ),
                format!(
                    "last route unavailable: {}",
                    route_unavailable_summary(
                        status
                            .last_route
                            .as_ref()
                            .and_then(|record| record.unavailable_reason.as_ref())
                    )
                ),
                format!(
                    "route facts: {}",
                    route_capability_summary(status.platform_capabilities.as_ref())
                ),
                format!(
                    "window profile: {}",
                    window_profile_summary(&status.window_profiles)
                ),
                format!(
                    "coordinate facts: {}",
                    coordinate_status_summary(&status.viewport_lifecycle)
                ),
                format!(
                    "last platform sync: {}",
                    platform_sync_summary(status.last_platform_dispatch.as_ref())
                ),
                format!("preview proof: {}", preview_proof_summary()),
                format!("motion proof: {}", motion_runtime_proof_summary()),
                format!(
                    "placement restore: {}",
                    placement_restore_summary(status.placement_restore.as_ref())
                ),
                format!(
                    "last drop: {}",
                    debug_option(status.last_drop_outcome.as_ref().map(|record| &record.kind))
                ),
                format!(
                    "last activation: {}",
                    debug_option(status.last_activation.as_ref())
                ),
                format!("last close: {}", debug_option(status.last_close.as_ref())),
                format!(
                    "last should-close: {}",
                    debug_option(status.last_should_close.as_ref())
                ),
                format!(
                    "last tear-off: {}",
                    debug_option(status.last_tear_off.as_ref().map(|record| &record.kind))
                ),
                format!(
                    "last tear-off placement: {}",
                    tear_off_placement_summary(
                        status
                            .last_tear_off
                            .as_ref()
                            .and_then(|record| record.placement_source.as_ref())
                    )
                ),
            ]);
            lines.extend(status.visual_affordances.iter().map(|record| {
                format!(
                    "affordance {}: {}",
                    record.space.as_str(),
                    affordance_debug_summary(&record.summary)
                )
            }));
            lines
        };
        let last_operation = self.last_operation.clone();

        div()
            .flex()
            .flex_col()
            .size_full()
            .gap_3()
            .p_4()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x111827))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(4.0)).h(px(28.0)).bg(rgb(0x0f766e)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().text_lg().child("Runtime"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x5f6b7a))
                                    .child("Viewport dogfood state"),
                            ),
                    ),
            )
            .child(self.render_devtools_panel(cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(lines.into_iter().map(|line| {
                        div()
                            .px_2()
                            .py_1()
                            .bg(rgb(0xf4f6f8))
                            .text_color(rgb(0x253041))
                            .child(line)
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(control_button(
                                "Prevent",
                                cx.listener(|this, _, _, cx| {
                                    this.set_close_policy(DockViewportClosePolicy::Prevent, cx);
                                }),
                            ))
                            .child(control_button(
                                "Retain",
                                cx.listener(|this, _, _, cx| {
                                    this.set_close_policy(
                                        DockViewportClosePolicy::RetainLayout,
                                        cx,
                                    );
                                }),
                            ))
                            .child(control_button(
                                "Merge back",
                                cx.listener(|this, _, _, cx| {
                                    this.set_close_policy(
                                        DockViewportClosePolicy::MergeBack {
                                            target_space: SPACE.into(),
                                        },
                                        cx,
                                    );
                                }),
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(control_button(
                                "Open primary",
                                cx.listener(|this, _, window, cx| {
                                    this.open_demo_viewport(SPACE, window, cx);
                                }),
                            ))
                            .child(control_button(
                                "Open secondary",
                                cx.listener(|this, _, window, cx| {
                                    this.open_demo_viewport(SECONDARY_SPACE, window, cx);
                                }),
                            ))
                            .child(control_button(
                                "Open central",
                                cx.listener(|this, _, window, cx| {
                                    this.open_demo_viewport(CENTRAL_SPACE, window, cx);
                                }),
                            ))
                            .child(control_button(
                                "Check placement",
                                cx.listener(|this, _, _, cx| {
                                    this.check_saved_placement_restore(cx);
                                }),
                            )),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(control_button(
                                "Restore secondary tabs",
                                cx.listener(|this, _, _, cx| {
                                    this.restore_secondary_panels(cx);
                                }),
                            ))
                            .child(control_button(
                                "Restore outline",
                                cx.listener(|this, _, _, cx| {
                                    this.restore_outline_panel(cx);
                                }),
                            ))
                            .child(control_button(
                                "Restore central note",
                                cx.listener(|this, _, _, cx| {
                                    this.restore_central_note_panel(cx);
                                }),
                            )),
                    ),
            )
            .when_some(last_operation, |element, operation| {
                element.child(
                    div()
                        .px_2()
                        .py_1()
                        .bg(rgb(0xecfdf5))
                        .text_color(rgb(0x065f46))
                        .child(operation),
                )
            })
    }
}

fn control_button(
    label: &str,
    listener: impl Fn(&TargetedEvent<ClickEvent>, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    control_button_with_id(format!("runtime-control:{label}"), label, listener)
}

fn control_button_with_id(
    id: impl Into<String>,
    label: &str,
    listener: impl Fn(&TargetedEvent<ClickEvent>, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let id = id.into();
    let debug_id = id.clone();
    div()
        .id(id)
        .debug_selector(|| debug_id)
        .px_2()
        .py_1()
        .border_1()
        .border_color(rgb(0xcbd5e1))
        .bg(rgb(0xf8fafc))
        .hover(|style| style.bg(rgb(0xe2e8f0)))
        .active(|style| style.opacity(0.78))
        .cursor_pointer()
        .text_color(rgb(0x1f2937))
        .on_click(listener)
        .child(label.to_string())
}

fn docking_devtools_status_pill(
    id: &'static str,
    label: &'static str,
    value: impl Into<String>,
) -> impl IntoElement {
    let debug_id = format!("docking-devtools:{id}");
    div()
        .id(debug_id.clone())
        .debug_selector(|| debug_id)
        .flex()
        .items_center()
        .gap_1()
        .px_2()
        .py_1()
        .border_1()
        .border_color(rgb(0xd6d8ce))
        .bg(rgb(0xf8fafc))
        .text_color(rgb(0x253041))
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x667085))
                .child(label.to_string()),
        )
        .child(div().text_xs().child(value.into()))
}

fn restore_secondary_panels_on_surface(surface: &DockSurface, cx: &mut App) -> String {
    let secondary_space = DockSpaceId::from(SECONDARY_SPACE);
    let preview = DockItemId::from("preview");
    let diff = DockItemId::from("diff");
    let mut results = Vec::new();

    match surface.panel_location(preview.clone(), cx) {
        Some(location) if location.space() == &secondary_space => {
            results.push("preview already in secondary".to_owned());
        }
        Some(_) => results.push("preview is open outside secondary".to_owned()),
        None => results.push(surface_open_item_result(
            "preview",
            surface.open_panel_at_in_space(
                secondary_space.clone(),
                DockPanelPlacement::center(preview.clone()).selected(),
                cx,
            ),
        )),
    }

    match surface.panel_location(diff.clone(), cx) {
        Some(location) if location.space() == &secondary_space => {
            results.push("diff already in secondary".to_owned());
        }
        Some(_) => results.push("diff is open outside secondary".to_owned()),
        None => results.push(surface_open_item_result(
            "diff",
            surface.open_panel_at_in_space(
                secondary_space,
                DockPanelPlacement::stacked_with(diff, preview),
                cx,
            ),
        )),
    }

    results.join("; ")
}

fn restore_outline_panel_on_surface(surface: &DockSurface, cx: &mut App) -> String {
    let main_space = DockSpaceId::from(SPACE);
    let outline = DockItemId::from("outline");
    match surface.panel_location(outline.clone(), cx) {
        Some(location) if location.space() == &main_space => {
            "outline already in primary".to_owned()
        }
        Some(_) => "outline is open outside primary".to_owned(),
        None => surface_open_item_result(
            "outline",
            surface.open_panel_in_space(main_space, outline, cx),
        ),
    }
}

fn restore_central_note_panel_on_surface(surface: &DockSurface, cx: &mut App) -> String {
    let central_space = DockSpaceId::from(CENTRAL_SPACE);
    let note = DockItemId::from("central-note");
    match surface.panel_location(note.clone(), cx) {
        Some(location) if location.space() == &central_space => {
            "central note already in central".to_owned()
        }
        Some(_) => "central note is open outside central".to_owned(),
        None => surface_open_item_result(
            "central note",
            surface.open_panel_at_in_space(central_space, DockPanelPlacement::center(note), cx),
        ),
    }
}

fn surface_open_item_result<T: std::fmt::Debug, E: std::fmt::Display>(
    label: &str,
    result: Result<T, E>,
) -> String {
    match result {
        Ok(outcome) => format!("{label}: {outcome:?}"),
        Err(error) => format!("{label} failed: {error}"),
    }
}

#[cfg(test)]
fn restore_secondary_panels(controller: &mut DockController) -> String {
    let secondary_space = DockSpaceId::from(SECONDARY_SPACE);
    let preview = DockItemId::from("preview");
    let diff = DockItemId::from("diff");
    let mut results = Vec::new();

    if controller
        .graph()
        .find_item_in_space(&secondary_space, &preview)
        .is_some()
    {
        results.push("preview already in secondary".to_string());
    } else if controller.graph().contains_item(&preview) {
        results.push("preview is open outside secondary".to_string());
    } else {
        results.push(open_item_result(
            "preview",
            controller.open_panel_at_placement(
                secondary_space.clone(),
                DockPanelPlacement::center(preview.clone()).selected(),
            ),
        ));
    }

    if controller
        .graph()
        .find_item_in_space(&secondary_space, &diff)
        .is_some()
    {
        results.push("diff already in secondary".to_string());
    } else if controller.graph().contains_item(&diff) {
        results.push("diff is open outside secondary".to_string());
    } else {
        results.push(open_item_result(
            "diff",
            controller.open_panel_at_placement(
                secondary_space,
                DockPanelPlacement::stacked_with(diff, "preview"),
            ),
        ));
    }

    results.join("; ")
}

#[cfg(test)]
fn restore_outline_panel(controller: &mut DockController) -> String {
    let main_space = DockSpaceId::from(SPACE);
    let outline = DockItemId::from("outline");
    if controller
        .graph()
        .find_item_in_space(&main_space, &outline)
        .is_some()
    {
        return "outline already in primary".to_string();
    }
    if controller.graph().contains_item(&outline) {
        return "outline is open outside primary".to_string();
    }

    open_item_result("outline", controller.reopen_panel(main_space, outline))
}

#[cfg(test)]
fn restore_central_note_panel(controller: &mut DockController) -> String {
    let central_space = DockSpaceId::from(CENTRAL_SPACE);
    let note = DockItemId::from("central-note");
    if controller
        .graph()
        .find_item_in_space(&central_space, &note)
        .is_some()
    {
        return "central note already in central".to_string();
    }
    if controller.graph().contains_item(&note) {
        return "central note is open outside central".to_string();
    }

    open_item_result(
        "central note",
        controller.open_panel_at_placement(central_space, DockPanelPlacement::center(note)),
    )
}

#[cfg(test)]
fn open_item_result(
    label: &str,
    result: std::result::Result<DockPanelOpenOutcome, DockActionApplyError>,
) -> String {
    match result {
        Ok(outcome) => format!(
            "opened {label}: {} via {:?}",
            if outcome.changed() {
                "Changed"
            } else {
                "Unchanged"
            },
            outcome.placement_source()
        ),
        Err(error) => format!("open {label} failed: {error}"),
    }
}

fn docking_runtime_devtools_summary(status: &DockViewportRuntimeStatus) -> String {
    let inspection = open_gpui_devtools::docking::docking_runtime_inspection(status);
    format!(
        "viewports={}, events={}, affordances={}, diagnostics={}",
        inspection.summary.viewport_lifecycle_count,
        inspection.summary.runtime_event_count,
        inspection.summary.visual_affordance_count,
        inspection.summary.diagnostic_count
    )
}

fn docking_panel_devtools_session(status: Arc<Mutex<DockingDevtoolsStatus>>) -> DevtoolsSession {
    let mut registry = DevtoolsRegistry::default();
    registry
        .register_capture_provider_fn("docking.runtime", move || {
            let status = status
                .lock()
                .map_err(|_| {
                    ProbeSnapshotError::CollectionFailed(
                        "docking panel status lock poisoned".to_owned(),
                    )
                })?
                .clone();
            Ok(match status.window_session {
                Some(window_session) => open_gpui_devtools::docking::docking_surface_capture(
                    window_session,
                    &status.runtime,
                ),
                None => open_gpui_devtools::docking::docking_runtime_capture(&status.runtime),
            })
        })
        .expect("docking panel capture provider id should be valid");
    DevtoolsSession::new("docking.runtime", registry).with_history_limit(4)
}

fn docking_runtime_devtools_session(
    status: Arc<Mutex<DockViewportRuntimeStatus>>,
) -> DevtoolsSession {
    let mut registry = DevtoolsRegistry::default();
    registry
        .register_capture_provider_fn("docking.runtime", move || {
            let status = status
                .lock()
                .map_err(|_| {
                    ProbeSnapshotError::CollectionFailed(
                        "docking runtime status lock poisoned".to_string(),
                    )
                })?
                .clone();
            Ok(open_gpui_devtools::docking::docking_runtime_capture(
                &status,
            ))
        })
        .expect("docking runtime capture provider id should be valid");
    DevtoolsSession::new("docking.runtime", registry).with_history_limit(4)
}

#[derive(Clone, Debug)]
pub struct DockingNativeHeadlessArtifacts {
    pub session_export: DevtoolsSessionExport,
    pub report: DevtoolsReport,
    pub session_record: DevtoolsArtifactRecord,
    pub report_record: DevtoolsArtifactRecord,
}

pub fn docking_native_headless_artifacts() -> DockingNativeHeadlessArtifacts {
    let status_slot = Arc::new(Mutex::new(docking_native_headless_status(1)));
    let mut session = docking_runtime_devtools_session(Arc::clone(&status_slot));
    session
        .refresh()
        .expect("docking native headless first refresh succeeds");
    *status_slot
        .lock()
        .expect("docking native headless status lock is not poisoned") =
        docking_native_headless_status(2);
    session
        .refresh()
        .expect("docking native headless second refresh succeeds");

    let session_export = session.export();
    let report = DevtoolsReport::from_session_export(&session_export);
    let session_record = DevtoolsArtifactRecord::new(
        docking_native_artifact_metadata(0, "fixture-session"),
        DevtoolsArtifact::session_export(&session_export),
    );
    let report_record = DevtoolsArtifactRecord::new(
        docking_native_artifact_metadata(1, "fixture-report"),
        DevtoolsArtifact::report(&report),
    );

    DockingNativeHeadlessArtifacts {
        session_export,
        report,
        session_record,
        report_record,
    }
}

pub fn docking_native_artifact_metadata(
    sequence: u64,
    flush_reason: &str,
) -> DevtoolsArtifactMetadata {
    DevtoolsArtifactMetadata::new(DOCKING_NATIVE_ARTIFACT_PRODUCER_ID)
        .scenario_id(DOCKING_NATIVE_ARTIFACT_SCENARIO_ID)
        .sequence(sequence)
        .flush_reason(flush_reason)
        .timestamp_ms(DOCKING_NATIVE_ARTIFACT_TIMESTAMP_MS + sequence)
}

pub fn docking_native_headless_status(generation: u64) -> DockViewportRuntimeStatus {
    let mut status = DockViewportRuntimeStatus::default();
    let window_id = open_gpui::WindowId::from(10 + generation);
    status.platform_capabilities = Some(DockViewportPlatformCapabilityRecord {
        platform_viewport_windows: false,
        global_window_bounds: true,
        window_stack: true,
        display_work_area: true,
        dpi_scale: true,
        hovered_window_ignores_no_input: false,
    });
    status
        .window_profiles
        .push(DockViewportWindowProfileRecord {
            space: DockSpaceId::from(SPACE),
            window_id,
            window_kind: open_gpui::WindowKind::Normal,
            capabilities: PlatformWindowCapabilities {
                creation: PlatformWindowCreationCapabilities {
                    focus_on_appearing: WindowCreationSupport::Supported,
                    transient_for: WindowCreationSupport::Supported,
                    initial_presentation_order: WindowInitialPresentationOrder::BeforeVisibility,
                },
                mutations: PlatformWindowMutationCapabilities {
                    position: WindowMutationSupport::Live,
                    size: WindowMutationSupport::Live,
                    windowed: WindowMutationSupport::Live,
                    maximized: WindowMutationSupport::Live,
                    fullscreen: WindowMutationSupport::Live,
                    minimized: WindowMutationSupport::Live,
                    restore_bounds: WindowMutationSupport::Live,
                    pointer_input: WindowMutationSupport::Live,
                    activation_policy: WindowMutationSupport::Live,
                    alpha: WindowMutationSupport::CreationOnly,
                    coordinate_space: open_gpui::WindowCoordinateSpace::GlobalScreen,
                    ..Default::default()
                },
            },
        });
    status.placement_restore = Some(DockViewportRestoreReadinessRecord {
        matched: 1,
        missing: 1,
    });
    status.viewport_lifecycle.push(DockViewportLifecycleRecord {
        space: DockSpaceId::from(SPACE),
        window_id,
        route_status: if generation == 1 {
            DockViewportRouteStatus::MissingRouteFacts
        } else {
            DockViewportRouteStatus::RouteReady
        },
        input_status: DockViewportInputStatus::ReceivesInput,
        platform_request_status: DockViewportPlatformRequestStatus {
            close_requested: false,
            resize_requested: generation > 1,
        },
        coordinate_status: (generation > 1).then_some(DockViewportCoordinateStatusRecord {
            display_id: None,
            coordinate_space: DockViewportCoordinateSpaceRecord::GlobalScreen,
            facts_generation: generation,
        }),
        facts_generation: generation,
    });
    if generation > 1 {
        let secondary_window_id = open_gpui::WindowId::from(20 + generation);
        status
            .window_profiles
            .push(DockViewportWindowProfileRecord {
                space: DockSpaceId::from(SECONDARY_SPACE),
                window_id: secondary_window_id,
                window_kind: open_gpui::WindowKind::Floating,
                capabilities: PlatformWindowCapabilities {
                    creation: PlatformWindowCreationCapabilities {
                        focus_on_appearing: WindowCreationSupport::Supported,
                        transient_for: WindowCreationSupport::Unsupported,
                        initial_presentation_order: WindowInitialPresentationOrder::AfterVisibility,
                    },
                    mutations: PlatformWindowMutationCapabilities {
                        position: WindowMutationSupport::CreationOnly,
                        size: WindowMutationSupport::Live,
                        pointer_input: WindowMutationSupport::Unsupported,
                        activation_policy: WindowMutationSupport::CreationOnly,
                        alpha: WindowMutationSupport::CreationOnly,
                        coordinate_space: open_gpui::WindowCoordinateSpace::WindowLocal,
                        ..Default::default()
                    },
                },
            });
        status.viewport_lifecycle.push(DockViewportLifecycleRecord {
            space: DockSpaceId::from(SECONDARY_SPACE),
            window_id: secondary_window_id,
            route_status: DockViewportRouteStatus::Stale {
                reason: DockViewportStaleStatusReason::WindowFactsChanged,
            },
            input_status: DockViewportInputStatus::NoInputPassThrough,
            platform_request_status: DockViewportPlatformRequestStatus {
                close_requested: true,
                resize_requested: false,
            },
            coordinate_status: Some(DockViewportCoordinateStatusRecord {
                display_id: None,
                coordinate_space: DockViewportCoordinateSpaceRecord::WindowLocal,
                facts_generation: generation,
            }),
            facts_generation: generation,
        });
        status.last_drop_outcome = Some(DockViewportDropOutcomeRecord {
            kind: DockViewportDropOutcomeKind::Error,
            action: None,
            error: None,
        });
        status.last_tear_off = Some(DockViewportTearOffRecord {
            kind: DockViewportTearOffOutcomeKind::Completed,
            placement_source: Some(DockViewportTearOffPlacementRecord::Suggested),
            source_space: DockSpaceId::from(SPACE),
            target_space: DockSpaceId::from(SECONDARY_SPACE),
            payload: DockViewportPayloadRecord::Item {
                item: DockItemId::from("headless-editor"),
            },
        });
        let sync_window_id = secondary_window_id;
        let observed_bounds = Bounds::new(point(px(40.0), px(50.0)), size(px(360.0), px(240.0)));
        let observation = DockViewportPlatformSyncObservation {
            domain: DockViewportPlatformSyncDomain::PointerInput,
            generation,
            request: WindowMutationRequest::PointerInput(false),
            outcome: DockViewportPlatformSyncObservationOutcome::Adjusted,
            facts: WindowPlatformFacts {
                bounds: observed_bounds,
                coordinate_space: WindowCoordinateSpace::WindowLocal,
                window_bounds: WindowBounds::Windowed(observed_bounds),
                inner_window_bounds: WindowBounds::Windowed(observed_bounds),
                content_size: observed_bounds.size,
                scale_factor: 1.25,
                display_id: None,
                is_minimized: false,
                is_maximized: false,
                is_fullscreen: false,
                accepts_pointer_input: true,
                accepts_activation: true,
                focus_on_click: true,
                background_appearance: open_gpui::WindowBackgroundAppearance::Opaque,
                topmost: false,
                taskbar_visible: true,
                is_active: false,
            },
        };
        status.last_platform_dispatch = Some(DockViewportPlatformSyncRecord {
            window_id: sync_window_id,
            dispatches: vec![DockViewportPlatformSyncDispatch::Queued {
                request: DockViewportPlatformSyncRequest::PointerInput { requested: false },
                domain: DockViewportPlatformSyncDomain::PointerInput,
                generation,
            }],
            observations: vec![observation.clone()],
        });
        status
            .recent_platform_observations
            .push(DockViewportPlatformSyncObservedRecord {
                window_id: sync_window_id,
                observation,
            });
        status
            .visual_affordances
            .push(DockViewportVisualAffordanceRecord {
                space: DockSpaceId::from(SECONDARY_SPACE),
                window_id: open_gpui::WindowId::from(20 + generation),
                summary: DockVisualAffordanceDebugSummary {
                    space: Some(SECONDARY_SPACE.to_owned()),
                    frame_generation: Some(generation),
                    layer_count: 2,
                    active_count: 1,
                    active: Some(DockVisualAffordanceDebugLayer {
                        id: "headless-active-layer".to_owned(),
                        kind: "drop-guide".to_owned(),
                        scope: "viewport".to_owned(),
                        state: "active".to_owned(),
                        target_node: Some(42),
                        zone: None,
                        payload_index: Some(0),
                        label: Some("Headless Editor".to_owned()),
                    }),
                    motion_state: Some("settled".to_owned()),
                    churn_signature: "docking-preview:2:1".to_owned(),
                },
            });
    }

    status
}

#[cfg(test)]
fn docking_runtime_devtools_capture(
    status: &DockViewportRuntimeStatus,
) -> open_gpui_devtools::DevtoolsCapture {
    open_gpui_devtools::docking::docking_runtime_capture(status)
}

fn debug_option<T: std::fmt::Debug>(value: Option<T>) -> String {
    value
        .map(|value| format!("{value:?}"))
        .unwrap_or_else(|| "none".to_string())
}

fn affordance_debug_summary(summary: &DockVisualAffordanceDebugSummary) -> String {
    let active = summary
        .active
        .as_ref()
        .map(|layer| {
            format!(
                "{} {} state={} node={} zone={} payload={} label={}",
                layer.kind,
                layer.scope,
                layer.state,
                layer
                    .target_node
                    .map(|node| node.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                layer
                    .zone
                    .map(|zone| format!("{zone:?}"))
                    .unwrap_or_else(|| "none".to_string()),
                layer
                    .payload_index
                    .map(|index| index.to_string())
                    .unwrap_or_else(|| "none".to_string()),
                layer.label.as_deref().unwrap_or("none"),
            )
        })
        .unwrap_or_else(|| "none".to_string());
    format!(
        "layers={} active={} motion={} frame={} active={}",
        summary.layer_count,
        summary.active_count,
        summary.motion_state.as_deref().unwrap_or("none"),
        summary
            .frame_generation
            .map(|generation| generation.to_string())
            .unwrap_or_else(|| "none".to_string()),
        active,
    )
}

fn capability_flag(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

fn route_capability_summary(capabilities: Option<&DockViewportPlatformCapabilityRecord>) -> String {
    capabilities
        .map(|capabilities| {
            format!(
                "platform-windows={}, bounds={}, stack={}, hover-through-no-input={}",
                capability_flag(capabilities.platform_viewport_windows),
                capability_flag(capabilities.global_window_bounds),
                capability_flag(capabilities.window_stack),
                capability_flag(capabilities.hovered_window_ignores_no_input),
            )
        })
        .unwrap_or_else(|| "unavailable".to_string())
}

fn window_profile_summary(viewports: &[DockViewportWindowProfileRecord]) -> String {
    if viewports.is_empty() {
        return "unavailable".to_string();
    }
    viewports
        .iter()
        .map(|viewport| {
            let creation = viewport.capabilities.creation;
            let mutations = viewport.capabilities.mutations;
            format!(
                "{}#{}({}): nonactivating-appear={}, transient-owner={}, first-present={}, position={}, size={}, windowed={}, maximized={}, fullscreen={}, minimized={}, restore={}, pointer={}, activation-policy={}, alpha={}, topmost={}, taskbar={}",
                viewport.space,
                viewport.window_id.as_u64(),
                viewport.window_kind.as_str(),
                creation_support(creation.focus_on_appearing),
                creation_support(creation.transient_for),
                initial_presentation_order(creation.initial_presentation_order),
                mutation_support(mutations.position),
                mutation_support(mutations.size),
                mutation_support(mutations.windowed),
                mutation_support(mutations.maximized),
                mutation_support(mutations.fullscreen),
                mutation_support(mutations.minimized),
                mutation_support(mutations.restore_bounds),
                mutation_support(mutations.pointer_input),
                mutation_support(mutations.activation_policy),
                mutation_support(mutations.alpha),
                mutation_support(mutations.topmost),
                mutation_support(mutations.taskbar_visibility),
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

fn creation_support(support: WindowCreationSupport) -> &'static str {
    match support {
        WindowCreationSupport::Unsupported => "unsupported",
        WindowCreationSupport::Supported => "supported",
    }
}

fn initial_presentation_order(order: WindowInitialPresentationOrder) -> &'static str {
    match order {
        WindowInitialPresentationOrder::BeforeVisibility => "before-visibility",
        WindowInitialPresentationOrder::AfterVisibility => "after-visibility",
        WindowInitialPresentationOrder::PresentationEstablishesVisibility => {
            "presentation-establishes-visibility"
        }
    }
}

fn mutation_support(support: WindowMutationSupport) -> &'static str {
    match support {
        WindowMutationSupport::Unsupported => "unsupported",
        WindowMutationSupport::CreationOnly => "creation-only",
        WindowMutationSupport::Live => "live",
    }
}

fn route_unavailable_summary(
    reason: Option<&DockViewportReleaseUnavailableRecord>,
) -> &'static str {
    match reason {
        Some(DockViewportReleaseUnavailableRecord::PlatformViewportWindowsUnsupported) => {
            "platform-windows-unsupported"
        }
        Some(DockViewportReleaseUnavailableRecord::BlockedByViewportWindow) => "blocked-window",
        Some(DockViewportReleaseUnavailableRecord::NoViewportRouteSelection) => {
            "no-route-selection"
        }
        Some(DockViewportReleaseUnavailableRecord::TrustedHoveredNone) => "trusted-hovered-none",
        Some(_) => "unknown",
        None => "none",
    }
}

fn coordinate_status_summary(lifecycle: &[DockViewportLifecycleRecord]) -> String {
    if lifecycle.is_empty() {
        return "none".to_string();
    }
    lifecycle
        .iter()
        .map(|record| {
            let coordinate = record
                .coordinate_status
                .map(|status| {
                    let space = match status.coordinate_space {
                        DockViewportCoordinateSpaceRecord::GlobalScreen => "global",
                        DockViewportCoordinateSpaceRecord::WindowLocal => "local",
                        _ => "unknown",
                    };
                    format!("{space}@gen{}", status.facts_generation)
                })
                .unwrap_or_else(|| "missing".to_string());
            format!("{}={coordinate}", record.space.as_str())
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn platform_sync_summary(sync: Option<&DockViewportPlatformSyncRecord>) -> String {
    sync.map(|sync| {
        format!(
            "dispatches={}, observations={}",
            sync.dispatches.len(),
            sync.observations.len()
        )
    })
    .unwrap_or_else(|| "none".to_string())
}

fn preview_proof_summary() -> &'static str {
    "presentation-scene+real-content-reveal+overlay-motion+tab-insertion+retargeting+splitter-motion+zoom-focus+divider-hit-map+corner-drag+a11y+route-cleanup+reduced-motion"
}

fn motion_runtime_proof_summary() -> &'static str {
    "shared-runtime+run-state+scalar-value+scalar-sample+explicit-models+policy-gates+layout-projection+projection-clips+sampled-progress+retargeted-identity+reduced-motion-final-state+high-frequency-bypass"
}

fn tear_off_placement_summary(source: Option<&DockViewportTearOffPlacementRecord>) -> &'static str {
    match source {
        Some(DockViewportTearOffPlacementRecord::Suggested) => "suggested",
        Some(DockViewportTearOffPlacementRecord::DragGeometry) => "drag-geometry",
        None => "unavailable",
    }
}

fn placement_restore_summary(readiness: Option<&DockViewportRestoreReadinessRecord>) -> String {
    readiness
        .map(|readiness| {
            format!(
                "matched={}, missing={}",
                readiness.matched, readiness.missing
            )
        })
        .unwrap_or_else(|| "unavailable".to_string())
}

impl DemoPanel {
    fn new(
        title: &'static str,
        subtitle: &'static str,
        accent: u32,
        lines: &'static [&'static str],
    ) -> Self {
        Self {
            title,
            subtitle,
            accent,
            lines,
        }
    }
}

impl Render for DemoPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let accent = rgb(self.accent);

        div()
            .flex()
            .flex_col()
            .size_full()
            .gap_3()
            .p_4()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x111827))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(4.0)).h(px(28.0)).bg(accent))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .child(div().text_lg().child(self.title))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(rgb(0x5f6b7a))
                                    .child(self.subtitle),
                            ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(self.lines.iter().map(|line| {
                        div()
                            .px_2()
                            .py_1()
                            .bg(rgb(0xf4f6f8))
                            .text_color(rgb(0x253041))
                            .child(*line)
                    })),
            )
    }
}

fn restored_demo_layout() -> DockLayout {
    let mut controller = DockController::builder(SPACE)
        .panel_placements([
            DockPanelPlacement::left_rail("explorer")
                .fraction(0.24)
                .selected(),
            DockPanelPlacement::stacked_with("outline", "explorer"),
            DockPanelPlacement::stacked_with("workspace", "explorer"),
            DockPanelPlacement::center("editor").selected(),
            DockPanelPlacement::stacked_with("preview", "editor"),
            DockPanelPlacement::bottom_rail("terminal").fraction(0.32),
            DockPanelPlacement::stacked_with("problems", "terminal"),
            DockPanelPlacement::stacked_with("runtime", "terminal"),
        ])
        .allow_floating(true)
        .allow_platform_viewports(true)
        .allow_dock_class_in_space(SPACE, PRIMARY_DOCK_CLASS)
        .allow_dock_class_in_space(SPACE, SECONDARY_DOCK_CLASS)
        .allow_dock_class_in_space(SPACE, CENTRAL_DOCK_CLASS)
        .allow_dock_class_in_space(SECONDARY_SPACE, SECONDARY_DOCK_CLASS)
        .allow_dock_class_in_space(CENTRAL_SPACE, CENTRAL_DOCK_CLASS)
        .panel_descriptor(
            "explorer",
            dogfood_descriptor("Explorer", PRIMARY_DOCK_CLASS),
        )
        .panel_descriptor("outline", dogfood_descriptor("Outline", PRIMARY_DOCK_CLASS))
        .panel_descriptor(
            "workspace",
            dogfood_descriptor("Workspace", PRIMARY_DOCK_CLASS),
        )
        .panel_descriptor("editor", dogfood_descriptor("Editor", PRIMARY_DOCK_CLASS))
        .panel_descriptor(
            "preview",
            dogfood_descriptor("Preview", SECONDARY_DOCK_CLASS),
        )
        .panel_descriptor("diff", dogfood_descriptor("Diff", SECONDARY_DOCK_CLASS))
        .panel_descriptor(
            "terminal",
            dogfood_descriptor("Terminal", PRIMARY_DOCK_CLASS),
        )
        .panel_descriptor(
            "problems",
            dogfood_descriptor("Problems", PRIMARY_DOCK_CLASS),
        )
        .panel_descriptor("runtime", dogfood_descriptor("Runtime", PRIMARY_DOCK_CLASS))
        .try_build()
        .expect("demo controller setup should validate");

    let main_space = DockSpaceId::from(SPACE);
    let preview_item: open_gpui_docking::DockItemId = "preview".into();
    controller
        .close_panel(main_space.clone(), preview_item.clone())
        .expect("preview panel should close before reopening into secondary space");
    controller
        .open_panel_at_placement(
            SECONDARY_SPACE,
            DockPanelPlacement::center(preview_item.clone()).selected(),
        )
        .expect("preview panel should reopen into the secondary demo dock space");
    let secondary_space = DockSpaceId::from(SECONDARY_SPACE);
    let diff_item: open_gpui_docking::DockItemId = "diff".into();
    controller
        .open_panel_at_placement(
            secondary_space,
            DockPanelPlacement::stacked_with(diff_item, preview_item.clone()).insert_index(1),
        )
        .expect("diff panel should join the secondary demo tab stack");
    controller
        .float_item_in_window(
            SPACE,
            "problems",
            SPACE,
            Bounds::new(point(px(620.0), px(72.0)), size(px(300.0), px(220.0))),
        )
        .expect("problems panel should float inside the demo dock space");

    let outline_item: open_gpui_docking::DockItemId = "outline".into();
    assert!(
        controller
            .graph()
            .find_item_in_space(&main_space, &outline_item)
            .is_some(),
        "outline panel should be in the restored demo layout"
    );
    controller
        .close_panel(main_space.clone(), outline_item.clone())
        .expect("outline panel should close while its registration remains available");
    controller
        .reopen_panel(main_space, outline_item)
        .expect("outline panel should reopen into its original tab stack");

    let (mut spaces, nodes) = layout_into_raw_parts(controller.graph().export_layout());
    spaces.push(DockLayoutSpace {
        id: CENTRAL_SPACE.into(),
        root: None,
        floatings: Vec::new(),
        central: Some(DockLayoutCentralRegion {
            node: None,
            keep_alive_when_empty: true,
            passthrough_when_empty: true,
        }),
    });
    layout_from_raw_parts(spaces, nodes)
}

macro_rules! configure_demo_builder {
    ($builder:expr, $runtime_panel:expr) => {
        ($builder)
            .try_layout(&restored_demo_layout())
            .expect("demo dock layout should restore")
            .allow_floating(true)
            .allow_platform_viewports(true)
            .allow_dock_class_in_space(SPACE, PRIMARY_DOCK_CLASS)
            .allow_dock_class_in_space(SPACE, SECONDARY_DOCK_CLASS)
            .allow_dock_class_in_space(SPACE, CENTRAL_DOCK_CLASS)
            .allow_dock_class_in_space(SECONDARY_SPACE, SECONDARY_DOCK_CLASS)
            .allow_dock_class_in_space(CENTRAL_SPACE, CENTRAL_DOCK_CLASS)
            .panel("runtime", $runtime_panel)
            .panel(
                "explorer",
                DockPanel::lazy("Explorer", |cx| {
                    cx.new(|_| {
                        DemoPanel::new(
                            "Explorer",
                            "Project structure",
                            0x2563eb,
                            &[
                                "crates/gpui_docking",
                                "examples/docking-native",
                                "docs/plans",
                                "target/doc",
                            ],
                        )
                    })
                    .into()
                })
                .with_dock_class(PRIMARY_DOCK_CLASS),
            )
            .panel(
                "outline",
                DockPanel::lazy("Outline", |cx| {
                    cx.new(|_| {
                        DemoPanel::new(
                            "Outline",
                            "Symbols in the active file",
                            0x0891b2,
                            &[
                                "DockHost",
                                "DockController::builder",
                                "DockPanelPlacement",
                                "Render for DockHost",
                            ],
                        )
                    })
                    .into()
                })
                .with_dock_class(PRIMARY_DOCK_CLASS),
            )
            .panel(
                "workspace",
                DockPanel::lazy("Workspace", |cx| {
                    cx.new(|_| {
                        DemoPanel::new(
                            "Workspace",
                            "Pinned overview",
                            0x0f766e,
                            &[
                                "open-gpui",
                                "gpui_docking",
                                "runtime viewports",
                                "retained panels",
                            ],
                        )
                    })
                    .into()
                })
                .with_dock_class(PRIMARY_DOCK_CLASS),
            )
            .panel(
                "editor",
                DockPanel::lazy("Editor", |cx| {
                    cx.new(|_| {
                        DemoPanel::new(
                            "Editor",
                            "Active document",
                            0x16a34a,
                            &[
                                "Controller-backed rendering is active.",
                                "Tabs route through resolved drop transactions.",
                                "Splits use normalized graph fractions.",
                                "Registered panel factories stay outside the graph.",
                            ],
                        )
                    })
                    .into()
                })
                .with_dock_class(PRIMARY_DOCK_CLASS),
            )
            .panel(
                "preview",
                DockPanel::lazy("Preview", |cx| {
                    cx.new(|_| {
                        DemoPanel::new(
                            "Preview",
                            "Rendered layout notes",
                            0x9333ea,
                            &[
                                "DockHost observes DockController.",
                                "Tab selection updates graph state.",
                                "Layout round-trips through DockLayout.",
                                "Splitter handles resize panes.",
                                "Tabs can drag/drop between stacks.",
                                "Secondary viewport placement lives in the adapter.",
                            ],
                        )
                    })
                    .into()
                })
                .with_dock_class(SECONDARY_DOCK_CLASS),
            )
            .panel(
                "diff",
                DockPanel::lazy("Diff", |cx| {
                    cx.new(|_| {
                        DemoPanel::new(
                            "Diff",
                            "Secondary stack",
                            0x7c3aed,
                            &[
                                "drop_runtime.rs",
                                "viewport_runtime.rs",
                                "render_tabs.rs",
                                "host_interactions.rs",
                            ],
                        )
                    })
                    .into()
                })
                .with_dock_class(SECONDARY_DOCK_CLASS),
            )
            .panel(
                "terminal",
                DockPanel::lazy("Terminal", |cx| {
                    cx.new(|_| {
                        DemoPanel::new(
                            "Terminal",
                            "Command output",
                            0xea580c,
                            &[
                                "$ cargo nextest run -p open-gpui-docking",
                                "Docking public API tests passed",
                                "$ cargo doc -p open-gpui-docking --no-deps",
                                "DockController::builder restores DockLayout.",
                            ],
                        )
                    })
                    .into()
                })
                .with_dock_class(PRIMARY_DOCK_CLASS),
            )
            .panel(
                "problems",
                DockPanel::lazy("Problems", |cx| {
                    cx.new(|_| {
                        DemoPanel::new(
                            "Problems",
                            "Diagnostics",
                            0xdc2626,
                            &[
                                "No active diagnostics.",
                                "Missing panels render placeholders.",
                                "OS windows remain adapter state.",
                            ],
                        )
                    })
                    .into()
                })
                .with_dock_class(PRIMARY_DOCK_CLASS),
            )
            .panel(
                "central-note",
                DockPanel::lazy("Central note", |cx| {
                    cx.new(|_| {
                        DemoPanel::new(
                            "Central note",
                            "Central-only dogfood panel",
                            0x4f46e5,
                            &[
                                "This panel is classed for the empty central viewport.",
                                "Secondary-class panels should reject here.",
                                "Opening content recovers the central region identity.",
                            ],
                        )
                    })
                    .into()
                })
                .with_dock_class(CENTRAL_DOCK_CLASS),
            )
    };
}

#[cfg(test)]
fn unmanaged_runtime_placeholder_panel() -> DockPanel {
    DockPanel::lazy("Runtime", |cx| {
        cx.new(|_| {
            DemoPanel::new(
                "Runtime",
                "Advanced unmanaged runtime fixture",
                0x475569,
                &["Tests replace this placeholder with the live runtime panel."],
            )
        })
        .into()
    })
    .with_dock_class(PRIMARY_DOCK_CLASS)
}

fn managed_surface_runtime_panel(
    surface: Rc<RefCell<Option<DockSurface>>>,
    placement: DockViewportPlacementLayout,
    primary_bounds: Bounds<Pixels>,
    secondary_bounds: Bounds<Pixels>,
    central_bounds: Bounds<Pixels>,
) -> DockPanel {
    DockPanel::lazy("Runtime", move |cx| {
        let surface = surface
            .borrow()
            .as_ref()
            .cloned()
            .expect("the managed DockSurface must exist before its first panel resolves");
        let placement = placement.clone();
        cx.new(|cx| {
            RuntimeStatusPanel::new_managed(
                surface,
                placement,
                primary_bounds,
                secondary_bounds,
                central_bounds,
                cx,
            )
        })
        .into()
    })
    .with_dock_class(PRIMARY_DOCK_CLASS)
}

#[cfg(test)]
fn build_controller() -> DockController {
    configure_demo_builder!(
        DockController::builder(SPACE),
        unmanaged_runtime_placeholder_panel()
    )
    .try_build()
    .expect("demo controller setup should validate")
}

fn build_managed_surface(
    surface_slot: Rc<RefCell<Option<DockSurface>>>,
    placement: DockViewportPlacementLayout,
    primary_bounds: Bounds<Pixels>,
    secondary_bounds: Bounds<Pixels>,
    central_bounds: Bounds<Pixels>,
    cx: &mut App,
) -> DockSurface {
    configure_demo_builder!(
        DockSurface::builder(SPACE),
        managed_surface_runtime_panel(
            surface_slot,
            placement,
            primary_bounds,
            secondary_bounds,
            central_bounds
        )
    )
    .visual_style_resolver(dock_visual_style_resolver())
    .build(cx)
    .expect("managed demo surface setup should validate")
}

fn dogfood_descriptor(title: impl Into<String>, dock_class: &str) -> DockPanelDescriptor {
    DockPanelDescriptor::new(title).with_dock_class(dock_class)
}

fn viewport_window_options(bounds: Bounds<open_gpui::Pixels>) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        ..Default::default()
    }
}

fn saved_viewport_placement(
    primary_bounds: Bounds<open_gpui::Pixels>,
    secondary_bounds: Bounds<open_gpui::Pixels>,
    central_bounds: Bounds<open_gpui::Pixels>,
) -> DockViewportPlacementLayout {
    DockViewportPlacementLayout::new(vec![
        DockViewportPlacement {
            space: SPACE.into(),
            display_id: None,
            window_bounds: Some(DockViewportWindowBounds::from_window_bounds(
                WindowBounds::Windowed(primary_bounds),
            )),
            host_bounds: None,
        },
        DockViewportPlacement {
            space: SECONDARY_SPACE.into(),
            display_id: None,
            window_bounds: Some(DockViewportWindowBounds::from_window_bounds(
                WindowBounds::Windowed(secondary_bounds),
            )),
            host_bounds: None,
        },
        DockViewportPlacement {
            space: CENTRAL_SPACE.into(),
            display_id: None,
            window_bounds: Some(DockViewportWindowBounds::from_window_bounds(
                WindowBounds::Windowed(central_bounds),
            )),
            host_bounds: None,
        },
    ])
}

fn restored_viewport_options(
    placement: &DockViewportPlacementLayout,
    space: impl Into<DockSpaceId>,
    fallback_bounds: Bounds<open_gpui::Pixels>,
) -> WindowOptions {
    let space = space.into();
    let mut options = placement
        .window_options_for_space(&space, viewport_window_options(fallback_bounds))
        .expect("demo viewport placement should produce window options");
    if space.as_str() != SPACE {
        options.focus_on_appearing = false;
    }
    if let Some(titlebar) = options.titlebar.as_mut() {
        titlebar.title = Some(viewport_title(&space).into());
    }
    options
}

fn viewport_title(space: &DockSpaceId) -> &'static str {
    match space.as_str() {
        SPACE => "Docking demo",
        SECONDARY_SPACE => "Docking preview",
        CENTRAL_SPACE => "Empty central dogfood",
        _ => "Docking viewport",
    }
}

fn dock_visual_style_from_theme(theme: &ThemeSnapshot) -> DockVisualStyle {
    let mut palette = DockVisualPalette::built_in();
    palette.surface = theme_color(
        theme,
        semantic::SURFACE,
        ColorState::Default,
        palette.surface,
    );
    palette.surface_muted = theme_color(
        theme,
        semantic::SURFACE_MUTED,
        ColorState::Default,
        palette.surface_muted,
    );
    palette.surface_hovered = theme_color(
        theme,
        semantic::SURFACE_MUTED,
        ColorState::Hover,
        palette.surface_hovered,
    );
    palette.surface_disabled = theme_color(
        theme,
        semantic::SURFACE_MUTED,
        ColorState::Disabled,
        palette.surface_disabled,
    );
    palette.border = theme_color(theme, semantic::BORDER, ColorState::Default, palette.border);
    palette.text = theme_color(theme, semantic::TEXT, ColorState::Default, palette.text);
    palette.text_muted = theme_color(
        theme,
        semantic::TEXT_MUTED,
        ColorState::Default,
        palette.text_muted,
    );
    palette.text_disabled = theme_color(
        theme,
        semantic::TEXT,
        ColorState::Disabled,
        palette.text_disabled,
    );
    palette.accent = theme_color(theme, semantic::ACCENT, ColorState::Default, palette.accent);
    palette.accent_hovered = theme_color(
        theme,
        semantic::ACCENT,
        ColorState::Hover,
        palette.accent_hovered,
    );
    palette.accent_foreground = theme_color(
        theme,
        semantic::ACCENT_FOREGROUND,
        ColorState::Default,
        palette.accent_foreground,
    );
    palette.focus_ring = theme_color(
        theme,
        semantic::FOCUS_RING,
        ColorState::FocusVisible,
        palette.focus_ring,
    );
    palette.destructive = theme_color(
        theme,
        semantic::DESTRUCTIVE,
        ColorState::Default,
        palette.destructive,
    );
    palette.destructive_foreground = theme_color(
        theme,
        semantic::DESTRUCTIVE_FOREGROUND,
        ColorState::Default,
        palette.destructive_foreground,
    );
    let shadow = theme_color(
        theme,
        semantic::OVERLAY,
        ColorState::Overlay,
        palette.shadow,
    );
    palette.shadow = Rgba {
        a: palette.shadow.a,
        ..shadow
    };
    DockVisualStyle::from_palette(palette)
}

fn theme_color(theme: &ThemeSnapshot, token: TokenKey, state: ColorState, fallback: Rgba) -> Rgba {
    theme.color_rgb(token, state).map(rgb).unwrap_or(fallback)
}

fn dock_visual_style_resolver() -> DockVisualStyleResolver {
    DockVisualStyleResolver::new(|window, cx| {
        dock_visual_style_from_theme(&ThemeResolver::current_snapshot(window, cx))
    })
}

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info,open_gpui_docking=info,open_gpui=info"),
    )
    .format_timestamp_millis()
    .init();
    log::info!("{DOCKING_DEBUG_PREFIX} starting docking native example");
    application().run(|cx: &mut App| {
        let primary_bounds = Bounds::centered(None, size(px(920.0), px(640.0)), cx);
        let secondary_bounds = Bounds::new(
            point(
                primary_bounds.origin.x + primary_bounds.size.width + px(24.0),
                primary_bounds.origin.y,
            ),
            size(px(460.0), px(360.0)),
        );
        let central_bounds = Bounds::new(
            point(
                primary_bounds.origin.x + primary_bounds.size.width + px(24.0),
                primary_bounds.origin.y + secondary_bounds.size.height + px(24.0),
            ),
            size(px(460.0), px(220.0)),
        );
        let placement = saved_viewport_placement(primary_bounds, secondary_bounds, central_bounds);
        let surface_slot = Rc::new(RefCell::new(None));
        let surface = build_managed_surface(
            surface_slot.clone(),
            placement.clone(),
            primary_bounds,
            secondary_bounds,
            central_bounds,
            cx,
        );
        surface_slot.replace(Some(surface.clone()));
        let viewports = surface.viewports();
        log::info!("{DOCKING_DEBUG_PREFIX} built managed DockSurface");

        let primary_options = restored_viewport_options(&placement, SPACE, primary_bounds);
        let primary_opened = match surface.open_primary_window(primary_options, cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened,
            outcome => panic!("failed to open managed DockSurface primary: {outcome:?}"),
        };
        let primary_window = primary_opened.window();
        cx.update_window(primary_window, |_, window, cx| {
            set_window_theme(window, cx, LIGHT_THEME_ID)
                .expect("primary light theme should be registered");
        })
        .expect("primary docking viewport should remain open");
        log::info!(
            "{DOCKING_DEBUG_PREFIX} opened managed primary space={} generation={} window_id={:?}",
            SPACE,
            primary_opened.generation(),
            primary_window.window_id()
        );

        let secondary_options =
            restored_viewport_options(&placement, SECONDARY_SPACE, secondary_bounds);
        let secondary_window = match viewports.open(SECONDARY_SPACE, secondary_options, cx) {
            DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("failed to open managed secondary viewport: {outcome:?}"),
        };
        cx.update_window(secondary_window, |_, window, cx| {
            set_window_theme(window, cx, DARK_THEME_ID)
                .expect("secondary dark theme should be registered");
        })
        .expect("secondary docking viewport should remain open");
        log::info!("{DOCKING_DEBUG_PREFIX} opened secondary viewport space={SECONDARY_SPACE}");

        let central_options = restored_viewport_options(&placement, CENTRAL_SPACE, central_bounds);
        let central_window = match viewports.open(CENTRAL_SPACE, central_options, cx) {
            DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("failed to open managed central viewport: {outcome:?}"),
        };
        cx.update_window(central_window, |_, window, cx| {
            set_window_theme(window, cx, HIGH_CONTRAST_THEME_ID)
                .expect("central high-contrast theme should be registered");
        })
        .expect("central docking viewport should remain open");
        log::info!("{DOCKING_DEBUG_PREFIX} opened central viewport space={CENTRAL_SPACE}");

        cx.activate(true);
        log::info!("{DOCKING_DEBUG_PREFIX} application activated");
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui::{Modifiers, MouseButton, TestAppContext, VisualTestContext};
    use open_gpui_docking::{
        DockClassId, DockPolicyError, DockSurfaceWindowSessionPhase,
        advanced::{
            DockViewportCoordinateStatusRecord, DockViewportInputStatus,
            DockViewportPlatformRequestStatus, DockViewportRouteStatus,
        },
        model::{
            DockActionApplyError, DockActionOutcome, DockGraph, DockNode, DockNodeId, DropZone,
            SplitAxis,
        },
        runtime::DockHost,
    };

    fn item(id: &str) -> DockItemId {
        DockItemId::from(id)
    }

    #[test]
    fn theme_adapter_produces_distinct_complete_dock_styles() {
        let light = dock_visual_style_from_theme(&ThemeSnapshot::light());
        let dark = dock_visual_style_from_theme(&ThemeSnapshot::dark());
        let high_contrast = dock_visual_style_from_theme(&ThemeSnapshot::high_contrast());

        assert_ne!(light, dark);
        assert_ne!(dark, high_contrast);
        assert_ne!(light, high_contrast);
        assert_eq!(
            high_contrast.host.background,
            ThemeSnapshot::high_contrast()
                .color_rgb(semantic::SURFACE_MUTED, ColorState::Default)
                .map(rgb)
                .expect("built-in high-contrast theme should define muted surface")
        );
    }

    fn tabs_items(graph: &DockGraph, tabs: DockNodeId) -> (Vec<DockItemId>, usize) {
        let DockNode::Tabs { items, selected } = graph.node(tabs).expect("tabs node should exist")
        else {
            panic!("node should be tabs");
        };
        let selected_index = selected
            .as_ref()
            .and_then(|selected| items.iter().position(|item| item == selected))
            .expect("tabs node should select one of its items");
        (items.clone(), selected_index)
    }

    fn tab_selector(space: &str, tabs: DockNodeId, item: &str) -> String {
        format!("dock:{space}:tabs:{}:tab:{item}", tabs.as_u64())
    }

    fn tabs_selector(space: &str, tabs: DockNodeId) -> String {
        format!("dock:{space}:tabs:{}", tabs.as_u64())
    }

    fn floating_handle_selector(space: &str, floating: DockNodeId) -> String {
        format!("dock:{space}:floating:{}:handle", floating.as_u64())
    }

    fn drop_preview_selector(space: &str) -> String {
        format!("dock:{space}:drop-preview")
    }

    fn drop_guide_selector(space: &str, tabs: DockNodeId, zone: DropZone) -> String {
        format!("dock:{space}:drop-guide:inner:{}:{zone:?}", tabs.as_u64())
    }

    fn debug_bounds(cx: &mut VisualTestContext, selector: impl Into<String>) -> Bounds<Pixels> {
        let selector: &'static str = Box::leak(selector.into().into_boxed_str());
        cx.debug_bounds(selector)
            .unwrap_or_else(|| panic!("debug selector {selector} should have bounds"))
    }

    fn axis_for_zone(zone: DropZone) -> SplitAxis {
        match zone {
            DropZone::Left | DropZone::Right => SplitAxis::Horizontal,
            DropZone::Top | DropZone::Bottom => SplitAxis::Vertical,
            DropZone::Center => unreachable!("center does not create a split"),
        }
    }

    fn simulate_cross_window_left_drag(
        source: &mut VisualTestContext,
        target: &mut VisualTestContext,
        start: open_gpui::Point<Pixels>,
        end: open_gpui::Point<Pixels>,
    ) {
        let threshold = point(start.x + px(24.0), start.y);
        begin_left_drag(source, start, threshold);
        continue_cross_window_left_drag(target, end);
        end_cross_window_left_drag(target, end);
    }

    fn begin_left_drag(
        source: &mut VisualTestContext,
        start: open_gpui::Point<Pixels>,
        threshold: open_gpui::Point<Pixels>,
    ) {
        source.set_platform_hovered_window(Some(source.window_handle()));
        source.update(|window, _| window.activate_window());
        source.run_until_parked();
        source.simulate_mouse_down(start, MouseButton::Left, Modifiers::none());
        source.simulate_mouse_move(threshold, MouseButton::Left, Modifiers::none());
        assert!(
            source.update(|_, cx| cx.has_active_drag()),
            "cross-window dogfood drag should cross GPUI's activation and movement threshold"
        );
    }

    fn continue_cross_window_left_drag(
        target: &mut VisualTestContext,
        position: open_gpui::Point<Pixels>,
    ) {
        target.set_platform_hovered_window(Some(target.window_handle()));
        target.simulate_mouse_move(position, MouseButton::Left, Modifiers::none());
    }

    fn end_cross_window_left_drag(
        target: &mut VisualTestContext,
        position: open_gpui::Point<Pixels>,
    ) {
        target.simulate_mouse_up(position, MouseButton::Left, Modifiers::none());
        target.set_platform_hovered_window(None);
    }

    fn open_dogfood_viewport(
        cx: &mut TestAppContext,
        runtime: &DockViewportRuntimeHandle,
        space: &str,
        bounds: Bounds<Pixels>,
    ) -> (Entity<DockHost>, VisualTestContext) {
        let opened = cx
            .update(|app| runtime.open_viewport(space, viewport_window_options(bounds), app))
            .expect("dogfood viewport should open");
        let window = opened
            .window()
            .downcast::<DockHost>()
            .expect("dogfood viewport should render DockHost");
        let host = window
            .root(cx)
            .expect("dogfood viewport should expose DockHost root");
        cx.run_until_parked();
        let visual = VisualTestContext::from_window(opened.window(), cx);
        (host, visual)
    }

    fn attach_runtime_status_panel(
        cx: &mut TestAppContext,
        controller: &Entity<DockController>,
        runtime: &DockViewportRuntimeHandle,
    ) -> Entity<RuntimeStatusPanel> {
        let primary_bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(920.0), px(640.0)));
        let secondary_bounds = Bounds::new(point(px(944.0), px(0.0)), size(px(460.0), px(360.0)));
        let central_bounds = Bounds::new(point(px(944.0), px(384.0)), size(px(460.0), px(220.0)));
        let placement = saved_viewport_placement(primary_bounds, secondary_bounds, central_bounds);
        let panel = cx.new(|cx| {
            RuntimeStatusPanel::new(
                runtime.clone(),
                controller.clone(),
                placement,
                primary_bounds,
                secondary_bounds,
                central_bounds,
                cx,
            )
        });
        controller.update(cx, |controller, _| {
            controller
                .attach_panel_view("runtime", panel.clone())
                .expect("runtime panel descriptor should exist");
            controller
                .select_item_in_space("runtime")
                .expect("runtime panel should be selectable in the primary space");
        });
        panel
    }

    #[test]
    fn restored_layout_exposes_native_dogfood_spaces() {
        let layout = restored_demo_layout();
        let graph = DockGraph::import_layout(&layout).expect("demo layout should import");
        let primary = DockSpaceId::from(SPACE);
        let secondary = DockSpaceId::from(SECONDARY_SPACE);
        let central = DockSpaceId::from(CENTRAL_SPACE);

        assert!(graph.root(&primary).is_some());
        assert!(graph.root(&secondary).is_some());
        assert_eq!(graph.root(&central), None);

        let preview = item("preview");
        let diff = item("diff");
        let (secondary_tabs, preview_index) = graph
            .find_item_in_space(&secondary, &preview)
            .expect("preview should start in the secondary space");
        let (diff_tabs, diff_index) = graph
            .find_item_in_space(&secondary, &diff)
            .expect("diff should start in the secondary space");
        assert_eq!(secondary_tabs, diff_tabs);
        assert_eq!(preview_index, 0);
        assert_eq!(diff_index, 1);
        let DockNode::Tabs { items, .. } = graph
            .node(secondary_tabs)
            .expect("secondary stack should exist")
        else {
            panic!("secondary dogfood node should be tabs");
        };
        assert_eq!(items.as_slice(), &[preview, diff]);

        let problems = item("problems");
        let (problem_tabs, _) = graph
            .find_item_in_space(&primary, &problems)
            .expect("problems should start in an in-window floating stack");
        let problem_root = graph
            .root_for_node_in_space(&primary, problem_tabs)
            .expect("problems stack should have a root in the primary space");
        assert!(
            graph
                .floating_containers(&primary)
                .iter()
                .any(|floating| floating.node == problem_root),
            "problems should be reachable through a floating container"
        );

        let central_region = graph
            .central_region(&central)
            .expect("empty central dogfood space should keep central metadata");
        assert_eq!(central_region.node, None);
        assert!(central_region.keep_alive_when_empty);
        assert!(central_region.passthrough_when_empty);
    }

    #[open_gpui::test]
    fn managed_example_primary_close_converges_without_app_quit(cx: &mut TestAppContext) {
        cx.update(|cx| cx.set_quit_mode(open_gpui::QuitMode::Explicit));
        cx.set_platform_viewport_windows(true);
        let surface_slot = Rc::new(RefCell::new(None));
        let (surface, anchor, dependent) = cx.update(|cx| {
            let primary_bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(360.0), px(240.0)));
            let secondary_bounds =
                Bounds::new(point(px(400.0), px(0.0)), size(px(320.0), px(240.0)));
            let central_bounds =
                Bounds::new(point(px(400.0), px(280.0)), size(px(320.0), px(200.0)));
            let placement =
                saved_viewport_placement(primary_bounds, secondary_bounds, central_bounds);
            let surface = build_managed_surface(
                surface_slot.clone(),
                placement,
                primary_bounds,
                secondary_bounds,
                central_bounds,
                cx,
            );
            surface_slot.replace(Some(surface.clone()));
            let anchor = match surface.open_primary_window(WindowOptions::default(), cx) {
                DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
                outcome => panic!("managed primary should open, got {outcome:?}"),
            };
            let dependent = match surface.viewports().open(
                SECONDARY_SPACE,
                viewport_window_options(Bounds::new(
                    point(px(400.0), px(0.0)),
                    size(px(320.0), px(240.0)),
                )),
                cx,
            ) {
                DockSurfaceViewportOpenOutcome::Opened(opened) => opened.window(),
                outcome => panic!("managed dependent should open, got {outcome:?}"),
            };
            (surface, anchor, dependent)
        });

        assert!(!cx.simulate_window_close(anchor));
        cx.run_until_parked();

        assert!(!cx.windows().contains(&dependent));
        assert!(!cx.windows().contains(&anchor));
        assert!(
            !cx.did_quit(),
            "DockSurface teardown must not call App::quit"
        );
        assert_eq!(
            cx.update(|cx| surface.window_session_status(cx).phase()),
            DockSurfaceWindowSessionPhase::Closed
        );
    }

    #[test]
    fn built_controller_preserves_dogfood_panel_policy() {
        let controller = build_controller();
        let workspace = controller
            .panels()
            .descriptor(&item("workspace"))
            .expect("workspace descriptor should be registered");
        assert!(
            workspace.is_closable(),
            "demo default should let the primary window close; Prevent policy remains available in the runtime panel"
        );
        assert_eq!(
            workspace.dock_class(),
            Some(&DockClassId::from(PRIMARY_DOCK_CLASS))
        );

        for id in ["preview", "diff", "runtime", "problems", "central-note"] {
            assert!(
                controller.panels().descriptor(&item(id)).is_some(),
                "{id} descriptor should be registered for native dogfood"
            );
        }
        assert_eq!(
            controller
                .panels()
                .descriptor(&item("preview"))
                .and_then(|descriptor| descriptor.dock_class()),
            Some(&DockClassId::from(SECONDARY_DOCK_CLASS))
        );
        assert!(controller.policy().allows_dock_class_in_space(
            &DockSpaceId::from(SECONDARY_SPACE),
            Some(&DockClassId::from(SECONDARY_DOCK_CLASS)),
        ));
        assert!(!controller.policy().allows_dock_class_in_space(
            &DockSpaceId::from(CENTRAL_SPACE),
            Some(&DockClassId::from(SECONDARY_DOCK_CLASS)),
        ));
    }

    #[open_gpui::test]
    fn default_runtime_policy_allows_primary_dogfood_window_close(cx: &mut TestAppContext) {
        let controller = cx.new(|_| build_controller());
        let runtime = DockViewportRuntimeHandle::new(controller);
        let (_primary_host, mut primary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SPACE,
            Bounds::new(point(px(0.0), px(0.0)), size(px(920.0), px(640.0))),
        );

        assert_eq!(
            runtime.close_policy(),
            DockViewportClosePolicy::RetainLayout
        );
        assert!(
            primary_visual.simulate_close(),
            "default dogfood close policy should not veto the primary window"
        );
    }

    #[test]
    fn dogfood_restore_controls_reopen_registered_panels() {
        let mut controller = build_controller();
        let primary = DockSpaceId::from(SPACE);
        let secondary = DockSpaceId::from(SECONDARY_SPACE);
        let preview = item("preview");
        let diff = item("diff");
        let outline = item("outline");

        assert_eq!(
            controller
                .close_item(secondary.clone(), preview.clone())
                .expect("preview should close from secondary dogfood stack"),
            DockActionOutcome::Changed
        );
        assert_eq!(
            controller
                .close_item(secondary.clone(), diff.clone())
                .expect("diff should close from secondary dogfood stack"),
            DockActionOutcome::Changed
        );
        assert!(
            controller
                .graph()
                .find_item_in_space(&secondary, &preview)
                .is_none()
        );
        assert!(
            controller
                .graph()
                .find_item_in_space(&secondary, &diff)
                .is_none()
        );

        assert_eq!(
            restore_secondary_panels(&mut controller),
            "opened preview: Changed via Explicit; opened diff: Changed via Explicit"
        );
        let (preview_tabs, preview_index) = controller
            .graph()
            .find_item_in_space(&secondary, &preview)
            .expect("preview should reopen into secondary dogfood stack");
        let (diff_tabs, diff_index) = controller
            .graph()
            .find_item_in_space(&secondary, &diff)
            .expect("diff should reopen into secondary dogfood stack");
        assert_eq!(preview_tabs, diff_tabs);
        assert_eq!(preview_index, 0);
        assert_eq!(diff_index, 1);

        assert_eq!(
            controller
                .close_item(primary.clone(), outline.clone())
                .expect("outline should close while descriptor remains registered"),
            DockActionOutcome::Changed
        );
        assert!(
            controller
                .graph()
                .find_item_in_space(&primary, &outline)
                .is_none()
        );

        assert_eq!(
            restore_outline_panel(&mut controller),
            "opened outline: Changed via LastKnown"
        );
        assert!(
            controller
                .graph()
                .find_item_in_space(&primary, &outline)
                .is_some()
        );
    }

    #[test]
    fn dogfood_class_policy_rejects_secondary_stack_in_central_but_allows_central_note() {
        let mut controller = build_controller();
        let secondary = DockSpaceId::from(SECONDARY_SPACE);
        let central = DockSpaceId::from(CENTRAL_SPACE);
        let preview = item("preview");
        let (secondary_tabs, _) = controller
            .graph()
            .find_item_in_space(&secondary, &preview)
            .expect("preview should start in secondary dogfood space");

        let err = controller
            .float_tabs_in_window(
                secondary,
                secondary_tabs,
                central.clone(),
                Bounds::new(point(px(80.0), px(40.0)), size(px(260.0), px(180.0))),
            )
            .expect_err("secondary-class stack should reject the central-only dogfood space");

        assert_eq!(
            err,
            DockActionApplyError::Policy(DockPolicyError::DockClassRejected {
                space: central.clone(),
                item: preview,
                dock_class: Some(DockClassId::from(SECONDARY_DOCK_CLASS)),
            })
        );
        assert_eq!(
            restore_central_note_panel(&mut controller),
            "opened central note: Changed via Explicit"
        );
        assert!(
            controller
                .graph()
                .find_item_in_space(&central, &item("central-note"))
                .is_some()
        );
        let central_region = controller
            .graph()
            .central_region(&central)
            .expect("central dogfood space should keep central metadata");
        assert!(
            central_region.node.is_some(),
            "opening central-note should recover central identity instead of ordinary root-only state"
        );
    }

    #[test]
    fn dogfood_whole_stack_can_float_and_merge_back_without_reordering() {
        let mut controller = build_controller();
        let primary = DockSpaceId::from(SPACE);
        let secondary = DockSpaceId::from(SECONDARY_SPACE);
        let preview = item("preview");
        let diff = item("diff");
        let editor = item("editor");
        let (secondary_tabs, _) = controller
            .graph()
            .find_item_in_space(&secondary, &preview)
            .expect("secondary dogfood stack should contain preview");
        let (secondary_items, secondary_active) = tabs_items(controller.graph(), secondary_tabs);
        assert_eq!(secondary_items, vec![preview.clone(), diff.clone()]);
        let secondary_active_item = secondary_items[secondary_active].clone();

        assert_eq!(
            controller
                .float_tabs_in_window(
                    secondary.clone(),
                    secondary_tabs,
                    primary.clone(),
                    Bounds::new(point(px(560.0), px(96.0)), size(px(320.0), px(220.0))),
                )
                .expect("secondary stack should float into primary dogfood space"),
            DockActionOutcome::Changed
        );
        assert_eq!(controller.graph().root(&secondary), None);
        assert_eq!(
            controller.graph().floating_containers(&primary).len(),
            2,
            "primary should keep its existing problems floating stack plus the moved secondary stack"
        );
        let moved_floating = controller
            .graph()
            .floating_containers(&primary)
            .iter()
            .find(|floating| {
                controller.graph().collect_items_in_subtree(floating.node)
                    == vec![preview.clone(), diff.clone()]
            })
            .expect("moved secondary stack should be represented as a primary floating container")
            .node;
        let (editor_tabs, _) = controller
            .graph()
            .find_item_in_space(&primary, &editor)
            .expect("editor target stack should stay in primary space");

        assert_eq!(
            controller
                .merge_floating_into(primary.clone(), moved_floating, editor_tabs)
                .expect("moved stack should merge into primary editor tabs"),
            DockActionOutcome::Changed
        );
        assert!(
            controller
                .graph()
                .floating_containers(&primary)
                .iter()
                .all(|floating| floating.node != moved_floating)
        );
        let (items, active) = tabs_items(controller.graph(), editor_tabs);
        let expected_items = vec![editor, preview, diff];
        let expected_active = expected_items
            .iter()
            .position(|item| item == &secondary_active_item)
            .expect("merged stack should keep its active item");
        assert_eq!(items, expected_items);
        assert_eq!(active, expected_active);
    }

    #[open_gpui::test]
    fn runtime_viewports_accept_rendered_cross_window_tab_drag(cx: &mut TestAppContext) {
        let controller = cx.new(|_| build_controller());
        let runtime = DockViewportRuntimeHandle::new(controller.clone());
        let primary = DockSpaceId::from(SPACE);
        let secondary = DockSpaceId::from(SECONDARY_SPACE);
        let preview = item("preview");
        let editor = item("editor");
        let (secondary_tabs, _) = controller.read_with(cx, |controller, _| {
            controller
                .graph()
                .find_item_in_space(&secondary, &preview)
                .expect("preview should start in secondary dogfood space")
        });
        let (editor_tabs, _) = controller.read_with(cx, |controller, _| {
            controller
                .graph()
                .find_item_in_space(&primary, &editor)
                .expect("editor should start in primary dogfood space")
        });

        let (_primary_host, mut primary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SPACE,
            Bounds::new(point(px(0.0), px(0.0)), size(px(920.0), px(640.0))),
        );
        let (_secondary_host, mut secondary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SECONDARY_SPACE,
            Bounds::new(point(px(944.0), px(0.0)), size(px(460.0), px(360.0))),
        );

        let start = debug_bounds(
            &mut secondary_visual,
            tab_selector(SECONDARY_SPACE, secondary_tabs, "preview"),
        )
        .center();
        let end = debug_bounds(&mut primary_visual, tabs_selector(SPACE, editor_tabs)).center();
        let threshold = point(start.x + px(24.0), start.y);

        begin_left_drag(&mut secondary_visual, start, threshold);
        continue_cross_window_left_drag(&mut primary_visual, end);
        cx.run_until_parked();
        assert!(
            debug_bounds(&mut primary_visual, drop_preview_selector(SPACE))
                .size
                .width
                > px(0.0),
            "primary viewport should render a host-local drop preview during cross-window drag"
        );

        end_cross_window_left_drag(&mut primary_visual, end);
        cx.run_until_parked();

        controller.read_with(cx, |controller, _| {
            assert!(
                controller
                    .graph()
                    .find_item_in_space(&secondary, &preview)
                    .is_none(),
                "preview should leave the secondary viewport after rendered drop"
            );
            let (preview_tabs, preview_index) = controller
                .graph()
                .find_item_in_space(&primary, &preview)
                .expect("preview should dock into the primary editor stack");
            assert_eq!(preview_tabs, editor_tabs);
            assert_eq!(preview_index, 1);
            let (items, active) = tabs_items(controller.graph(), editor_tabs);
            assert_eq!(items, vec![editor, preview]);
            assert_eq!(active, 1);
        });
    }

    #[open_gpui::test]
    fn runtime_viewports_accept_rendered_cross_window_stack_drag(cx: &mut TestAppContext) {
        let controller = cx.new(|_| build_controller());
        let runtime = DockViewportRuntimeHandle::new(controller.clone());
        let primary = DockSpaceId::from(SPACE);
        let secondary = DockSpaceId::from(SECONDARY_SPACE);
        let preview = item("preview");
        let diff = item("diff");
        let editor = item("editor");
        let (secondary_tabs, secondary_items, secondary_active_item) =
            controller.read_with(cx, |controller, _| {
                let (tabs, _) = controller
                    .graph()
                    .find_item_in_space(&secondary, &preview)
                    .expect("preview should start in secondary dogfood space");
                let (items, active) = tabs_items(controller.graph(), tabs);
                let active_item = items[active].clone();
                (tabs, items, active_item)
            });
        assert_eq!(secondary_items, vec![preview.clone(), diff.clone()]);
        let (editor_tabs, _) = controller.read_with(cx, |controller, _| {
            controller
                .graph()
                .find_item_in_space(&primary, &editor)
                .expect("editor should start in primary dogfood space")
        });

        let (_primary_host, mut primary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SPACE,
            Bounds::new(point(px(0.0), px(0.0)), size(px(920.0), px(640.0))),
        );
        let (_secondary_host, mut secondary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SECONDARY_SPACE,
            Bounds::new(point(px(944.0), px(0.0)), size(px(460.0), px(360.0))),
        );

        let source_bounds = debug_bounds(
            &mut secondary_visual,
            tabs_selector(SECONDARY_SPACE, secondary_tabs),
        );
        let start = point(
            source_bounds.origin.x + source_bounds.size.width - px(8.0),
            source_bounds.origin.y + px(12.0),
        );
        let end = debug_bounds(&mut primary_visual, tabs_selector(SPACE, editor_tabs)).center();

        simulate_cross_window_left_drag(&mut secondary_visual, &mut primary_visual, start, end);
        cx.run_until_parked();

        controller.read_with(cx, |controller, _| {
            assert_eq!(
                controller.graph().root(&secondary),
                None,
                "whole-stack drag should empty the secondary viewport root"
            );
            let (items, active) = tabs_items(controller.graph(), editor_tabs);
            let expected_items = vec![editor, preview, diff];
            let expected_active = expected_items
                .iter()
                .position(|item| item == &secondary_active_item)
                .expect("merged stack should keep its active item");
            assert_eq!(items, expected_items);
            assert_eq!(active, expected_active);
        });
    }

    #[open_gpui::test]
    fn runtime_viewports_accept_rendered_cross_window_tab_drag_into_primary_editor_left_edge(
        cx: &mut TestAppContext,
    ) {
        let controller = cx.new(|_| build_controller());
        let runtime = DockViewportRuntimeHandle::new(controller.clone());
        let primary = DockSpaceId::from(SPACE);
        let secondary = DockSpaceId::from(SECONDARY_SPACE);
        let preview = item("preview");
        let diff = item("diff");
        let editor = item("editor");
        let (secondary_tabs, _) = controller.read_with(cx, |controller, _| {
            controller
                .graph()
                .find_item_in_space(&secondary, &preview)
                .expect("preview should start in secondary dogfood space")
        });
        let (editor_tabs, _) = controller.read_with(cx, |controller, _| {
            controller
                .graph()
                .find_item_in_space(&primary, &editor)
                .expect("editor should start in primary dogfood space")
        });

        let (_primary_host, mut primary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SPACE,
            Bounds::new(point(px(0.0), px(0.0)), size(px(920.0), px(640.0))),
        );
        let (_secondary_host, mut secondary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SECONDARY_SPACE,
            Bounds::new(point(px(944.0), px(0.0)), size(px(460.0), px(360.0))),
        );

        let start = debug_bounds(
            &mut secondary_visual,
            tab_selector(SECONDARY_SPACE, secondary_tabs, "preview"),
        )
        .center();
        let editor_bounds = debug_bounds(&mut primary_visual, tabs_selector(SPACE, editor_tabs));
        let editor_hover = editor_bounds.center();
        let threshold = point(start.x + px(24.0), start.y);

        begin_left_drag(&mut secondary_visual, start, threshold);
        continue_cross_window_left_drag(&mut primary_visual, editor_hover);
        cx.run_until_parked();

        let left_edge = debug_bounds(
            &mut primary_visual,
            drop_guide_selector(SPACE, editor_tabs, DropZone::Left),
        )
        .center();
        continue_cross_window_left_drag(&mut primary_visual, left_edge);
        cx.run_until_parked();
        assert!(
            debug_bounds(&mut primary_visual, drop_preview_selector(SPACE))
                .size
                .width
                > px(0.0),
            "primary editor stack should render a left-edge split preview"
        );

        end_cross_window_left_drag(&mut primary_visual, left_edge);
        cx.run_until_parked();

        controller.read_with(cx, |controller, _| {
            assert!(
                controller
                    .graph()
                    .find_item_in_space(&secondary, &preview)
                    .is_none(),
                "preview should leave the secondary viewport after left-edge drop"
            );
            assert!(
                controller
                    .graph()
                    .find_item_in_space(&secondary, &diff)
                    .is_some(),
                "diff should remain in the secondary viewport after moving only preview"
            );
            let (preview_tabs, preview_index) = controller
                .graph()
                .find_item_in_space(&primary, &preview)
                .expect("preview should dock into the primary editor area");
            assert_ne!(
                preview_tabs, editor_tabs,
                "left-edge drop must split the editor stack instead of center-merging"
            );
            assert_eq!(preview_index, 0);
            assert_eq!(
                controller.graph().collect_items_in_subtree(preview_tabs),
                vec![preview.clone()],
                "left split child should contain only the moved preview tab"
            );
            assert_eq!(
                controller.graph().collect_items_in_subtree(editor_tabs),
                vec![editor],
                "original editor stack should remain intact on the right"
            );
        });
    }

    #[open_gpui::test]
    fn runtime_viewports_accept_rendered_cross_window_tab_drag_into_primary_floating_title_bar(
        cx: &mut TestAppContext,
    ) {
        let controller = cx.new(|_| build_controller());
        let runtime = DockViewportRuntimeHandle::new(controller.clone());
        let primary = DockSpaceId::from(SPACE);
        let secondary = DockSpaceId::from(SECONDARY_SPACE);
        let preview = item("preview");
        let problems = item("problems");
        let (secondary_tabs, _) = controller.read_with(cx, |controller, _| {
            controller
                .graph()
                .find_item_in_space(&secondary, &preview)
                .expect("preview should start in secondary dogfood space")
        });
        let (problem_tabs, problem_floating) = controller.read_with(cx, |controller, _| {
            let (tabs, _) = controller
                .graph()
                .find_item_in_space(&primary, &problems)
                .expect("problems should start in the primary floating stack");
            let floating = controller
                .graph()
                .root_for_node_in_space(&primary, tabs)
                .expect("primary problems stack should have a floating root");
            (tabs, floating)
        });

        let (_primary_host, mut primary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SPACE,
            Bounds::new(point(px(0.0), px(0.0)), size(px(920.0), px(640.0))),
        );
        let (_secondary_host, mut secondary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SECONDARY_SPACE,
            Bounds::new(point(px(944.0), px(0.0)), size(px(460.0), px(360.0))),
        );

        let start = debug_bounds(
            &mut secondary_visual,
            tab_selector(SECONDARY_SPACE, secondary_tabs, "preview"),
        )
        .center();
        let end = debug_bounds(
            &mut primary_visual,
            floating_handle_selector(SPACE, problem_floating),
        )
        .center();
        let threshold = point(start.x + px(24.0), start.y);

        begin_left_drag(&mut secondary_visual, start, threshold);
        continue_cross_window_left_drag(&mut primary_visual, end);
        cx.run_until_parked();

        assert!(
            debug_bounds(&mut primary_visual, drop_preview_selector(SPACE))
                .size
                .width
                > px(0.0),
            "primary viewport should render a drop preview on the floating title bar"
        );

        end_cross_window_left_drag(&mut primary_visual, end);
        cx.run_until_parked();

        controller.read_with(cx, |controller, _| {
            assert!(
                controller
                    .graph()
                    .find_item_in_space(&secondary, &preview)
                    .is_none(),
                "preview should leave the secondary viewport after floating-title-bar drop"
            );
            let (preview_tabs, preview_index) = controller
                .graph()
                .find_item_in_space(&primary, &preview)
                .expect("preview should dock into the primary floating stack");
            assert_eq!(preview_tabs, problem_tabs);
            assert_eq!(preview_index, 1);
            let (items, active) = tabs_items(controller.graph(), problem_tabs);
            assert_eq!(items, vec![problems, preview]);
            assert_eq!(active, 1);
            assert_eq!(
                controller.graph().floating_containers(&primary).len(),
                1,
                "primary should keep a single floating container after merging into the title bar"
            );
        });
    }

    fn assert_cross_window_tab_drag_into_primary_floating_stack_guide(
        cx: &mut TestAppContext,
        zone: DropZone,
    ) {
        let controller = cx.new(|_| build_controller());
        let runtime = DockViewportRuntimeHandle::new(controller.clone());
        let primary = DockSpaceId::from(SPACE);
        let secondary = DockSpaceId::from(SECONDARY_SPACE);
        let preview = item("preview");
        let diff = item("diff");
        let problems = item("problems");
        let (secondary_tabs, _) = controller.read_with(cx, |controller, _| {
            controller
                .graph()
                .find_item_in_space(&secondary, &preview)
                .expect("preview should start in secondary dogfood space")
        });
        let (problem_tabs, problem_floating) = controller.read_with(cx, |controller, _| {
            let (tabs, _) = controller
                .graph()
                .find_item_in_space(&primary, &problems)
                .expect("problems should start in the primary floating stack");
            let floating = controller
                .graph()
                .root_for_node_in_space(&primary, tabs)
                .expect("primary problems stack should have a floating root");
            (tabs, floating)
        });

        let (_primary_host, mut primary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SPACE,
            Bounds::new(point(px(0.0), px(0.0)), size(px(920.0), px(640.0))),
        );
        let (_secondary_host, mut secondary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SECONDARY_SPACE,
            Bounds::new(point(px(944.0), px(0.0)), size(px(460.0), px(360.0))),
        );

        let start = debug_bounds(
            &mut secondary_visual,
            tab_selector(SECONDARY_SPACE, secondary_tabs, "preview"),
        )
        .center();
        let target_bounds = debug_bounds(&mut primary_visual, tabs_selector(SPACE, problem_tabs));
        let target_hover = target_bounds.center();
        let threshold = point(start.x + px(24.0), start.y);

        begin_left_drag(&mut secondary_visual, start, threshold);
        continue_cross_window_left_drag(&mut primary_visual, target_hover);
        cx.run_until_parked();
        let end = debug_bounds(
            &mut primary_visual,
            drop_guide_selector(SPACE, problem_tabs, zone),
        )
        .center();
        continue_cross_window_left_drag(&mut primary_visual, end);
        cx.run_until_parked();

        assert!(
            debug_bounds(&mut primary_visual, drop_preview_selector(SPACE))
                .size
                .width
                > px(0.0),
            "primary viewport should render a {zone:?} drop preview inside the floating stack"
        );

        end_cross_window_left_drag(&mut primary_visual, end);
        cx.run_until_parked();

        controller.read_with(cx, |controller, _| {
            assert!(
                controller
                    .graph()
                    .find_item_in_space(&secondary, &preview)
                    .is_none(),
                "preview should leave the secondary viewport after a {zone:?} floating-stack drop"
            );
            assert!(
                controller
                    .graph()
                    .find_item_in_space(&secondary, &diff)
                    .is_some(),
                "diff should remain in the secondary viewport after moving only preview"
            );
            assert_eq!(
                controller.graph().floating_containers(&primary).len(),
                1,
                "primary should keep one floating container after a {zone:?} drop"
            );

            if zone == DropZone::Center {
                let (preview_tabs, preview_index) = controller
                    .graph()
                    .find_item_in_space(&primary, &preview)
                    .expect("preview should merge into the primary floating stack");
                assert_eq!(preview_tabs, problem_tabs);
                assert_eq!(preview_index, 1);
                let (items, active) = tabs_items(controller.graph(), problem_tabs);
                assert_eq!(items, vec![problems.clone(), preview.clone()]);
                assert_eq!(active, 1);
                return;
            }

            let DockNode::Floating { child } = controller
                .graph()
                .node(problem_floating)
                .expect("primary floating root should still exist")
            else {
                panic!("primary floating root should remain floating after a {zone:?} drop");
            };
            let DockNode::Split { axis, children, .. } = controller
                .graph()
                .node(*child)
                .expect("primary floating child should become a split")
            else {
                panic!("primary floating child should be split after a {zone:?} drop");
            };
            assert_eq!(*axis, axis_for_zone(zone), "{zone:?}");
            assert_eq!(children.len(), 2, "{zone:?}");
            let (first_expected, second_expected) = match zone {
                DropZone::Left | DropZone::Top => (vec![preview.clone()], vec![problems.clone()]),
                DropZone::Right | DropZone::Bottom => {
                    (vec![problems.clone()], vec![preview.clone()])
                }
                DropZone::Center => unreachable!("center returned earlier"),
            };
            assert_eq!(
                controller.graph().collect_items_in_subtree(children[0]),
                first_expected,
                "{zone:?} should place the expected items in the first floating child"
            );
            assert_eq!(
                controller.graph().collect_items_in_subtree(children[1]),
                second_expected,
                "{zone:?} should place the expected items in the second floating child"
            );
        });
    }

    #[open_gpui::test]
    fn runtime_viewports_accept_rendered_cross_window_tab_drag_into_primary_floating_stack_center(
        cx: &mut TestAppContext,
    ) {
        assert_cross_window_tab_drag_into_primary_floating_stack_guide(cx, DropZone::Center);
    }

    #[open_gpui::test]
    fn runtime_viewports_accept_rendered_cross_window_tab_drag_into_primary_floating_stack_left(
        cx: &mut TestAppContext,
    ) {
        assert_cross_window_tab_drag_into_primary_floating_stack_guide(cx, DropZone::Left);
    }

    #[open_gpui::test]
    fn runtime_viewports_accept_rendered_cross_window_tab_drag_into_primary_floating_stack_right(
        cx: &mut TestAppContext,
    ) {
        assert_cross_window_tab_drag_into_primary_floating_stack_guide(cx, DropZone::Right);
    }

    #[open_gpui::test]
    fn runtime_viewports_accept_rendered_cross_window_tab_drag_into_primary_floating_stack_top(
        cx: &mut TestAppContext,
    ) {
        assert_cross_window_tab_drag_into_primary_floating_stack_guide(cx, DropZone::Top);
    }

    #[open_gpui::test]
    fn runtime_viewports_accept_rendered_cross_window_tab_drag_into_primary_floating_stack_bottom(
        cx: &mut TestAppContext,
    ) {
        assert_cross_window_tab_drag_into_primary_floating_stack_guide(cx, DropZone::Bottom);
    }

    #[open_gpui::test]
    fn runtime_viewports_accept_rendered_cross_window_tab_drag_then_split_primary_floating_stack(
        cx: &mut TestAppContext,
    ) {
        let controller = cx.new(|_| build_controller());
        let runtime = DockViewportRuntimeHandle::new(controller.clone());
        let primary = DockSpaceId::from(SPACE);
        let secondary = DockSpaceId::from(SECONDARY_SPACE);
        let preview = item("preview");
        let diff = item("diff");
        let problems = item("problems");
        let (secondary_tabs, _) = controller.read_with(cx, |controller, _| {
            controller
                .graph()
                .find_item_in_space(&secondary, &preview)
                .expect("preview should start in secondary dogfood space")
        });
        let (_problem_tabs, problem_floating) = controller.read_with(cx, |controller, _| {
            let (tabs, _) = controller
                .graph()
                .find_item_in_space(&primary, &problems)
                .expect("problems should start in the primary floating stack");
            let floating = controller
                .graph()
                .root_for_node_in_space(&primary, tabs)
                .expect("primary problems stack should have a floating root");
            (tabs, floating)
        });

        let (_primary_host, mut primary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SPACE,
            Bounds::new(point(px(0.0), px(0.0)), size(px(920.0), px(640.0))),
        );
        let (_secondary_host, mut secondary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SECONDARY_SPACE,
            Bounds::new(point(px(944.0), px(0.0)), size(px(460.0), px(360.0))),
        );

        let start = debug_bounds(
            &mut secondary_visual,
            tab_selector(SECONDARY_SPACE, secondary_tabs, "preview"),
        )
        .center();
        let end = debug_bounds(
            &mut primary_visual,
            floating_handle_selector(SPACE, problem_floating),
        )
        .center();
        let threshold = point(start.x + px(24.0), start.y);

        begin_left_drag(&mut secondary_visual, start, threshold);
        continue_cross_window_left_drag(&mut primary_visual, end);
        cx.run_until_parked();
        end_cross_window_left_drag(&mut primary_visual, end);
        cx.run_until_parked();

        let merged_problem_tabs = controller.read_with(cx, |controller, _| {
            controller
                .graph()
                .find_item_in_space(&primary, &problems)
                .map(|(tabs, _)| tabs)
                .expect("problems should stay in the primary floating stack after merge")
        });

        let diff_start = debug_bounds(
            &mut secondary_visual,
            tab_selector(SECONDARY_SPACE, secondary_tabs, "diff"),
        )
        .center();
        let diff_end = debug_bounds(
            &mut primary_visual,
            tabs_selector(SPACE, merged_problem_tabs),
        );
        let diff_hover = diff_end.center();
        let diff_threshold = point(diff_start.x + px(24.0), diff_start.y);

        begin_left_drag(&mut secondary_visual, diff_start, diff_threshold);
        continue_cross_window_left_drag(&mut primary_visual, diff_hover);
        cx.run_until_parked();
        let diff_target = debug_bounds(
            &mut primary_visual,
            drop_guide_selector(SPACE, merged_problem_tabs, DropZone::Bottom),
        )
        .center();
        continue_cross_window_left_drag(&mut primary_visual, diff_target);
        cx.run_until_parked();

        assert!(
            debug_bounds(&mut primary_visual, drop_preview_selector(SPACE))
                .size
                .width
                > px(0.0),
            "primary viewport should render a split preview inside the floating stack"
        );

        end_cross_window_left_drag(&mut primary_visual, diff_target);
        cx.run_until_parked();

        controller.read_with(cx, |controller, _| {
            assert!(
                controller
                    .graph()
                    .find_item_in_space(&secondary, &diff)
                    .is_none(),
                "diff should leave the secondary viewport after the nested split"
            );
            assert_eq!(
                controller.graph().root(&secondary),
                None,
                "secondary viewport should become empty after moving its last tab"
            );
            assert_eq!(
                controller.graph().floating_containers(&primary).len(),
                1,
                "primary should still have one floating container after the nested split"
            );
            let DockNode::Floating { child } = controller
                .graph()
                .node(problem_floating)
                .expect("primary floating root should still exist")
            else {
                panic!("primary floating root should remain floating");
            };
            let DockNode::Split { axis, children, .. } = controller
                .graph()
                .node(*child)
                .expect("primary floating child should become a split")
            else {
                panic!("primary floating child should be split after docking diff");
            };
            assert_eq!(*axis, SplitAxis::Vertical);
            assert_eq!(children.len(), 2);
            assert_eq!(
                controller.graph().collect_items_in_subtree(children[0]),
                vec![problems, preview],
                "top child should preserve the original floating stack"
            );
            assert_eq!(
                controller.graph().collect_items_in_subtree(children[1]),
                vec![diff],
                "bottom child should contain the newly docked diff tab"
            );
        });
    }

    #[test]
    fn saved_placement_restores_all_dogfood_viewport_titles() {
        let primary_bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(920.0), px(640.0)));
        let secondary_bounds = Bounds::new(point(px(944.0), px(0.0)), size(px(460.0), px(360.0)));
        let central_bounds = Bounds::new(point(px(944.0), px(384.0)), size(px(460.0), px(220.0)));
        let placement = saved_viewport_placement(primary_bounds, secondary_bounds, central_bounds);
        assert_eq!(placement.viewports.len(), 3);

        assert_viewport_title(&placement, SPACE, primary_bounds, "Docking demo");
        assert_viewport_title(
            &placement,
            SECONDARY_SPACE,
            secondary_bounds,
            "Docking preview",
        );
        assert_viewport_title(
            &placement,
            CENTRAL_SPACE,
            central_bounds,
            "Empty central dogfood",
        );
        assert!(restored_viewport_options(&placement, SPACE, primary_bounds).focus_on_appearing);
        assert!(
            !restored_viewport_options(&placement, SECONDARY_SPACE, secondary_bounds)
                .focus_on_appearing
        );
        assert!(
            !restored_viewport_options(&placement, CENTRAL_SPACE, central_bounds)
                .focus_on_appearing
        );
        let secondary_options =
            restored_viewport_options(&placement, SECONDARY_SPACE, secondary_bounds);
        assert!(secondary_options.activation_policy.accepts_activation);
        assert!(secondary_options.activation_policy.focus_on_click);
    }

    #[test]
    fn docking_devtools_session_refreshes_bounded_runtime_frames() {
        let status_slot = Arc::new(Mutex::new(DockViewportRuntimeStatus::default()));
        let mut session = docking_runtime_devtools_session(Arc::clone(&status_slot));

        let first = session
            .refresh()
            .expect("first docking devtools refresh should succeed");
        assert_eq!(first.generation, 1);

        for generation in 2..=7 {
            status_slot
                .lock()
                .expect("test status lock should not be poisoned")
                .viewport_lifecycle = vec![DockViewportLifecycleRecord {
                space: DockSpaceId::from(SPACE),
                window_id: open_gpui::WindowId::from(generation),
                route_status: DockViewportRouteStatus::RouteReady,
                input_status: DockViewportInputStatus::ReceivesInput,
                platform_request_status: DockViewportPlatformRequestStatus::default(),
                coordinate_status: None,
                facts_generation: generation,
            }];
            let frame = session
                .refresh()
                .expect("docking devtools refresh should keep succeeding");
            assert_eq!(frame.generation, generation);
        }

        assert_eq!(session.history_limit(), 4);
        assert_eq!(session.frames().len(), 4);
        let current = session
            .current_frame()
            .expect("bounded session should retain the latest frame");
        assert!(current.diff_from_previous.is_some());
        assert!(
            current
                .capture
                .targets
                .targets
                .iter()
                .any(|target| target.label == "Docking runtime")
        );
        assert!(
            current
                .capture
                .domains
                .iter()
                .any(|domain| domain.kind.as_label() == "docking")
        );
        let export = session.export();
        assert_eq!(export.frames.len(), 4);
        let serialized = serde_json::to_string(&export).unwrap();
        assert!(serialized.contains("Docking runtime"));
    }

    #[test]
    fn docking_native_headless_artifacts_use_shared_records() {
        let artifacts = docking_native_headless_artifacts();
        let mut sink = open_gpui_devtools::DevtoolsArtifactJsonlSink::new(Vec::new());

        open_gpui_devtools::DevtoolsArtifactSink::write_record(
            &mut sink,
            &artifacts.session_record,
        )
        .expect("session artifact record writes");
        open_gpui_devtools::DevtoolsArtifactSink::write_record(&mut sink, &artifacts.report_record)
            .expect("report artifact record writes");

        let jsonl = String::from_utf8(sink.into_inner()).expect("artifact JSONL is utf8");
        let lines = jsonl.lines().collect::<Vec<_>>();

        assert_eq!(artifacts.session_export.current_generation, Some(2));
        assert_eq!(artifacts.report.source.generation, Some(2));
        assert_eq!(
            artifacts.session_record.metadata.producer_id,
            DOCKING_NATIVE_ARTIFACT_PRODUCER_ID
        );
        assert_eq!(
            artifacts.session_record.metadata.scenario_id.as_deref(),
            Some(DOCKING_NATIVE_ARTIFACT_SCENARIO_ID)
        );
        assert_eq!(artifacts.session_record.metadata.generation, Some(2));
        assert_eq!(artifacts.report_record.metadata.generation, Some(2));
        let session_json =
            serde_json::to_string(&artifacts.session_export).expect("session export serializes");
        assert!(session_json.contains("\"kind\":\"queued\""));
        assert!(session_json.contains("\"outcome\":\"Adjusted\""));
        assert!(session_json.contains("\"kind\":\"pointer-input\""));
        assert!(session_json.contains("\"accepts_pointer_input\":true"));
        assert!(session_json.contains("\"accepts_activation\":true"));
        assert!(session_json.contains("\"focus_on_click\":true"));
        assert!(artifacts.report.findings.iter().any(|finding| {
            finding
                .id
                .contains("docking.platform_viewport_windows.unsupported")
        }));
        assert!(
            artifacts
                .report
                .findings
                .iter()
                .any(|finding| { finding.id.contains("docking.viewport.route_facts.stale") })
        );
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"artifact_kind\":\"session-export\""));
        assert!(lines[1].contains("\"artifact_kind\":\"report\""));
    }

    #[test]
    fn docking_native_headless_fixtures_match_producer_output() {
        let artifacts = docking_native_headless_artifacts();

        assert_devtools_fixture_matches(
            "docking-session.json",
            &serde_json::to_string_pretty(&artifacts.session_export).unwrap(),
        );
        assert_devtools_fixture_matches(
            "docking-report.json",
            &serde_json::to_string_pretty(&artifacts.report).unwrap(),
        );
    }

    #[test]
    #[ignore = "regenerates checked-in DevTools fixtures"]
    fn regenerate_docking_native_headless_fixtures() {
        let artifacts = docking_native_headless_artifacts();
        let fixture_dir = devtools_fixture_dir();

        std::fs::create_dir_all(&fixture_dir).expect("fixture directory can be created");
        std::fs::write(
            fixture_dir.join("docking-session.json"),
            format!(
                "{}\n",
                serde_json::to_string_pretty(&artifacts.session_export).unwrap()
            ),
        )
        .expect("docking session fixture can be written");
        std::fs::write(
            fixture_dir.join("docking-report.json"),
            format!(
                "{}\n",
                serde_json::to_string_pretty(&artifacts.report).unwrap()
            ),
        )
        .expect("docking report fixture can be written");
    }

    #[open_gpui::test]
    fn runtime_status_panel_refreshes_embedded_devtools_outside_render(cx: &mut TestAppContext) {
        let controller = cx.new(|_| build_controller());
        let runtime = DockViewportRuntimeHandle::new(controller.clone());
        let panel = attach_runtime_status_panel(cx, &controller, &runtime);

        let before_render =
            panel.read_with(cx, |panel, _| panel.devtools_panel.current_generation());
        assert_eq!(before_render, Some(1));

        let (_primary_host, _primary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SPACE,
            Bounds::new(point(px(0.0), px(0.0)), size(px(920.0), px(640.0))),
        );

        let after_render =
            panel.read_with(cx, |panel, _| panel.devtools_panel.current_generation());
        assert_eq!(
            after_render, before_render,
            "rendering the runtime panel must not advance the devtools session"
        );

        panel.update(cx, |panel, cx| {
            panel.refresh_devtools_inspector(cx);
            assert_eq!(panel.devtools_panel.current_generation(), Some(2));
            assert_eq!(panel.devtools_panel.refresh_status_label(), "changed");
            assert!(panel.devtools_panel.retained_frames() <= panel.devtools_panel.history_limit());
        });
    }

    #[open_gpui::test]
    fn runtime_status_panel_renders_embedded_devtools_inspector(cx: &mut TestAppContext) {
        let controller = cx.new(|_| build_controller());
        let runtime = DockViewportRuntimeHandle::new(controller.clone());
        let panel = attach_runtime_status_panel(cx, &controller, &runtime);
        let (_primary_host, mut primary_visual) = open_dogfood_viewport(
            cx,
            &runtime,
            SPACE,
            Bounds::new(point(px(0.0), px(0.0)), size(px(920.0), px(640.0))),
        );

        assert!(
            primary_visual
                .debug_bounds("docking-devtools:panel")
                .is_some()
        );
        assert!(
            primary_visual
                .debug_bounds("docking-devtools:refresh")
                .is_some()
        );
        assert!(
            primary_visual
                .debug_bounds("devtools-inspector:docking-devtools-inspector:root")
                .is_some()
        );

        let before = panel.read_with(cx, |panel, _| panel.devtools_panel.current_generation());
        let refresh_bounds = debug_bounds(&mut primary_visual, "docking-devtools:refresh");
        primary_visual.simulate_click(refresh_bounds.center(), Default::default());
        cx.run_until_parked();

        let after = panel.read_with(cx, |panel, _| panel.devtools_panel.current_generation());
        assert!(
            after > before,
            "clicking the rendered refresh control should advance session generation"
        );
        assert!(
            primary_visual
                .debug_bounds("docking-devtools:generation")
                .is_some()
        );
        assert!(
            primary_visual
                .debug_bounds("docking-devtools:frame-history")
                .is_some()
        );
        assert!(
            primary_visual
                .debug_bounds("docking-devtools:diff-state")
                .is_some()
        );
    }

    #[test]
    fn runtime_status_panel_formats_platform_capabilities() {
        let capabilities = DockViewportPlatformCapabilityRecord {
            platform_viewport_windows: true,
            global_window_bounds: true,
            window_stack: false,
            display_work_area: true,
            dpi_scale: false,
            hovered_window_ignores_no_input: true,
        };
        let window_capabilities = PlatformWindowCapabilities {
            creation: PlatformWindowCreationCapabilities {
                focus_on_appearing: WindowCreationSupport::Supported,
                transient_for: WindowCreationSupport::Unsupported,
                initial_presentation_order: WindowInitialPresentationOrder::BeforeVisibility,
            },
            mutations: PlatformWindowMutationCapabilities {
                position: WindowMutationSupport::CreationOnly,
                size: WindowMutationSupport::Live,
                activation_policy: WindowMutationSupport::Live,
                alpha: WindowMutationSupport::CreationOnly,
                ..Default::default()
            },
        };
        let window_profiles = [DockViewportWindowProfileRecord {
            space: DockSpaceId::from("primary"),
            window_id: open_gpui::WindowId::from(7),
            window_kind: open_gpui::WindowKind::Floating,
            capabilities: window_capabilities,
        }];

        assert_eq!(
            route_capability_summary(Some(&capabilities)),
            "platform-windows=yes, bounds=yes, stack=no, hover-through-no-input=yes"
        );
        assert_eq!(
            window_profile_summary(&window_profiles),
            format!(
                "primary#{}(floating): nonactivating-appear=supported, transient-owner=unsupported, first-present=before-visibility, position=creation-only, size=live, windowed=unsupported, maximized=unsupported, fullscreen=unsupported, minimized=unsupported, restore=unsupported, pointer=unsupported, activation-policy=live, alpha=creation-only, topmost=unsupported, taskbar=unsupported",
                open_gpui::WindowId::from(7).as_u64()
            )
        );
        assert_eq!(route_capability_summary(None), "unavailable");
        assert_eq!(window_profile_summary(&[]), "unavailable");
        assert_eq!(route_unavailable_summary(None), "none");
        assert_eq!(
            route_unavailable_summary(Some(
                &DockViewportReleaseUnavailableRecord::BlockedByViewportWindow
            )),
            "blocked-window"
        );
        assert_eq!(
            route_unavailable_summary(Some(
                &DockViewportReleaseUnavailableRecord::PlatformViewportWindowsUnsupported
            )),
            "platform-windows-unsupported"
        );
        assert_eq!(coordinate_status_summary(&[]), "none");
        assert_eq!(
            coordinate_status_summary(&[
                DockViewportLifecycleRecord {
                    space: DockSpaceId::from(SPACE),
                    window_id: open_gpui::WindowId::from(1),
                    route_status: DockViewportRouteStatus::RouteReady,
                    input_status: DockViewportInputStatus::ReceivesInput,
                    platform_request_status: DockViewportPlatformRequestStatus::default(),
                    coordinate_status: Some(DockViewportCoordinateStatusRecord {
                        display_id: None,
                        coordinate_space: DockViewportCoordinateSpaceRecord::GlobalScreen,
                        facts_generation: 7,
                    }),
                    facts_generation: 7,
                },
                DockViewportLifecycleRecord {
                    space: DockSpaceId::from(SECONDARY_SPACE),
                    window_id: open_gpui::WindowId::from(2),
                    route_status: DockViewportRouteStatus::RegisteredNotReady,
                    input_status: DockViewportInputStatus::ReceivesInput,
                    platform_request_status: DockViewportPlatformRequestStatus::default(),
                    coordinate_status: Some(DockViewportCoordinateStatusRecord {
                        display_id: None,
                        coordinate_space: DockViewportCoordinateSpaceRecord::WindowLocal,
                        facts_generation: 8,
                    }),
                    facts_generation: 8,
                },
                DockViewportLifecycleRecord {
                    space: DockSpaceId::from(CENTRAL_SPACE),
                    window_id: open_gpui::WindowId::from(3),
                    route_status: DockViewportRouteStatus::RegisteredNotReady,
                    input_status: DockViewportInputStatus::ReceivesInput,
                    platform_request_status: DockViewportPlatformRequestStatus::default(),
                    coordinate_status: None,
                    facts_generation: 0,
                },
            ]),
            "docking-demo=global@gen7, docking-preview=local@gen8, docking-empty-central=missing"
        );
        assert_eq!(platform_sync_summary(None), "none");
        assert_eq!(
            platform_sync_summary(Some(&DockViewportPlatformSyncRecord {
                window_id: open_gpui::WindowId::from(4),
                dispatches: Vec::new(),
                observations: Vec::new(),
            })),
            "dispatches=0, observations=0"
        );
        assert_eq!(
            preview_proof_summary(),
            "presentation-scene+real-content-reveal+overlay-motion+tab-insertion+retargeting+splitter-motion+zoom-focus+divider-hit-map+corner-drag+a11y+route-cleanup+reduced-motion"
        );
        assert_eq!(
            motion_runtime_proof_summary(),
            "shared-runtime+run-state+scalar-value+scalar-sample+explicit-models+policy-gates+layout-projection+projection-clips+sampled-progress+retargeted-identity+reduced-motion-final-state+high-frequency-bypass"
        );
        assert_eq!(
            placement_restore_summary(Some(&DockViewportRestoreReadinessRecord {
                matched: 2,
                missing: 1,
            })),
            "matched=2, missing=1"
        );
        assert_eq!(placement_restore_summary(None), "unavailable");
        assert_eq!(
            tear_off_placement_summary(Some(&DockViewportTearOffPlacementRecord::Suggested)),
            "suggested"
        );
        assert_eq!(
            tear_off_placement_summary(Some(&DockViewportTearOffPlacementRecord::DragGeometry)),
            "drag-geometry"
        );
        assert_eq!(tear_off_placement_summary(None), "unavailable");
        assert_eq!(capability_flag(true), "yes");
        assert_eq!(capability_flag(false), "no");
    }

    #[test]
    fn runtime_status_panel_exports_devtools_dogfood_capture() {
        let mut status = DockViewportRuntimeStatus::default();
        status.platform_capabilities = Some(DockViewportPlatformCapabilityRecord {
            platform_viewport_windows: false,
            global_window_bounds: true,
            window_stack: false,
            display_work_area: true,
            dpi_scale: true,
            hovered_window_ignores_no_input: false,
        });
        status.viewport_lifecycle.push(DockViewportLifecycleRecord {
            space: DockSpaceId::from(SPACE),
            window_id: open_gpui::WindowId::from(1),
            route_status: DockViewportRouteStatus::RouteReady,
            input_status: DockViewportInputStatus::ReceivesInput,
            platform_request_status: DockViewportPlatformRequestStatus::default(),
            coordinate_status: None,
            facts_generation: 1,
        });

        let capture = docking_runtime_devtools_capture(&status);
        let summary = docking_runtime_devtools_summary(&status);
        let serialized = serde_json::to_string(&capture).unwrap();

        assert_eq!(
            summary,
            "viewports=1, events=0, affordances=0, diagnostics=1"
        );
        assert!(serialized.contains("Docking runtime"));
        assert!(serialized.contains("docking.platform_viewport_windows.unsupported"));
        assert!(
            capture
                .domains
                .iter()
                .any(|domain| domain.kind.as_label() == "docking")
        );
        assert_eq!(capture.diagnostics.len(), 1);
    }

    fn assert_devtools_fixture_matches(name: &str, actual: &str) {
        let expected = std::fs::read_to_string(devtools_fixture_dir().join(name))
            .unwrap_or_else(|error| panic!("failed to read fixture {name}: {error}"));
        assert_eq!(normalize_json(&expected), normalize_json(actual));
    }

    fn devtools_fixture_dir() -> std::path::PathBuf {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .expect("docking-native example is under examples")
            .parent()
            .expect("examples has a workspace parent")
            .join("crates")
            .join("devtools")
            .join("tests")
            .join("fixtures")
    }

    fn normalize_json(value: &str) -> String {
        value.replace("\r\n", "\n").trim_end().to_owned()
    }

    fn assert_viewport_title(
        placement: &DockViewportPlacementLayout,
        space: &str,
        fallback_bounds: Bounds<Pixels>,
        expected: &str,
    ) {
        let options = restored_viewport_options(placement, space, fallback_bounds);
        let title = options
            .titlebar
            .as_ref()
            .and_then(|titlebar| titlebar.title.as_ref())
            .map(ToString::to_string);
        assert_eq!(title.as_deref(), Some(expected));
    }
}
