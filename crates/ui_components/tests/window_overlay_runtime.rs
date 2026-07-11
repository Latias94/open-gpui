use std::cell::{Cell, RefCell};
use std::rc::Rc;

use open_gpui::{
    AnyView, AppContext as _, Bounds, Context, DispatchPhase, Entity, FocusHandle, HitboxBehavior,
    IntoElement, KeyDownEvent, Keystroke, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, ParentElement, PinchEvent, PointerCancelEvent, PointerCaptureHandle, Render,
    ScrollDelta, ScrollWheelEvent, StyleRefinement, Styled, TouchPhase, VisualContext as _, Window,
    canvas, div, point, prelude::*, px, size,
};
use open_gpui_ui_components::gpui_adapter::{
    FocusTargetRegistration, OverlayFocusMode, OverlayFocusRestoreCondition,
    OverlayFocusTargetLease, OverlayInsideRegionId, OverlayLayerBinding, OverlayLayerPhase,
    OverlayLayerRegistration, OverlayLayerSnapshot, OverlayOwnership, OverlayTabBehavior,
    WindowOverlayRuntime, WindowOverlayRuntimeError,
};
use open_gpui_ui_core::{
    DismissReason, EscapeKeyPolicy, FocusTargetId, OutsidePressPolicy, OverlayLayerId,
    OverlayLayerKind, OverlayLayerPolicy, OverlayPresence,
};

struct RenderedLayer {
    binding: OverlayLayerBinding,
    render_trigger: bool,
    render_surface: bool,
    capture_pointer: bool,
}

struct ProjectedInsideRegion {
    layer_id: String,
    region_id: String,
    bounds: Bounds<open_gpui::Pixels>,
}

struct RuntimeProbe {
    runtime: WindowOverlayRuntime,
    layers: Vec<RenderedLayer>,
    projected_inside_regions: Vec<ProjectedInsideRegion>,
    render_count: usize,
    underlay_focus: FocusHandle,
    fallback_focus: FocusHandle,
    first_extra_focus: FocusHandle,
    second_extra_focus: FocusHandle,
    underlay_clicks: Rc<Cell<usize>>,
    underlay_escape_keys: Rc<Cell<usize>>,
    underlay_pointer_events: Rc<RefCell<Vec<&'static str>>>,
    surface_pointer_events: Rc<RefCell<Vec<(String, &'static str)>>>,
    surface_pointer_capture: PointerCaptureHandle,
}

struct CachedOverlaySurfaceRoot {
    runtime: WindowOverlayRuntime,
    binding: OverlayLayerBinding,
    child: Entity<CachedOverlaySurfaceProbe>,
}

struct CachedOverlaySurfaceProbe {
    runtime: WindowOverlayRuntime,
    binding: OverlayLayerBinding,
    renders: Rc<Cell<usize>>,
}

struct LateInstalledRuntimeProbe {
    runtime_binding: Option<(WindowOverlayRuntime, OverlayLayerBinding)>,
    pointer_capture: PointerCaptureHandle,
}

struct ForeignOverlaySurfaceProbe {
    runtime: WindowOverlayRuntime,
    binding: OverlayLayerBinding,
}

impl Render for CachedOverlaySurfaceRoot {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().relative().size_full().child(
            AnyView::from(self.child.clone()).cached(
                StyleRefinement::default()
                    .absolute()
                    .left(px(240.0))
                    .top(px(96.0))
                    .w(px(180.0))
                    .h(px(28.0)),
            ),
        )
    }
}

impl Render for CachedOverlaySurfaceProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        self.renders.set(self.renders.get() + 1);
        self.runtime.surface(
            &self.binding,
            OverlayInsideRegionId::new("surface"),
            "cached-overlay-surface-wrapper",
            div()
                .id("cached-overlay-surface")
                .debug_selector(|| "window-overlay-runtime:cached-surface:surface".to_owned())
                .size_full()
                .occlude(),
        )
    }
}

impl LateInstalledRuntimeProbe {
    fn new(window: &mut Window, _: &mut Context<Self>) -> Self {
        Self {
            runtime_binding: None,
            pointer_capture: window.new_pointer_capture_handle(),
        }
    }
}

