//! Owned observations of the session and its existing physical owners.

use dockspace::{
    model::{DockspaceSurfaceView, SurfaceId},
    runtime::{NativeSurfaceBinding, NativeWindowBinding},
};
use open_gpui::WindowFrameSubmissionSnapshot;

use super::{DockSurface, host_native, input};

/// A read-only inspection could not borrow the shared host.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DockInspectionError {
    /// An update or native callback currently holds the host's mutable borrow.
    #[error("docking host is busy")]
    Busy,
}

/// An owned, read-only observation of one shared docking workspace.
///
/// This snapshot contains no views, tickets, callbacks or mutation authority.
/// Counts describe the named owner at observation time; they are not a second
/// layout model or proof that pending work has completed.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DockSurfaceInspection {
    /// Number of logical surfaces in the committed session view.
    pub model_surface_count: usize,
    /// Number of open pane occurrences in the committed session view.
    pub open_pane_count: usize,
    /// Number of retained application views, including registered closed panes.
    pub registered_pane_count: usize,
    /// Committed logical surfaces in stable identity order.
    pub surfaces: Vec<DockModelSurfaceInspection>,
    /// Physical mounts in stable GPUI window identity order.
    pub mounts: Vec<DockMountInspection>,
    /// Whether an unaccepted physical candidate holds the semantic update lease.
    pub update_pending: bool,
    /// Whether the session owns an active pointer gesture.
    pub active_gesture: bool,
    /// Whether the session retains unresolved presentation work.
    pub pending_presentation: bool,
    /// Whether the host has presentation observations awaiting settlement.
    pub pending_presentation_reports: bool,
    /// Local pointer inputs retained by the host outside physical candidates.
    pub queued_input_count: usize,
    /// Native effects retained by the unmanaged host.
    pub queued_native_effect_count: usize,
    /// Committed native window bindings retained by the session.
    pub native_window_binding_count: usize,
    /// Whether recorded native input still awaits a successful commit.
    pub pending_native_input: bool,
    /// Whether native creation retains preparation or first-live obligations.
    pub pending_native_create: bool,
    /// Managed native owner facts, absent for an unmanaged workspace.
    pub native: Option<DockNativeInspection>,
    /// Latest retained transport error, without diagnostic redaction.
    pub last_error: Option<String>,
}

/// Counts derived from one committed logical surface, including floating roots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DockModelSurfaceInspection {
    /// Stable logical surface identity.
    pub surface: SurfaceId,
    /// Number of open panes across the main and contained roots.
    pub open_pane_count: usize,
    /// Number of contained floating presentations.
    pub contained_count: usize,
}

/// Facts retained by one physical GPUI mount.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DockMountInspection {
    /// Exact GPUI window identity.
    pub window_id: u64,
    /// Whether the docking element remains attached to this window.
    pub attached: bool,
    /// Current physical candidate serial.
    pub serial: u64,
    /// Whether this mount is waiting for another mount's semantic lease.
    pub waiting_for_update: bool,
    /// Whether this mount's pending candidate owns an unaccepted update.
    pub update_pending: bool,
    /// The pending original ticket's exact accepted/submitted evidence.
    ///
    /// Absence means no ticket is retained by this mount. It does not imply
    /// that a previous frame was never accepted or submitted.
    pub submission: Option<WindowFrameSubmissionSnapshot>,
    /// Original managed callback binding, when this mount has one.
    pub native_binding: Option<NativeWindowBinding>,
    /// Retained logical regions, including vacant slots after model pruning.
    pub regions: Vec<DockRegionInspection>,
}

/// Accepted packets, input bindings and submitted facts for one physical region.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DockRegionInspection {
    /// Logical identity assigned to this physical region.
    pub surface: SurfaceId,
    /// Whether a fresh encoding has been requested.
    pub dirty: bool,
    /// Whether a packet is retained, and whether it deferred program execution.
    pub packet_deferred: Option<bool>,
    /// Serial of the retained input frame, when present.
    pub input_serial: Option<u64>,
    /// Original native placement retained by the input frame, when present.
    pub input_native_binding: Option<NativeSurfaceBinding>,
    /// Whether that exact input binding is still current in the session.
    pub input_binding_current: Option<bool>,
    /// Serial of the last exact presentation reported for this region.
    pub last_presented_serial: Option<u64>,
    /// Whether that presentation matches the current serial and session artifact.
    pub current_presentation: bool,
}

