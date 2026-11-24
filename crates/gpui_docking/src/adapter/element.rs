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
        DeferredSurfaceAction, DockspaceControlId, DockspacePaneFocusObservation,
        DockspacePaneFocusRequest, DockspaceReceiverDescriptor, DockspaceUpdate, DockspaceVisualId,
        HostFrameReport, PaneContentGeometry, SurfaceMeasurementAnswer as Answer,
        SurfaceMeasurementRequest as Request, SurfacePointerButton, SurfacePointerCancelReason,
        SurfacePointerEvent, SurfaceWork, SurfaceWorkDeferred, TabListMenuMetrics,
        TabListMenuRowId, TabStripControlMetric, TabStripControlMetrics, TabStripControlPlacement,
        TabStripMetrics,
    },
};
use open_gpui::{
    AcceptedFrameFence, AccessibleAction, AnyElement, App, AvailableSpace, Bounds, Element,
    ElementGeometry, ElementId, FocusHandle, GlobalElementId, HitboxBehavior, HitboxId, Hsla,
    InspectorElementId, IntoElement, KeyDownEvent, LayoutId, MouseButton, MouseDownEvent,
    MouseExitEvent, MouseMoveEvent, MouseUpEvent, Pixels, PointerCaptureHandle,
    PreparedSubtreeClip, Role, Style, SubtreeClip, SubtreePresentation, SubtreePresentationExt,
    TextRun, Window, div, fill, point, prelude::*, px, relative, size,
};

use crate::adapter::{
    GpuiDockspaceConfig, HostState, SharedHost,
    host::{DockRegion, PaneFocusClaim, WindowMount},
    host_native,
    input::{self, InputFrame, PanePlacement, PanePresentation, RegionGeometry},
    presentation,
};

mod chrome;
mod chrome_controls;
mod keyboard;
mod layout_paint;
mod pointer_control;
mod presentation_menu;
mod recording;
mod splitter;
mod tab_label;
mod wheel;

use chrome_controls::{
    activate_action, activate_action_with_continuation, apply_chrome_focus, begin_tab_list_opening,
    refresh_menu,
};
use layout_paint::{begin_candidate, definite, encode_candidate, outline, to_gpui};
use pointer_control::hovered_tab_at;

use chrome::{Control, ControlAction, ControlKind, GuideState, Icon, PresentationMenu};
use recording::record;
use tab_label::CustomTabLabel;

#[cfg(test)]
mod interaction_tests;

pub(super) struct Packet {
    draws: Vec<Draw>,
    pane_bars: BTreeMap<ItemId, Option<DockspaceVisualId>>,
    pane_placements: BTreeMap<ItemId, PanePlacement>,
    pane_receivers: BTreeMap<ItemId, DockspaceReceiverDescriptor>,
    state: PacketState,
    pub(super) native_preview: Option<host_native::provisional::Preview>,
    tab_list: Option<TabListPopup>,
}

struct TabListPopup {
    keys: Rc<keyboard::MenuKeys>,
    dismiss: DeferredSurfaceAction,
}

enum PacketState {
    Ready,
    Waiting,
    Unmeasured,
    Vacant,
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
            pane_placements: BTreeMap::new(),
            pane_receivers: BTreeMap::new(),
            state,
            native_preview: None,
            tab_list: None,
        })
    }

    pub(super) fn is_deferred(&self) -> bool {
        matches!(self.state, PacketState::Waiting | PacketState::Unmeasured)
    }

    pub(super) fn is_vacant(&self) -> bool {
        matches!(self.state, PacketState::Vacant)
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
    DragGhost(String),
    Icon(LogicalRect, Icon, bool),
    Control(Rc<Control>),
    PresentationMenu(Rc<PresentationMenu>),
    TabList(LogicalRect, DockspaceVisualId),
    TabListMenuFocus(Rc<Control>),
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
    menu_sequence: Cell<u64>,
    menu: RefCell<Option<presentation_menu::MenuState>>,
    pane_focus: RefCell<BTreeMap<SurfaceId, PaneFocusAttempt>>,
    pending_tab_focus: RefCell<Option<PendingTabFocus>>,
    tab_list_focus: RefCell<Option<TabListFocus>>,
    superseded_tab_list: RefCell<Option<presentation_menu::SupersededTabList>>,
    tab_list_press: RefCell<Option<TabListPress>>,
    wheel_gesture: Cell<Option<wheel::WheelGesture>>,
    keyboard: RefCell<Option<keyboard::Continuation>>,
    keyboard_subscription: RefCell<Option<open_gpui::Subscription>>,
    splitter_press: RefCell<Option<splitter::Press>>,
    content_terminal: Cell<bool>,
}