impl Render for LateInstalledRuntimeProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let pointer_capture = self.pointer_capture;
        canvas(
            move |bounds, window, _| {
                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                window
                    .bind_pointer_capture(&pointer_capture, hitbox.id)
                    .expect("capture target should bind before the overlay runtime exists");
                hitbox
            },
            move |_, hitbox, window, _| {
                let hitbox_id = hitbox.id;
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, _| {
                    if phase == DispatchPhase::Bubble
                        && event.button == MouseButton::Left
                        && hitbox_id.is_mouse_event_target(window)
                    {
                        window
                            .capture_pointer(&pointer_capture, MouseButton::Left)
                            .expect("pre-runtime target should capture the pointer");
                    }
                });
            },
        )
        .size_full()
    }
}

impl Render for ForeignOverlaySurfaceProbe {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().relative().size_full().child(
            self.runtime.surface(
                &self.binding,
                OverlayInsideRegionId::new("foreign-window-region"),
                "foreign-window-overlay-surface-wrapper",
                div()
                    .absolute()
                    .left(px(40.0))
                    .top(px(40.0))
                    .w(px(120.0))
                    .h(px(80.0)),
            ),
        )
    }
}

impl RuntimeProbe {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            runtime: WindowOverlayRuntime::for_window(window, cx),
            layers: Vec::new(),
            projected_inside_regions: Vec::new(),
            render_count: 0,
            underlay_focus: cx.focus_handle(),
            fallback_focus: cx.focus_handle(),
            first_extra_focus: cx.focus_handle(),
            second_extra_focus: cx.focus_handle(),
            underlay_clicks: Rc::new(Cell::new(0)),
            underlay_escape_keys: Rc::new(Cell::new(0)),
            underlay_pointer_events: Rc::new(RefCell::new(Vec::new())),
            surface_pointer_events: Rc::new(RefCell::new(Vec::new())),
            surface_pointer_capture: window.new_pointer_capture_handle(),
        }
    }

    fn add_layer(&mut self, binding: OverlayLayerBinding) {
        self.layers.push(RenderedLayer {
            binding,
            render_trigger: true,
            render_surface: true,
            capture_pointer: false,
        });
    }

    fn binding(&self, id: &str) -> OverlayLayerBinding {
        self.layers
            .iter()
            .find(|layer| layer.binding.lease().layer_id().as_str() == id)
            .unwrap_or_else(|| panic!("missing rendered layer `{id}`"))
            .binding
            .clone()
    }

    fn remove_layer(&mut self, id: &str) {
        self.layers
            .retain(|layer| layer.binding.lease().layer_id().as_str() != id);
        self.projected_inside_regions
            .retain(|region| region.layer_id != id);
    }

    fn set_trigger_rendered(&mut self, id: &str, rendered: bool) {
        self.layers
            .iter_mut()
            .find(|layer| layer.binding.lease().layer_id().as_str() == id)
            .unwrap_or_else(|| panic!("missing rendered layer `{id}`"))
            .render_trigger = rendered;
    }

    fn set_surface_rendered(&mut self, id: &str, rendered: bool) {
        self.layers
            .iter_mut()
            .find(|layer| layer.binding.lease().layer_id().as_str() == id)
            .unwrap_or_else(|| panic!("missing rendered layer `{id}`"))
            .render_surface = rendered;
    }

    fn set_pointer_capture(&mut self, id: &str, capture: bool) {
        self.layers
            .iter_mut()
            .find(|layer| layer.binding.lease().layer_id().as_str() == id)
            .unwrap_or_else(|| panic!("missing rendered layer `{id}`"))
            .capture_pointer = capture;
    }
}