/// Facts read directly from the managed host's existing owners.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DockNativeInspection {
    /// Whether the transport has frozen admission for shutdown.
    pub admission_frozen: bool,
    /// Whether the bridge is retained here rather than checked out for dispatch.
    pub bridge_available: bool,
    /// The same full convergence predicate as `DockSurface::native_is_quiescent`.
    pub quiescent: bool,
    /// Whether the native transport FIFO is empty.
    pub transport_empty: bool,
    /// Application-facing native reports retained by the managed host.
    ///
    /// This excludes reports still privately retained by a checked-out bridge.
    pub retained_report_count: usize,
    /// Committed host reports waiting to be handed to the bridge.
    pub pending_host_report_count: usize,
    /// Callback routes still retained by the managed host.
    pub callback_window_count: usize,
    /// Registered root window identities retained by the managed host.
    pub root_window_count: usize,
    /// Placement bindings waiting for the host's quiescence boundary.
    pub retiring_binding_count: usize,
    /// Roots whose original registration transaction still needs recovery.
    pub pending_root_registration_count: usize,
    /// Original root rollback tickets retained by the managed host.
    pub root_retirement_count: usize,
    /// Retained root rollback tickets without a native terminal observation.
    pub pending_root_terminal_count: usize,
    /// Retained root rollback tickets without GPUI presentation quiescence.
    pub pending_root_quiescence_count: usize,
    /// Whether a native pump has already been scheduled.
    pub pump_scheduled: bool,
    /// Whether the managed host retains a native pointer continuation task.
    pub continuation_pending: bool,
}

impl DockSurface {
    /// Copies diagnostic facts without advancing or consuming any owner.
    ///
    /// This borrows the shared host immutably once. It never begins a frame,
    /// commits, pumps native work, drains reports or acknowledges presentation.
    /// The returned data can be retained after the host changes or is dropped.
    ///
    /// Returns [`DockInspectionError::Busy`] during a mutable host borrow,
    /// including inspection reentered from an application update callback.
    pub fn inspection(&self) -> Result<DockSurfaceInspection, DockInspectionError> {
        let host = self
            .host
            .try_borrow()
            .map_err(|_| DockInspectionError::Busy)?;
        let surfaces: Vec<_> = host
            .session
            .view()
            .surfaces()
            .map(inspect_model_surface)
            .collect();
        let mounts = host
            .mounts
            .iter()
            .map(|(owner, mount)| DockMountInspection {
                window_id: owner.as_u64(),
                attached: mount.attached,
                serial: mount.serial,
                waiting_for_update: mount.waiting_for_update,
                update_pending: mount
                    .pending
                    .as_ref()
                    .is_some_and(|pending| pending.update.is_some()),
                submission: mount
                    .pending
                    .as_ref()
                    .map(|pending| pending.ticket.snapshot()),
                native_binding: host
                    .native
                    .as_ref()
                    .and_then(|native| native.callbacks.get(owner))
                    .map(|callbacks| callbacks.binding()),
                regions: mount
                    .regions
                    .iter()
                    .map(|(surface, region)| {
                        let frame = mount
                            .input_frames
                            .iter()
                            .find(|frame| frame.surface == *surface);
                        let binding = frame.and_then(|frame| frame.native_binding);
                        DockRegionInspection {
                            surface: *surface,
                            dirty: region.dirty,
                            packet_deferred: region
                                .packet
                                .as_ref()
                                .map(|packet| packet.is_deferred()),
                            input_serial: frame.map(|frame| frame.serial),
                            input_native_binding: binding,
                            input_binding_current: binding
                                .map(|binding| host.session.is_current_native_binding(binding)),
                            last_presented_serial: region.presented.map(|(serial, _)| serial),
                            current_presentation: input::is_presented(
                                &host,
                                *owner,
                                *surface,
                                mount.serial,
                            ),
                        }
                    })
                    .collect(),
            })
            .collect();
        Ok(DockSurfaceInspection {
            model_surface_count: surfaces.len(),
            open_pane_count: surfaces.iter().map(|surface| surface.open_pane_count).sum(),
            registered_pane_count: host.panels.len(),
            surfaces,
            mounts,
            update_pending: host.has_unaccepted_update(),
            active_gesture: host.session.has_active_gesture(),
            pending_presentation: host.session.has_pending_presentation(),
            pending_presentation_reports: host.presentation_pending,
            queued_input_count: host.inputs.len(),
            queued_native_effect_count: host.native_effects.len(),
            native_window_binding_count: host.session.native_window_bindings().count(),
            pending_native_input: host.session.has_pending_native_input(),
            pending_native_create: host.session.has_pending_native_create(),
            native: host_native::inspection(&host),
            last_error: host.error.clone(),
        })
    }
}

