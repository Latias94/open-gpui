//! Native drag-capture routing, capture wrapper isolation, and pointer ingress.

use std::{
    cell::RefCell,
    rc::{Rc, Weak},
    sync::Arc,
};

use dockspace::{
    geometry::{PhysicalPoint, ScaleFactor},
    model::SurfaceId,
    runtime::{DockspaceSession, NativeWindowBinding},
};
use open_gpui::{App, AppContext as _, Pixels, Subscription, Window, WindowId, div, prelude::*};

use super::super::{
    HostState, SharedHost,
    native::{GPUI_MOUSE_POINTER, MousePressLane, NativeCallbacks},
};

struct NativeRouter {
    _subscription: Subscription,
}
impl open_gpui::Global for NativeRouter {}

struct NativeDragPayload {
    host: Weak<RefCell<HostState>>,
}

pub(in crate::adapter) fn routes_captured_drag(
    shared: &SharedHost,
    source: WindowId,
    cx: &App,
) -> bool {
    cx.active_native_captured_drag_source_window() == Some(source)
        && cx
            .active_drag_value::<NativeDragPayload>()
            .is_some_and(|payload| payload.host.as_ptr() == Rc::as_ptr(shared))
}

pub(super) fn install_router(cx: &mut App) {
    if cx.has_global::<NativeRouter>() {
        return;
    }
    let subscription = cx.consume_native_captured_drag(
        |event, _, cx| {
            if let Some(shared) = event
                .payload::<NativeDragPayload>()
                .and_then(|payload| payload.host.upgrade())
                && let Some(native) = &shared.borrow().native
                && let Some(callbacks) = native.callbacks.get(&event.source_window())
            {
                callbacks.observe_capture_release(event, cx);
            }
            Arc::new(())
        },
        |event, cx| {
            let Some(shared) = event
                .payload::<NativeDragPayload>()
                .and_then(|payload| payload.host.upgrade())
            else {
                return;
            };
            {
                let mut host = shared.borrow_mut();
                let metadata = host
                    .config
                    .input
                    .pointer_metadata(host.observed_at(), event.modifiers());
                if let Some(native) = &mut host.native {
                    let owner = event.source_window();
                    if let Some(callbacks) = native.callbacks.get(&owner) {
                        // A cancellation's barrier is reserved by GPUI while it
                        // settles the window callback. MouseUp has an earlier locker.
                        if matches!(
                            event.phase(),
                            open_gpui::NativeCapturedDragPhase::Cancelled(_)
                        ) {
                            callbacks.observe_capture_release(&event, cx);
                        }
                        // Real ingress may rearm an unchanged finite challenge.
                        // The callback is queued before scheduling can sample it.
                        native.blocked_continuation = None;
                        native.transport.captured_drag(
                            callbacks.binding(),
                            super::super::native::native_capture_owner(
                                event.capture_owner(),
                                |owner| native.callbacks.get(&owner).map(NativeCallbacks::binding),
                            ),
                            event,
                            GPUI_MOUSE_POINTER,
                            metadata,
                            cx,
                        );
                    }
                }
            }
            super::pump(&shared, cx);
        },
    );
    cx.set_global(NativeRouter {
        _subscription: subscription,
    });
}

