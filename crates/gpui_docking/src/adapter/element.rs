//! A single retained Element consuming the core's complete ordered paint program.

use std::{cell::Cell, collections::BTreeMap, rc::Rc};

use anyhow::{Context as _, Result, anyhow};
use dockspace::{
    geometry::{LogicalRect, LogicalSize},
    model::{ItemId, SurfaceId},
    runtime::{
        DockspaceReceiverDescriptor, DockspaceUpdate, SurfaceMeasurementAnswer as Answer,
        SurfaceMeasurementRequest as Request, SurfacePointerButton, SurfacePointerCancelReason,
        SurfacePointerEvent, SurfaceUnavailableReason, SurfaceWork, SurfaceWorkDeferred,
        TabStripMetrics,
    },
};
use open_gpui::{
    AnyElement, App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId, HitboxBehavior,
    HitboxId, Hsla, InspectorElementId, IntoElement, KeyDownEvent, LayoutId, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, PointerCaptureHandle,
    PreparedSubtreeClip, Style, SubtreeClip, SubtreePresentation, SubtreePresentationExt, TextRun,
    Window, div, fill, point, prelude::*, px, relative, size,
};

use crate::adapter::{
    HostState, SharedHost, continuation::InputFrame, host::DockRegion, host_native, input,
    presentation,
};

mod chrome;
mod recording;

use chrome::GuideState;
use recording::record;

pub(super) struct Packet {
    draws: Vec<Draw>,
    deferred: bool,
}

impl Packet {
    pub(super) fn is_deferred(&self) -> bool {
        self.deferred
    }

    #[cfg(test)]
    pub(super) fn pane_items(&self) -> Vec<ItemId> {
        self.draws
            .iter()
            .filter_map(|draw| match draw {
                Draw::Pane(_, item) => Some(*item),
                _ => None,
            })
            .collect()
    }
}

enum Draw {
    Fill(LogicalRect, Hsla),
    Tab(LogicalRect, ItemId, Hsla, bool),
    ContainedFrame(LogicalRect),
    ContainedTitle(LogicalRect, Hsla),
    Guide(LogicalRect, GuideState),
    Outline(LogicalRect, Hsla),
    Text(LogicalRect, String, Hsla),
    TabText(LogicalRect, LogicalRect, ItemId, String, Hsla),
    Pane(LogicalRect, ItemId),
    Receiver(DockspaceReceiverDescriptor),
    Preview(LogicalRect),
}

enum PreparedDraw {
    Fill(Bounds<Pixels>, Hsla),
    Tab(Bounds<Pixels>, ItemId, Hsla, bool),
    ContainedFrame(Bounds<Pixels>),
    ContainedTitle(Bounds<Pixels>, Hsla),
    Guide(Bounds<Pixels>, GuideState),
    Outline(Bounds<Pixels>, Hsla),
    ClippedElement(AnyElement, PreparedSubtreeClip),
    Preview(Bounds<Pixels>, Hsla, Hsla),
}

pub(crate) struct DockspaceElement {
    host: SharedHost,
    hovered_tab: Rc<Cell<Option<ItemId>>>,
    regions: Vec<DockRegion>,
}

pub(crate) struct PreparedFrame {
    regions: Vec<PreparedRegion>,
    waiting_for_update: bool,
}

pub(crate) struct PreparedRegion {
    surface: SurfaceId,
    draws: Vec<PreparedDraw>,
    hitboxes: Vec<HitboxId>,
    tabs: Vec<(ItemId, HitboxId)>,
    capture: PointerCaptureHandle,
    serial: u64,
    bounds: Bounds<Pixels>,
    receivers: Vec<(DockspaceReceiverDescriptor, HitboxId)>,
}

pub(crate) enum PreparedState {
    Frame(PreparedFrame),
    Failure(AnyElement),
}

impl DockspaceElement {
    pub(super) fn new(host: SharedHost, regions: Vec<DockRegion>) -> Self {
        Self {
            host,
            regions,
            hovered_tab: Rc::default(),
        }
    }
}

impl Element for DockspaceElement {
    type RequestLayoutState = ();
    type PrepaintState = PreparedState;