fn inspect_model_surface(surface: DockspaceSurfaceView<'_>) -> DockModelSurfaceInspection {
    let mut nodes = Vec::new();
    nodes.extend(surface.main_root().and_then(|root| root.content()));
    nodes.extend(
        surface
            .contained()
            .filter_map(|contained| contained.root().and_then(|root| root.content())),
    );
    let mut open_pane_count = 0;
    while let Some(node) = nodes.pop() {
        if let Some(tabs) = node.tabs() {
            open_pane_count += tabs.items().len();
        } else if let Some(split) = node.split() {
            nodes.extend(split.children());
        }
    }
    DockModelSurfaceInspection {
        surface: surface.id(),
        open_pane_count,
        contained_count: surface.contained_count(),
    }
}

#[cfg(test)]
mod tests {
    use dockspace::{
        geometry::LogicalRect,
        model::{
            DockAnchor, DockPlacement, DockspaceAxis, DockspaceContainedLayout, DockspaceLayout,
            DockspaceNode, DockspaceRootLayout, DockspaceSurfaceLayout, FloatingPresentationId,
            ItemId, RootId, SurfaceId,
        },
        policy::DockPolicy,
        runtime::DockspaceSession,
    };
    use open_gpui::{
        App, AppContext as _, Context, Empty, IntoElement, PlatformWindowPresentOutcome, Render,
        RequestFrameOptions, TestAppContext, VisualTestContext, Window,
        WindowFrameSubmissionOutcome, WindowOptions, div, prelude::*, px, size,
    };

    use super::*;
    use crate::adapter::{DockPane, GpuiDockspaceConfig, presentation::PendingSubmission};

    const FIRST: SurfaceId = SurfaceId::new(1);
    const SECOND: SurfaceId = SurfaceId::new(2);

    fn workspace(cx: &mut App) -> DockSurface {
        let split = DockspaceNode::split(
            DockspaceAxis::Horizontal,
            [1, 2].map(|item| (DockspaceNode::tabs([ItemId::new(item)]), 1.0)),
        )
        .unwrap();
        let layout = DockspaceLayout::new([
            DockspaceSurfaceLayout::new(FIRST, DockspaceRootLayout::new(RootId::new(1), split)),
            DockspaceSurfaceLayout::rootless(SECOND).with_contained(DockspaceContainedLayout::new(
                FloatingPresentationId::new(1),
                DockspaceRootLayout::new(RootId::new(2), DockspaceNode::tabs([ItemId::new(3)])),
                LogicalRect::new(10.0, 10.0, 300.0, 200.0).unwrap(),
            )),
        ])
        .unwrap();
        let session = DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap();
        let panes = (1..=4)
            .map(|item| {
                DockPane::new(
                    ItemId::new(item),
                    "Private application title",
                    cx.new(|_| Empty),
                )
            })
            .collect::<Vec<_>>();
        DockSurface::new(session, panes, GpuiDockspaceConfig::default(), cx).unwrap()
    }

    struct Root(DockSurface);