pub(in crate::adapter) fn wrap(
    shared: &SharedHost,
    element: open_gpui::AnyElement,
) -> open_gpui::AnyElement {
    let Some(tag) = shared
        .borrow()
        .native
        .as_ref()
        .map(|native| native.wrapper_hitbox_tag)
    else {
        return element;
    };
    let guard_shared = shared.clone();
    div()
        .id(format!(
            "dockspace-native-capture-{:x}",
            Rc::as_ptr(shared) as usize
        ))
        .size_full()
        .hitbox_tag(tag)
        .on_drag(
            NativeDragPayload {
                host: Rc::downgrade(shared),
            },
            |payload, geometry, window, cx| {
                if let Some(shared) = payload.host.upgrade() {
                    start_drag(&shared, geometry, window, cx);
                }
                cx.new(|_| open_gpui::Empty)
            },
        )
        .child(
            div()
                .size_full()
                .hitbox_tag(tag)
                .on_mouse_down(open_gpui::MouseButton::Left, move |_, window, cx| {
                    // Only allow dock-owned chrome/splitter presses to arm outer
                    // native drag capture; pane content presses stay isolated.
                    let allow_capture = {
                        let host = guard_shared.borrow();
                        host.native.as_ref().is_some_and(|native| {
                            native
                                .callbacks
                                .get(&window.window_handle().window_id())
                                .is_some_and(|callbacks| {
                                    matches!(
                                        native.transport.mouse_press_lane(callbacks.binding()),
                                        Some(MousePressLane::Local | MousePressLane::Desktop)
                                    )
                                })
                        })
                    };
                    if !allow_capture {
                        cx.stop_propagation();
                    }
                })
                .child(element),
        )
        .into_any_element()
}

fn start_drag(
    shared: &SharedHost,
    geometry: &open_gpui::DragStartGeometry,
    window: &Window,
    cx: &mut App,
) {
    let consumer = geometry.prepare_native_captured_drag_consumer();
    let host = shared.borrow();
    let Some(native) = &host.native else {
        return;
    };
    let Some(callbacks) = native.callbacks.get(&window.window_handle().window_id()) else {
        return;
    };
    let binding = callbacks.binding();
    let sample = local_sample(&host.session, binding, geometry.window_position(), window);
    let metadata = host
        .config
        .input
        .pointer_metadata(host.observed_at(), Some(window.modifiers()));
    let capture = super::super::native::native_capture_owner(
        geometry.native_drag_start_snapshot().map_or(
            open_gpui::PlatformNativePointerCaptureOwner::Unavailable,
            |snapshot| snapshot.pointer_frame().capture_owner(),
        ),
        |owner| native.callbacks.get(&owner).map(NativeCallbacks::binding),
    );
    let callbacks = callbacks.clone();
    let source = window.window_handle().window_id();
    let transport = native.transport.clone();
    let weak = Rc::downgrade(shared);
    drop(host);
    cx.defer(move |cx| {
        if !consumer.is_active() {
            return;
        }
        callbacks.retain_capture_start(source, &consumer);
        if !matches!(
            transport.mouse_press_lane(binding),
            Some(MousePressLane::Local | MousePressLane::Desktop)
        ) {
            // The initiating press was already settled (e.g. Escape before the
            // drag threshold) or belongs to pane content; release capture now.
            callbacks.cancel_capture(cx);
            if let Some(shared) = weak.upgrade() {
                super::pump(&shared, cx);
            }
            return;
        }
        let Some(sample) = sample else {
            return;
        };
        use dockspace::runtime::{
            NativeLocalPointerLocation, NativePointerEvent, NativePointerInput, NativePointerOwner,
        };
        transport.pointer(
            NativePointerInput::new_local(
                GPUI_MOUSE_POINTER,
                NativePointerEvent::Moved,
                NativeLocalPointerLocation::Exact(sample),
                NativePointerOwner::Native(binding),
                capture,
            )
            .with_metadata(metadata),
            cx,
        );
        if let Some(shared) = weak.upgrade() {
            super::pump(&shared, cx);
        }
    });
}