#[derive(Clone, PartialEq, Eq, Hash)]
enum ControlFocusId {
    Visual(DockspaceVisualId),
    Control(DockspaceControlId),
    MenuRow(TabListMenuRowId),
}

struct PaneFocusAttempt {
    request: DockspacePaneFocusRequest,
    claim: Option<PaneFocusClaim>,
    applied: bool,
    observation: Option<DockspacePaneFocusObservation>,
}

impl PaneFocusAttempt {
    fn new(request: DockspacePaneFocusRequest, claim: Option<PaneFocusClaim>) -> Self {
        Self {
            request,
            claim,
            applied: false,
            observation: None,
        }
    }

    fn owns_focus_claim(&self, window: &Window) -> bool {
        self.claim.is_none_or(|claim| {
            claim.owner == window.window_handle().window_id()
                && claim.revision == window.focus_claim_revision()
        })
    }
}

struct PendingTabFocus {
    surface: SurfaceId,
    target: ControlFocusId,
    version: dockspace::model::WorkspaceVersion,
    claim_revision: u64,
}

struct TabListFocus {
    surface: SurfaceId,
    bar: DockspaceVisualId,
    row: Option<TabListMenuRowId>,
    opener: FocusHandle,
    claim_revision: u64,
    opening: Option<keyboard::MenuOpening>,
}

struct TabListPress {
    surface: SurfaceId,
    opener: chrome::TabListOpener,
}

enum TabListOpenSource {
    Action,
    PointerRelease,
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
    panes: BTreeMap<ItemId, PanePresentation>,
    tabs: Vec<(TabKey, HitboxId)>,
    capture: PointerCaptureHandle,
    serial: u64,
    geometry: ElementGeometry,
    receivers: Vec<(DockspaceReceiverDescriptor, HitboxId)>,
    controls: Vec<(Rc<Control>, FocusHandle, HitboxId)>,
    menu_focus: Option<(Rc<Control>, FocusHandle)>,
    visible_panes: Vec<ItemId>,
    pane_bars: BTreeMap<ItemId, Option<DockspaceVisualId>>,
    admitted: bool,
    scrolls: Vec<wheel::ScrollTarget>,
}

