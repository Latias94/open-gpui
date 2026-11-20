//! A single retained Element consuming the core's complete ordered paint program.

use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap, HashSet},
    rc::Rc,
};

use anyhow::{Context as _, Result};
use dockspace::{
    geometry::{LogicalRect, LogicalSize},
    model::{ItemId, SurfaceId, TabKey},
    runtime::{
        DockspacePaneFocusObservation, DockspacePaneFocusRequest, DockspaceReceiverDescriptor,
        DockspaceUpdate, DockspaceVisualId, HostFrameReport, PaneContentGeometry,
        SurfaceMeasurementAnswer as Answer, SurfaceMeasurementRequest as Request,
        SurfacePointerButton, SurfacePointerCancelReason, SurfacePointerEvent, SurfaceWork,
        SurfaceWorkDeferred, TabListMenuMetrics, TabListMenuRowId, TabStripControlMetric,
        TabStripControlMetrics, TabStripControlPlacement, TabStripMetrics,
    },
};
use open_gpui::{
    AccessibleAction, AnyElement, App, AvailableSpace, Bounds, Element, ElementId, FocusHandle,
    GlobalElementId, HitboxBehavior, HitboxId, Hsla, InspectorElementId, IntoElement, KeyDownEvent,
    LayoutId, MouseButton, MouseDownEvent, MouseExitEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    PointerCaptureHandle, PreparedSubtreeClip, Role, Style, SubtreeClip, SubtreePresentation,
    SubtreePresentationExt, TextRun, Window, div, fill, point, prelude::*, px, relative, size,
};

use crate::adapter::{
    GpuiDockspaceConfig, HostState, SharedHost,
    host::DockRegion,
    host_native,
    input::{self, InputFrame},
    presentation,
};

mod chrome;
mod keyboard;
mod presentation_menu;
mod recording;
mod splitter;
mod tab_label;
mod wheel;

use chrome::{Control, ControlAction, ControlKind, GuideState, Icon, PresentationMenu};
use recording::record;
use tab_label::CustomTabLabel;

#[cfg(test)]
mod interaction_tests;

pub(super) struct Packet {
    draws: Vec<Draw>,
    pane_bars: BTreeMap<ItemId, Option<DockspaceVisualId>>,
    pane_receivers: BTreeMap<ItemId, DockspaceReceiverDescriptor>,
    state: PacketState,
    pub(super) native_preview: Option<host_native::provisional::Preview>,
}

enum PacketState {
    Ready,
    Waiting,
    Unmeasured,
}

enum Candidate {
    Painted {
        packets: BTreeMap<SurfaceId, Rc<Packet>>,
        update: DockspaceUpdate,
    },
    CapacityDeferred(HostFrameReport),
}

impl Packet {
    fn empty(state: PacketState) -> Rc<Self> {
        Rc::new(Self {
            draws: Vec::new(),
            pane_bars: BTreeMap::new(),
            pane_receivers: BTreeMap::new(),
            state,
            native_preview: None,
        })
    }

    pub(super) fn is_deferred(&self) -> bool {
        !matches!(self.state, PacketState::Ready)
    }

    pub(super) fn preserves_repaint_request(&self) -> bool {
        matches!(self.state, PacketState::Unmeasured)
    }

    #[cfg(test)]
    pub(super) fn pane_items(&self) -> Vec<ItemId> {
        self.draws
            .iter()
            .filter_map(|draw| match draw {
                Draw::Pane(item, _) => Some(*item),
                _ => None,
            })
            .collect()
    }
}

enum Draw {
    Fill(LogicalRect, Hsla),
    Tab(LogicalRect, TabKey, Hsla, bool),
    ContainedFrame(LogicalRect),
    ContainedTitle(LogicalRect, Hsla),
    Guide(LogicalRect, GuideState),
    Outline(LogicalRect, Hsla),
    Text {
        layout: LogicalRect,
        clip: LogicalRect,
        title: String,
        color: Hsla,
    },
    TabLabel {
        layout: LogicalRect,
        clip: LogicalRect,
        key: TabKey,
        title: String,
        content: Option<CustomTabLabel>,
        selected: bool,
        color: Hsla,
    },
    Pane(ItemId, PaneContentGeometry),
    Receiver(DockspaceReceiverDescriptor),
    Preview(LogicalRect),
    NativePreview(host_native::provisional::Preview),
    Icon(LogicalRect, Icon, bool),
    Control(Rc<Control>),
    PresentationMenu(Rc<PresentationMenu>),
    TabList(LogicalRect, DockspaceVisualId),
    TabListMenuFocus {
        menu: DockspaceVisualId,
        row: Rc<Control>,
    },
    Scroll(wheel::ScrollTarget),
}

