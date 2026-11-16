//! A single retained Element consuming the core's complete ordered paint program.

use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
};

use anyhow::{Context as _, Result};
use dockspace::{
    geometry::{LogicalRect, LogicalSize},
    model::{ItemId, SurfaceId},
    runtime::{
        DockspacePaneFocusObservation, DockspacePaneFocusRequest, DockspaceReceiverDescriptor,
        DockspaceUpdate, DockspaceVisualId, HostFrameReport, SurfaceMeasurementAnswer as Answer,
        SurfaceMeasurementRequest as Request, SurfacePointerButton, SurfacePointerCancelReason,
        SurfacePointerEvent, SurfaceWork, SurfaceWorkDeferred, TabListMenuMetrics,
        TabStripControlMetric, TabStripControlMetrics, TabStripControlPlacement, TabStripMetrics,
    },
};
use open_gpui::{
    AccessibleAction, AnyElement, App, AvailableSpace, Bounds, Element, ElementId, FocusHandle,
    GlobalElementId, HitboxBehavior, HitboxId, Hsla, InspectorElementId, IntoElement, KeyDownEvent,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    PointerCaptureHandle, PreparedSubtreeClip, Role, Style, SubtreeClip, SubtreePresentation,
    SubtreePresentationExt, TextRun, Window, div, fill, point, prelude::*, px, relative, size,
};

use crate::adapter::{
    HostState, SharedHost,
    host::DockRegion,
    host_native,
    input::{self, InputFrame},
    presentation,
};

mod chrome;
mod recording;

use chrome::{Control, ControlAction, ControlKind, GuideState, Icon, PresentationMenu};
use recording::record;

#[cfg(test)]
mod interaction_tests;

pub(super) struct Packet {
    draws: Vec<Draw>,
    deferred: bool,
    pub(super) native_preview: Option<host_native::provisional::Preview>,
}

enum Candidate {
    Painted {
        packets: BTreeMap<SurfaceId, Rc<Packet>>,
        update: DockspaceUpdate,
    },
    CapacityDeferred(HostFrameReport),
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
    NativePreview(host_native::provisional::Preview),
    Icon(LogicalRect, Icon, bool),
    Control(Rc<Control>),
    PresentationMenu(Rc<PresentationMenu>),
    TabList(LogicalRect, DockspaceVisualId),
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
    Icon(Bounds<Pixels>, Icon, bool),
}

#[derive(Default)]
struct InteractionState {
    hovered_tab: Rc<Cell<Option<ItemId>>>,
    focus: RefCell<BTreeMap<DockspaceVisualId, FocusHandle>>,
    menu: Cell<Option<DockspaceVisualId>>,
    menu_row: Cell<usize>,
    pane_focus: RefCell<BTreeMap<SurfaceId, PaneFocusAttempt>>,
    pending_tab_focus: RefCell<Option<PendingTabFocus>>,
    tab_list_focus: RefCell<Option<TabListFocus>>,
}

struct PaneFocusAttempt {
    request: DockspacePaneFocusRequest,
    applied: bool,
    observation: Option<DockspacePaneFocusObservation>,
}

struct PendingTabFocus {
    surface: SurfaceId,
    visual: DockspaceVisualId,
    claim_revision: u64,
}

struct TabListFocus {
    surface: SurfaceId,
    bar: DockspaceVisualId,
    row: DockspaceVisualId,
    opener: FocusHandle,
    claim_revision: u64,
}