    impl Render for Root {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div().size_full().child(self.0.element(FIRST))
        }
    }

    fn frames(visual: &mut VisualTestContext, dock: &DockSurface) {
        for _ in 0..4 {
            assert!(visual.simulate_frame(RequestFrameOptions {
                require_presentation: true,
                force_render: true,
                ..Default::default()
            }));
            assert!(dock.error().is_none(), "{:?}", dock.error());
        }
    }

    #[open_gpui::test]
    fn inspection_counts_the_committed_model_including_rootless_floating_panes(
        cx: &mut TestAppContext,
    ) {
        let dock = cx.update(workspace);
        let before = {
            let host = dock.host.borrow();
            let version = host.session.version();
            let inspection = dock.inspection().unwrap();
            assert_eq!(inspection, dock.clone().inspection().unwrap());
            assert_eq!(host.session.version(), version);
            inspection
        };
        assert_eq!(before.model_surface_count, 2);
        assert_eq!(before.open_pane_count, 3);
        assert_eq!(before.registered_pane_count, 4);
        assert_eq!(before.surfaces[0].open_pane_count, 2);
        assert_eq!(before.surfaces[1].open_pane_count, 1);
        assert_eq!(before.surfaces[1].contained_count, 1);
        assert!(before.mounts.is_empty());
        assert!(before.native.is_none());
        assert_eq!(before.native_window_binding_count, 0);

        cx.update(|cx| {
            dock.update(
                |frame| {
                    frame.dock_item_current(
                        ItemId::new(3),
                        DockPlacement::Center(DockAnchor::Item(ItemId::new(1))),
                    )?;
                    Ok(())
                },
                cx,
            )
            .unwrap();
        });
        let after = dock.inspection().unwrap();
        assert_eq!(
            after.model_surface_count,
            dock.with_view(|view| view.surfaces().count())
        );
        assert_eq!(after.model_surface_count, 1);
        assert_eq!(after.open_pane_count, 3);
        assert_eq!(after.registered_pane_count, 4);
        assert_eq!(before.surfaces[0].open_pane_count, 2);
        assert_eq!(before.model_surface_count, 2);
    }

    #[open_gpui::test]
    fn inspection_does_not_accept_or_commit_a_suspended_candidate(cx: &mut TestAppContext) {
        cx.defer_next_window_frame_requests();
        let dock = cx.update(workspace);
        let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root(dock.clone()));
        frames(
            &mut VisualTestContext::from_window(window.into(), cx),
            &dock,
        );
        cx.update_window(window.into(), |_, window, cx| {
            let version = dock.host.borrow().session.version();
            let ticket = window.begin_exact_frame_submission(cx).unwrap();
            {
                let mut host = dock.host.borrow_mut();
                let mut frame = host.session.begin_host_frame().unwrap();
                frame.select_item_current(ItemId::new(2)).unwrap();
                let update = frame.suspend();
                let mount = host
                    .mounts
                    .get_mut(&window.window_handle().window_id())
                    .unwrap();
                assert!(mount.pending.is_none());
                mount.pending = Some(PendingSubmission::new(
                    mount.serial,
                    ticket.clone(),
                    Default::default(),
                    update,
                    Vec::new(),
                ));
            }
            let before = dock.inspection().unwrap();
            assert_eq!(before, dock.inspection().unwrap());
            assert!(before.update_pending);
            assert!(before.mounts[0].update_pending);
            assert_eq!(
                before.mounts[0].submission.unwrap().outcome(),
                WindowFrameSubmissionOutcome::Pending
            );
            assert_eq!(
                ticket.snapshot().outcome(),
                WindowFrameSubmissionOutcome::Pending
            );
            assert_eq!(dock.host.borrow().session.version(), version);
            assert!(dock.is_update_pending());
            let pending = dock
                .host
                .borrow_mut()
                .mounts
                .get_mut(&window.window_handle().window_id())
                .unwrap()
                .pending
                .take()
                .unwrap();
            window.retire_exact_frame_submission(&pending.ticket);
            drop(pending);
        })
        .unwrap();
    }

    #[open_gpui::test]
    fn inspection_distinguishes_acceptance_from_exact_submission(cx: &mut TestAppContext) {
        cx.defer_next_window_frame_requests();
        let dock = cx.update(workspace);
        let window = cx.open_window(size(px(800.0), px(600.0)), |_, _| Root(dock.clone()));
        let mut visual = VisualTestContext::from_window(window.into(), cx);
        frames(&mut visual, &dock);
        visual.set_window_present_outcome(window.into(), PlatformWindowPresentOutcome::Deferred);
        visual.update(|_, cx| {
            dock.update(
                |frame| {
                    frame.dock_item_current(
                        ItemId::new(2),
                        DockPlacement::Center(DockAnchor::Item(ItemId::new(1))),
                    )?;
                    Ok(())
                },
                cx,
            )
            .unwrap();
        });
        frames(&mut visual, &dock);
        let accepted = dock.inspection().unwrap();
        assert_eq!(accepted, dock.inspection().unwrap());
        let submission = accepted.mounts[0]
            .submission
            .expect("original pending ticket");
        assert_eq!(submission.outcome(), WindowFrameSubmissionOutcome::Accepted);
        assert!(submission.accepted_frame_generation().is_some());
        assert_eq!(submission.submitted_frame_generation(), None);
        assert!(!accepted.update_pending);
        assert!(!accepted.mounts[0].regions[0].current_presentation);

        visual.set_window_present_outcome(window.into(), PlatformWindowPresentOutcome::Submitted);
        frames(&mut visual, &dock);
        let submitted = dock.inspection().unwrap();
        assert!(submitted.mounts[0].submission.is_none());
        assert!(submitted.mounts[0].regions[0].current_presentation);
        assert_eq!(accepted.mounts[0].submission, Some(submission));
    }

    #[open_gpui::test]
    fn inspection_observes_original_root_terminal_tickets_without_retiring_them(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            let dock = DockSurface::new_managed(
                DockspaceSession::from_layout(DockspaceLayout::empty(), DockPolicy::default())
                    .unwrap(),
                [],
                GpuiDockspaceConfig::default(),
                cx,
            )
            .unwrap();
            let error = dock
                .open_native_root(FIRST, false, WindowOptions::default(), cx)
                .unwrap_err();
            let ticket = error
                .retirement_ticket
                .as_ref()
                .expect("original GPUI rollback ticket");
            let original = ticket.snapshot();
            let before = dock.inspection().unwrap();
            assert_eq!(before, dock.inspection().unwrap());
            let native = before.native.unwrap();
            assert_eq!(native.root_retirement_count, 1);
            assert_eq!(
                native.pending_root_terminal_count,
                usize::from(!original.native_terminal())
            );
            assert_eq!(
                native.pending_root_quiescence_count,
                usize::from(!original.quiesced())
            );
            assert!(!native.quiescent);
            assert!(!dock.native_is_quiescent());
            assert_eq!(
                ticket.snapshot().native_terminal(),
                original.native_terminal()
            );
            assert_eq!(ticket.snapshot().quiesced(), original.quiesced());
        });
    }

    #[open_gpui::test]
    fn inspection_returns_busy_during_an_update_without_advancing_the_session(
        cx: &mut TestAppContext,
    ) {
        let dock = cx.update(workspace);
        cx.update(|cx| {
            dock.update(
                |frame| {
                    let before = frame.view().item(ItemId::new(1)).unwrap().surface();
                    assert_eq!(dock.inspection(), Err(DockInspectionError::Busy));
                    assert_eq!(dock.clone().inspection(), Err(DockInspectionError::Busy));
                    assert_eq!(frame.view().item(ItemId::new(1)).unwrap().surface(), before);
                    frame.dock_item_current(
                        ItemId::new(3),
                        DockPlacement::Center(DockAnchor::Item(ItemId::new(1))),
                    )?;
                    Ok(())
                },
                cx,
            )
            .unwrap();
        });
        assert_eq!(dock.inspection().unwrap().open_pane_count, 3);
        assert_eq!(
            dock.with_view(|view| view.item(ItemId::new(3)).unwrap().surface()),
            FIRST
        );
    }
}