enum PreparedDraw {
    Fill(Bounds<Pixels>, Hsla),
    Tab(Bounds<Pixels>, TabKey, Hsla, bool),
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
    hovered_tab: Rc<Cell<Option<TabKey>>>,
    focus: RefCell<HashMap<ControlFocusId, FocusHandle>>,
    menu: Cell<Option<presentation_menu::OpenMenu>>,
    menu_sequence: Cell<u64>,
    menu_input: RefCell<Option<presentation_menu::MenuInput>>,
    menu_row: Cell<usize>,
    pane_focus: RefCell<BTreeMap<SurfaceId, PaneFocusAttempt>>,
    pending_tab_focus: RefCell<Option<PendingTabFocus>>,
    tab_list_focus: RefCell<Option<TabListFocus>>,
    wheel_gesture: Cell<Option<wheel::WheelGesture>>,
    keyboard: RefCell<Option<keyboard::Continuation>>,
    keyboard_subscription: RefCell<Option<open_gpui::Subscription>>,
    splitter_press: RefCell<Option<splitter::Press>>,
    content_press: Cell<Option<SurfaceId>>,
    content_terminal: Cell<bool>,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum ControlFocusId {
    Visual(DockspaceVisualId),
    MenuRow(TabListMenuRowId),
}

struct PaneFocusAttempt {
    request: DockspacePaneFocusRequest,
    claim_revision: Option<u64>,
    applied: bool,
    observation: Option<DockspacePaneFocusObservation>,
}

impl PaneFocusAttempt {
    fn new(request: DockspacePaneFocusRequest) -> Self {
        Self {
            request,
            claim_revision: None,
            applied: false,
            observation: None,
        }
    }

