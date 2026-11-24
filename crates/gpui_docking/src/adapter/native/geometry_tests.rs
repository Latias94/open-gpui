use std::{cell::Cell, rc::Rc};

use dockspace::{
    geometry::LogicalPoint,
    model::{
        DockspaceLayout, DockspaceNode, DockspaceRootLayout, DockspaceSurfaceLayout, ItemId,
        RootId, SurfaceId,
    },
    policy::DockPolicy,
    runtime::{DockspaceReceiverDescriptor, DockspaceSession},
};
use open_gpui::{
    AnyWindowHandle, AppContext as _, Bounds, Context, DevicePixels, DisplayId, Empty, IntoElement,
    PlatformFrameDeliveryMode, Render, RequestFrameOptions, TestAppContext, VisualTestContext,
    Window, WindowVisibility, div, point, prelude::*, px, size,
};

use super::NativeCallbacks;
use crate::adapter::{
    DockPane, DockSurface, GpuiDockspaceConfig, HostState, host_native,
    presentation::PendingSubmission,
};

const SOURCE: SurfaceId = SurfaceId::new(71);
const TARGET: SurfaceId = SurfaceId::new(72);

struct Root {
    dock: DockSurface,
    surface: SurfaceId,
    renders: Rc<Cell<usize>>,
}

impl Render for Root {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        div().size_full().child(self.dock.element(self.surface))
    }
}

struct Fixture {
    dock: DockSurface,
    windows: [AnyWindowHandle; 2],
    renders: [Rc<Cell<usize>>; 2],
}

impl Fixture {
    fn new(cx: &mut TestAppContext) -> Self {
        cx.simulate_display_added(DisplayId::new(2), None);
        let dock = cx.update(|cx| {
            let layout = DockspaceLayout::new([SOURCE, TARGET].map(|surface| {
                DockspaceSurfaceLayout::new(
                    surface,
                    DockspaceRootLayout::new(
                        RootId::new(surface.get()),
                        DockspaceNode::tabs([ItemId::new(surface.get())]),
                    ),
                )
            }))
            .unwrap();
            let session = DockspaceSession::from_layout(layout, DockPolicy::default()).unwrap();
            let panes = [SOURCE, TARGET].map(|surface| {
                DockPane::new(ItemId::new(surface.get()), "Pane", cx.new(|_| Empty))
            });
            DockSurface::new_managed(session, panes, GpuiDockspaceConfig::default(), cx).unwrap()
        });
        let renders = [Rc::new(Cell::new(0)), Rc::new(Cell::new(0))];
        let windows: Vec<_> = [(SOURCE, true), (TARGET, false)]
            .into_iter()
            .zip(&renders)
            .map(|((surface, anchor), renders)| {
                cx.defer_next_window_frame_requests();
                cx.open_window(size(px(640.0), px(480.0)), |window, cx| {
                    dock.bind_native_root(surface, anchor, window, cx).unwrap();
                    Root {
                        dock: dock.clone(),
                        surface,
                        renders: renders.clone(),
                    }
                })
                .into()
            })
            .collect();
        let windows: [AnyWindowHandle; 2] = windows.try_into().unwrap();
        for (window, origin) in windows.into_iter().zip([0, 900]) {
            cx.set_platform_window_physical_client_geometry(
                window,
                Some(Bounds::new(
                    point(DevicePixels(origin), DevicePixels(0)),
                    size(DevicePixels(640), DevicePixels(480)),
                )),
                1.0,
            );
            cx.update_window(window, |_, window, cx| window.bounds_changed(cx))
                .unwrap();
        }
        let fixture = Self {
            dock,
            windows,
            renders,
        };
        for window in windows {
            fixture.present(window, cx);
        }
        cx.run_until_parked();
        assert!(fixture.dock.native_transport().unwrap().is_empty());
        fixture
    }

    fn present(&self, window: AnyWindowHandle, cx: &mut TestAppContext) {
        let mut visual = VisualTestContext::from_window(window, cx);
        for _ in 0..4 {
            assert!(visual.simulate_frame(RequestFrameOptions {
                require_presentation: true,
                force_render: true,
                ..Default::default()
            }));
            assert!(self.dock.error().is_none(), "{:?}", self.dock.error());
        }
    }

    fn target_callbacks(&self) -> NativeCallbacks {
        self.dock
            .native_callbacks_for_window(self.windows[1].window_id())
            .unwrap()
    }

    fn target_receiver(&self) -> DockspaceReceiverDescriptor {
        let host = self.dock.host.borrow();
        let frame = &host.mounts[&self.windows[1].window_id()].input_frames[0];
        frame
            .receivers
            .iter()
            .find_map(|(descriptor, _)| {
                host.session
                    .bind_presented_receiver(descriptor)
                    .map(|_| *descriptor)
            })
            .expect("the original GPUI output must own a presented receiver")
    }