impl Render for RuntimeProbe {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_count += 1;
        let snapshot = self
            .runtime
            .snapshot(window, cx)
            .expect("probe runtime should belong to its render window");
        let underlay_clicks = self.underlay_clicks.clone();
        let underlay_escape_keys = self.underlay_escape_keys.clone();
        let underlay_pointer_events = self.underlay_pointer_events.clone();
        let mut root = div()
            .id("window-overlay-runtime-probe")
            .relative()
            .size_full()
            .on_key_down(move |event, _, _| {
                if event.keystroke.key.as_str() == "escape" {
                    underlay_escape_keys.set(underlay_escape_keys.get() + 1);
                }
            })
            .child(
                div()
                    .id("window-overlay-runtime-underlay")
                    .debug_selector(|| "window-overlay-runtime:underlay".to_owned())
                    .absolute()
                    .left(px(0.0))
                    .top(px(0.0))
                    .w(px(640.0))
                    .h(px(420.0))
                    .focusable()
                    .track_focus(&self.underlay_focus)
                    .tab_index(0)
                    .on_mouse_down(MouseButton::Left, {
                        let events = underlay_pointer_events.clone();
                        move |_, _, _| events.borrow_mut().push("down")
                    })
                    .on_mouse_move({
                        let events = underlay_pointer_events.clone();
                        move |_, _, _| events.borrow_mut().push("move")
                    })
                    .on_mouse_up(MouseButton::Left, {
                        let events = underlay_pointer_events;
                        move |_, _, _| events.borrow_mut().push("up")
                    })
                    .on_click(move |_, _, _| {
                        underlay_clicks.set(underlay_clicks.get() + 1);
                    }),
            )
            .child(
                div()
                    .id("window-overlay-runtime-fallback")
                    .debug_selector(|| "window-overlay-runtime:fallback".to_owned())
                    .absolute()
                    .left(px(8.0))
                    .top(px(360.0))
                    .w(px(120.0))
                    .h(px(24.0))
                    .focusable()
                    .track_focus(&self.fallback_focus)
                    .tab_index(1),
            )
            .child(
                div()
                    .id("window-overlay-runtime-extra-a")
                    .debug_selector(|| "window-overlay-runtime:extra-a".to_owned())
                    .absolute()
                    .left(px(144.0))
                    .top(px(360.0))
                    .w(px(120.0))
                    .h(px(24.0))
                    .focusable()
                    .track_focus(&self.first_extra_focus)
                    .tab_index(2),
            )
            .child(
                div()
                    .id("window-overlay-runtime-extra-b")
                    .debug_selector(|| "window-overlay-runtime:extra-b".to_owned())
                    .absolute()
                    .left(px(280.0))
                    .top(px(360.0))
                    .w(px(120.0))
                    .h(px(24.0))
                    .focusable()
                    .track_focus(&self.second_extra_focus)
                    .tab_index(3),
            );