    fn id(&self) -> Option<ElementId> {
        Some(ElementId::named_usize(
            "dockspace-gpui-host",
            Rc::as_ptr(&self.host) as usize,
        ))
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let style = Style {
            size: size(relative(1.0).into(), relative(1.0).into()),
            flex_grow: 1.0,
            ..Default::default()
        };
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> PreparedState {
        presentation::abandon_unaccepted(&mut self.host.borrow_mut(), window);
        host_native::prepare_window(&self.host, window, cx);
        let mount = self
            .host
            .borrow_mut()
            .ensure_mount(id, &self.regions, window, cx);
        match mount {
            Ok(true) => {
                let weak = Rc::downgrade(&self.host);
                let terminal_host = weak.clone();
                let owner = window.window_handle().window_id();
                cx.on_window_closed(move |cx, closed| {
                    if closed == owner {
                        if let Some(host) = weak.upgrade() {
                            presentation::close(&mut host.borrow_mut(), owner, cx);
                        }
                    }
                })
                .detach();
                // Reservation rollback has no ordinary logical-close event.
                // The original native terminal retires its provisional mount.
                cx.on_window_native_terminal(move |cx, closed| {
                    if closed == owner {
                        if let Some(host) = terminal_host.upgrade() {
                            presentation::close(&mut host.borrow_mut(), owner, cx);
                        }
                    }
                })
                .detach();
            }
            Ok(false) => {}
            Err(error) => {
                self.host.borrow_mut().fail(&error);
                return self.failure(error, bounds, window, cx);
            }
        }
        let owner = window.window_handle().window_id();
        let capture = self.host.borrow().mounts[&owner].capture;
        // Claim the framework's once-per-frame capture binding before touching
        // a pending core candidate. Failed duplicate mounts retract their hitbox.
        if let Err(error) = window.transact(|window| {
            let background = window.insert_hitbox(bounds, HitboxBehavior::Normal);
            window.bind_pointer_capture(&capture, background.id)
        }) {
            return self.failure(error, bounds, window, cx);
        }
        if let Some(id) = id {
            self.hovered_tab =
                window.with_element_state::<Rc<Cell<Option<ItemId>>>, _>(id, |state, _| {
                    let state = state.unwrap_or_default();
                    (state.clone(), state)
                });
        }
        window.set_focus_handle(&self.host.borrow().mounts[&owner].focus, cx);
        match self.prepare(bounds, capture, window, cx) {
            Ok(prepared) => PreparedState::Frame(prepared),
            Err(error) => {
                self.host.borrow_mut().fail(&error);
                self.failure(error, bounds, window, cx)
            }
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request: &mut (),
        prepared: &mut PreparedState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let background = self.host.borrow().config.colors.background;
        window.paint_quad(fill(bounds, background));
        let prepared = match prepared {
            PreparedState::Frame(prepared) => prepared,
            PreparedState::Failure(error) => {
                error.paint(window, cx);
                return;
            }
        };
        if !prepared.waiting_for_update {
            for region in &prepared.regions {
                self.listen(region, window);
            }
        }
        let colors = self.host.borrow().config.colors.clone();
        // Keep the core order, including pane widgets between chrome and overlays.
        for region in &mut prepared.regions {
            for draw in &mut region.draws {
                match draw {
                    PreparedDraw::Fill(bounds, color) => window.paint_quad(fill(*bounds, *color)),
                    PreparedDraw::Tab(bounds, item, color, selected) => {
                        let hovered = self.hovered_tab.get() == Some(*item);
                        let background = if hovered && !*selected {
                            colors.hovered_tab
                        } else {
                            *color
                        };
                        chrome::paint_tab(*bounds, background, *selected, &colors, window);
                    }
                    PreparedDraw::ContainedFrame(bounds) => {
                        chrome::paint_contained_frame(*bounds, &colors, window)
                    }
                    PreparedDraw::ContainedTitle(bounds, color) => {
                        chrome::paint_title(*bounds, *color, window)
                    }
                    PreparedDraw::Guide(bounds, state) => {
                        chrome::paint_guide(*bounds, *state, &colors, window)
                    }
                    PreparedDraw::Outline(bounds, color) => outline(*bounds, *color, window),
                    PreparedDraw::Preview(bounds, color, border) => {
                        window.paint_quad(fill(*bounds, *color));
                        outline(*bounds, *border, window);
                    }
                    PreparedDraw::ClippedElement(element, clip) => {
                        let _ = window
                            .with_prepared_subtree_clip(clip, |window| element.paint(window, cx));
                    }
                }
            }
        }
    }
}

impl DockspaceElement {
    fn failure(
        &self,
        error: impl std::fmt::Display,
        bounds: Bounds<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) -> PreparedState {
        let config = self.host.borrow().config.clone();
        let mut diagnostic = div()
            .w(bounds.size.width)
            .h(bounds.size.height)
            .overflow_hidden()
            .font(config.font)
            .text_size(config.text_size)
            .text_color(config.colors.text)
            .child(format!("Cannot mount dockspace: {error}"))
            .into_any_element();
        diagnostic.prepaint_as_root(bounds.origin, definite(bounds), window, cx);
        PreparedState::Failure(diagnostic)
    }

    fn prepare(
        &mut self,
        bounds: Bounds<Pixels>,
        capture: PointerCaptureHandle,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<PreparedFrame> {
        let owner = window.window_handle().window_id();
        let mut region_bounds = BTreeMap::new();
        for region in &self.regions {
            let rect = match region.bounds {
                Some(rect) => to_gpui(rect, bounds),
                None => bounds,
            };
            anyhow::ensure!(
                rect.size.width > px(0.0)
                    && rect.size.height > px(0.0)
                    && rect.origin.x >= bounds.origin.x
                    && rect.origin.y >= bounds.origin.y
                    && rect.right() <= bounds.right()
                    && rect.bottom() <= bounds.bottom(),
                "docking regions must be positive and contained in their element"
            );
            for previous in region_bounds.values() {
                let previous: &Bounds<Pixels> = previous;
                anyhow::ensure!(
                    rect.intersect(previous).is_empty(),
                    "docking regions must be disjoint"
                );
            }
            region_bounds.insert(region.surface, rect);
        }
        let (packets, serial, publication, waiting_for_update) = {
            let mut host = self.host.borrow_mut();
            let mount = &host.mounts[&owner];
            let publication = mount.publication;
            if let Some(pending) = &mount.pending {
                (pending.packets.clone(), pending.serial, publication, false)
            } else if host.has_unaccepted_update() {
                let mount = host.mounts.get_mut(&owner).expect("physical mount");
                mount.waiting_for_update = true;
                for region in mount.regions.values_mut() {
                    region.dirty = true;
                }
                // Another original Window still owns the affine candidate.
                // Retain visual content without admitting new docking input.
                // Its acceptance or withdrawal wakes this mount exactly once.
                (
                    mount
                        .regions
                        .iter()
                        .map(|(surface, region)| {
                            (
                                *surface,
                                region.packet.clone().unwrap_or_else(|| {
                                    Rc::new(Packet {
                                        draws: Vec::new(),
                                        deferred: true,
                                    })
                                }),
                            )
                        })
                        .collect(),
                    mount.serial,
                    publication,
                    true,
                )
            } else if host.inputs.is_empty()
                && mount.regions.iter().all(|(surface, region)| {
                    !region.dirty
                        && region.bounds == region_bounds.get(surface).copied()
                        && region.packet.is_some()
                })
            {
                (
                    mount
                        .regions
                        .iter()
                        .map(|(surface, region)| {
                            (*surface, region.packet.clone().expect("retained region"))
                        })
                        .collect(),
                    mount.serial,
                    publication,
                    false,
                )
            } else {
                let inputs = std::mem::take(&mut host.inputs);
                let candidate = build_candidate(&mut host, &inputs, &region_bounds, window, cx);
                let (packets, update) = match candidate {
                    Ok(candidate) => candidate,
                    Err(error) => {
                        let mut inputs = inputs;
                        inputs.append(&mut host.inputs);
                        host.inputs = inputs;
                        return Err(error);
                    }
                };
                let ticket = match window.begin_exact_frame_submission(cx) {
                    Ok(ticket) => ticket,
                    Err(error) => {
                        drop(update);
                        let mut inputs = inputs;
                        inputs.append(&mut host.inputs);
                        host.inputs = inputs;
                        return Err(error);
                    }
                };
                if let Err(error) = host_native::watch_submission(&host, &ticket, window) {
                    window.retire_exact_frame_submission(&ticket);
                    drop(update);
                    let mut inputs = inputs;
                    inputs.append(&mut host.inputs);
                    host.inputs = inputs;
                    return Err(error);
                }
                let mount = host.mounts.get_mut(&owner).expect("physical mount");
                mount.serial = mount
                    .serial
                    .checked_add(1)
                    .context("frame serial exhausted")?;
                let serial = mount.serial;
                for (surface, bounds) in &region_bounds {
                    let region = mount.regions.get_mut(surface).expect("mounted region");
                    region.bounds = Some(*bounds);
                    region.dirty = false;
                }
                mount.pending = Some(presentation::PendingSubmission::new(
                    serial,
                    ticket.clone(),
                    packets.clone(),
                    update,
                    inputs,
                ));
                presentation::observe(&self.host, &ticket, serial, window);
                (packets, serial, publication, false)
            }
        };
        let mut regions = Vec::new();
        for (surface, packet) in &packets {
            let bounds = region_bounds[surface];
            let background = window.insert_hitbox(bounds, HitboxBehavior::Normal).id;
            regions.push(self.prepare_region(
                *surface,
                packet,
                serial,
                bounds,
                background,
                capture,
                !waiting_for_update,
                window,
                cx,
            ));
        }
        let shared = self.host.clone();
        let discard = self.host.clone();
        let hovered_tab = self.hovered_tab.clone();
        let accepted_tabs: Vec<_> = regions
            .iter()
            .flat_map(|region| region.tabs.iter().copied())
            .collect();
        let input_frames: Vec<_> = regions
            .iter()
            .filter(|_| !waiting_for_update)
            .map(|region| InputFrame {
                surface: region.surface,
                serial,
                bounds: region.bounds,
                hitboxes: region.hitboxes.clone(),
                capture,
                receivers: region.receivers.clone(),
                scale: f64::from(window.scale_factor()),
                native_binding: None,
            })
            .collect();
        window.record_prepaint_window_transaction(
            publication,
            move |fence, window, cx| {
                presentation::accept(&shared, serial, input_frames.clone(), fence, window, cx);
                let current = hovered_tab_at(&accepted_tabs, window.mouse_position(), window);
                if hovered_tab.replace(current) != current {
                    window.refresh();
                }
            },
            move |_, window, cx| presentation::discard(&discard, window, cx),
        );
        let shared = self.host.clone();
        window.record_prepaint_focus_stable_commit(move |_, window, cx| {
            presentation::accept_focus_stable(&shared, serial, window, cx);
        });
        Ok(PreparedFrame {
            regions,
            waiting_for_update,
        })
    }

    fn prepare_region(
        &self,
        surface: SurfaceId,
        packet: &Packet,
        serial: u64,
        bounds: Bounds<Pixels>,
        background: HitboxId,
        capture: PointerCaptureHandle,
        allow_input: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> PreparedRegion {
        let mut hitboxes = vec![background];
        let mut draws = Vec::new();
        let mut tabs = Vec::new();
        let mut receivers = Vec::new();
        let config = self.host.borrow().config.clone();
        for draw in &packet.draws {
            match draw {
                Draw::Fill(rect, color) => {
                    draws.push(PreparedDraw::Fill(to_gpui(*rect, bounds), *color))
                }
                Draw::Tab(rect, item, color, selected) => {
                    if let Some(hitbox) = hitboxes.last() {
                        tabs.push((*item, *hitbox));
                    }
                    draws.push(PreparedDraw::Tab(
                        to_gpui(*rect, bounds),
                        *item,
                        *color,
                        *selected,
                    ));
                }
                Draw::ContainedFrame(rect) => {
                    draws.push(PreparedDraw::ContainedFrame(to_gpui(*rect, bounds)))
                }
                Draw::ContainedTitle(rect, color) => {
                    draws.push(PreparedDraw::ContainedTitle(to_gpui(*rect, bounds), *color))
                }
                Draw::Guide(rect, state) => {
                    draws.push(PreparedDraw::Guide(to_gpui(*rect, bounds), *state))
                }
                Draw::Outline(rect, color) => {
                    draws.push(PreparedDraw::Outline(to_gpui(*rect, bounds), *color))
                }
                Draw::Preview(rect) => draws.push(PreparedDraw::Preview(
                    to_gpui(*rect, bounds),
                    config.colors.preview,
                    config.colors.accent,
                )),
                Draw::Receiver(receiver) => {
                    let hitbox = window
                        .insert_hitbox(to_gpui(receiver.bounds(), bounds), HitboxBehavior::Normal);
                    hitboxes.push(hitbox.id);
                    receivers.push((*receiver, hitbox.id));
                }
                Draw::Text(rect, title, color) | Draw::TabText(rect, _, _, title, color) => {
                    let clip = match draw {
                        Draw::TabText(_, clip, _, _, _) => to_gpui(*clip, bounds),
                        _ => to_gpui(*rect, bounds),
                    };
                    let rect = to_gpui(*rect, bounds);
                    let mut element = div()
                        .w(rect.size.width)
                        .h(rect.size.height)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .flex()
                        .items_center()
                        .font(config.font.clone())
                        .text_size(config.text_size)
                        .text_color(*color)
                        .when_some(
                            match draw {
                                Draw::TabText(_, _, item, _, _) => Some(*item),
                                _ => None,
                            },
                            |text, item| {
                                text.debug_selector(move || format!("dockspace:tab:{}", item.get()))
                            },
                        )
                        .child(title.clone())
                        .into_any_element();
                    let clip = window.prepare_subtree_clip(&SubtreeClip::own_border_box(), clip);
                    if window
                        .with_prepared_subtree_clip(&clip, |window| {
                            element.prepaint_as_root(rect.origin, definite(rect), window, cx);
                        })
                        .is_some()
                    {
                        draws.push(PreparedDraw::ClippedElement(element, clip));
                    }
                }
                Draw::Pane(rect, item) => {
                    let rect = to_gpui(*rect, bounds);
                    let view = self
                        .host
                        .borrow()
                        .panels
                        .get(item)
                        .map(|panel| panel.view.clone());
                    let presentation = if allow_input
                        && input::is_presented(
                            &self.host.borrow(),
                            window.window_handle().window_id(),
                            surface,
                            serial,
                        ) {
                        SubtreePresentation::Visible
                    } else {
                        SubtreePresentation::Inert
                    };
                    let mut element = match view {
                        Some(view) => div()
                            .id(format!("pane-{}", item.get()))
                            .w(rect.size.width)
                            .h(rect.size.height)
                            .overflow_hidden()
                            .child(view)
                            .with_subtree_presentation(presentation)
                            .into_any_element(),
                        None => div()
                            .w(rect.size.width)
                            .h(rect.size.height)
                            .child("Pane unavailable")
                            .into_any_element(),
                    };
                    let clip = window.prepare_subtree_clip(&SubtreeClip::own_border_box(), rect);
                    if window
                        .with_prepared_subtree_clip(&clip, |window| {
                            element.prepaint_as_root(rect.origin, definite(rect), window, cx);
                        })
                        .is_some()
                    {
                        draws.push(PreparedDraw::ClippedElement(element, clip));
                    }
                }
            }
        }

        PreparedRegion {
            surface,
            draws,
            hitboxes,
            tabs,
            capture,
            serial,
            bounds,
            receivers,
        }
    }

    fn listen(&self, prepared: &PreparedRegion, window: &mut Window) {
        let surface = prepared.surface;
        let owner = window.window_handle().window_id();
        let tabs = prepared.tabs.clone();
        let hovered = self.hovered_tab.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, _| {
            if !phase.capture() {
                return;
            }
            let current = hovered_tab_at(&tabs, event.position, window);
            if hovered.replace(current) != current {
                // Hover is visual state only: the retained packet and exact core
                // output remain current, without adding a pointer edge.
                window.refresh();
            }
        });
        let shared = self.host.clone();
        let hitboxes = prepared.hitboxes.clone();
        let capture = prepared.capture;
        let serial = prepared.serial;
        let bounds = prepared.bounds;
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            if !phase.bubble() || event.button != MouseButton::Left {
                return;
            }
            let clear = hitboxes
                .iter()
                .any(|id| id.window_point_target_rank(event.position, window) == Some(0));
            if !clear || !input::is_presented(&shared.borrow(), owner, surface, serial) {
                return;
            }
            let managed = shared.borrow().native.is_some();
            if !managed && window.capture_pointer(&capture, MouseButton::Left).is_err() {
                return;
            }
            if input::queue(
                &mut shared.borrow_mut(),
                surface,
                serial,
                SurfacePointerEvent::ButtonPressed(SurfacePointerButton::Primary),
                event.position,
                bounds,
                &hitboxes,
                !managed,
                event.modifiers,
                window,
            ) {
                let focus = shared.borrow().mounts[&owner].focus.clone();
                window.focus(&focus, cx);
                window.refresh();
                if !managed {
                    cx.stop_propagation();
                }
            } else {
                let _ = window.release_pointer(&capture);
            }
        });
        let shared = self.host.clone();
        let hitboxes = prepared.hitboxes.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, _| {
            if !phase.bubble()
                || !capture.is_captured(window)
                || !routes_to(&shared.borrow(), surface, event.position, window)
            {
                return;
            }
            if input::queue(
                &mut shared.borrow_mut(),
                surface,
                serial,
                SurfacePointerEvent::Moved,
                event.position,
                bounds,
                &hitboxes,
                true,
                event.modifiers,
                window,
            ) {
                window.refresh();
            }
        });
        let shared = self.host.clone();
        let hitboxes = prepared.hitboxes.clone();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
            if !phase.bubble()
                || event.button != MouseButton::Left
                || (!capture.is_captured(window) && shared.borrow().native.is_none())
                || !routes_to(&shared.borrow(), surface, event.position, window)
            {
                return;
            }
            if shared.borrow().native.is_some() && cx.has_active_drag() {
                return;
            }
            if input::queue(
                &mut shared.borrow_mut(),
                surface,
                serial,
                SurfacePointerEvent::ButtonReleased(SurfacePointerButton::Primary),
                event.position,
                bounds,
                &hitboxes,
                capture.is_captured(window),
                event.modifiers,
                window,
            ) {
                let _ = window.release_pointer(&capture);
                window.refresh();
                cx.stop_propagation();
            }
        });
        let shared = self.host.clone();
        let hitboxes = prepared.hitboxes.clone();
        window.on_modifiers_changed(move |event, window, _| {
            if !capture.is_captured(window)
                || !routes_to(&shared.borrow(), surface, window.mouse_position(), window)
            {
                return;
            }
            let position = window.mouse_position();
            let enabled = shared
                .borrow()
                .config
                .input
                .docking_enabled(event.modifiers);
            if input::queue(
                &mut shared.borrow_mut(),
                surface,
                serial,
                SurfacePointerEvent::DockingEnabledChanged { enabled },
                position,
                bounds,
                &hitboxes,
                true,
                event.modifiers,
                window,
            ) {
                window.refresh();
            }
        });
        let shared = self.host.clone();
        let hitboxes = prepared.hitboxes.clone();
        window.on_key_event(move |event: &KeyDownEvent, phase, window, cx| {
            if !phase.bubble()
                || !capture.is_captured(window)
                || !routes_to(&shared.borrow(), surface, window.mouse_position(), window)
                || !shared
                    .borrow()
                    .config
                    .input
                    .cancel_matches(&event.keystroke)
            {
                return;
            }
            input::queue(
                &mut shared.borrow_mut(),
                surface,
                serial,
                SurfacePointerEvent::StreamCancelled(
                    SurfacePointerCancelReason::ExplicitCancellation,
                ),
                window.mouse_position(),
                bounds,
                &hitboxes,
                true,
                event.keystroke.modifiers,
                window,
            );
            let _ = window.release_pointer(&capture);
            window.refresh();
            cx.stop_propagation();
        });
        let shared = self.host.clone();
        let hitboxes = prepared.hitboxes.clone();
        window.on_pointer_cancel(move |_, phase, window, _| {
            if !phase.bubble() {
                return;
            }
            let mut host = shared.borrow_mut();
            let pending_input = host
                .mounts
                .get(&owner)
                .and_then(|mount| mount.pending.as_ref())
                .is_some_and(|pending| !pending.inputs.is_empty());
            if !routes_to(&host, surface, window.mouse_position(), window) {
                return;
            }
            if !host.session.has_active_gesture() && host.inputs.is_empty() && !pending_input {
                return;
            }
            let position = window.mouse_position();
            input::queue(
                &mut host,
                surface,
                serial,
                SurfacePointerEvent::StreamCancelled(
                    SurfacePointerCancelReason::ExplicitPlatformCancellation,
                ),
                position,
                bounds,
                &hitboxes,
                false,
                window.modifiers(),
                window,
            );
            window.refresh();
        });
    }
}

