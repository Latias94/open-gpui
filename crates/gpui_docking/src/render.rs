use crate::{
    DockEdgeDockSizing, DockGraph, DockHost, DockNode, DockNodeId, DropZone,
    debug::DockDebugRegion,
    drag::DockDragPayload,
    drop_preview::{DockDropPreview, DockDropRoutePreview},
    drop_runtime::DockHostDropSceneFact,
    drop_scene_fact,
    drop_target::{
        DockDropResolution, DockDropResolveSource, DockResolvedDropTarget,
        DockResolvedDropTargetKind, validate_resolved_drop_target,
    },
    geometry,
    host_render_session::{DockHostRenderSession, selected_index},
    interaction::{
        DockPayloadDropRelease, DockRenderedOutsideReleaseDecision,
        DockRenderedOutsideReleaseRequest,
    },
    render_split::DockRenderSplitInput,
    viewport_drop_scene::DockViewportHostSceneFrame,
    workspace_move_validation::dock_target_validator,
};
use open_gpui::{
    AnyElement, Bounds, Context, DragMoveEvent, InteractiveElement, IntoElement, MouseButton,
    MouseUpEvent, ParentElement, Pixels, Render, Rgba, Styled, Window, black, canvas, div, point,
    px, relative, rgb, rgba,
};
use std::{cell::RefCell, rc::Rc};

pub(crate) type DockViewportHostSceneFrameSlot = Rc<RefCell<Option<DockViewportHostSceneFrame>>>;

const DROP_GUIDE_ZONES: [DropZone; 5] = [
    DropZone::Center,
    DropZone::Left,
    DropZone::Right,
    DropZone::Top,
    DropZone::Bottom,
];

impl Render for DockHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.clear_debug_selectors();
        self.ensure_viewport_activation_subscription(window, cx);
        self.ensure_viewport_bounds_subscription(window, cx);
        self.ensure_viewport_release_subscription(window, cx);
        let session = self.render_session(cx);
        self.sync_panel_focus_trackers(session.visible_panel_items(), window, cx);
        let drop_host_space = session.space().clone();
        let outside_release_host_space = session.space().clone();
        let viewport_host_scene_frame = Rc::new(RefCell::new(None));

        let selector = self.record_debug_selector(
            DockDebugRegion::Host,
            format!("{}:host", session.selector_prefix()),
        );

        let mut host = div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .overflow_hidden()
            .text_color(black())
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DockDragPayload>, window, cx| {
                    let payload = event.drag(cx).clone();
                    this.begin_host_drop_scene_from_render(
                        &payload,
                        event.bounds,
                        event.event.position,
                        window,
                        cx,
                    );
                },
            ))
            .on_drop(
                cx.listener(move |this, payload: &DockDragPayload, window, cx| {
                    let drag_session = this.active_payload_drag_session(payload);
                    let event_receiver_local_scene_proof =
                        this.interaction().viewport_host_scene_frame().cloned();
                    this.drop_payload_release_from_render(
                        DockPayloadDropRelease::hovered_host_with_session(
                            payload.clone(),
                            drop_host_space.clone(),
                            window.mouse_position(),
                            drag_session,
                        )
                        .with_event_receiver_local_scene_proof(event_receiver_local_scene_proof),
                        window,
                        cx,
                    );
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseUpEvent, window, cx| {
                    let payload = cx.active_drag_value::<DockDragPayload>().cloned();
                    let drag_session = payload
                        .as_ref()
                        .and_then(|payload| this.active_payload_drag_session(payload));
                    let tear_off_geometry = drag_session.as_ref().and_then(|session| {
                        this.active_payload_drag_tear_off_geometry(Some(session))
                    });
                    let platform_viewports_allowed = this.with_workspace(cx, |workspace| {
                        workspace.policy().allows_platform_viewports()
                    });
                    let request = DockRenderedOutsideReleaseRequest::new(
                        platform_viewports_allowed,
                        payload,
                        cx.mouse_button_is_pressed(MouseButton::Left),
                        outside_release_host_space.clone(),
                        event.position,
                    )
                    .with_drag_session(drag_session)
                    .with_tear_off_geometry(tear_off_geometry);
                    match this.interaction_mut().rendered_outside_release(request) {
                        DockRenderedOutsideReleaseDecision::Inactive => {}
                        DockRenderedOutsideReleaseDecision::StopDragSession(drag_session) => {
                            this.finish_payload_drag_session(&drag_session, cx);
                            this.clear_drop_preview_interaction();
                            this.viewport_runtime().clear_routed_drop_preview(cx);
                            window.refresh();
                        }
                        DockRenderedOutsideReleaseDecision::CommitRelease(release) => {
                            this.drop_payload_release_from_render(release, window, cx);
                            cx.stop_active_drag(window);
                            cx.stop_propagation();
                        }
                    }
                }),
            );

        if session.empty_central_passthrough() {
            host = host.bg(rgba(0x00000000));
        } else {
            host = host.bg(rgb(0xf7f8fa));
        }

        host = host.child(self.render_viewport_host_scene_probe(
            &viewport_host_scene_frame,
            session.drop_guide_style(),
            session.empty_central_requests_platform_pointer_passthrough(),
        ));

        if let Some(root) = session.root() {
            host = host.child(self.render_root_node(
                root,
                &session,
                &viewport_host_scene_frame,
                window,
                cx,
            ));
        } else if session.empty_central_passthrough() {
            host = host.child(self.render_passthrough_empty_central_space(
                &session,
                &viewport_host_scene_frame,
                window,
                cx,
            ));
        } else {
            host = host.child(self.render_empty_space(
                &session,
                &viewport_host_scene_frame,
                window,
                cx,
            ));
        }

        for floating in session.floating_containers() {
            host = host.child(self.render_floating_container(
                *floating,
                &session,
                &viewport_host_scene_frame,
                window,
                cx,
            ));
        }

        if let Some(preview) = self.render_host_drop_preview(&session, window, cx) {
            host = host.child(preview);
        }

        self.apply_pending_focus_from_render(&session, window, cx);

        host
    }
}