    fn hold_collection(&self, cx: &mut TestAppContext) {
        for window in self.windows {
            cx.set_window_frame_delivery_mode(window, PlatformFrameDeliveryMode::Deferred);
        }
        // TestAppContext also draws dirty windows directly in flush_effects,
        // independently of the platform's deferred frame requests. Keep those
        // draws out of Root and DockElement so prepaint cannot supply the facts;
        // deferred delivery also preserves the previously accepted frame.
        cx.skip_drawing();
        cx.update_window(self.windows[0], |_, window, cx| {
            // Request the ticket before borrowing the host: it can request a frame.
            let ticket = window.begin_exact_frame_submission(cx).unwrap();
            let mut host = self.dock.host.borrow_mut();
            let owner = window.window_handle().window_id();
            assert!(host.mounts[&owner].pending.is_none());
            let HostState {
                session,
                mounts,
                native,
                ..
            } = &mut *host;
            let update =
                host_native::begin_frame(session, mounts, native.as_ref(), Some(window), cx)
                    .unwrap()
                    .suspend();
            let mount = host.mounts.get_mut(&owner).unwrap();
            let packets = mount
                .regions
                .iter()
                .map(|(surface, region)| (*surface, region.packet.clone().unwrap()))
                .collect();
            mount.serial += 1;
            mount.pending = Some(PendingSubmission::new(
                mount.serial,
                ticket,
                packets,
                update,
                Vec::new(),
            ));
        })
        .unwrap();
        assert!(self.dock.is_update_pending());
    }

    fn release_collection(&self, cx: &mut TestAppContext) {
        cx.update_window(self.windows[0], |_, window, _| {
            let pending = {
                let mut host = self.dock.host.borrow_mut();
                host.mounts
                    .get_mut(&window.window_handle().window_id())
                    .unwrap()
                    .pending
                    .take()
                    .expect("the source candidate must retain the collection lease")
            };
            assert!(pending.update.is_some());
            assert!(window.retire_exact_frame_submission(&pending.ticket));
            // This fixture never encoded or accepted an output for the candidate.
            // Dropping its original update rolls back the sole semantic lease.
            drop(pending);
        })
        .unwrap();
        assert!(!self.dock.is_update_pending());
    }

    fn pump(&self, cx: &mut TestAppContext) {
        cx.update(|cx| self.dock.pump_native(cx));
        cx.run_until_parked();
        assert!(self.dock.error().is_none(), "{:?}", self.dock.error());
        assert!(self.dock.native_transport().unwrap().is_empty());
    }

    fn close(self, cx: &mut TestAppContext) {
        cx.update(|cx| self.dock.begin_native_shutdown(cx)).unwrap();
        self.pump(cx);
        assert!(self.dock.native_is_quiescent());
    }
}

#[derive(Clone, Copy, Debug)]
enum Change {
    Bounds,
    Scale,
    Minimize,
    Display,
}

impl Change {
    fn apply(self, window: AnyWindowHandle, changed: bool, cx: &TestAppContext) {
        match self {
            Self::Bounds => cx.simulate_window_resize(
                window,
                if changed {
                    size(px(720.0), px(520.0))
                } else {
                    size(px(640.0), px(480.0))
                },
            ),
            Self::Scale => {
                cx.simulate_window_scale_factor_change(window, if changed { 2.0 } else { 1.0 })
            }
            Self::Minimize => {
                if changed {
                    cx.simulate_window_minimize(window);
                }
            }
            Self::Display => {
                let origin = if changed { -900 } else { 900 };
                cx.set_platform_window_physical_client_geometry(
                    window,
                    Some(Bounds::new(
                        point(DevicePixels(origin), DevicePixels(0)),
                        size(DevicePixels(640), DevicePixels(480)),
                    )),
                    1.0,
                );
                cx.simulate_window_move_to_display(
                    window,
                    DisplayId::new(if changed { 2 } else { 1 }),
                );
            }
        }
    }
}