        for (index, layer) in self.layers.iter().enumerate() {
            let id = layer.binding.lease().layer_id().as_str().to_owned();
            let top = 48.0 + index as f32 * 44.0;
            if layer.render_trigger {
                let selector = format!("window-overlay-runtime:{id}:trigger");
                root = root.child(
                    div()
                        .id(format!("window-overlay-runtime:{id}:trigger"))
                        .debug_selector(move || selector.clone())
                        .absolute()
                        .left(px(48.0))
                        .top(px(top))
                        .w(px(140.0))
                        .h(px(28.0))
                        .focusable()
                        .track_focus(layer.binding.trigger_focus())
                        .tab_index(0),
                );
            }
            let present = snapshot
                .layers()
                .iter()
                .find(|snapshot| snapshot.id().as_str() == id)
                .is_some_and(|snapshot| snapshot.presence().present());
            if layer.render_surface && present {
                let selector = format!("window-overlay-runtime:{id}:surface");
                let mut surface = div()
                    .id(format!("window-overlay-runtime:{id}:surface"))
                    .debug_selector(move || selector.clone())
                    .absolute()
                    .left(px(240.0))
                    .top(px(top))
                    .w(px(180.0))
                    .h(px(28.0))
                    .track_focus(layer.binding.surface_focus())
                    .tab_group()
                    .tab_stop(false)
                    .occlude();
                if layer.capture_pointer {
                    let pointer_events = self.surface_pointer_events.clone();
                    let pointer_capture = self.surface_pointer_capture;
                    let layer_id = id.clone();
                    surface = surface.child(
                        canvas(
                            move |bounds, window, _| {
                                let hitbox = window.insert_hitbox(bounds, HitboxBehavior::Normal);
                                window
                                    .bind_pointer_capture(&pointer_capture, hitbox.id)
                                    .expect("modal surface capture should bind in every frame");
                                hitbox
                            },
                            move |_, hitbox, window, _| {
                                let down_id = hitbox.id;
                                let down_events = pointer_events.clone();
                                let down_layer = layer_id.clone();
                                let down_capture = pointer_capture;
                                window.on_mouse_event(
                                    move |event: &MouseDownEvent, phase, window, _| {
                                        if phase == DispatchPhase::Bubble
                                            && event.button == MouseButton::Left
                                            && down_id.is_mouse_event_target(window)
                                        {
                                            down_events
                                                .borrow_mut()
                                                .push((down_layer.clone(), "down"));
                                            window
                                                .capture_pointer(&down_capture, MouseButton::Left)
                                                .expect("rendered modal surface should capture");
                                        }
                                    },
                                );
                                let move_id = hitbox.id;
                                let move_events = pointer_events.clone();
                                let move_layer = layer_id.clone();
                                window.on_mouse_event(
                                    move |_: &MouseMoveEvent, phase, window, _| {
                                        if phase == DispatchPhase::Bubble
                                            && move_id.is_mouse_event_target(window)
                                        {
                                            move_events
                                                .borrow_mut()
                                                .push((move_layer.clone(), "move"));
                                        }
                                    },
                                );
                                let up_id = hitbox.id;
                                let up_events = pointer_events.clone();
                                let up_layer = layer_id.clone();
                                window.on_mouse_event(
                                    move |event: &MouseUpEvent, phase, window, _| {
                                        if phase == DispatchPhase::Bubble
                                            && event.button == MouseButton::Left
                                            && up_id.is_mouse_event_target(window)
                                        {
                                            up_events.borrow_mut().push((up_layer.clone(), "up"));
                                        }
                                    },
                                );
                                let cancel_id = hitbox.id;
                                window.on_pointer_cancel(
                                    move |_: &PointerCancelEvent, phase, window, _| {
                                        if phase == DispatchPhase::Bubble
                                            && cancel_id.is_mouse_event_target(window)
                                        {
                                            pointer_events
                                                .borrow_mut()
                                                .push((layer_id.clone(), "cancel"));
                                        }
                                    },
                                );
                            },
                        )
                        .size_full(),
                    );
                }
                root = root.child(self.runtime.surface(
                    &layer.binding,
                    OverlayInsideRegionId::new("surface"),
                    format!("window-overlay-runtime:{id}:surface-wrapper"),
                    surface,
                ));
            }
        }

        for projection in &self.projected_inside_regions {
            let Some(layer) = self
                .layers
                .iter()
                .find(|layer| layer.binding.lease().layer_id().as_str() == projection.layer_id)
            else {
                continue;
            };
            let present = snapshot
                .layers()
                .iter()
                .find(|layer| layer.id().as_str() == projection.layer_id)
                .is_some_and(|layer| layer.presence().present());
            if !present {
                continue;
            }
            let bounds = projection.bounds;
            root = root.child(
                self.runtime.surface(
                    &layer.binding,
                    OverlayInsideRegionId::new(projection.region_id.clone()),
                    format!(
                        "window-overlay-runtime:{}:{}:inside-region",
                        projection.layer_id, projection.region_id
                    ),
                    div()
                        .absolute()
                        .left(bounds.origin.x)
                        .top(bounds.origin.y)
                        .w(bounds.size.width)
                        .h(bounds.size.height),
                ),
            );
        }

        root
    }
}

fn policy(
    kind: OverlayLayerKind,
    presence: OverlayPresence,
    outside_press: OutsidePressPolicy,
) -> OverlayLayerPolicy {
    OverlayLayerPolicy::new(kind, presence).with_outside_press_policy(outside_press)
}

fn controlled_registration(
    id: impl Into<String>,
    policy: OverlayLayerPolicy,
    events: Rc<RefCell<Vec<bool>>>,
) -> OverlayLayerRegistration {
    OverlayLayerRegistration::new(id, policy, OverlayOwnership::Controlled).on_open_change(
        move |open, _, _| {
            events.borrow_mut().push(open);
        },
    )
}