impl DockHost {
    pub(crate) fn render_node(
        &mut self,
        node_id: DockNodeId,
        session: &DockHostRenderSession,
        viewport_host_scene_frame: &DockViewportHostSceneFrameSlot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(node) = session.node(node_id).cloned() else {
            return self.render_missing_node(node_id, session);
        };

        match node {
            DockNode::Split {
                axis,
                children,
                fractions,
            } => self.render_split(
                DockRenderSplitInput::new(node_id, axis, children, fractions),
                session,
                viewport_host_scene_frame,
                window,
                cx,
            ),
            DockNode::Tabs { items, selected } => {
                let Some(selected) = selected_index(&items, &selected) else {
                    return self.render_missing_node(node_id, session);
                };
                self.render_tabs(
                    node_id,
                    items,
                    selected,
                    session,
                    viewport_host_scene_frame,
                    window,
                    cx,
                )
            }
            DockNode::Floating { child } => self.render_floating_node(
                node_id,
                child,
                session,
                viewport_host_scene_frame,
                window,
                cx,
            ),
        }
    }

    fn render_root_node(
        &mut self,
        root: DockNodeId,
        session: &DockHostRenderSession,
        viewport_host_scene_frame: &DockViewportHostSceneFrameSlot,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let root_child = self.render_node(root, session, viewport_host_scene_frame, window, cx);
        let mut root_container = div()
            .relative()
            .flex()
            .size_full()
            .overflow_hidden()
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DockDragPayload>, window, cx| {
                    let payload = event.drag(cx).clone();
                    this.update_root_drop_scene_from_render(
                        &payload,
                        root,
                        event.bounds,
                        event.event.position,
                        window,
                        cx,
                    );
                },
            ));
        root_container = root_container.child(
            self.render_viewport_drop_scene_fact_probe(viewport_host_scene_frame, move |bounds| {
                drop_scene_fact::root(root, bounds)
            }),
        );
        root_container = root_container.child(root_child);
        if let Some(guides) = self.render_drop_guides(session, None, cx) {
            root_container = root_container.child(guides);
        }
        root_container.into_any_element()
    }

    fn render_empty_space(
        &mut self,
        session: &DockHostRenderSession,
        viewport_host_scene_frame: &DockViewportHostSceneFrameSlot,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::EmptySpace,
            format!("{}:empty", session.selector_prefix()),
        );
        let space = session.space().clone();
        let mut empty = div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .relative()
            .flex()
            .size_full()
            .items_center()
            .justify_center()
            .border_1()
            .border_color(rgb(0xd8dde6))
            .text_color(rgb(0x657083))
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DockDragPayload>, window, cx| {
                    let payload = event.drag(cx).clone();
                    this.update_empty_space_drop_scene_from_render(
                        &payload,
                        event.event.position,
                        event.bounds,
                        false,
                        window,
                        cx,
                    );
                },
            ));
        empty = empty.child(
            self.render_viewport_drop_scene_fact_probe(viewport_host_scene_frame, move |bounds| {
                drop_scene_fact::empty_space(space, bounds)
            }),
        );
        empty = empty.child(session.empty_message().to_string());
        if let Some(guides) = self.render_drop_guides(session, None, cx) {
            empty = empty.child(guides);
        }
        empty.into_any_element()
    }

    fn render_passthrough_empty_central_space(
        &mut self,
        session: &DockHostRenderSession,
        viewport_host_scene_frame: &DockViewportHostSceneFrameSlot,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::EmptySpace,
            format!("{}:empty-central", session.selector_prefix()),
        );
        let space = session.space().clone();
        let mut empty = div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .relative()
            .flex()
            .size_full()
            .bg(rgba(0x00000000))
            .on_drag_move(cx.listener(
                move |this, event: &DragMoveEvent<DockDragPayload>, window, cx| {
                    let payload = event.drag(cx).clone();
                    this.update_empty_space_drop_scene_from_render(
                        &payload,
                        event.event.position,
                        event.bounds,
                        true,
                        window,
                        cx,
                    );
                },
            ));
        empty = empty.child(
            self.render_viewport_drop_scene_fact_probe(viewport_host_scene_frame, move |bounds| {
                drop_scene_fact::empty_central_space(space, bounds)
            }),
        );
        if let Some(guides) = self.render_drop_guides(session, None, cx) {
            empty = empty.child(guides);
        }
        empty.into_any_element()
    }

    pub(crate) fn render_missing_node(
        &mut self,
        node: DockNodeId,
        session: &DockHostRenderSession,
    ) -> AnyElement {
        let selector = self.record_debug_selector(
            DockDebugRegion::MissingNode { node },
            format!(
                "{}:missing-node:{}",
                session.selector_prefix(),
                node.as_u64()
            ),
        );
        div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .flex()
            .size_full()
            .items_center()
            .justify_center()
            .border_1()
            .border_color(rgb(0xb42318))
            .text_color(rgb(0xb42318))
            .child(format!("Missing dock node: {}", node.as_u64()))
            .into_any_element()
    }

    fn render_host_drop_preview(
        &mut self,
        session: &DockHostRenderSession,
        window: &Window,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let active_payload_title = cx
            .active_drag_value::<DockDragPayload>()
            .map(|payload| payload.title().to_string());
        let routed_preview = self
            .viewport_runtime()
            .routed_drop_preview_for(self.space(), window.window_handle().window_id());
        let local_preview = self.interaction().drop_preview();
        let route_preview = self
            .viewport_runtime()
            .routed_drop_route_preview_for(self.space(), window.window_handle().window_id());
        let routed_target_preview =
            routed_preview.map(|preview| (preview.preview, Some(preview.payload_title)));

        if let Some(preview) = local_preview {
            self.interaction_mut().finish_drop_acceptance_pass();
            return Some(self.render_target_drop_preview(session, preview, active_payload_title));
        }

        if let Some((preview, payload_title)) = routed_target_preview {
            self.viewport_runtime().finish_routed_drop_acceptance_pass(
                self.space(),
                window.window_handle().window_id(),
            );
            return Some(self.render_target_drop_preview(session, preview, payload_title));
        }

        route_preview.map(|preview| self.render_route_drop_preview(session, preview))
    }

    fn render_target_drop_preview(
        &mut self,
        session: &DockHostRenderSession,
        preview: DockDropPreview,
        payload_title: Option<String>,
    ) -> AnyElement {
        let bounds = preview.bounds;
        let selector = self.record_debug_selector(
            DockDebugRegion::DropPreview,
            format!("{}:drop-preview", session.selector_prefix()),
        );
        let (border, background) = drop_preview_colors(&preview);
        let mut element = div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .absolute()
            .left(bounds.origin.x)
            .top(bounds.origin.y)
            .w(bounds.size.width)
            .h(bounds.size.height)
            .border_1()
            .border_color(border)
            .bg(background);

        if preview.payload_tab
            && let Some(title) = payload_title
        {
            let tab_left = if bounds.size.width > px(176.0) {
                px(8.0)
            } else {
                px(0.0)
            };
            let tab_width = if bounds.size.width > px(176.0) {
                px(160.0)
            } else {
                bounds.size.width
            };
            let tab_height = if bounds.size.height > px(26.0) {
                px(26.0)
            } else {
                bounds.size.height
            };
            let tab_selector = self.record_debug_selector(
                DockDebugRegion::DropPayloadTabPreview,
                format!("{}:drop-preview:payload-tab", session.selector_prefix()),
            );
            element = element.child(
                div()
                    .id(tab_selector.clone())
                    .debug_selector(move || tab_selector)
                    .absolute()
                    .left(tab_left)
                    .top(px(0.0))
                    .flex()
                    .items_center()
                    .h(tab_height)
                    .w(tab_width)
                    .px_2()
                    .border_1()
                    .border_color(border)
                    .bg(rgb(0xffffff))
                    .text_color(rgb(0x1f2937))
                    .text_sm()
                    .truncate()
                    .child(title),
            );
        }

        element.into_any_element()
    }

    fn render_route_drop_preview(
        &mut self,
        session: &DockHostRenderSession,
        preview: DockDropRoutePreview,
    ) -> AnyElement {
        let bounds = preview.bounds;
        let selector = self.record_debug_selector(
            DockDebugRegion::DropRoutePreview { kind: preview.kind },
            format!("{}:drop-route-preview", session.selector_prefix()),
        );
        let (border, background) = drop_route_preview_colors(&preview);

        div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .absolute()
            .left(bounds.origin.x)
            .top(bounds.origin.y)
            .w(bounds.size.width)
            .h(bounds.size.height)
            .border_1()
            .border_color(border)
            .bg(background)
            .into_any_element()
    }

    pub(crate) fn render_drop_guides(
        &mut self,
        session: &DockHostRenderSession,
        node: Option<DockNodeId>,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let payload = cx.active_drag_value::<DockDragPayload>()?;
        let zones = self.available_drop_guide_zones(session, node, payload, cx);
        if zones.is_empty() {
            return None;
        }

        let mut overlay = div().absolute().top(px(0.0)).left(px(0.0)).size_full();
        for zone in zones {
            overlay = overlay.child(self.render_drop_guide(zone, session, node));
        }
        Some(overlay.into_any_element())
    }

    fn available_drop_guide_zones(
        &self,
        session: &DockHostRenderSession,
        node: Option<DockNodeId>,
        payload: &DockDragPayload,
        cx: &Context<Self>,
    ) -> Vec<DropZone> {
        self.with_workspace(cx, |workspace| {
            let policy = workspace.policy();
            let payload_classes = workspace.payload_dock_classes_for_drag_payload(payload);
            let target_validator = dock_target_validator(session.space(), &payload_classes, policy);
            DROP_GUIDE_ZONES
                .into_iter()
                .filter(|zone| {
                    let Some(target) =
                        drop_guide_target_for_zone(session, node, *zone, workspace.graph())
                    else {
                        return false;
                    };
                    matches!(
                        validate_resolved_drop_target(target, policy, Some(&target_validator)),
                        DockDropResolution::Valid(_)
                    )
                })
                .collect()
        })
    }

    fn render_drop_guide(
        &mut self,
        zone: DropZone,
        session: &DockHostRenderSession,
        node: Option<DockNodeId>,
    ) -> AnyElement {
        let selector_suffix = match node {
            Some(node) => format!("{}:{zone:?}", node.as_u64()),
            None => format!("{zone:?}"),
        };
        let selector = self.record_debug_selector(
            DockDebugRegion::DropGuide { node, zone },
            format!("{}:drop-guide:{selector_suffix}", session.selector_prefix()),
        );

        let guide_layout = geometry::nominal_drop_guide_layout(session.drop_guide_style());
        let center_size = guide_layout.center_size;
        let side_long = guide_layout.inner_side_long;
        let side_short = guide_layout.inner_side_short;
        let offset = guide_layout.inner_offset;
        let half_center = -center_size / 2.0;
        let half_long = -side_long / 2.0;
        let half_short = -side_short / 2.0;

        let mut guide = div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .absolute()
            .flex()
            .items_center()
            .justify_center()
            .border_1()
            .border_color(rgb(0x1d4ed8))
            .rounded_sm()
            .bg(if zone == DropZone::Center {
                rgba(0xbfdbfecc)
            } else {
                rgba(0xdbeafecc)
            })
            .opacity(0.92);

        guide = match zone {
            DropZone::Center => guide
                .left(relative(0.5))
                .top(relative(0.5))
                .w(center_size)
                .h(center_size)
                .ml(half_center)
                .mt(half_center)
                .child(div().w(px(8.0)).h(px(1.0)).bg(rgb(0x1d4ed8))),
            DropZone::Left => guide
                .left(relative(0.5))
                .top(relative(0.5))
                .w(side_short)
                .h(side_long)
                .ml(-offset + half_short)
                .mt(half_long)
                .child(div().w(px(1.0)).h(side_long - px(8.0)).bg(rgb(0x1d4ed8))),
            DropZone::Right => guide
                .left(relative(0.5))
                .top(relative(0.5))
                .w(side_short)
                .h(side_long)
                .ml(offset + half_short)
                .mt(half_long)
                .child(div().w(px(1.0)).h(side_long - px(8.0)).bg(rgb(0x1d4ed8))),
            DropZone::Top => guide
                .left(relative(0.5))
                .top(relative(0.5))
                .w(side_long)
                .h(side_short)
                .ml(half_long)
                .mt(-offset + half_short)
                .child(div().w(side_long - px(8.0)).h(px(1.0)).bg(rgb(0x1d4ed8))),
            DropZone::Bottom => guide
                .left(relative(0.5))
                .top(relative(0.5))
                .w(side_long)
                .h(side_short)
                .ml(half_long)
                .mt(offset + half_short)
                .child(div().w(side_long - px(8.0)).h(px(1.0)).bg(rgb(0x1d4ed8))),
        };

        guide.into_any_element()
    }

    /// Publishes viewport bounds during prepaint so cross-window releases can resolve even when
    /// the target window did not receive the drag-move event.
    pub(crate) fn render_viewport_host_scene_probe(
        &self,
        frame_slot: &DockViewportHostSceneFrameSlot,
        drop_guide_style: geometry::DockDropGuideStyle,
        passthrough_pointer_input: bool,
    ) -> AnyElement {
        let runtime = self.viewport_runtime().clone();
        let space = self.space().clone();
        let frame_slot = frame_slot.clone();
        canvas(
            move |bounds, window, app| {
                let mouse_position = window.mouse_position();
                let host_position = point(
                    mouse_position.x - bounds.origin.x,
                    mouse_position.y - bounds.origin.y,
                );
                let preparation = runtime.prepare_rendered_viewport_host_scene_frame(
                    space.clone(),
                    window,
                    app,
                    bounds,
                    host_position,
                    drop_guide_style,
                    passthrough_pointer_input,
                );
                *frame_slot.borrow_mut() = preparation.frame;
                if preparation.changed {
                    window.refresh();
                }
                if let Some(token) = preparation.render_token {
                    let runtime = runtime.clone();
                    window.request_animation_frame();
                    // The next-frame callback runs before that frame renders. Check one frame later
                    // so a normally repainted host can publish a newer token first.
                    window.on_next_frame(move |window, _| {
                        let runtime = runtime.clone();
                        window.refresh();
                        window.on_next_frame(move |window, app| {
                            if runtime.expire_viewport_host_scene_if_not_rendered_after(token, app)
                            {
                                window.refresh();
                            }
                        });
                    });
                }
            },
            |_, _, _, _| (),
        )
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .size_full()
        .into_any_element()
    }

    /// Publishes target bounds during prepaint for runtime-routed drops.
    pub(crate) fn render_viewport_drop_scene_fact_probe(
        &self,
        frame_slot: &DockViewportHostSceneFrameSlot,
        fact_for_bounds: impl FnOnce(Bounds<Pixels>) -> DockHostDropSceneFact + 'static,
    ) -> AnyElement {
        let runtime = self.viewport_runtime().clone();
        let frame_slot = frame_slot.clone();
        canvas(
            move |bounds, _window, _| {
                let Some(frame) = frame_slot.borrow().as_ref().cloned() else {
                    return;
                };
                if let Some(next_frame) =
                    runtime.push_viewport_host_scene_frame_fact(&frame, fact_for_bounds(bounds))
                {
                    *frame_slot.borrow_mut() = Some(next_frame);
                }
            },
            |_, _, _, _| (),
        )
        .absolute()
        .top(px(0.0))
        .left(px(0.0))
        .size_full()
        .into_any_element()
    }
}