fn hovered_tab_at(
    tabs: &[(ItemId, HitboxId)],
    position: open_gpui::Point<Pixels>,
    window: &Window,
) -> Option<ItemId> {
    if window.last_input_was_keyboard() || window.last_input_was_touch() {
        return None;
    }
    tabs.iter().find_map(|(item, hitbox)| {
        (hitbox.window_point_target_rank(position, window) == Some(0)).then_some(*item)
    })
}

fn build_candidate(
    host: &mut HostState,
    inputs: &[input::QueuedPointer],
    regions: &BTreeMap<SurfaceId, Bounds<Pixels>>,
    window: &Window,
    cx: &mut App,
) -> Result<(BTreeMap<SurfaceId, Rc<Packet>>, DockspaceUpdate)> {
    let panels = &host.panels;
    let config = &host.config;
    let managed = host.native.is_some();
    let owner = window.window_handle().window_id();
    let native_window = host
        .native
        .as_ref()
        .and_then(|native| native.callbacks.get(&owner))
        .map(|callbacks| callbacks.binding());
    let pointers: BTreeMap<_, _> = host.mounts[&owner]
        .regions
        .iter()
        .map(|(surface, region)| (*surface, region.pointer))
        .collect();
    let mut frame = host_native::begin_frame(
        &mut host.session,
        &host.mounts,
        host.native.as_ref(),
        Some(window),
        cx,
    )
    .context("beginning retained docking candidate")?;
    host_native::set_placements(&mut frame, host.native.as_ref(), owner, regions, window)?;
    for input in inputs {
        input.submit(&mut frame)?;
    }
    frame.begin_encoding_pass()?;
    let presentation_surfaces = frame.surfaces();
    let mut packets = BTreeMap::new();
    for (surface, bounds) in regions {
        let surface = *surface;
        if frame.view().surface(surface).is_none() && !presentation_surfaces.contains(&surface) {
            // A pending tear-off is not yet in the committed layout. Retain its
            // staging slot until the core can encode the first-live output.
            // An ordinary removed surface instead leaves a vacant physical slot.
            let deferred = native_window.is_some_and(|binding| {
                frame
                    .native_surface_bindings_for_window(binding)
                    .any(|binding| binding.surface() == surface)
            });
            packets.insert(
                surface,
                Rc::new(Packet {
                    draws: Vec::new(),
                    deferred,
                }),
            );
            continue;
        }
        if !presentation_surfaces.contains(&surface) {
            anyhow::ensure!(managed, "surface is outside the presentation roster");
            // The real child can draw before its Created/Prepared observations
            // admit it to the core's presentation roster. This staging packet
            // supplies no painted output or native content-readiness fact.
            packets.insert(
                surface,
                Rc::new(Packet {
                    draws: Vec::new(),
                    deferred: true,
                }),
            );
            continue;
        }
        let bounds = LogicalRect::new(
            0.0,
            0.0,
            f64::from(f32::from(bounds.size.width)),
            f64::from(f32::from(bounds.size.height)),
        )?;
        let measure = match frame.surface_work(surface)? {
            SurfaceWork::Ready(plan) => plan.measurement_refresh().is_allowed(),
            SurfaceWork::Deferred(SurfaceWorkDeferred::Unmeasured) => true,
            SurfaceWork::Deferred(SurfaceWorkDeferred::OutputCapacity) => {
                return Err(anyhow!("renderer output capacity is exhausted"));
            }
        };
        if measure {
            frame.measure_surface_with(surface, |request| match request {
                Request::DockBounds { .. } | Request::PopupPlaneBounds { .. } => {
                    Answer::Bounds(bounds)
                }
                Request::PaneMinimum { item, .. } => {
                    Answer::PaneMinimum(item.and_then(|item| panels.get(&item)).map_or_else(
                        || LogicalSize::new(120.0, 90.0).expect("default minimum is valid"),
                        |pane| pane.minimum_size,
                    ))
                }
                Request::TabIntrinsic { item, .. } => {
                    let width = panels.get(&item).map_or(120.0, |pane| {
                        let line = window.text_system().shape_line(
                            pane.title.clone(),
                            config.text_size,
                            &[TextRun {
                                len: pane.title.len(),
                                font: config.font.clone(),
                                color: config.colors.text,
                                ..Default::default()
                            }],
                            None,
                        );
                        f64::from(f32::from(line.width))
                    });
                    Answer::TabIntrinsic(width)
                }
                Request::TabStrip { .. } => Answer::TabStrip(
                    TabStripMetrics::new(0.0, 0.0).expect("zero chrome reservation is valid"),
                ),
            })?;
        }
        let Some(plan) = frame.paint_plan(surface)? else {
            anyhow::ensure!(managed, "measured surface has no ready paint plan");
            // Native creation can render its real initial GPUI frame before
            // the core receives Created/Prepared. This is a deferred region,
            // with no painted output or content-presentation acknowledgement.
            packets.insert(
                surface,
                Rc::new(Packet {
                    draws: Vec::new(),
                    deferred: true,
                }),
            );
            continue;
        };
        let (packet, receipt) = record(plan, panels, pointers[&surface], &config.colors)?;
        frame.confirm_surface_painted(receipt)?;
        packets.insert(surface, Rc::new(packet));
    }
    frame.complete_unpainted_surfaces(SurfaceUnavailableReason::Deferred)?;
    Ok((packets, frame.suspend()))
}