fn observe_without_target_redraw(cx: &mut TestAppContext, change: Change, round_trip: bool) {
    let fixture = Fixture::new(cx);
    let target = fixture.windows[1];
    let receiver = fixture.target_receiver();
    let callbacks = fixture.target_callbacks();
    let transport = fixture.dock.native_transport().unwrap();
    let original_facts = callbacks.facts().unwrap();
    let original_renders = fixture.renders[1].get();
    let original_frame = cx
        .update_window(target, |_, window, _| {
            window.presentation_facts().frame_accepted_generation
        })
        .unwrap();
    let original_source = fixture.dock.host.borrow().session.presented_surface(SOURCE);
    let original_target = fixture.dock.host.borrow().session.presented_surface(TARGET);
    assert!(original_source.is_some());
    assert!(original_target.is_some());
    let version = fixture.dock.host.borrow().session.version();
    fixture.hold_collection(cx);
    cx.update_window(target, |_, window, cx| window.bounds_changed(cx))
        .unwrap();
    assert!(
        transport.is_empty(),
        "unchanged bounds must not enqueue facts"
    );
    change.apply(target, true, cx);
    assert!(
        !transport.is_empty(),
        "{change:?} must enter transport before the target paints"
    );
    assert_eq!(callbacks.facts(), Some(original_facts));
    {
        let host = fixture.dock.host.borrow();
        assert!(host.session.bind_presented_receiver(&receiver).is_some());
        assert!(
            host.session
                .host_pointer_hit_for_device(
                    TARGET,
                    LogicalPoint::new(320.0, 240.0).unwrap(),
                    dockspace::runtime::PointerDeviceKind::Mouse,
                )
                .is_some()
        );
    }
    if round_trip {
        let queued = transport.inspection().queued_record_count;
        change.apply(target, false, cx);
        assert!(transport.inspection().queued_record_count > queued);
    }
    assert_eq!(fixture.renders[1].get(), original_renders);
    fixture.release_collection(cx);
    fixture.pump(cx);
    let live_facts = cx
        .update_window(target, |_, window, _| {
            callbacks.window_facts(window, || None)
        })
        .unwrap();
    assert_eq!(callbacks.facts(), live_facts);
    if round_trip {
        assert_eq!(callbacks.facts(), Some(original_facts));
    } else {
        assert_ne!(callbacks.facts(), Some(original_facts));
    }
    {
        let host = fixture.dock.host.borrow();
        assert_eq!(host.session.version(), version);
        assert_eq!(host.session.presented_surface(SOURCE), original_source);
        assert!(
            host.session.bind_presented_receiver(&receiver).is_none(),
            "{change:?} must revoke the original presented receiver"
        );
        assert!(
            host.session
                .host_pointer_hit_for_device(
                    TARGET,
                    LogicalPoint::new(320.0, 240.0).unwrap(),
                    dockspace::runtime::PointerDeviceKind::Mouse,
                )
                .is_none()
        );
    }
    assert_eq!(fixture.renders[1].get(), original_renders);
    assert_eq!(
        cx.update_window(target, |_, window, _| {
            window.presentation_facts().frame_accepted_generation
        })
        .unwrap(),
        original_frame
    );
    fixture.hold_collection(cx);
    change.apply(target, !round_trip, cx);
    assert!(
        transport.is_empty(),
        "identical facts must stay deduplicated"
    );
    if !round_trip && !matches!(change, Change::Minimize) {
        change.apply(target, false, cx);
        assert!(!transport.is_empty());
    }
    fixture.release_collection(cx);
    fixture.pump(cx);
    assert!(
        fixture
            .dock
            .host
            .borrow()
            .session
            .bind_presented_receiver(&receiver)
            .is_none()
    );
    assert_eq!(fixture.renders[1].get(), original_renders);
    assert_eq!(
        cx.update_window(target, |_, window, _| {
            window.presentation_facts().frame_accepted_generation
        })
        .unwrap(),
        original_frame
    );
    fixture.close(cx);
}

#[open_gpui::test]
fn bounds_observation_revokes_target_receivers_before_redraw(cx: &mut TestAppContext) {
    observe_without_target_redraw(cx, Change::Bounds, false);
}

#[open_gpui::test]
fn scale_observation_revokes_target_receivers_before_redraw(cx: &mut TestAppContext) {
    observe_without_target_redraw(cx, Change::Scale, false);
}

#[open_gpui::test]
fn minimizing_revokes_target_receivers_before_redraw(cx: &mut TestAppContext) {
    observe_without_target_redraw(cx, Change::Minimize, false);
}

#[open_gpui::test]
fn display_observation_revokes_target_receivers_before_redraw(cx: &mut TestAppContext) {
    observe_without_target_redraw(cx, Change::Display, false);
}

#[open_gpui::test]
fn geometry_round_trips_preserve_event_time_revocation_without_redraw(cx: &mut TestAppContext) {
    for change in [Change::Bounds, Change::Scale, Change::Display] {
        let mut isolated = cx.new_app();
        observe_without_target_redraw(&mut isolated, change, true);
        isolated.quit();
    }
}

#[open_gpui::test]
fn occlusion_preserves_shown_window_receivers_without_redraw(cx: &mut TestAppContext) {
    let fixture = Fixture::new(cx);
    let target = fixture.windows[1];
    let receiver = fixture.target_receiver();
    let callbacks = fixture.target_callbacks();
    let facts = callbacks.facts().unwrap();
    let renders = fixture.renders[1].get();
    fixture.hold_collection(cx);
    cx.simulate_window_visibility_change(target, WindowVisibility::Hidden);
    cx.update_window(target, |_, window, _| {
        assert!(!window.is_visible());
        assert!(window.presentation_facts().native_visible);
        assert!(!window.platform_facts().is_minimized);
    })
    .unwrap();
    assert_eq!(callbacks.facts(), Some(facts));
    assert!(fixture.dock.native_transport().unwrap().is_empty());
    fixture.release_collection(cx);
    fixture.pump(cx);
    assert!(
        fixture
            .dock
            .host
            .borrow()
            .session
            .bind_presented_receiver(&receiver)
            .is_some()
    );
    assert_eq!(fixture.renders[1].get(), renders);
    fixture.close(cx);
}