pub(in crate::adapter) fn queue_mouse_terminal(
    host: &mut HostState,
    owner: WindowId,
    only_content: bool,
    event: dockspace::runtime::SurfacePointerEvent,
    metadata: dockspace::runtime::PointerEventMetadata,
) -> bool {
    use dockspace::runtime::{
        NativeDesktopPointerLocation, NativeDesktopPosition, NativeLocalPointerLocation,
        NativePointerButton, NativePointerCancelReason, NativePointerEvent, NativePointerHover,
        NativePointerInput, NativePointerOwner, SurfacePointerEvent,
    };
    let Some(native) = host
        .native
        .as_mut()
        .filter(|native| !native.transport.is_frozen())
    else {
        return false;
    };
    let Some(binding) = native.callbacks.get(&owner).map(NativeCallbacks::binding) else {
        return false;
    };
    if native.transport.captured_mouse_press(binding).is_some() {
        // The original native generation will submit the terminal after GPUI
        // finishes its window callback, even if the active drag is already gone.
        return false;
    }
    let event = match event {
        SurfacePointerEvent::ButtonReleased(_) => {
            NativePointerEvent::ButtonReleased(NativePointerButton::Primary)
        }
        SurfacePointerEvent::StreamCancelled(reason) => NativePointerEvent::StreamCancelled(
            if reason == dockspace::runtime::SurfacePointerCancelReason::ExplicitCancellation {
                NativePointerCancelReason::ExplicitCancellation
            } else {
                NativePointerCancelReason::ExplicitPlatformCancellation
            },
        ),
        _ => return false,
    };
    let Some(lane) = native.transport.mouse_press_lane(binding) else {
        return false;
    };
    if only_content && lane != MousePressLane::Content {
        return false;
    }
    let delivery = NativePointerOwner::Native(binding);
    let input = if lane == MousePressLane::Desktop {
        NativePointerInput::new(
            GPUI_MOUSE_POINTER,
            event,
            NativeDesktopPointerLocation::new(
                NativeDesktopPosition::Unknown,
                NativePointerHover::Unknown,
                None,
            ),
            delivery,
            NativePointerOwner::Unknown,
        )
    } else {
        NativePointerInput::new_local(
            GPUI_MOUSE_POINTER,
            event,
            NativeLocalPointerLocation::Unknown,
            delivery,
            NativePointerOwner::Unknown,
        )
    };
    if !native
        .transport
        .pointer_in_window(input.with_metadata(metadata), false)
    {
        return false;
    }
    native.blocked_continuation = None;
    true
}