fn drop_guide_target_for_zone(
    session: &DockHostRenderSession,
    node: Option<DockNodeId>,
    zone: DropZone,
    graph: &DockGraph,
) -> Option<DockResolvedDropTarget> {
    match node {
        Some(tabs) => tabs_drop_guide_target(session, tabs, zone, graph),
        None => host_drop_guide_target(session, zone, graph),
    }
}

fn tabs_drop_guide_target(
    session: &DockHostRenderSession,
    tabs: DockNodeId,
    zone: DropZone,
    graph: &DockGraph,
) -> Option<DockResolvedDropTarget> {
    if !matches!(session.node(tabs), Some(DockNode::Tabs { .. })) {
        return None;
    }

    let root = session.drop_root_for_tabs(tabs)?;
    let is_central_region = session.is_central_tabs(tabs);

    if zone == DropZone::Center {
        return Some(DockResolvedDropTarget {
            kind: DockResolvedDropTargetKind::LeafCenter {
                root,
                target_tabs: tabs,
            },
            source: DockDropResolveSource::LeafBody,
            drop_box: None,
            preview_bounds: None,
            edge_sizing: None,
            edge_plan: None,
            is_central_region,
        });
    }

    // Central-node side splits are represented by the host-level outer guides.
    if is_central_region {
        return None;
    }

    let edge_sizing = DockEdgeDockSizing::fallback();
    let edge_plan = graph.edge_dock_plan_with_sizing(session.space(), tabs, zone, edge_sizing)?;
    Some(DockResolvedDropTarget {
        kind: DockResolvedDropTargetKind::InnerEdge {
            root,
            target_tabs: tabs,
            zone,
        },
        source: DockDropResolveSource::InnerEdge,
        drop_box: None,
        preview_bounds: None,
        edge_sizing: Some(edge_sizing),
        edge_plan: Some(edge_plan),
        is_central_region,
    })
}