    fn owns_focus_claim(&self, window: &Window) -> bool {
        self.claim_revision
            .is_none_or(|claim| claim == window.focus_claim_revision())
    }
}

struct PendingTabFocus {
    surface: SurfaceId,
    visual: DockspaceVisualId,
    version: dockspace::model::WorkspaceVersion,
    claim_revision: u64,
}

struct TabListFocus {
    surface: SurfaceId,
    bar: DockspaceVisualId,
    row: Option<TabListMenuRowId>,
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
    content_hitboxes: Vec<HitboxId>,
    panes: Vec<(ItemId, open_gpui::HitTestSnapshot)>,
    tabs: Vec<(TabKey, HitboxId)>,
    capture: PointerCaptureHandle,
    serial: u64,
    bounds: Bounds<Pixels>,
    receivers: Vec<(DockspaceReceiverDescriptor, HitboxId)>,
    controls: Vec<(Rc<Control>, FocusHandle, HitboxId)>,
    menu_focus: Option<(Rc<Control>, FocusHandle)>,
    visible_panes: Vec<ItemId>,
    pane_bars: BTreeMap<ItemId, Option<DockspaceVisualId>>,
    admitted: bool,
    scrolls: Vec<wheel::ScrollTarget>,
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
        keyboard::install(&self.host, &self.interaction, window, cx);
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
        self.listen_content_terminals(window);
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
                                region
                                    .packet
                                    .clone()
                                    .unwrap_or_else(|| Packet::empty(PacketState::Waiting)),
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
                                region
                                    .packet
                                    .clone()
                                    .unwrap_or_else(|| Packet::empty(PacketState::Waiting)),
                            )
                        })
                        .collect(),
                    mount.serial,
                    publication,
                    false,
                )
            } else {
                let inputs = std::mem::take(&mut host.inputs);
                let candidate =
                    match begin_candidate(&mut host, &inputs, &region_bounds, window, cx) {
                        Ok((update, height, labels)) => {
                            let widths = if labels.is_empty() {
                                BTreeMap::new()
                            } else {
                                let config = host.config.clone();
                                drop(host);
                                let widths = labels
                                    .into_iter()
                                    .map(|(item, label)| {
                                        (item, label.measure(&config, height, window, cx))
                                    })
                                    .collect();
                                host = self.host.borrow_mut();
                                widths
                            };
                            encode_candidate(
                                &mut host,
                                update,
                                &region_bounds,
                                self.interaction.menu.get(),
                                &widths,
                                window,
                            )
                        }
                        Err(error) => Err(error),
                    };
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
                                        region
                                            .packet
                                            .clone()
                                            .unwrap_or_else(|| Packet::empty(PacketState::Waiting)),
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
                content_hitboxes: region.content_hitboxes.clone(),
                panes: region.panes.clone(),
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
                region
                    .controls
                    .iter()
                    .map(|(control, focus, _)| (control, focus))
                    .chain(
                        region
                            .menu_focus
                            .iter()
                            .map(|(control, focus)| (control, focus)),
                    )
                    .map(move |(control, focus)| (region.surface, control.clone(), focus.clone()))
            })
            .collect();
        let retained_focus: HashSet<_> = packets
            .values()
            .flat_map(|packet| packet.draws.iter())
            .filter_map(|draw| match draw {
                Draw::Control(control) => Some(control.focus_id()),
                Draw::TabList(_, id) | Draw::TabListMenuFocus { menu: id, .. } => {
                    Some(ControlFocusId::Visual(*id))
                }
                Draw::PresentationMenu(menu) => Some(ControlFocusId::Visual(menu.id)),
                _ => None,
            })
            .collect();
        window.record_prepaint_window_transaction(
            publication,
            move |fence, window, cx| {
                presentation::accept(&shared, serial, input_frames.clone(), fence, window, cx);
                presentation_menu::accept(&shared, &interaction, window);
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
                    .map(|focus| ControlFocusId::Visual(focus.visual));
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
                input::revoke_blocked_hover(&shared, window);
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
        let mut content_hitboxes = Vec::new();
        let mut panes = Vec::new();
        let mut draws = Vec::new();
        let mut tabs = Vec::new();
        let mut receivers = Vec::new();
        let mut controls = Vec::new();
        let mut menu_focus = None;
        let mut visible_panes = Vec::new();
        let mut menus = Vec::new();
        let mut scrolls = Vec::new();
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
                    let focus = self.control_focus(control.focus_id(), cx);
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
                Draw::Scroll(target) => scrolls.push(*target),
                Draw::TabList(rect, id) => {
                    let rect = to_gpui(*rect, bounds);
                    let identity = self.control_focus(ControlFocusId::Visual(*id), cx);
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
                Draw::TabListMenuFocus { menu, row } => {
                    if !admitted {
                        continue;
                    }
                    let rect = to_gpui(row.bounds, bounds);
                    let focus = self.control_focus(ControlFocusId::Visual(*menu), cx);
                    let shared = self.host.clone();
                    let interaction = self.interaction.clone();
                    let keyboard_row = row.clone();
                    let element = div()
                        .id(&focus)
                        .w(rect.size.width)
                        .h(rect.size.height)
                        .track_focus(&focus)
                        .tab_stop(false)
                        .role(Role::Menu)
                        .aria_label("Tab list")
                        .on_key_down(move |event, window, cx| {
                            if keyboard::handle(
                                &shared,
                                &interaction,
                                surface,
                                serial,
                                &keyboard_row,
                                &event.keystroke,
                                window,
                                cx,
                            ) {
                                cx.stop_propagation();
                            }
                        })
                        .into_any_element();
                    if let Some(draw) = prepare_label(element, rect, rect, window, cx) {
                        draws.push(draw);
                        menu_focus = Some((row.clone(), focus));
                    }
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
                        let focus = self.control_focus(control.focus_id(), cx);
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
                                if matches!(receiver.role(), dockspace::runtime::DockspaceReceiverRole::TabListMenuBlocker | dockspace::runtime::DockspaceReceiverRole::TabListMenuBackdrop) {
                                    HitboxBehavior::BlockMouse
                                } else {
                                    HitboxBehavior::Normal
                                },
                            )
                            .id
                    });
                    hitboxes.push(hitbox);
                    receivers.push((*receiver, hitbox));
                }
                Draw::Text {
                    layout,
                    clip,
                    title,
                    color,
                } => {
                    let rect = to_gpui(*layout, bounds);
                    let label = tab_label_style(&config, *color)
                        .w(rect.size.width)
                        .h(rect.size.height)
                        .overflow_hidden()
                        .child(title.clone())
                        .into_any_element();
                    draws.extend(prepare_label(
                        label,
                        rect,
                        to_gpui(*clip, bounds),
                        window,
                        cx,
                    ));
                }
                Draw::TabLabel {
                    layout,
                    clip,
                    key,
                    title,
                    content,
                    selected,
                    color,
                } => {
                    let rect = to_gpui(*layout, bounds);
                    let label = if let Some(content) = content {
                        content
                            .element(&config, *selected, *color, rect.size.height, cx)
                            .w(rect.size.width)
                            .overflow_hidden()
                            .debug_selector({
                                let item = key.item().expect("custom tab labels belong to panes");
                                move || format!("dockspace:tab:{}", item.get())
                            })
                            .into_any_element()
                    } else {
                        tab_label_style(&config, *color)
                            .w(rect.size.width)
                            .h(rect.size.height)
                            .overflow_hidden()
                            .when_some(key.item(), |label, item| {
                                label
                                    .debug_selector(move || format!("dockspace:tab:{}", item.get()))
                            })
                            .child(title.clone())
                            .into_any_element()
                    }
                    .with_subtree_presentation(SubtreePresentation::Inert)
                    .into_any_element();
                    draws.extend(prepare_label(
                        label,
                        rect,
                        to_gpui(*clip, bounds),
                        window,
                        cx,
                    ));
                }
                Draw::Pane(item, geometry) => {
                    let rect = to_gpui(geometry.layout_bounds(), bounds);
                    let clip = window.prepare_subtree_clip(
                        &SubtreeClip::own_border_box(),
                        to_gpui(geometry.clip(), bounds),
                    );
                    let geometry = window
                        .with_prepared_subtree_clip(&clip, |window| window.hit_test_snapshot(rect));
                    let unchanged = geometry.as_ref().is_some_and(|geometry| {
                        let host = self.host.borrow();
                        let owner = window.window_handle().window_id();
                        host.mounts[&owner]
                            .regions
                            .get(&surface)
                            .filter(|region| {
                                region.presented.is_some_and(|(serial, _)| {
                                    input::is_presented(&host, owner, surface, serial)
                                })
                            })
                            .and_then(|region| region.presented_panes.get(item))
                            == Some(geometry)
                    });
                    if let Some(geometry) = geometry {
                        panes.push((*item, geometry));
                    }
                    let pane = self.host.borrow().panels.get(item).cloned();
                    let presentation = if allow_input
                        && (unchanged
                            || input::is_presented(
                                &self.host.borrow(),
                                window.window_handle().window_id(),
                                surface,
                                serial,
                            )) {
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
                    let mut element = match pane.as_ref() {
                        Some(pane) => div()
                            .id(format!("pane-{}", item.get()))
                            .w(rect.size.width)
                            .h(rect.size.height)
                            .overflow_hidden()
                            .role(Role::TabPanel)
                            .aria_label(label.unwrap_or_else(|| "Pane".into()))
                            .aria_actions([])
                            .child(pane.view.clone())
                            .with_subtree_presentation(presentation)
                            .into_any_element(),
                        None => div()
                            .w(rect.size.width)
                            .h(rect.size.height)
                            .child("Pane unavailable")
                            .into_any_element(),
                    };
                    if window
                        .with_prepared_subtree_clip(&clip, |window| {
                            if let Some(pane) = &pane
                                && !pane.content_receivers.is_empty()
                                && let Some(receiver) = packet.pane_receivers.get(item)
                            {
                                let ((), targets) = window.capture_prepaint_hitboxes(|window| {
                                    element.prepaint_as_root(
                                        rect.origin,
                                        definite(rect),
                                        window,
                                        cx,
                                    );
                                });
                                for hitbox in targets {
                                    if hitbox
                                        .tag()
                                        .is_some_and(|tag| pane.content_receivers.contains(&tag))
                                    {
                                        content_hitboxes.push(hitbox.id);
                                        receivers.push((*receiver, hitbox.id));
                                    }
                                }
                            } else {
                                element.prepaint_as_root(rect.origin, definite(rect), window, cx);
                            }
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
            content_hitboxes,
            panes,
            tabs,
            capture,
            serial,
            bounds,
            receivers,
            controls,
            menu_focus,
            visible_panes,
            pane_bars: packet.pane_bars.clone(),
            admitted,
            scrolls,
        }
    }

    fn control_focus(&self, id: ControlFocusId, cx: &mut App) -> FocusHandle {
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
        let menu_row = control.menu_row.is_some();
        let enabled = menu_row
            || control
                .actions
                .iter()
                .any(|(_, action)| action.borrow().is_some());
        let mut actions = Vec::new();
        if enabled {
            actions.push(AccessibleAction::Focus);
            if control.has_action(ControlAction::Activate) {
                actions.push(AccessibleAction::Click);
            }
            if control.has_action(ControlAction::Increment) {
                actions.push(AccessibleAction::Increment);
            }
            if control.has_action(ControlAction::Decrement) {
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
                if keyboard::handle(
                    &shared,
                    &interaction,
                    surface,
                    serial,
                    &keyboard_control,
                    &event.keystroke,
                    window,
                    cx,
                ) {
                    cx.stop_propagation();
                }
            });
        if let ControlKind::Splitter(axis) = control.kind {
            element = element.aria_orientation(match axis {
                dockspace::model::DockspaceAxis::Horizontal => open_gpui::Orientation::Vertical,
                dockspace::model::DockspaceAxis::Vertical => open_gpui::Orientation::Horizontal,
            });
        }
        if enabled {
            element = element.cursor(match control.kind {
                ControlKind::Splitter(dockspace::model::DockspaceAxis::Horizontal) => {
                    open_gpui::CursorStyle::ResizeLeftRight
                }
                ControlKind::Splitter(dockspace::model::DockspaceAxis::Vertical) => {
                    open_gpui::CursorStyle::ResizeUpDown
                }
                ControlKind::Junction => open_gpui::CursorStyle::Crosshair,
                _ => open_gpui::CursorStyle::PointingHand,
            });
        }
        for (action, key) in [
            (AccessibleAction::Click, ControlAction::Activate),
            (AccessibleAction::Increment, ControlAction::Increment),
            (AccessibleAction::Decrement, ControlAction::Decrement),
        ] {
            if control.has_action(key) {
                let shared = self.host.clone();
                let interaction = self.interaction.clone();
                let control = control.clone();
                element = element.on_a11y_action(action, move |_, window, cx| {
                    if let Some(slot) = control.slot(key) {
                        activate_action(&shared, &interaction, surface, serial, &slot, window, cx);
                    }
                });
            }
        }
        if self.host.borrow().native.is_none()
            && matches!(control.kind, ControlKind::Button | ControlKind::MenuRow)
            && control.has_action(ControlAction::Activate)
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
            let control = control.clone();
            element = element.on_click(move |_, window, cx| {
                if let Some(slot) = control.slot(ControlAction::Activate) {
                    activate_action(&shared, &interaction, surface, serial, &slot, window, cx);
                }
                cx.stop_propagation();
            });
        }
        if enabled {
            let shared = self.host.clone();
            let interaction = self.interaction.clone();
            let control = control.clone();
            element = element.on_a11y_action(AccessibleAction::Focus, move |_, window, cx| {
                if control.menu_row.is_some() {
                    keyboard::focus_menu_row(
                        &shared,
                        &interaction,
                        surface,
                        serial,
                        &control,
                        &focus,
                        window,
                        cx,
                    );
                } else if input::is_presented(
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

    fn listen_content_terminals(&self, window: &mut Window) {
        let shared = self.host.clone();
        let interaction = self.interaction.clone();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, _| {
            if phase.capture() {
                interaction.content_terminal.set(false);
                if event.button == MouseButton::Left {
                    interaction.content_terminal.set(finish_content_press(
                        &shared,
                        &interaction,
                        SurfacePointerEvent::ButtonReleased(SurfacePointerButton::Primary),
                        event.position,
                        event.modifiers,
                        window,
                    ));
                }
            }
        });
        let shared = self.host.clone();
        let interaction = self.interaction.clone();
        window.on_pointer_cancel(move |_, phase, window, _| {
            if phase.capture() {
                interaction.content_terminal.set(finish_content_press(
                    &shared,
                    &interaction,
                    SurfacePointerEvent::StreamCancelled(
                        SurfacePointerCancelReason::ExplicitPlatformCancellation,
                    ),
                    window.mouse_position(),
                    window.modifiers(),
                    window,
                ));
            }
        });
    }

    fn listen(&self, prepared: &PreparedRegion, window: &mut Window) {
        wheel::listen(&self.host, &self.interaction, prepared, window);
        let surface = prepared.surface;
        let capture = prepared.capture;
        let serial = prepared.serial;
        let bounds = prepared.bounds;
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
        let content_hitboxes = prepared.content_hitboxes.clone();
        let interaction = self.interaction.clone();
        window.on_mouse_event(move |event: &MouseExitEvent, phase, window, _| {
            if !phase.capture() {
                return;
            }
            if interaction.hovered_tab.take().is_some() {
                window.refresh();
            }
            let mut host = shared.borrow_mut();
            if host.native.is_none()
                && capture.is_captured(window)
                && routes_to(&host, surface, event.position, window)
                && input::queue(
                    &mut host,
                    surface,
                    serial,
                    SurfacePointerEvent::Moved,
                    event.position,
                    bounds,
                    &hitboxes,
                    &content_hitboxes,
                    true,
                    window.modifiers(),
                    window,
                )
            {
                window.refresh();
            }
        });
        let shared = self.host.clone();
        let hitboxes = prepared.hitboxes.clone();
        let content_hitboxes = prepared.content_hitboxes.clone();
        let controls = prepared.controls.clone();
        let interaction = self.interaction.clone();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
            if phase.capture() && event.button == MouseButton::Left {
                interaction.splitter_press.borrow_mut().take();
                if !capture.is_captured(window)
                    && input::is_presented(&shared.borrow(), owner, surface, serial)
                    && content_hitboxes
                        .iter()
                        .any(|id| id.window_point_target_rank(event.position, window) == Some(0))
                    && input::queue(
                        &mut shared.borrow_mut(),
                        surface,
                        serial,
                        SurfacePointerEvent::ButtonPressed(SurfacePointerButton::Primary),
                        event.position,
                        bounds,
                        &hitboxes,
                        &content_hitboxes,
                        false,
                        event.modifiers,
                        window,
                    )
                {
                    // Observe pane activation without capturing or consuming the widget's input.
                    interaction.content_press.set(Some(surface));
                    window.refresh();
                }
            }
            if !phase.bubble() || event.button != MouseButton::Left {
                return;
            }
            let clear = hitboxes
                .iter()
                .any(|id| id.window_point_target_rank(event.position, window) == Some(0));
            if !input::is_presented(&shared.borrow(), owner, surface, serial) {
                return;
            }
            if !clear {
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
                &content_hitboxes,
                !managed,
                event.modifiers,
                window,
            ) {
                splitter::pressed(
                    &shared.borrow(),
                    &interaction,
                    surface,
                    &controls,
                    event,
                    window,
                );
                let control = controls.iter().rev().find_map(|(control, focus, hitbox)| {
                    (hitbox.window_point_target_rank(event.position, window) == Some(0))
                        .then_some((control, focus))
                });
                if let Some((control, focus)) = control {
                    window.focus(focus, cx);
                    if let Some(bar) = control.opens_menu {
                        keyboard::remember_menu_opener(&interaction, surface, bar, focus, window);
                    }
                } else {
                    let focus = shared.borrow().mounts[&owner].focus.clone();
                    window.focus(&focus, cx);
                }
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
        let content_hitboxes = prepared.content_hitboxes.clone();
        let interaction = self.interaction.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, _| {
            if phase.capture() {
                splitter::moved(&interaction, event.position);
            }
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
                &content_hitboxes,
                true,
                event.modifiers,
                window,
            ) {
                window.refresh();
            }
        });
        let shared = self.host.clone();
        let hitboxes = prepared.hitboxes.clone();
        let content_hitboxes = prepared.content_hitboxes.clone();
        let receivers = prepared.receivers.clone();
        let interaction = self.interaction.clone();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
            if !phase.bubble()
                || interaction.content_terminal.get()
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
                &content_hitboxes,
                capture.is_captured(window),
                event.modifiers,
                window,
            ) {
                splitter::released(
                    &mut shared.borrow_mut(),
                    &interaction,
                    surface,
                    &receivers,
                    event,
                    window,
                    cx,
                );
                let _ = window.release_pointer(&capture);
                window.refresh();
                cx.stop_propagation();
            }
        });
        let shared = self.host.clone();
        let hitboxes = prepared.hitboxes.clone();
        let content_hitboxes = prepared.content_hitboxes.clone();
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
                &content_hitboxes,
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
        let pane_bars = prepared.pane_bars.clone();
        window.on_key_event(move |event: &KeyDownEvent, phase, window, cx| {
            if !phase.bubble() || capture.is_captured(window) {
                return;
            }
            if keyboard::document_cycle(&event.keystroke).is_none() {
                return;
            }
            let control = {
                let host = shared.borrow();
                let scope = &host.mounts[&owner].focus;
                let focused_header = controls.iter().find(|(control, focus, _)| {
                    matches!(control.kind, ControlKind::Tab(_)) && focus.is_focused(window)
                });
                let focused_bar = focused_header
                    .and_then(|(control, _, _)| Some(control.tab_strip.as_ref()?.visual_id()))
                    .or_else(|| {
                        pane_bars.iter().find_map(|(item, bar)| {
                            host.panels
                                .get(item)
                                .and_then(|pane| pane.content_focus.as_ref())
                                .is_some_and(|focus| focus.contains_focused(window, cx))
                                .then_some(*bar)
                                .flatten()
                        })
                    });
                focused_header
                    .filter(|(control, _, _)| control.receiver.is_some())
                    .map(|(control, _, _)| control.clone())
                    .or_else(|| {
                        controls.iter().rev().find_map(|(control, _, _)| {
                            if !matches!(control.kind, ControlKind::Tab(_))
                                || control.receiver.is_none()
                            {
                                return None;
                            }
                            let owns_focus = focused_bar.map_or_else(
                                || scope.is_focused(window) && control.selected == Some(true),
                                |bar| {
                                    control
                                        .tab_strip
                                        .as_ref()
                                        .is_some_and(|strip| strip.visual_id() == bar)
                                },
                            );
                            owns_focus.then(|| control.clone())
                        })
                    })
            };
            if let Some(control) = control
                && keyboard::handle(
                    &shared,
                    &interaction,
                    surface,
                    serial,
                    &control,
                    &event.keystroke,
                    window,
                    cx,
                )
            {
                cx.stop_propagation();
            }
        });
        let shared = self.host.clone();
        let hitboxes = prepared.hitboxes.clone();
        let content_hitboxes = prepared.content_hitboxes.clone();
        let interaction = self.interaction.clone();
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
            interaction.splitter_press.borrow_mut().take();
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
                &content_hitboxes,
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
        let content_hitboxes = prepared.content_hitboxes.clone();
        let interaction = self.interaction.clone();
        window.on_pointer_cancel(move |_, phase, window, _| {
            if phase.capture() {
                interaction.splitter_press.borrow_mut().take();
            }
            if !phase.bubble() || interaction.content_terminal.get() {
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
                &content_hitboxes,
                false,
                window.modifiers(),
                window,
            );
            window.refresh();
        });
    }
}