pub(in crate::adapter) fn queue_pointer(
    host: &mut HostState,
    surface: SurfaceId,
    serial: u64,
    event: dockspace::runtime::SurfacePointerEvent,
    position: open_gpui::Point<Pixels>,
    geometry: open_gpui::ElementGeometry,
    chrome_clear: bool,
    content_clear: bool,
    captured: bool,
    modifiers: open_gpui::Modifiers,
    window: &Window,
) -> bool {
    use dockspace::runtime::{
        NativeDesktopPointerLocation, NativeDesktopPosition, NativeLocalPointerLocation,
        NativePointerButton, NativePointerCancelReason, NativePointerEvent, NativePointerHover,
        NativePointerInput, NativePointerOwner, SurfacePointerEvent,
    };
    let owner = window.window_handle().window_id();
    let terminal = matches!(
        event,
        SurfacePointerEvent::ButtonReleased(_) | SurfacePointerEvent::StreamCancelled(_)
    );
    // A replacement can be inert between the original press and release.
    // Cleanup still enters the same producer; it grants no new target or
    // presentation authority and core resolves the original held gesture.
    if !terminal && !super::super::input::is_presented(host, owner, surface, serial) {
        return false;
    }
    let metadata = host
        .config
        .input
        .pointer_metadata(host.observed_at(), Some(modifiers));
    let Some(native) = &mut host.native else {
        return false;
    };
    if native.transport.is_frozen() {
        return false;
    }
    let Some(callbacks) = native.callbacks.get(&owner) else {
        return false;
    };
    let binding = callbacks.binding();
    let event = match event {
        SurfacePointerEvent::ButtonPressed(_) => {
            NativePointerEvent::ButtonPressed(NativePointerButton::Primary)
        }
        SurfacePointerEvent::ButtonReleased(_) => {
            NativePointerEvent::ButtonReleased(NativePointerButton::Primary)
        }
        SurfacePointerEvent::Moved => NativePointerEvent::Moved,
        SurfacePointerEvent::StreamCancelled(reason) => NativePointerEvent::StreamCancelled(
            if reason == dockspace::runtime::SurfacePointerCancelReason::ExplicitCancellation {
                NativePointerCancelReason::ExplicitCancellation
            } else {
                NativePointerCancelReason::ExplicitPlatformCancellation
            },
        ),
        _ => return false,
    };
    if !captured && !chrome_clear && !content_clear && !terminal {
        return false;
    }
    let press = native.transport.mouse_press_lane(binding);
    if terminal && (press.is_none() || native.transport.captured_mouse_press(binding).is_some()) {
        return false;
    }
    let delivery = NativePointerOwner::Native(binding);
    let packet = if terminal && press == Some(MousePressLane::Desktop) {
        let position = window
            .platform_facts()
            .physical_geometry
            .and_then(|geometry| geometry.local_to_global(position))
            .and_then(|point| PhysicalPoint::new(f64::from(point.x.0), f64::from(point.y.0)).ok())
            .map_or(NativeDesktopPosition::Unknown, NativeDesktopPosition::Exact);
        // The window callback establishes delivery. It supplies no independent
        // desktop hover or native capture observation.
        NativePointerInput::new(
            GPUI_MOUSE_POINTER,
            event,
            NativeDesktopPointerLocation::new(position, NativePointerHover::Unknown, None),
            delivery,
            NativePointerOwner::Unknown,
        )
    } else {
        let location = local_sample(&host.session, binding, position, window).map_or(
            NativeLocalPointerLocation::Unknown,
            NativeLocalPointerLocation::Exact,
        );
        NativePointerInput::new_local(
            GPUI_MOUSE_POINTER,
            event,
            location,
            delivery,
            NativePointerOwner::Unknown,
        )
    };
    if !native.transport.pointer_in_window(
        packet.with_metadata(metadata),
        !captured && !chrome_clear && content_clear,
    ) {
        return false;
    }
    native.blocked_continuation = None;
    if let Some(region) = host
        .mounts
        .get_mut(&owner)
        .and_then(|mount| mount.regions.get_mut(&surface))
    {
        // Pointer position feeds the retained drag ghost. The committed core
        // report drives paint program invalidation when interaction changes it.
        region.pointer = super::super::input::logical_point(position, geometry);
    }
    true
}

pub(super) fn local_sample(
    session: &DockspaceSession,
    binding: NativeWindowBinding,
    position: open_gpui::Point<Pixels>,
    window: &Window,
) -> Option<dockspace::runtime::NativeLocalPointerSample> {
    use dockspace::{
        geometry::PhysicalSize,
        runtime::{NativeLocalGeometry, NativeLocalPointerFact},
    };
    let facts = window.platform_facts();
    let scale = ScaleFactor::new(f64::from(facts.scale_factor)).ok()?;
    let size = if let Some(geometry) = facts.physical_geometry {
        PhysicalSize::new(
            f64::from(geometry.client_bounds().size.width.0),
            f64::from(geometry.client_bounds().size.height.0),
        )
        .ok()?
    } else {
        PhysicalSize::new(
            f64::from(f32::from(facts.content_size.width)) * scale.get(),
            f64::from(f32::from(facts.content_size.height)) * scale.get(),
        )
        .ok()?
    };
    session
        .native_local_coordinates(binding)?
        .capture(
            binding,
            NativeLocalPointerFact::new(
                PhysicalPoint::new(
                    f64::from(f32::from(position.x)) * scale.get(),
                    f64::from(f32::from(position.y)) * scale.get(),
                )
                .ok()?,
                size,
                scale,
            ),
            NativeLocalGeometry::new(size, scale),
        )
        .ok()
}