pub(crate) struct DockspaceElement {
    host: SharedHost,
    interaction: Rc<InteractionState>,
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
    controls: Vec<(Rc<Control>, FocusHandle, HitboxId)>,
    visible_panes: Vec<ItemId>,
    admitted: bool,
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
            interaction: Rc::default(),
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
        presentation::abandon_unaccepted(&mut self.host.borrow_mut(), window, cx);
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
                let closed = cx.on_window_closed(move |cx, closed| {
                    if closed == owner {
                        if let Some(host) = weak.upgrade() {
                            presentation::close(&mut host.borrow_mut(), owner, cx);
                        }
                    }
                });
                // Reservation rollback has no ordinary logical-close event.
                // The original native terminal retires its provisional mount.
                let terminal = cx.on_window_native_terminal(move |cx, closed| {
                    if closed == owner {
                        if let Some(host) = terminal_host.upgrade() {
                            presentation::close(&mut host.borrow_mut(), owner, cx);
                        }
                    }
                });
                // Both application-wide subscriptions leave with their mount,
                // including removal from inside either terminal callback.
                self.host
                    .borrow_mut()
                    .mounts
                    .get_mut(&owner)
                    .unwrap()
                    ._close_observers = Some(open_gpui::Subscription::join(closed, terminal));
            }
            Ok(false) => {}
            Err(error) => {
                self.host.borrow_mut().fail(error.as_ref());
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
            self.interaction =
                window.with_element_state::<Rc<InteractionState>, _>(id, |state, _| {
                    let state = state.unwrap_or_default();
                    (state.clone(), state)
                });
        }
        window.set_focus_handle(&self.host.borrow().mounts[&owner].focus, cx);
        match self.prepare(bounds, capture, window, cx) {
            Ok(prepared) => PreparedState::Frame(prepared),
            Err(error) => {
                self.host.borrow_mut().fail(error.as_ref());
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
                        let hovered = self.interaction.hovered_tab.get() == Some(*item);
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
                    PreparedDraw::Icon(bounds, icon, enabled) => {
                        chrome::paint_icon(*bounds, *icon, *enabled, &colors, window);
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
        let mut schedule_host_work = false;
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
                                        native_preview: None,
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
                    region.bounds == region_bounds.get(surface).copied()
                        && (mount.capacity_blocked || (!region.dirty && region.packet.is_some()))
                })
            {
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
                                        native_preview: None,
                                    })
                                }),
                            )
                        })
                        .collect(),
                    mount.serial,
                    publication,
                    false,
                )
            } else {
                let inputs = std::mem::take(&mut host.inputs);
                let candidate = build_candidate(
                    &mut host,
                    &inputs,
                    &region_bounds,
                    self.interaction.menu.get(),
                    window,
                    cx,
                );
                let candidate = match candidate {
                    Ok(candidate) => candidate,
                    Err(error) => {
                        let mut inputs = inputs;
                        inputs.append(&mut host.inputs);
                        host.inputs = inputs;
                        return Err(error);
                    }
                };
                match candidate {
                    Candidate::CapacityDeferred(report) => {
                        // This input-only publication installs no new artifact or
                        // submission identity. Keep the displayed cohort dirty
                        // until its renderer budget is released by a real terminal.
                        schedule_host_work = !report.inputs().is_empty()
                            || report.native_input_pending()
                            || !report.surface_presentation_commits().is_empty()
                            || !report.native_admissions().is_empty()
                            || !report.native_placement_detachments().is_empty()
                            || !report.native_root_continuations().is_empty();
                        let released_capacity = !report.surface_presentation_commits().is_empty();
                        if released_capacity {
                            presentation::wake_capacity_blocked(&mut host, cx);
                        }
                        host.apply_report(report);
                        let mount = host.mounts.get_mut(&owner).expect("physical mount");
                        mount.capacity_blocked = true;
                        for region in mount.regions.values_mut() {
                            region.dirty = true;
                        }
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
                                                native_preview: None,
                                            })
                                        }),
                                    )
                                })
                                .collect(),
                            mount.serial,
                            publication,
                            false,
                        )
                    }
                    Candidate::Painted { packets, update } => {
                        let ticket =
                            match host_native::provisional::begin_submission(&host, window, cx) {
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
                        mount.capacity_blocked = false;
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
                }
            }
        };
        if schedule_host_work {
            host_native::schedule(&self.host, cx);
        }
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
        let hovered_tab = self.interaction.hovered_tab.clone();
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
        let interaction = self.interaction.clone();
        let visible_panes: Vec<_> = regions
            .iter()
            .map(|region| (region.surface, region.visible_panes.clone()))
            .collect();
        let applied_panes = visible_panes.clone();
        let admitted_surfaces: Vec<_> = regions
            .iter()
            .filter(|region| region.admitted)
            .map(|region| region.surface)
            .collect();
        let focus_controls: Vec<_> = regions
            .iter()
            .flat_map(|region| {
                region.controls.iter().map(move |(control, focus, _)| {
                    (region.surface, control.clone(), focus.clone())
                })
            })
            .collect();
        let retained_focus: std::collections::BTreeSet<_> = packets
            .values()
            .flat_map(|packet| packet.draws.iter())
            .filter_map(|draw| match draw {
                Draw::Control(control) => Some(control.id),
                Draw::TabList(_, id) => Some(*id),
                Draw::PresentationMenu(menu) => Some(menu.id),
                _ => None,
            })
            .collect();
        window.record_prepaint_window_transaction(
            publication,
            move |fence, window, cx| {
                presentation::accept(&shared, serial, input_frames.clone(), fence, window, cx);
                apply_chrome_focus(
                    &shared,
                    &interaction,
                    serial,
                    &admitted_surfaces,
                    &focus_controls,
                    window,
                    cx,
                );
                apply_pane_focus(&shared, &interaction, serial, &applied_panes, window, cx);
                // Old prepared elements retain their own handles until actual
                // retirement; the identity cache need not pin closed controls.
                let pending = interaction
                    .pending_tab_focus
                    .borrow()
                    .as_ref()
                    .map(|focus| focus.visual);
                interaction
                    .focus
                    .borrow_mut()
                    .retain(|id, _| retained_focus.contains(id) || pending == Some(*id));
                interaction
                    .pane_focus
                    .borrow_mut()
                    .retain(|surface, attempt| {
                        shared.borrow().session.pane_focus_request(*surface)
                            == Some(attempt.request)
                    });
                let current = hovered_tab_at(&accepted_tabs, window.mouse_position(), window);
                if hovered_tab.replace(current) != current {
                    window.refresh();
                }
            },
            move |_, window, cx| presentation::discard(&discard, window, cx),
        );
        let shared = self.host.clone();
        let interaction = self.interaction.clone();
        window.record_prepaint_focus_stable_commit(move |_, window, cx| {
            presentation::accept_focus_stable(&shared, serial, window, cx);
            observe_pane_focus(&shared, &interaction, serial, &visible_panes, window, cx);
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
        let mut controls = Vec::new();
        let mut visible_panes = Vec::new();
        let mut menus = Vec::new();
        let receiver_controls = packet
            .draws
            .iter()
            .filter_map(|draw| match draw {
                Draw::Control(control) => control
                    .receiver
                    .map(|receiver| (receiver.visual_id(), control)),
                _ => None,
            })
            .collect::<BTreeMap<_, _>>();
        let mut prepared_controls = BTreeMap::new();
        let config = self.host.borrow().config.clone();
        let admitted = allow_input
            && input::is_presented(
                &self.host.borrow(),
                window.window_handle().window_id(),
                surface,
                serial,
            );
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
                Draw::NativePreview(preview) => {
                    let native = host_native::inspection(&self.host.borrow());
                    if preview.host_surface == surface
                        && native.is_none_or(|native| {
                            !native.provisional_opening && native.provisional_promotion_count == 0
                        })
                    {
                        // When the physical placement lies outside this client (or
                        // its desktop origin is unavailable), use a small source cue.
                        // This decoration never claims to be destination geometry.
                        let rect = source_preview_bounds(
                            *preview,
                            bounds,
                            window.platform_facts().physical_geometry,
                        )
                        .unwrap_or_else(|| {
                            Bounds::new(
                                bounds.origin,
                                size(
                                    px(180.0).min(bounds.size.width),
                                    px(28.0).min(bounds.size.height),
                                ),
                            )
                        });
                        draws.push(PreparedDraw::Preview(
                            rect,
                            config.colors.preview,
                            config.colors.accent,
                        ));
                        let mut label = div()
                            .w(rect.size.width)
                            .h(px(28.0).min(rect.size.height))
                            .overflow_hidden()
                            .font(config.font.clone())
                            .text_size(config.text_size)
                            .text_color(config.colors.text)
                            .child("Move to New Window")
                            .into_any_element();
                        let clip =
                            window.prepare_subtree_clip(&SubtreeClip::own_border_box(), rect);
                        let _ = window.with_prepared_subtree_clip(&clip, |window| {
                            label.prepaint_as_root(rect.origin, definite(rect), window, cx);
                        });
                        draws.push(PreparedDraw::ClippedElement(label, clip));
                    }
                }
                Draw::Icon(rect, icon, enabled) => {
                    draws.push(PreparedDraw::Icon(to_gpui(*rect, bounds), *icon, *enabled));
                }
                Draw::Control(control) => {
                    if let Some(element) = prepared_controls.remove(&control.id) {
                        if let Some((element, clip)) = element {
                            draws.push(PreparedDraw::ClippedElement(element, clip));
                        }
                        continue;
                    }
                    let focus = self.control_focus(control.id, cx);
                    let (element, hitbox) = self.prepare_control(
                        control,
                        focus.clone(),
                        surface,
                        serial,
                        bounds,
                        allow_input,
                        window,
                        cx,
                    );
                    if let Some((element, clip)) = element {
                        draws.push(PreparedDraw::ClippedElement(element, clip));
                    }
                    if let Some(hitbox) = hitbox {
                        hitboxes.push(hitbox);
                        if let Some(receiver) = control.receiver {
                            receivers.push((receiver, hitbox));
                        }
                        controls.push((control.clone(), focus, hitbox));
                        if let ControlKind::Tab(item) = control.kind {
                            tabs.retain(|(tab, _)| *tab != item);
                            tabs.push((item, hitbox));
                        }
                    }
                }
                Draw::PresentationMenu(anchor) => {
                    menus.push(anchor.clone());
                }
                Draw::TabList(rect, id) => {
                    let rect = to_gpui(*rect, bounds);
                    let identity = self.control_focus(*id, cx);
                    let mut element = div()
                        .id(&identity)
                        .w(rect.size.width)
                        .h(rect.size.height)
                        .role(Role::TabList)
                        .aria_label("Pane tabs")
                        .aria_actions([])
                        .into_any_element();
                    element.prepaint_as_root(rect.origin, definite(rect), window, cx);
                    let clip = window.prepare_subtree_clip(&SubtreeClip::own_border_box(), rect);
                    draws.push(PreparedDraw::ClippedElement(element, clip));
                }
                Draw::Receiver(receiver) => {
                    // Prepaint focusable chrome at the original core receiver's
                    // position in the hit graph, using its one actual hitbox as
                    // the receiver authority. Inert chrome retains the raw
                    // receiver until the matching output is admitted.
                    let control_hitbox = if let Some(&control) = receiver_controls
                        .get(&receiver.visual_id())
                        .filter(|control| control.receiver == Some(*receiver))
                    {
                        let focus = self.control_focus(control.id, cx);
                        let (element, hitbox) = self.prepare_control(
                            control,
                            focus.clone(),
                            surface,
                            serial,
                            bounds,
                            allow_input,
                            window,
                            cx,
                        );
                        prepared_controls.insert(control.id, element);
                        if let Some(hitbox) = hitbox {
                            controls.push((control.clone(), focus, hitbox));
                        }
                        hitbox
                    } else {
                        None
                    };
                    let hitbox = control_hitbox.unwrap_or_else(|| {
                        window
                            .insert_hitbox(
                                to_gpui(receiver.bounds(), bounds),
                                HitboxBehavior::Normal,
                            )
                            .id
                    });
                    hitboxes.push(hitbox);
                    receivers.push((*receiver, hitbox));
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
                    if presentation == SubtreePresentation::Visible {
                        visible_panes.push(*item);
                    }
                    let label = self
                        .host
                        .borrow()
                        .panels
                        .get(item)
                        .map(|pane| pane.title.clone());
                    let mut element = match view {
                        Some(view) => div()
                            .id(format!("pane-{}", item.get()))
                            .w(rect.size.width)
                            .h(rect.size.height)
                            .overflow_hidden()
                            .role(Role::TabPanel)
                            .aria_label(label.unwrap_or_else(|| "Pane".into()))
                            .aria_actions([])
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

        for menu in menus {
            self.prepare_presentation_menu(
                &menu,
                surface,
                serial,
                bounds,
                allow_input,
                &mut draws,
                window,
                cx,
            );
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
            controls,
            visible_panes,
            admitted,
        }
    }

    fn control_focus(&self, id: DockspaceVisualId, cx: &mut App) -> FocusHandle {
        self.interaction
            .focus
            .borrow_mut()
            .entry(id)
            .or_insert_with(|| cx.focus_handle())
            .clone()
    }

    fn prepare_control(
        &self,
        control: &Rc<Control>,
        focus: FocusHandle,
        surface: SurfaceId,
        serial: u64,
        bounds: Bounds<Pixels>,
        allow_input: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> (Option<(AnyElement, PreparedSubtreeClip)>, Option<HitboxId>) {
        let admitted = allow_input
            && input::is_presented(
                &self.host.borrow(),
                window.window_handle().window_id(),
                surface,
                serial,
            );
        let enabled = control
            .actions
            .iter()
            .any(|(_, action)| action.borrow().is_some());
        let mut actions = Vec::new();
        if enabled {
            actions.push(AccessibleAction::Focus);
            if control.slot(ControlAction::Activate).is_some() {
                actions.push(AccessibleAction::Click);
            }
            if control.slot(ControlAction::Increment).is_some() {
                actions.push(AccessibleAction::Increment);
            }
            if control.slot(ControlAction::Decrement).is_some() {
                actions.push(AccessibleAction::Decrement);
            }
        }
        let rect = to_gpui(control.bounds, bounds);
        let shared = self.host.clone();
        let interaction = self.interaction.clone();
        let keyboard_control = control.clone();
        let mut element = div()
            .id(&focus)
            .debug_selector({
                let selector = format!("dockspace:control:{focus:?}");
                move || selector.clone()
            })
            .w(rect.size.width)
            .h(rect.size.height)
            .role(control.role())
            .aria_label(control.label.clone())
            .aria_disabled(!enabled)
            .aria_actions(actions)
            .when(enabled, |element| {
                element.track_focus(&focus).tab_stop(match control.kind {
                    ControlKind::Tab(_) => control.selected == Some(true),
                    _ => true,
                })
            })
            .when_some(control.selected, |element, selected| {
                element.aria_selected(selected)
            })
            .when_some(control.expanded, |element, expanded| {
                element.aria_expanded(expanded)
            })
            .when_some(control.value, |element, (value, minimum, maximum)| {
                element
                    .aria_numeric_value(value)
                    .aria_min_numeric_value(minimum)
                    .aria_max_numeric_value(maximum)
            })
            .on_key_down(move |event, window, cx| {
                if event.keystroke.modifiers != open_gpui::Modifiers::none() {
                    return;
                }
                if let Some(slot) = keyboard_control
                    .key_action(&event.keystroke.key)
                    .and_then(|key| keyboard_control.slot(key))
                    && activate_action(&shared, &interaction, surface, serial, &slot, window, cx)
                {
                    cx.stop_propagation();
                }
            });
        if let ControlKind::Splitter(axis) = control.kind {
            element = element.aria_orientation(match axis {
                dockspace::model::DockspaceAxis::Horizontal => open_gpui::Orientation::Vertical,
                dockspace::model::DockspaceAxis::Vertical => open_gpui::Orientation::Horizontal,
            });
        }
        for (action, key) in [
            (AccessibleAction::Click, ControlAction::Activate),
            (AccessibleAction::Increment, ControlAction::Increment),
            (AccessibleAction::Decrement, ControlAction::Decrement),
        ] {
            if let Some(slot) = control.slot(key) {
                let shared = self.host.clone();
                let interaction = self.interaction.clone();
                element = element.on_a11y_action(action, move |_, window, cx| {
                    activate_action(&shared, &interaction, surface, serial, &slot, window, cx);
                });
            }
        }
        if self.host.borrow().native.is_none()
            && matches!(control.kind, ControlKind::Button | ControlKind::MenuRow)
            && let Some(slot) = control.slot(ControlAction::Activate)
        {
            let shared = self.host.clone();
            let pointer_focus = focus.clone();
            element = element.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                if input::is_presented(
                    &shared.borrow(),
                    window.window_handle().window_id(),
                    surface,
                    serial,
                ) {
                    window.focus(&pointer_focus, cx);
                }
                cx.stop_propagation();
            });
            let shared = self.host.clone();
            let interaction = self.interaction.clone();
            element = element.on_click(move |_, window, cx| {
                activate_action(&shared, &interaction, surface, serial, &slot, window, cx);
                cx.stop_propagation();
            });
        }
        if enabled {
            let shared = self.host.clone();
            element = element.on_a11y_action(AccessibleAction::Focus, move |_, window, cx| {
                if input::is_presented(
                    &shared.borrow(),
                    window.window_handle().window_id(),
                    surface,
                    serial,
                ) {
                    window.focus(&focus, cx);
                }
            });
        }
        let hitbox = Rc::new(Cell::new(None));
        let mut element = chrome::ControlElement {
            inner: element,
            hitbox: hitbox.clone(),
        }
        .into_any_element()
        .with_subtree_presentation(if admitted {
            SubtreePresentation::Visible
        } else {
            SubtreePresentation::Inert
        })
        .into_any_element();
        let clip = window.prepare_subtree_clip(&SubtreeClip::own_border_box(), rect);
        let prepared = window
            .with_prepared_subtree_clip(&clip, |window| {
                element.prepaint_as_root(rect.origin, definite(rect), window, cx);
            })
            .map(|()| (element, clip));
        (prepared, hitbox.get())
    }

    fn prepare_presentation_menu(
        &self,
        menu: &Rc<PresentationMenu>,
        surface: SurfaceId,
        serial: u64,
        bounds: Bounds<Pixels>,
        allow_input: bool,
        draws: &mut Vec<PreparedDraw>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let rect = to_gpui(menu.bounds, bounds);
        let focus = self.control_focus(menu.id, cx);
        let admitted = allow_input
            && input::is_presented(
                &self.host.borrow(),
                window.window_handle().window_id(),
                surface,
                serial,
            );
        let enabled = admitted && menu.enabled;
        let open = enabled && self.interaction.menu.get() == Some(menu.id);
        draws.push(PreparedDraw::Icon(rect, Icon::Menu, menu.enabled));
        let interaction = self.interaction.clone();
        let id = menu.id;
        let shared = self.host.clone();
        let selector = format!("dockspace:presentation-menu:{focus:?}");
        let mut button = div()
            .id(&focus)
            .debug_selector(move || selector.clone())
            .w(rect.size.width)
            .h(rect.size.height)
            .role(Role::Button)
            .aria_label("Window actions")
            .aria_expanded(open)
            .aria_disabled(!menu.enabled)
            .aria_actions(if enabled {
                vec![AccessibleAction::Click, AccessibleAction::Focus]
            } else {
                vec![]
            })
            .when(enabled, |button| button.track_focus(&focus).tab_stop(true))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(move |_, window, cx| {
                if enabled
                    && input::is_presented(
                        &shared.borrow(),
                        window.window_handle().window_id(),
                        surface,
                        serial,
                    )
                {
                    interaction
                        .menu
                        .set((interaction.menu.get() != Some(id)).then_some(id));
                    interaction.menu_row.set(0);
                    refresh_menu(&shared, surface, window);
                    window.refresh();
                }
                cx.stop_propagation();
            });
        let interaction = self.interaction.clone();
        let shared = self.host.clone();
        let keyboard_menu = menu.clone();
        button = button.on_key_down(move |event, window, cx| {
            if enabled && event.keystroke.modifiers == open_gpui::Modifiers::none() {
                let open = interaction.menu.get() == Some(id);
                let row = interaction.menu_row.get();
                match event.keystroke.key.as_str() {
                    "enter" | "space" if open => {
                        if let Some((_, Some(action))) = keyboard_menu.choices.get(row)
                            && activate_action(
                                &shared,
                                &interaction,
                                surface,
                                serial,
                                action,
                                window,
                                cx,
                            )
                        {
                            interaction.menu.set(None);
                        }
                    }
                    "enter" | "space" => {
                        interaction.menu.set(Some(id));
                        interaction.menu_row.set(0);
                    }
                    "down" => {
                        interaction.menu.set(Some(id));
                        interaction.menu_row.set(if open {
                            (row + 1) % keyboard_menu.choices.len().max(1)
                        } else {
                            0
                        });
                    }
                    "up" => {
                        interaction.menu.set(Some(id));
                        interaction.menu_row.set(if row == 0 {
                            keyboard_menu.choices.len().saturating_sub(1)
                        } else {
                            row - 1
                        });
                    }
                    "home" if open => interaction.menu_row.set(0),
                    "end" if open => interaction
                        .menu_row
                        .set(keyboard_menu.choices.len().saturating_sub(1)),
                    "escape" => interaction.menu.set(None),
                    _ => return,
                }
                if !open {
                    refresh_menu(&shared, surface, window);
                }
                window.refresh();
                cx.stop_propagation();
            }
        });
        let interaction = self.interaction.clone();
        let shared = self.host.clone();
        button = button.on_a11y_action(AccessibleAction::Click, move |_, window, _| {
            if enabled {
                interaction
                    .menu
                    .set((interaction.menu.get() != Some(id)).then_some(id));
                interaction.menu_row.set(0);
                refresh_menu(&shared, surface, window);
                window.refresh();
            }
        });
        let mut button = button
            .with_subtree_presentation(if admitted {
                SubtreePresentation::Visible
            } else {
                SubtreePresentation::Inert
            })
            .into_any_element();
        button.prepaint_as_root(rect.origin, definite(rect), window, cx);
        draws.push(PreparedDraw::ClippedElement(
            button,
            window.prepare_subtree_clip(&SubtreeClip::own_border_box(), rect),
        ));
        if !open {
            return;
        }

        let colors = self.host.borrow().config.colors.clone();
        let width = px(210.0).min(bounds.size.width);
        let height = px(28.0) * menu.choices.len() as f32;
        let popup_bounds = Bounds::new(
            point(
                rect.right().min(bounds.right() - width).max(bounds.left()),
                rect.bottom()
                    .min((bounds.bottom() - height).max(bounds.top())),
            ),
            size(width, height.min(bounds.size.height)),
        );
        let interaction = self.interaction.clone();
        let popup_id = ElementId::NamedChild(std::sync::Arc::new((&focus).into()), "popup".into());
        let mut popup = div()
            .id(popup_id)
            .w(width)
            .h(popup_bounds.size.height)
            .flex()
            .flex_col()
            .overflow_hidden()
            .overflow_y_scroll()
            .bg(colors.selected_tab)
            .border_1()
            .border_color(colors.border)
            .role(Role::Menu)
            .aria_label("Window actions")
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down_out(move |event, window, _| {
                if !rect.contains(&event.window_event().position) {
                    interaction.menu.set(None);
                    window.refresh();
                }
            });
        for (index, (label, action)) in menu.choices.iter().enumerate() {
            let action = action.clone();
            let shared = self.host.clone();
            let interaction = self.interaction.clone();
            let mut row = div()
                .id(index)
                .w_full()
                .h(px(28.0))
                .flex_shrink_0()
                .px(px(8.0))
                .flex()
                .items_center()
                .role(Role::MenuItem)
                .aria_label(*label)
                .aria_disabled(action.is_none())
                .aria_selected(self.interaction.menu_row.get() == index)
                .aria_actions(if action.is_some() {
                    vec![AccessibleAction::Click]
                } else {
                    vec![]
                })
                .text_color(if action.is_some() {
                    colors.text
                } else {
                    colors.muted_text
                })
                .child(*label);
            if self.interaction.menu_row.get() == index {
                row = row.bg(colors.hovered_tab);
            }
            if let Some(action) = action {
                let accessible_action = action.clone();
                let accessible_shared = shared.clone();
                let accessible_interaction = interaction.clone();
                row = row
                    .on_click(move |_, window, cx| {
                        if activate_action(
                            &shared,
                            &interaction,
                            surface,
                            serial,
                            &action,
                            window,
                            cx,
                        ) {
                            interaction.menu.set(None);
                        }
                        cx.stop_propagation();
                    })
                    .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                        if activate_action(
                            &accessible_shared,
                            &accessible_interaction,
                            surface,
                            serial,
                            &accessible_action,
                            window,
                            cx,
                        ) {
                            accessible_interaction.menu.set(None);
                        }
                    });
            }
            popup = popup.child(row);
        }
        let mut popup = popup.into_any_element();
        let clip = window.prepare_subtree_clip(&SubtreeClip::own_border_box(), popup_bounds);
        let _ = window.with_prepared_subtree_clip(&clip, |window| {
            popup.prepaint_as_root(popup_bounds.origin, definite(popup_bounds), window, cx);
        });
        draws.push(PreparedDraw::ClippedElement(popup, clip));
    }

    fn listen(&self, prepared: &PreparedRegion, window: &mut Window) {
        let surface = prepared.surface;
        let owner = window.window_handle().window_id();
        let tabs = prepared.tabs.clone();
        let hovered = self.interaction.hovered_tab.clone();
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
        let controls = prepared.controls.clone();
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
                let focus = controls
                    .iter()
                    .rev()
                    .find_map(|(_, focus, hitbox)| {
                        (hitbox.window_point_target_rank(event.position, window) == Some(0))
                            .then(|| focus.clone())
                    })
                    .unwrap_or_else(|| shared.borrow().mounts[&owner].focus.clone());
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
        let interaction = self.interaction.clone();
        let controls = prepared.controls.clone();
        window.on_key_event(move |event: &KeyDownEvent, phase, window, cx| {
            if !phase.bubble() || capture.is_captured(window) {
                return;
            }
            let modifiers = event.keystroke.modifiers;
            if !modifiers.control || modifiers.alt || modifiers.platform || modifiers.function {
                return;
            }
            let key = match event.keystroke.key.as_str() {
                "tab" if modifiers.shift => ControlAction::Previous,
                "tab" | "pagedown" => ControlAction::Next,
                "pageup" => ControlAction::Previous,
                _ => return,
            };
            let action = {
                let host = shared.borrow();
                let scope = &host.mounts[&owner].focus;
                controls.iter().find_map(|(control, focus, _)| {
                    let ControlKind::Tab(item) = control.kind else {
                        return None;
                    };
                    let owns_focus = focus.is_focused(window)
                        || host
                            .panels
                            .get(&item)
                            .and_then(|pane| pane.content_focus.as_ref())
                            .is_some_and(|focus| focus.contains_focused(window, cx));
                    (owns_focus || (scope.is_focused(window) && control.selected == Some(true)))
                        .then(|| control.slot(key))
                        .flatten()
                })
            };
            if let Some(action) = action
                && activate_action(&shared, &interaction, surface, serial, &action, window, cx)
            {
                cx.stop_propagation();
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

fn activate_action(
    shared: &SharedHost,
    interaction: &InteractionState,
    surface: SurfaceId,
    serial: u64,
    slot: &chrome::ActionSlot,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let owner = window.window_handle().window_id();
    let mut host = shared.borrow_mut();
    if !input::is_presented(&host, owner, surface, serial)
        || host.has_unaccepted_update()
        || !host.inputs.is_empty()
        || host.session.has_pending_native_input()
        || host
            .native
            .as_ref()
            .is_some_and(|native| !native.transport.is_empty())
        || host.ensure_application_admission().is_err()
    {
        return false;
    }
    if slot
        .borrow()
        .as_ref()
        .is_some_and(|action| action.expected_version() != host.session.version())
    {
        return false;
    }
    let Some(action) = slot.borrow_mut().take() else {
        return false;
    };
    let tab_focus = action.tab_focus_visual().zip(action.tab_focus_target());
    let result = (|| -> Result<_> {
        let HostState {
            session,
            mounts,
            native,
            ..
        } = &mut *host;
        let mut frame =
            host_native::begin_frame(session, mounts, native.as_ref(), Some(window), cx)?;
        frame.submit_surface_action(action)?;
        Ok(frame.commit()?)
    })();
    match result {
        Ok(report) => {
            let rejected = report.inputs().iter().any(|input| {
                matches!(
                    input,
                    dockspace::runtime::HostInputOutcome::StaleRejected { .. }
                        | dockspace::runtime::HostInputOutcome::ProductActionRejected(_)
                        | dockspace::runtime::HostInputOutcome::CloseRejected(_)
                )
            });
            host.apply_report(report);
            // Capabilities are affine even for a valid no-op. Capture a fresh
            // plan for the next independent key or accessibility activation.
            if let Some(region) = host
                .mounts
                .get_mut(&owner)
                .and_then(|mount| mount.regions.get_mut(&surface))
            {
                region.dirty = true;
            }
            if rejected {
                window.refresh();
                return false;
            }
        }
        Err(error) => {
            host.fail(error.as_ref());
            return false;
        }
    }
    let focus = tab_focus
        .filter(|(_, item)| {
            host.session
                .view()
                .item(*item)
                .is_some_and(|item| item.surface() == surface && item.is_selected())
        })
        .map(|(visual, _)| {
            (
                visual,
                interaction
                    .focus
                    .borrow_mut()
                    .entry(visual)
                    .or_insert_with(|| cx.focus_handle())
                    .clone(),
            )
        });
    drop(host);
    if let Some((visual, focus)) = focus {
        window.focus(&focus, cx);
        *interaction.pending_tab_focus.borrow_mut() = Some(PendingTabFocus {
            surface,
            visual,
            claim_revision: window.focus_claim_revision(),
        });
    }
    window.refresh();
    cx.refresh_windows();
    host_native::schedule(shared, cx);
    true
}

fn refresh_menu(shared: &SharedHost, surface: SurfaceId, window: &mut Window) {
    if let Some(region) = shared
        .borrow_mut()
        .mounts
        .get_mut(&window.window_handle().window_id())
        .and_then(|mount| mount.regions.get_mut(&surface))
    {
        region.dirty = true;
    }
    window.refresh();
}

/// Restores framework chrome focus only after the matching output's interactive
/// replay. A later application focus claim always wins over retained UI intent.
fn apply_chrome_focus(
    shared: &SharedHost,
    interaction: &InteractionState,
    serial: u64,
    admitted: &[SurfaceId],
    controls: &[(SurfaceId, Rc<Control>, FocusHandle)],
    window: &mut Window,
    cx: &mut App,
) {
    if shared.borrow().ensure_application_admission().is_err() {
        return;
    }
    let owner = window.window_handle().window_id();
    let presented = |surface| {
        admitted.contains(&surface) && input::is_presented(&shared.borrow(), owner, surface, serial)
    };
    let pending = interaction.pending_tab_focus.borrow_mut().take();
    if let Some(pending) = pending {
        if pending.claim_revision == window.focus_claim_revision() {
            if presented(pending.surface) {
                if let Some((_, _, focus)) = controls.iter().find(|(surface, control, _)| {
                    *surface == pending.surface && control.id == pending.visual
                }) && window.is_focus_handle_rendered(focus)
                {
                    window.focus(focus, cx);
                }
            } else {
                *interaction.pending_tab_focus.borrow_mut() = Some(pending);
            }
        }
    }
    let focused_row = controls.iter().find(|(surface, control, _)| {
        presented(*surface) && control.focused && control.menu_owner.is_some()
    });
    if let Some((surface, control, focus)) = focused_row {
        let Some(bar) = control.menu_owner else {
            return;
        };
        let mut previous = interaction.tab_list_focus.borrow_mut();
        if let Some(previous) = previous.as_ref() {
            if previous.claim_revision != window.focus_claim_revision()
                || (previous.bar == bar && previous.row == control.id)
            {
                return;
            }
        } else {
            let scope = shared.borrow().mounts[&owner].focus.clone();
            if !scope.contains_focused(window, cx) && window.focused(cx).is_some() {
                return;
            }
        }
        let opener = controls
            .iter()
            .find(|(owner_surface, control, _)| {
                owner_surface == surface && control.opens_menu == Some(bar)
            })
            .map(|(_, _, focus)| focus.clone());
        if let Some(opener) = opener
            && window.is_focus_handle_rendered(focus)
        {
            window.focus(focus, cx);
            *previous = Some(TabListFocus {
                surface: *surface,
                bar,
                row: control.id,
                opener,
                claim_revision: window.focus_claim_revision(),
            });
        }
    } else {
        let mut previous = interaction.tab_list_focus.borrow_mut();
        if previous
            .as_ref()
            .is_some_and(|previous| presented(previous.surface))
            && let Some(previous) = previous.take()
        {
            let pane_requested = shared
                .borrow()
                .session
                .pane_focus_request(previous.surface)
                .is_some();
            if !pane_requested
                && previous.claim_revision == window.focus_claim_revision()
                && window.is_focus_handle_rendered(&previous.opener)
            {
                window.focus(&previous.opener, cx);
            }
        }
    }
}

fn pane_focus_requests(
    shared: &SharedHost,
    serial: u64,
    visible_panes: &[(SurfaceId, Vec<ItemId>)],
    window: &Window,
) -> Vec<(DockspacePaneFocusRequest, Option<FocusHandle>, FocusHandle)> {
    let owner = window.window_handle().window_id();
    let host = shared.borrow();
    if host.has_unaccepted_update()
        || !host.inputs.is_empty()
        || host
            .native
            .as_ref()
            .is_some_and(|native| native.transport.is_frozen())
    {
        return Vec::new();
    }
    visible_panes
        .iter()
        .filter_map(|(surface, panes)| {
            if !input::is_presented(&host, owner, *surface, serial) {
                return None;
            }
            let request = host.session.pane_focus_request(*surface)?;
            let focus = match request.item() {
                Some(item) => {
                    if !panes.contains(&item) {
                        return None;
                    }
                    host.panels
                        .get(&item)
                        .and_then(|pane| pane.content_focus.clone())
                }
                None => Some(host.mounts[&owner].focus.clone()),
            };
            Some((request, focus, host.mounts[&owner].focus.clone()))
        })
        .collect()
}

/// Applies an admitted content request in the normal accepted-publication phase.
/// FocusStable is deliberately reserved for observing the resulting authority.
fn apply_pane_focus(
    shared: &SharedHost,
    interaction: &InteractionState,
    serial: u64,
    visible_panes: &[(SurfaceId, Vec<ItemId>)],
    window: &mut Window,
    cx: &mut App,
) {
    for (request, target, scope) in pane_focus_requests(shared, serial, visible_panes, window) {
        let mut attempts = interaction.pane_focus.borrow_mut();
        let attempt = attempts
            .entry(request.surface())
            .or_insert(PaneFocusAttempt {
                request,
                applied: false,
                observation: None,
            });
        if attempt.request != request {
            *attempt = PaneFocusAttempt {
                request,
                applied: false,
                observation: None,
            };
        }
        if !attempt.applied {
            attempt.applied = true;
            if let Some(target) = target
                && window.is_focus_handle_rendered(&target)
                && (scope.contains_focused(window, cx) || window.focused(cx).is_none())
            {
                window.focus(&target, cx);
            }
        }
    }
}

fn observe_pane_focus(
    shared: &SharedHost,
    interaction: &InteractionState,
    serial: u64,
    visible_panes: &[(SurfaceId, Vec<ItemId>)],
    window: &mut Window,
    cx: &mut App,
) {
    let mut observations = Vec::new();
    for (request, target, _) in pane_focus_requests(shared, serial, visible_panes, window) {
        let observation = match target {
            Some(target) if window.is_focus_handle_rendered(&target) => {
                if target.contains_focused(window, cx) {
                    DockspacePaneFocusObservation::Focused
                } else {
                    DockspacePaneFocusObservation::NotFocused
                }
            }
            _ => DockspacePaneFocusObservation::Unavailable,
        };
        let mut attempts = interaction.pane_focus.borrow_mut();
        let attempt = attempts
            .entry(request.surface())
            .or_insert(PaneFocusAttempt {
                request,
                applied: false,
                observation: None,
            });
        if attempt.request != request {
            *attempt = PaneFocusAttempt {
                request,
                applied: false,
                observation: None,
            };
        }
        if attempt.observation != Some(observation) {
            observations.push((request, observation));
        }
    }
    if observations.is_empty() {
        return;
    }
    let mut host = shared.borrow_mut();
    let result = (|| -> Result<_> {
        let HostState {
            session,
            mounts,
            native,
            ..
        } = &mut *host;
        let mut frame =
            host_native::begin_frame(session, mounts, native.as_ref(), Some(window), cx)?;
        for (request, observation) in &observations {
            if frame.pane_focus_request(request.surface()) == Some(*request) {
                frame.submit_pane_focus_observation(request.prepare_observation(*observation))?;
            }
        }
        Ok(frame.commit()?)
    })();
    match result {
        Ok(report) => {
            let repaint = !report.repaint_surfaces().is_empty() || report.workspace_changed();
            host.apply_report(report);
            for (request, observation) in observations {
                if let Some(attempt) = interaction
                    .pane_focus
                    .borrow_mut()
                    .get_mut(&request.surface())
                {
                    attempt.observation = Some(observation);
                }
            }
            if repaint {
                window.refresh();
            }
        }
        Err(error) => host.fail(error.as_ref()),
    }
    drop(host);
    host_native::schedule(shared, cx);
}

fn source_preview_bounds(
    preview: host_native::provisional::Preview,
    surface: Bounds<Pixels>,
    geometry: Option<open_gpui::PlatformWindowPhysicalGeometry>,
) -> Option<Bounds<Pixels>> {
    // Do not derive desktop coordinates from logical window bounds. The observed
    // physical client origin and DPI must come from the same platform snapshot.
    let geometry = geometry?;
    let client = geometry.client_bounds();
    let scale = f64::from(geometry.scale_factor());
    let placement = preview.placement;
    let projected = Bounds::new(
        point(
            px(((placement.x() - f64::from(client.origin.x.0)) / scale) as f32),
            px(((placement.y() - f64::from(client.origin.y.0)) / scale) as f32),
        ),
        size(
            px((placement.width() / scale) as f32),
            px((placement.height() / scale) as f32),
        ),
    );
    let clipped = projected.intersect(&surface);
    (!clipped.is_empty()).then_some(clipped)
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
    open_menu: Option<DockspaceVisualId>,
    window: &Window,
    cx: &mut App,
) -> Result<Candidate> {
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
    let tab_strip = TabStripMetrics::new(0.0, 0.0)?
        .with_controls(
            TabStripControlMetrics::new(2.0)?
                .with_scroll_backward(TabStripControlMetric::new(
                    20.0,
                    TabStripControlPlacement::OverlayLeading,
                )?)
                .with_scroll_forward(TabStripControlMetric::new(
                    20.0,
                    TabStripControlPlacement::OverlayTrailing,
                )?)
                .with_tab_list_menu(TabStripControlMetric::new(
                    24.0,
                    TabStripControlPlacement::ReservedTrailing,
                )?),
        )
        .with_tab_list_menu(TabListMenuMetrics::new(28.0, 8.0, 4.0, 0.0, 280.0, 8.0)?);
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
                    native_preview: None,
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
                    native_preview: None,
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
        let work = frame.prepare_surface_with(surface, |request| -> Result<Answer> {
            Ok(match request {
                Request::DockBounds { .. } | Request::PopupPlaneBounds { .. } => {
                    Answer::Bounds(bounds)
                }
                Request::PaneSizing { item, .. } => {
                    Answer::PaneSizing(item.and_then(|item| panels.get(&item)).map_or_else(
                        || {
                            LogicalSize::new(120.0, 90.0)
                                .expect("default minimum is valid")
                                .into()
                        },
                        |pane| pane.sizing,
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
                Request::TabStrip { .. } => Answer::TabStrip(tab_strip),
            })
        })?;
        let plan = match work {
            SurfaceWork::Ready(plan) => plan,
            SurfaceWork::Deferred(SurfaceWorkDeferred::OutputCapacity) => {
                // A later region may be blocked after earlier regions confirmed
                // their paint. Discard that whole private pass while preserving
                // its accepted input, then publish without consuming output room.
                frame.restart_encoding_pass()?;
                return Ok(Candidate::CapacityDeferred(frame.commit()?));
            }
            SurfaceWork::Deferred(SurfaceWorkDeferred::Unmeasured) => {
                anyhow::ensure!(managed, "measured surface has no ready paint plan");
                // Native creation can render its real initial GPUI frame before
                // the core receives Created/Prepared. This staging packet grants
                // no painted output or content-presentation acknowledgement.
                packets.insert(
                    surface,
                    Rc::new(Packet {
                        draws: Vec::new(),
                        deferred: true,
                        native_preview: None,
                    }),
                );
                continue;
            }
        };
        let (packet, receipt) =
            record(plan, panels, pointers[&surface], &config.colors, open_menu)?;
        frame.confirm_surface_painted(receipt)?;
        packets.insert(surface, Rc::new(packet));
    }
    Ok(Candidate::Painted {
        packets,
        update: frame.suspend(),
    })
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