/// One physical terminal belongs to the original content press, even across logical regions.
fn finish_content_press(
    shared: &SharedHost,
    interaction: &InteractionState,
    event: SurfacePointerEvent,
    position: open_gpui::Point<Pixels>,
    modifiers: open_gpui::Modifiers,
    window: &mut Window,
) -> bool {
    let Some(surface) = interaction.content_press.get() else {
        return false;
    };
    let owner = window.window_handle().window_id();
    let mut host = shared.borrow_mut();
    if let Some(frame) = host
        .mounts
        .get(&owner)
        .and_then(|mount| {
            mount
                .input_frames
                .iter()
                .find(|frame| frame.surface == surface)
        })
        .cloned()
    {
        if input::queue(
            &mut host,
            surface,
            frame.serial,
            event,
            position,
            frame.bounds,
            &frame.hitboxes,
            &frame.content_hitboxes,
            false,
            modifiers,
            window,
        ) {
            interaction.content_press.set(None);
            window.refresh();
        }
    }
    true
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
    activate_action_with_continuation(
        shared,
        interaction,
        surface,
        serial,
        slot,
        false,
        window,
        cx,
    )
}

fn activate_action_with_continuation(
    shared: &SharedHost,
    interaction: &InteractionState,
    surface: SurfaceId,
    serial: u64,
    slot: &chrome::ActionSlot,
    continuing: bool,
    window: &mut Window,
    cx: &mut App,
) -> bool {
    let owner = window.window_handle().window_id();
    let mut host = shared.borrow_mut();
    // Semantic events retain the witness of the output still shown by GPUI.
    // A preceding selection may revoke current pointer receivers before the
    // next paint, but core can still validate that original deferred command.
    // A replaced output must never lend its witness to a later callback.
    if !keyboard::admits_action(&host, interaction, surface, serial, continuing, window) {
        return false;
    }
    let Some(action) = slot.borrow().clone() else {
        return false;
    };
    let tab_focus = action.tab_focus;
    let version = action.intent.expected_version();
    let yields_content = action.yields_content;
    let previous_focus = host.session.pane_focus_request(surface);
    let result = (|| -> Result<_> {
        let HostState {
            session,
            mounts,
            native,
            ..
        } = &mut *host;
        let mut frame =
            host_native::begin_frame(session, mounts, native.as_ref(), Some(window), cx)?;
        frame.submit_deferred_surface_action(action.intent)?;
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
            // Independent events keep their original witness until presentation
            // changes. Core revalidates that target after each earlier event;
            // a fresh packet must reflect the resulting chrome and focus state.
            if let Some(region) = host
                .mounts
                .get_mut(&owner)
                .and_then(|mount| mount.regions.get_mut(&surface))
            {
                region.dirty = true;
            }
            if rejected {
                interaction.keyboard.borrow_mut().take();
                interaction.pending_tab_focus.borrow_mut().take();
                window.refresh();
                return false;
            }
        }
        Err(error) => {
            host.fail(error.as_ref());
            return false;
        }
    }
    let request = host
        .session
        .pane_focus_request(surface)
        .filter(|request| yields_content && Some(*request) != previous_focus);
    let content_focus = request.and_then(|request| {
        request
            .item()
            .and_then(|item| host.panels.get(&item))
            .and_then(|pane| pane.content_focus.as_ref())
            .filter(|focus| window.is_focus_handle_rendered(focus))
            .cloned()
            .or_else(|| host.mounts.get(&owner).map(|mount| mount.focus.clone()))
    });
    let focus = tab_focus
        .filter(|_| !yields_content)
        .filter(|(_, key)| keyboard::tab_is_selected(host.session.view(), surface, *key))
        .map(|(visual, _)| {
            (
                visual,
                interaction
                    .focus
                    .borrow_mut()
                    .entry(ControlFocusId::Visual(visual))
                    .or_insert_with(|| cx.focus_handle())
                    .clone(),
            )
        });
    drop(host);
    if yields_content {
        // Activation ends the header's input prefix even before another paint.
        // A clipped pane receives focus through the ordinary presentation path;
        // its rendered mount scope owns the intervening suffix.
        interaction.keyboard.borrow_mut().take();
        interaction.pending_tab_focus.borrow_mut().take();
        if let Some(focus) = content_focus {
            window.focus(&focus, cx);
        }
        if let Some(request) = request {
            let mut attempt = PaneFocusAttempt::new(request);
            attempt.claim_revision = Some(window.focus_claim_revision());
            interaction.pane_focus.borrow_mut().insert(surface, attempt);
        }
    } else if let Some((visual, focus)) = focus {
        window.focus(&focus, cx);
        *interaction.pending_tab_focus.borrow_mut() = Some(PendingTabFocus {
            surface,
            visual,
            version,
            claim_revision: window.focus_claim_revision(),
        });
    } else if let Some((bar, opener)) = action.tab_list_opener {
        let focus = interaction
            .focus
            .borrow()
            .get(&ControlFocusId::Visual(opener))
            .cloned();
        if let Some(focus) = focus.filter(|focus| window.is_focus_handle_rendered(focus)) {
            if !focus.is_focused(window) {
                window.focus(&focus, cx);
            }
            keyboard::remember_menu_opener(interaction, surface, bar, &focus, window);
        }
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
    interaction: &Rc<InteractionState>,
    serial: u64,
    admitted: &[SurfaceId],
    controls: &[(SurfaceId, Rc<Control>, FocusHandle)],
    window: &mut Window,
    cx: &mut App,
) {
    if !window.is_window_active() || shared.borrow().ensure_application_admission().is_err() {
        return;
    }
    let owner = window.window_handle().window_id();
    let presented = |surface| {
        admitted.contains(&surface) && input::is_presented(&shared.borrow(), owner, surface, serial)
    };
    keyboard::restore(shared, interaction, serial, admitted, controls, window, cx);
    let pending = interaction.pending_tab_focus.borrow_mut().take();
    if let Some(pending) = pending {
        if pending
            .version
            .is_same_epoch(shared.borrow().session.version())
            && pending.claim_revision == window.focus_claim_revision()
        {
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
    if interaction
        .keyboard
        .borrow()
        .as_ref()
        .is_some_and(|prefix| prefix.owns_menu_focus(window))
    {
        return;
    }
    let focused_row = controls.iter().find_map(|(surface, control, focus)| {
        let row = control.menu_row.as_ref()?;
        (row.focused && presented(*surface)).then_some((surface, row, focus))
    });
    if let Some((surface, row, focus)) = focused_row {
        let bar = row.keys.bar;
        let mut previous = interaction.tab_list_focus.borrow_mut();
        if let Some(previous) = previous.as_ref() {
            if previous.surface != *surface
                || previous.bar != bar
                || previous.claim_revision != window.focus_claim_revision()
                || (previous.row == Some(row.id) && focus.is_focused(window))
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
                row: Some(row.id),
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
            .or_insert_with(|| PaneFocusAttempt::new(request));
        if attempt.request != request {
            *attempt = PaneFocusAttempt::new(request);
        }
        if !attempt.applied {
            attempt.applied = true;
            if let Some(target) = target
                && attempt.owns_focus_claim(window)
                && window.is_focus_handle_rendered(&target)
                && (scope.contains_focused(window, cx) || window.focused(cx).is_none())
            {
                window.focus(&target, cx);
                if attempt.claim_revision.is_some() {
                    attempt.claim_revision = Some(window.focus_claim_revision());
                }
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
        let mut attempts = interaction.pane_focus.borrow_mut();
        let attempt = attempts
            .entry(request.surface())
            .or_insert_with(|| PaneFocusAttempt::new(request));
        if attempt.request != request {
            *attempt = PaneFocusAttempt::new(request);
        }
        let observation = match target.filter(|target| window.is_focus_handle_rendered(target)) {
            Some(target) if target.contains_focused(window, cx) => {
                DockspacePaneFocusObservation::Focused
            }
            Some(_) if attempt.owns_focus_claim(window) => {
                DockspacePaneFocusObservation::NotFocused
            }
            _ => DockspacePaneFocusObservation::Unavailable,
        };
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
    tabs: &[(TabKey, HitboxId)],
    position: open_gpui::Point<Pixels>,
    window: &Window,
) -> Option<TabKey> {
    if !window.is_mouse_in_window()
        || window.last_input_was_keyboard()
        || window.last_input_was_touch()
    {
        return None;
    }
    tabs.iter().find_map(|(item, hitbox)| {
        (hitbox.window_point_target_rank(position, window) == Some(0)).then_some(*item)
    })
}

fn begin_candidate(
    host: &mut HostState,
    inputs: &[input::QueuedInput],
    regions: &BTreeMap<SurfaceId, Bounds<Pixels>>,
    window: &Window,
    cx: &mut App,
) -> Result<(DockspaceUpdate, Pixels, BTreeMap<ItemId, CustomTabLabel>)> {
    let mut frame = host_native::begin_frame(
        &mut host.session,
        &host.mounts,
        host.native.as_ref(),
        Some(window),
        cx,
    )
    .context("beginning retained docking candidate")?;
    let owner = window.window_handle().window_id();
    host_native::set_placements(&mut frame, host.native.as_ref(), owner, regions, window)?;
    for input in inputs {
        input.submit(&mut frame)?;
    }
    frame.begin_encoding_pass()?;
    let height = px(frame.presentation_config().tab_bar_height() as f32);
    let mut labels = BTreeMap::new();
    if host.panels.values().any(|pane| pane.tab_content.is_some()) {
        for &surface in regions.keys() {
            for item in frame.tab_measurement_items(surface)? {
                if let Some(label) = host.panels.get(&item).and_then(CustomTabLabel::new) {
                    labels.insert(item, label);
                }
            }
        }
    }
    Ok((frame.suspend(), height, labels))
}

fn encode_candidate(
    host: &mut HostState,
    update: DockspaceUpdate,
    regions: &BTreeMap<SurfaceId, Bounds<Pixels>>,
    open_menu: Option<presentation_menu::OpenMenu>,
    tab_widths: &BTreeMap<ItemId, f64>,
    window: &Window,
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
    let mut frame = host.session.resume_update(update)?;
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
            let state = if deferred {
                PacketState::Waiting
            } else {
                PacketState::Ready
            };
            packets.insert(surface, Packet::empty(state));
            continue;
        }
        if !presentation_surfaces.contains(&surface) {
            anyhow::ensure!(managed, "surface is outside the presentation roster");
            // The real child can draw before its Created/Prepared observations
            // admit it to the core's presentation roster. This staging packet
            // supplies no painted output or native content-readiness fact.
            packets.insert(surface, Packet::empty(PacketState::Waiting));
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
                Request::TabIntrinsic { key, label, .. } => {
                    let title: open_gpui::SharedString = recording::tab_label(label, panels).into();
                    let width = {
                        let line = window.text_system().shape_line(
                            title.clone(),
                            config.text_size,
                            &[TextRun {
                                len: title.len(),
                                font: config.font.clone(),
                                color: config.colors.text,
                                ..Default::default()
                            }],
                            None,
                        );
                        f64::from(f32::from(line.width))
                    };
                    match key.item().and_then(|item| tab_widths.get(&item)) {
                        Some(tab_width) => Answer::TabIntrinsicWithMenuWidth {
                            tab_width: *tab_width,
                            menu_width: width,
                        },
                        None => Answer::TabIntrinsic(width),
                    }
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
                // An unmeasured region owns no output this pass. Commit its
                // contribution alongside any ready sibling regions so core can
                // settle local conditions such as insufficient popup space.
                packets.insert(surface, Packet::empty(PacketState::Unmeasured));
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

fn tab_label_style(config: &GpuiDockspaceConfig, color: Hsla) -> open_gpui::Div {
    div()
        .whitespace_nowrap()
        .flex()
        .items_center()
        .font(config.font.clone())
        .text_size(config.text_size)
        .text_color(color)
}

fn prepare_label(
    mut label: AnyElement,
    rect: Bounds<Pixels>,
    clip: Bounds<Pixels>,
    window: &mut Window,
    cx: &mut App,
) -> Option<PreparedDraw> {
    let clip = window.prepare_subtree_clip(&SubtreeClip::own_border_box(), clip);
    window.with_prepared_subtree_clip(&clip, |window| {
        label.prepaint_as_root(rect.origin, definite(rect), window, cx);
    })?;
    Some(PreparedDraw::ClippedElement(label, clip))
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