fn to_gpui(rect: LogicalRect, surface: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        point(
            surface.origin.x + px(rect.x() as f32),
            surface.origin.y + px(rect.y() as f32),
        ),
        size(px(rect.width() as f32), px(rect.height() as f32)),
    )
}

fn definite(bounds: Bounds<Pixels>) -> open_gpui::Size<AvailableSpace> {
    size(
        AvailableSpace::Definite(bounds.size.width),
        AvailableSpace::Definite(bounds.size.height),
    )
}

fn outline(bounds: Bounds<Pixels>, color: Hsla, window: &mut Window) {
    let x = bounds.origin.x;
    let y = bounds.origin.y;
    let w = bounds.size.width;
    let h = bounds.size.height;
    for bounds in [
        Bounds::new(point(x, y), size(w, px(1.0))),
        Bounds::new(point(x, y + h - px(1.0)), size(w, px(1.0))),
        Bounds::new(point(x, y), size(px(1.0), h)),
        Bounds::new(point(x + w - px(1.0), y), size(px(1.0), h)),
    ] {
        window.paint_quad(fill(bounds, color));
    }
}

fn routes_to(
    host: &HostState,
    surface: SurfaceId,
    position: open_gpui::Point<Pixels>,
    window: &Window,
) -> bool {
    let owner = window.window_handle().window_id();
    let Some(mount) = host.mounts.get(&owner) else {
        return false;
    };
    let target = mount
        .regions
        .iter()
        .find_map(|(surface, region)| {
            region
                .bounds
                .filter(|bounds| bounds.contains(&position))
                .map(|_| *surface)
        })
        .or_else(|| {
            if host.native.is_some() {
                return None;
            }
            host.session
                .host_pointer_manipulation_surface(dockspace::runtime::SurfacePointerId::new(1))
                .map(|source| source.surface())
        });
    target == Some(surface)
}