pub(crate) enum PreparedState {
    StagedRoot,
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
        if host_native::stage_root_construction(&self.host.borrow(), window, cx) {
            return PreparedState::StagedRoot;
        }
        if self.host.borrow().native.is_some()
            && let Ok(geometry) = window.try_element_geometry(bounds)
            && geometry.window_scale().width != geometry.window_scale().height
        {
            return self.failure(
                "native docking regions require uniform scale",
                bounds,
                window,
                cx,
            );
        }
        host_native::prepare_window(&self.host, window, cx);
        let mount = {
            let mut host = self.host.borrow_mut();
            presentation::abandon_unaccepted(&mut host, window, cx);
            host.ensure_mount(id, &self.regions, window, cx)
        };
        match mount {
            Ok(true) => {
                let weak = Rc::downgrade(&self.host);
                let terminal_host = weak.clone();
                let visibility_host = weak.clone();
                let owner = window.window_handle().window_id();
                let closed = cx.on_window_closed(move |cx, closed| {
                    if closed == owner {
                        if let Some(host) = weak.upgrade() {
                            presentation::close(&host, owner, cx);
                        }
                    }
                });
                // Reservation rollback has no ordinary logical-close event.
                // The original native terminal retires its provisional mount.
                let terminal = cx.on_window_native_terminal(move |cx, closed| {
                    if closed == owner {
                        if let Some(host) = terminal_host.upgrade() {
                            presentation::close(&host, owner, cx);
                        }
                    }
                });
                let visibility = window.observe_window_visibility(move |visibility, window, cx| {
                    if let Some(host) = visibility_host.upgrade() {
                        presentation::visibility_changed(&host, visibility, window, cx);
                    }
                });
                // Observations leave with their mount, including removal from
                // inside either terminal callback.
                self.host
                    .borrow_mut()
                    .mounts
                    .get_mut(&owner)
                    .unwrap()
                    ._lifecycle_observers = Some(open_gpui::Subscription::join(
                    visibility,
                    open_gpui::Subscription::join(closed, terminal),
                ));
                host_native::schedule(&self.host, cx);
            }
            Ok(false) => {}
            Err(error) => {
                self.host.borrow_mut().fail(error.as_ref());
                return self.failure(error, bounds, window, cx);
            }
        }
        let owner = window.window_handle().window_id();
        let capture = self.host.borrow().mounts[&owner].capture;
        // Failed duplicate mounts retract their hitbox and keep the existing capture binding.
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
        match self.prepare(id, bounds, capture, window, cx) {
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
            PreparedState::StagedRoot => return,
            PreparedState::Frame(prepared) => prepared,
            PreparedState::Failure(error) => {
                error.paint(window, cx);
                return;
            }
        };
        self.listen_content_terminals(window);
        if !prepared.waiting_for_update {
            let tabs = prepared
                .regions
                .iter()
                .flat_map(|region| region.tabs.iter().copied())
                .collect::<Vec<_>>();
            let hovered = self.interaction.hovered_tab.clone();
            window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, _| {
                if phase.capture() {
                    let current = hovered_tab_at(&tabs, event.position, window);
                    if hovered.replace(current) != current {
                        window.refresh();
                    }
                }
            });
        }
        // Waiting on another window gates new actions. The current capture
        // still needs to receive motion, release, and cancellation.
        for region in &prepared.regions {
            self.listen(region, !prepared.waiting_for_update, window);
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
        id: Option<&GlobalElementId>,
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
        let region_geometry = region_bounds
            .iter()
            .map(|(&surface, &bounds)| Ok((surface, RegionGeometry::new(bounds, window)?)))
            .collect::<Result<BTreeMap<_, _>>>()?;
        let retained_packets = |mount: &WindowMount| {
            region_bounds
                .keys()
                .map(|surface| {
                    let packet = mount
                        .regions
                        .get(surface)
                        .and_then(|region| region.packet.clone());
                    (
                        *surface,
                        packet.unwrap_or_else(|| Packet::empty(PacketState::Waiting)),
                    )
                })
                .collect()
        };
        let mut schedule_host_work = false;
        let (packets, serial, publication, publication_attempt, waiting_for_update) = {
            let mut host = self.host.borrow_mut();
            let mount = &host.mounts[&owner];
            let publication = mount.publication;
            let paused = presentation::pauses_new_updates(window);
            if let Some(pending) = mount
                .pending
                .as_ref()
                .filter(|pending| pending.update.is_none())
            {
                (
                    pending.packets.clone(),
                    pending.serial,
                    publication,
                    None,
                    false,
                )
            } else if paused || host.has_unaccepted_update() {
                let mount = host.mounts.get_mut(&owner).expect("physical mount");
                mount.waiting_for_update = !paused;
                for region in mount.regions.values_mut() {
                    region.dirty = true;
                }
                // Keep the last packet while hidden or waiting for another
                // window's update to finish. Both paths wake this mount on progress.
                (
                    retained_packets(mount),
                    mount.serial,
                    publication,
                    None,
                    true,
                )
            } else if mount.attached
                && host.inputs.is_empty()
                && mount.regions.iter().all(|(surface, region)| {
                    region.geometry == region_geometry.get(surface).copied()
                        && (mount.capacity_blocked || (!region.dirty && region.packet.is_some()))
                })
            {
                (
                    retained_packets(mount),
                    mount.serial,
                    publication,
                    None,
                    false,
                )
            } else {
                let inputs = std::mem::take(&mut host.inputs);
                let candidate =
                    match begin_candidate(&mut host, &inputs, &region_geometry, window, cx) {
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
                                presentation_menu::open(&self.interaction),
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
                        for input in &inputs {
                            input.mark_committed();
                        }
                        host.apply_report(report);
                        let mount = host.mounts.get_mut(&owner).expect("physical mount");
                        mount.capacity_blocked = true;
                        for region in mount.regions.values_mut() {
                            region.dirty = true;
                        }
                        (
                            retained_packets(mount),
                            mount.serial,
                            publication,
                            None,
                            false,
                        )
                    }
                    Candidate::Painted { packets, update } => {
                        let submission = (|| {
                            let mount = &host.mounts[&owner];
                            let serial = mount
                                .last_attempt_serial
                                .max(mount.serial)
                                .checked_add(1)
                                .context("frame serial exhausted")?;
                            let ticket =
                                host_native::provisional::begin_submission(&host, window, cx)?;
                            if let Err(error) =
                                host_native::watch_submission(&host, &ticket, window)
                            {
                                window.retire_exact_frame_submission(&ticket);
                                return Err(error);
                            }
                            Ok((serial, ticket))
                        })();
                        let (serial, ticket) = match submission {
                            Ok(submission) => submission,
                            Err(error) => {
                                drop(update);
                                let mut inputs = inputs;
                                inputs.append(&mut host.inputs);
                                host.inputs = inputs;
                                return Err(error);
                            }
                        };
                        let attempt =
                            presentation::observe(&self.host, &ticket, serial, publication, window);
                        let mount = host.mounts.get_mut(&owner).expect("physical mount");
                        mount.capacity_blocked = false;
                        mount.last_attempt_serial = serial;
                        let mut pending = presentation::PendingSubmission::new(
                            serial,
                            ticket,
                            packets.clone(),
                            update,
                            inputs,
                        );
                        pending.element = id.cloned();
                        pending.publication_attempt = Some(attempt.clone());
                        mount.pending = Some(pending);
                        (packets, serial, publication, Some(attempt), false)
                    }
                }
            }
        };
        if schedule_host_work {
            host_native::schedule(&self.host, cx);
        }
        let tab_list = packets.iter().find_map(|(surface, packet)| {
            packet
                .tab_list
                .as_ref()
                .map(|menu| (*surface, menu.dismiss.clone()))
        });
        let mut regions = Vec::new();
        for (surface, packet) in &packets {
            if packet.is_vacant() {
                continue;
            }
            let bounds = region_bounds[surface];
            let background = window.insert_hitbox(bounds, HitboxBehavior::Normal);
            regions.push(self.prepare_region(
                *surface,
                packet,
                serial,
                background.geometry(),
                background.id,
                capture,
                !waiting_for_update,
                tab_list.as_ref(),
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
        let input_frames: Vec<_> = {
            let host = self.host.borrow();
            let scale = f64::from(window.scale_factor());
            regions
                .iter()
                .filter(|region| {
                    // GPUI replaces hitbox IDs on redraw. Keep the displayed
                    // artifact's receivers usable by the native ingress queue
                    // while the same geometry is still current.
                    !waiting_for_update
                        || (window.is_visible()
                            && input::is_presented(&host, owner, region.surface, serial)
                            && host.mounts[&owner].regions[&region.surface].geometry
                                == region_geometry.get(&region.surface).copied()
                            && host.mounts[&owner].input_frames.iter().any(|previous| {
                                previous.surface == region.surface
                                    && previous.serial == serial
                                    && previous.geometry == region.geometry
                                    && previous.scale == scale
                            }))
                })
                .filter_map(|region| {
                    Some(InputFrame {
                        surface: region.surface,
                        serial,
                        version: host.session.version(),
                        geometry: region.geometry,
                        clip: region_geometry.get(&region.surface)?.clip,
                        hitboxes: region.hitboxes.clone(),
                        content_hitboxes: region.content_hitboxes.clone(),
                        panes: region.panes.clone(),
                        capture,
                        receivers: region.receivers.clone(),
                        scale,
                        native_binding: None,
                    })
                })
                .collect()
        };
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
                Draw::Control(control) | Draw::TabListMenuFocus(control) => {
                    Some(control.focus_id())
                }
                Draw::TabList(_, id) => Some(ControlFocusId::Visual(*id)),
                Draw::PresentationMenu(menu) => {
                    Some(ControlFocusId::Control(menu.input_id.clone()))
                }
                _ => None,
            })
            .collect();
        let commit = move |fence: AcceptedFrameFence, window: &mut Window, cx: &mut App| {
            if !presentation::accept(
                &shared,
                serial,
                input_frames.clone(),
                &region_geometry,
                fence,
                window,
                cx,
            ) {
                return;
            }
            presentation_menu::accept(&shared, &interaction, serial, window, cx);
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
                .map(|focus| &focus.target)
                .cloned();
            interaction
                .focus
                .borrow_mut()
                .retain(|id, _| retained_focus.contains(id) || pending.as_ref() == Some(id));
            interaction
                .pane_focus
                .borrow_mut()
                .retain(|surface, attempt| {
                    shared.borrow().session.pane_focus_request(*surface) == Some(attempt.request)
                });
            let current = hovered_tab_at(&accepted_tabs, window.mouse_position(), window);
            if hovered_tab.replace(current) != current {
                window.refresh();
            }
            input::revoke_blocked_hover(&shared, window);
        };
        let discard = move |_, window: &mut Window, cx: &mut App| {
            presentation::discard(&discard, window, cx);
        };
        if let Some(attempt) = publication_attempt {
            window.record_prepaint_window_transaction_for_attempt(attempt, commit, discard);
        } else {
            window.record_prepaint_window_transaction(publication, commit, discard);
        }
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
}