fn uncontrolled_registration(
    id: impl Into<String>,
    policy: OverlayLayerPolicy,
) -> OverlayLayerRegistration {
    OverlayLayerRegistration::new(id, policy, OverlayOwnership::Uncontrolled)
        .uncontrolled_commit(|_, _, _| {})
}

fn draw(cx: &mut open_gpui::VisualTestContext) {
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
}

fn settle_focus_claims(cx: &mut open_gpui::VisualTestContext) {
    let mut idle_frames = 0;
    for _ in 0..8 {
        cx.run_until_parked();
        let callbacks = cx.update(|window, cx| {
            let callbacks = window.drain_next_frame_callbacks_for_test(cx);
            window.draw(cx).clear();
            callbacks
        });
        idle_frames = if callbacks == 0 { idle_frames + 1 } else { 0 };
        if idle_frames == 2 {
            return;
        }
    }
    panic!("window callbacks did not settle within eight frames");
}

fn register_layer(
    cx: &mut open_gpui::VisualTestContext,
    view: &Entity<RuntimeProbe>,
    registration: OverlayLayerRegistration,
) -> OverlayLayerBinding {
    cx.update_window_entity(view, |probe, window, cx| {
        let binding = probe
            .runtime
            .register_layer(registration, window, cx)
            .expect("layer registration should succeed");
        probe.add_layer(binding.clone());
        cx.notify();
        binding
    })
}

fn unregister_layer(cx: &mut open_gpui::VisualTestContext, view: &Entity<RuntimeProbe>, id: &str) {
    cx.update_window_entity(view, |probe, window, cx| {
        let binding = probe.binding(id);
        probe
            .runtime
            .unregister_layer(&binding, window, cx)
            .expect("layer unregistration should succeed");
        probe.remove_layer(id);
        cx.notify();
    });
    settle_focus_claims(cx);
}

fn snapshot_layer(
    cx: &mut open_gpui::VisualTestContext,
    view: &Entity<RuntimeProbe>,
    id: &str,
) -> OverlayLayerSnapshot {
    cx.update_window_entity(view, |probe, window, cx| {
        probe
            .runtime
            .snapshot(window, cx)
            .expect("snapshot should belong to the probe window")
            .layers()
            .iter()
            .find(|layer| layer.id().as_str() == id)
            .unwrap_or_else(|| panic!("missing snapshot layer `{id}`"))
            .clone()
    })
}

fn set_inside_region(
    cx: &mut open_gpui::VisualTestContext,
    view: &Entity<RuntimeProbe>,
    id: &str,
    region: &str,
    bounds: Bounds<open_gpui::Pixels>,
) {
    cx.update_window_entity(view, |probe, _, cx| {
        if let Some(projection) = probe
            .projected_inside_regions
            .iter_mut()
            .find(|projection| projection.layer_id == id && projection.region_id == region)
        {
            projection.bounds = bounds;
        } else {
            probe.projected_inside_regions.push(ProjectedInsideRegion {
                layer_id: id.to_owned(),
                region_id: region.to_owned(),
                bounds,
            });
        }
        cx.notify();
    });
    draw(cx);
}

fn rect(x: f32, y: f32, width: f32, height: f32) -> Bounds<open_gpui::Pixels> {
    Bounds {
        origin: point(px(x), px(y)),
        size: size(px(width), px(height)),
    }
}

fn mouse_down(x: f32, y: f32) -> MouseDownEvent {
    MouseDownEvent {
        position: point(px(x), px(y)),
        modifiers: Default::default(),
        button: MouseButton::Left,
        click_count: 1,
        first_mouse: false,
    }
}

#[path = "window_overlay_runtime/focus.rs"]
mod focus;
#[path = "window_overlay_runtime/lifecycle.rs"]
mod lifecycle;
#[path = "window_overlay_runtime/ownership.rs"]
mod ownership;
#[path = "window_overlay_runtime/pointer.rs"]
mod pointer;
#[path = "window_overlay_runtime/registration.rs"]
mod registration;
