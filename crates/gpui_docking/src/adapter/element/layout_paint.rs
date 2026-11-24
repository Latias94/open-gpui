//! Measure semantic surface programs and prepare their retained GPUI draw operations.

use super::*;

impl DockspaceElement {
    pub(super) fn prepare_region(
        &self,
        surface: SurfaceId,
        packet: &Packet,
        serial: u64,
        geometry: ElementGeometry,
        background: HitboxId,
        capture: PointerCaptureHandle,
        allow_input: bool,
        tab_list: Option<&(SurfaceId, DeferredSurfaceAction)>,
        window: &mut Window,
        cx: &mut App,
    ) -> PreparedRegion {
        let bounds = geometry.layout_bounds();
        let mut hitboxes = vec![background];
        let mut content_hitboxes = Vec::new();
        let mut panes = BTreeMap::new();
        let mut draws = Vec::new();
        let mut tabs = Vec::new();
        let mut receivers = Vec::new();
        let mut controls = Vec::new();
        let mut menu_focus = None;
        let mut visible_panes = Vec::new();
        let mut menus = Vec::new();
        let mut scrolls = Vec::new();
        let allow_menu_row_input = allow_input
            && !packet.tab_list.as_ref().is_some_and(|menu| {
                self.interaction
                    .superseded_tab_list
                    .borrow()
                    .as_ref()
                    .is_some_and(|previous| previous.matches(surface, menu))
            });
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
        let (config, admitted, pointer, keep_live_content) = {
            let host = self.host.borrow();
            let owner = window.window_handle().window_id();
            let mount = &host.mounts[&owner];
            (
                host.config.clone(),
                allow_input && input::is_presented(&host, owner, surface, serial),
                mount
                    .regions
                    .get(&surface)
                    .and_then(|region| region.pointer),
                window.is_visible()
                    && (allow_input || mount.waiting_for_update)
                    && host
                        .native
                        .as_ref()
                        .is_none_or(|native| !native.is_failed()),
            )
        };
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
                Draw::DragGhost(title) => {
                    if let Some(pointer) = pointer {
                        let rect = Bounds::new(
                            bounds.origin
                                + point(
                                    px((pointer.x() + 12.0) as f32),
                                    px((pointer.y() + 14.0) as f32),
                                ),
                            size(px(160.0), px(28.0)),
                        );
                        draws.push(PreparedDraw::Preview(
                            rect,
                            config.colors.drag_ghost,
                            config.colors.accent,
                        ));
                        let label = tab_label_style(&config, config.colors.text)
                            .w(rect.size.width)
                            .h(rect.size.height)
                            .overflow_hidden()
                            .child(title.clone())
                            .into_any_element();
                        draws.extend(prepare_label(label, rect, rect, window, cx));
                    }
                }
                Draw::NativePreview(preview) => {
                    if preview.host_surface == surface
                        && self
                            .host
                            .borrow()
                            .native
                            .as_ref()
                            .is_none_or(|native| !native.has_preview_window())
                    {
                        // When the physical placement lies outside this client (or
                        // its desktop origin is unavailable), use a small source cue.
                        // This decoration never claims to be destination geometry.
                        let rect = source_preview_bounds(*preview, geometry, window)
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
                        allow_input && (control.menu_row.is_none() || allow_menu_row_input),
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
                Draw::TabListMenuFocus(row) => {
                    if !admitted || !allow_menu_row_input {
                        continue;
                    }
                    let rect = to_gpui(row.bounds, bounds);
                    let focus = self.control_focus(row.focus_id(), cx);
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
                            allow_input && (control.menu_row.is_none() || allow_menu_row_input),
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
                    let placement = packet.pane_placements.get(item).copied();
                    let (pane, live, pointer) = {
                        let host = self.host.borrow();
                        let owner = window.window_handle().window_id();
                        let current = host.session.view().item(*item).is_some_and(|pane| {
                            pane.surface() == surface
                                && pane.is_active()
                                && host
                                    .session
                                    .open_item_identity(*item)
                                    .is_some_and(|identity| {
                                        Some(PanePlacement::of(pane, identity)) == placement
                                    })
                        });
                        let previous = host.mounts[&owner]
                            .regions
                            .get(&surface)
                            .and_then(|region| region.presented_panes.as_ref())
                            .filter(|panes| panes.version.is_same_epoch(host.session.version()))
                            .and_then(|panes| panes.panes.get(item))
                            .filter(|pane| Some(pane.placement) == placement);
                        let continuing = current && keep_live_content && previous.is_some();
                        (
                            host.panels.get(item).cloned(),
                            (current && admitted) || continuing,
                            (current && admitted)
                                || (continuing
                                    && previous.map(|pane| &pane.geometry) == geometry.as_ref()),
                        )
                    };
                    if let Some((placement, geometry)) = placement.zip(geometry) {
                        panes.insert(
                            *item,
                            PanePresentation {
                                placement,
                                geometry,
                            },
                        );
                    }
                    let presentation = if live {
                        SubtreePresentation::Visible
                    } else {
                        SubtreePresentation::Inert
                    };
                    if presentation == SubtreePresentation::Visible {
                        visible_panes.push(*item);
                    }
                    let mut element = match pane.as_ref() {
                        Some(pane) => div()
                            .id(format!("pane-{}", item.get()))
                            .w(rect.size.width)
                            .h(rect.size.height)
                            .overflow_hidden()
                            .role(Role::TabPanel)
                            .aria_label(pane.title.clone())
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
                            if pointer
                                && let Some(pane) = &pane
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
                            if live && !pointer {
                                window.insert_hitbox(rect, HitboxBehavior::BlockMouse);
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
                tab_list,
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
            geometry,
            receivers,
            controls,
            menu_focus,
            visible_panes,
            pane_bars: packet.pane_bars.clone(),
            admitted,
            scrolls,
        }
    }
}

fn source_preview_bounds(
    preview: host_native::provisional::Preview,
    surface: ElementGeometry,
    window: &Window,
) -> Option<Bounds<Pixels>> {
    let displayed = source_preview_display_bounds(
        preview.placement,
        window.platform_facts().physical_geometry,
        f64::from(window.scale_factor()),
    )?;
    let clip = surface.displayed_bounds().intersect(&window.clip_bounds());
    let clipped = displayed.intersect(&clip);
    if clipped.is_empty() {
        return None;
    }
    surface.window_to_layout_bounds(clipped).ok()
}

pub(super) fn source_preview_display_bounds(
    placement: dockspace::geometry::PhysicalRect,
    geometry: Option<open_gpui::PlatformWindowPhysicalGeometry>,
    scale: f64,
) -> Option<Bounds<Pixels>> {
    let client = geometry?.client_bounds();
    Some(Bounds::new(
        point(
            px(((placement.x() - f64::from(client.origin.x.0)) / scale) as f32),
            px(((placement.y() - f64::from(client.origin.y.0)) / scale) as f32),
        ),
        size(
            px((placement.width() / scale) as f32),
            px((placement.height() / scale) as f32),
        ),
    ))
}

pub(super) fn begin_candidate(
    host: &mut HostState,
    inputs: &[input::QueuedInput],
    regions: &BTreeMap<SurfaceId, RegionGeometry>,
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
    host_native::set_placements(&mut frame, host.native.as_ref(), owner, regions)?;
    for input in inputs {
        input.submit(&mut frame)?;
    }
    frame.begin_encoding_pass()?;
    let height = px(frame.presentation_config().tab_bar_height() as f32);
    let mut labels = BTreeMap::new();
    if host.panels.values().any(|pane| pane.tab_content.is_some()) {
        let roster = frame.surfaces();
        for &surface in regions.keys().filter(|surface| roster.contains(surface)) {
            for item in frame.tab_measurement_items(surface)? {
                if let Some(label) = host.panels.get(&item).and_then(CustomTabLabel::new) {
                    labels.insert(item, label);
                }
            }
        }
    }
    Ok((frame.suspend(), height, labels))
}

pub(super) fn encode_candidate(
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
                PacketState::Vacant
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
        let (mut packet, receipt) = record(plan, panels, &config.colors, open_menu)?;
        let open_items: Vec<_> = packet
            .pane_bars
            .keys()
            .filter_map(|&item| {
                plan.open_item_identity(item)
                    .map(|identity| (item, identity))
            })
            .collect();
        let view = frame.view();
        packet.pane_placements = open_items
            .into_iter()
            .filter_map(|(item, identity)| {
                let pane = view.item(item)?;
                Some((item, PanePlacement::of(pane, identity)))
            })
            .collect();
        frame.confirm_surface_painted(receipt)?;
        packets.insert(surface, Rc::new(packet));
    }
    Ok(Candidate::Painted {
        packets,
        update: frame.suspend(),
    })
}

pub(super) fn to_gpui(rect: LogicalRect, surface: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        point(
            surface.origin.x + px(rect.x() as f32),
            surface.origin.y + px(rect.y() as f32),
        ),
        size(px(rect.width() as f32), px(rect.height() as f32)),
    )
}

pub(super) fn definite(bounds: Bounds<Pixels>) -> open_gpui::Size<AvailableSpace> {
    size(
        AvailableSpace::Definite(bounds.size.width),
        AvailableSpace::Definite(bounds.size.height),
    )
}

pub(super) fn tab_label_style(config: &GpuiDockspaceConfig, color: Hsla) -> open_gpui::Div {
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

pub(super) fn outline(bounds: Bounds<Pixels>, color: Hsla, window: &mut Window) {
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