fn host_drop_guide_target(
    session: &DockHostRenderSession,
    zone: DropZone,
    graph: &DockGraph,
) -> Option<DockResolvedDropTarget> {
    if let Some(root) = session.root() {
        if zone == DropZone::Center {
            return None;
        }

        let edge_sizing = DockEdgeDockSizing::fallback();
        let edge_plan =
            graph.edge_dock_plan_with_sizing(session.space(), root, zone, edge_sizing)?;
        return Some(DockResolvedDropTarget {
            kind: DockResolvedDropTargetKind::RootEdge {
                root,
                leaf_tabs: None,
                zone,
            },
            source: DockDropResolveSource::RootEdge,
            drop_box: None,
            preview_bounds: None,
            edge_sizing: Some(edge_sizing),
            edge_plan: Some(edge_plan),
            is_central_region: false,
        });
    }

    if zone != DropZone::Center {
        return None;
    }

    let is_central = session.has_empty_central_region();
    Some(DockResolvedDropTarget {
        kind: DockResolvedDropTargetKind::EmptyDockSpace {
            space: session.space().clone(),
        },
        source: DockDropResolveSource::EmptyDockSpace,
        drop_box: None,
        preview_bounds: None,
        edge_sizing: None,
        edge_plan: None,
        is_central_region: is_central,
    })
}

fn drop_preview_colors(preview: &DockDropPreview) -> (Rgba, Rgba) {
    if preview.rejected {
        return (rgb(0xdc2626), rgba(0xfca5a547));
    }

    (rgb(0x2563eb), rgba(0x60a5fa47))
}

fn drop_route_preview_colors(preview: &DockDropRoutePreview) -> (Rgba, Rgba) {
    if preview.rejected {
        return (rgb(0xdc2626), rgba(0xfca5a547));
    }

    match preview.kind {
        crate::drop_preview::DockDropRoutePreviewKind::KnownViewport => {
            (rgb(0x059669), rgba(0x6ee7b747))
        }
        crate::drop_preview::DockDropRoutePreviewKind::TearOff => (rgb(0x7c3aed), rgba(0xc4b5fd47)),
        crate::drop_preview::DockDropRoutePreviewKind::Rejected => {
            (rgb(0xdc2626), rgba(0xfca5a547))
        }
    }
}