fn pane_focus_requests(
    shared: &SharedHost,
    serial: u64,
    visible_panes: &[(SurfaceId, Vec<ItemId>)],
    window: &Window,
) -> Vec<(
    DockspacePaneFocusRequest,
    Option<PaneFocusClaim>,
    Option<FocusHandle>,
    FocusHandle,
)> {
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
            Some((
                request,
                host.application_pane_focus_claim(request),
                focus,
                host.mounts[&owner].focus.clone(),
            ))
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
    for (request, claim, target, scope) in
        pane_focus_requests(shared, serial, visible_panes, window)
    {
        let mut attempts = interaction.pane_focus.borrow_mut();
        let attempt = attempts
            .entry(request.surface())
            .or_insert_with(|| PaneFocusAttempt::new(request, claim));
        if attempt.request != request {
            *attempt = PaneFocusAttempt::new(request, claim);
        }
        if !attempt.applied {
            attempt.applied = true;
            if let Some(target) = target
                && attempt.owns_focus_claim(window)
                && window.is_focus_handle_rendered(&target)
                && (scope.contains_focused(window, cx) || window.focused(cx).is_none())
            {
                window.focus(&target, cx);
                if let Some(claim) = &mut attempt.claim {
                    claim.revision = window.focus_claim_revision();
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
    for (request, claim, target, _) in pane_focus_requests(shared, serial, visible_panes, window) {
        let mut attempts = interaction.pane_focus.borrow_mut();
        let attempt = attempts
            .entry(request.surface())
            .or_insert_with(|| PaneFocusAttempt::new(request, claim));
        if attempt.request != request {
            *attempt = PaneFocusAttempt::new(request, claim);
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
