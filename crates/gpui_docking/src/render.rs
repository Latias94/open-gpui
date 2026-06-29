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
    MouseUpEvent, ParentElement, Pixels, Render, Rgba, SharedString, Styled, Window, WindowId,
    black, canvas, div, point, px, rgb, rgba,
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

const DROP_PREVIEW_TAB_HEIGHT: f32 = 26.0;
const DROP_PREVIEW_TAB_HORIZONTAL_INSET: f32 = 8.0;
const DROP_PREVIEW_TAB_GAP: f32 = 6.0;
const DROP_PREVIEW_TAB_MIN_WIDTH: f32 = 72.0;
const DROP_PREVIEW_TAB_MAX_WIDTH: f32 = 180.0;
const DROP_PREVIEW_TAB_TEXT_PADDING: f32 = 22.0;
const DROP_PREVIEW_TAB_MIN_VISIBLE_WIDTH: f32 = 18.0;

#[derive(Debug, Clone, Copy, PartialEq)]
struct DockDropPreviewTabLayout {
    body_bounds: Bounds<Pixels>,
    tab_bounds: Bounds<Pixels>,
}

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
    fn drop_preview_tab_layout(
        &self,
        session: &DockHostRenderSession,
        preview: &DockDropPreview,
        payload_title: &str,
        window: &Window,
    ) -> Option<DockDropPreviewTabLayout> {
        if !preview.payload_tab {
            return None;
        }

        let target_tabs = preview.target_tabs?;
        let DockNode::Tabs { items, .. } = session.node(target_tabs)?.clone() else {
            return None;
        };

        let preview_bounds = preview.bounds;
        let tab_height = px(f32::from(preview_bounds.size.height)
            .min(DROP_PREVIEW_TAB_HEIGHT)
            .max(0.0));
        if tab_height <= px(0.0) {
            return None;
        }

        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let payload_line = window.text_system().shape_line(
            SharedString::from(payload_title.to_string()),
            font_size,
            &[text_style.to_run(payload_title.len())],
            None,
        );
        let payload_tab_width = preview_tab_width(payload_line.width());
        let tab_gap = px(DROP_PREVIEW_TAB_GAP);
        let insert_index = preview.insert_index.unwrap_or(items.len()).min(items.len());
        let mut tab_left = self
            .viewport_runtime()
            .rendered_tab_bar_bounds_for_tabs(self.space(), None, target_tabs)
            .map(|tab_bar_bounds| tab_bar_bounds.origin.x + px(DROP_PREVIEW_TAB_HORIZONTAL_INSET))
            .unwrap_or(preview_bounds.origin.x + px(DROP_PREVIEW_TAB_HORIZONTAL_INSET));

        if let Some(label_bounds) = insert_index.checked_sub(1).and_then(|target_index| {
            self.viewport_runtime().rendered_tab_label_bounds_for_tabs(
                self.space(),
                None,
                target_tabs,
                target_index,
            )
        }) {
            tab_left = label_bounds.right() + tab_gap;
        } else {
            for item in items.iter().take(insert_index) {
                let title = session.panel_title(item);
                let title_line = window.text_system().shape_line(
                    SharedString::from(title.clone()),
                    font_size,
                    &[text_style.to_run(title.len())],
                    None,
                );
                tab_left += preview_tab_width(title_line.width()) + tab_gap;
            }
        }

        let tab_width = payload_tab_width
            .min(preview_bounds.size.width)
            .max(px(0.0));
        let max_tab_left = (preview_bounds.origin.x + preview_bounds.size.width
            - px(DROP_PREVIEW_TAB_MIN_VISIBLE_WIDTH))
        .max(preview_bounds.origin.x);
        let tab_left = tab_left.clamp(preview_bounds.origin.x, max_tab_left);
        let tab_bounds = Bounds::new(
            point(tab_left, preview_bounds.origin.y),
            open_gpui::size(tab_width, tab_height),
        );

        let body_origin_y = tab_bounds.origin.y + tab_bounds.size.height;
        let body_height =
            (preview_bounds.origin.y + preview_bounds.size.height - body_origin_y).max(px(0.0));
        let body_bounds = Bounds::new(
            point(preview_bounds.origin.x, body_origin_y),
            open_gpui::size(preview_bounds.size.width, body_height),
        );

        Some(DockDropPreviewTabLayout {
            body_bounds,
            tab_bounds,
        })
    }

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
                    if !event.bounds.contains(&event.event.position) {
                        return;
                    }
                    this.update_local_root_drop_scene_from_render(
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
        if let Some(guides) =
            self.render_drop_guides(session, None, window.window_handle().window_id(), cx)
        {
            root_container = root_container.child(guides);
        }
        root_container.into_any_element()
    }

    fn render_empty_space(
        &mut self,
        session: &DockHostRenderSession,
        viewport_host_scene_frame: &DockViewportHostSceneFrameSlot,
        window: &mut Window,
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
                    if !event.bounds.contains(&event.event.position) {
                        return;
                    }
                    this.update_local_empty_space_drop_scene_from_render(
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
        if let Some(guides) =
            self.render_drop_guides(session, None, window.window_handle().window_id(), cx)
        {
            empty = empty.child(guides);
        }
        empty.into_any_element()
    }

    fn render_passthrough_empty_central_space(
        &mut self,
        session: &DockHostRenderSession,
        viewport_host_scene_frame: &DockViewportHostSceneFrameSlot,
        window: &mut Window,
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
                    if !event.bounds.contains(&event.event.position) {
                        return;
                    }
                    this.update_local_empty_space_drop_scene_from_render(
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
        if let Some(guides) =
            self.render_drop_guides(session, None, window.window_handle().window_id(), cx)
        {
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
        window: &mut Window,
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
            return Some(self.render_target_drop_preview(
                session,
                preview,
                active_payload_title,
                window,
            ));
        }

        if let Some((preview, payload_title)) = routed_target_preview {
            return Some(self.render_target_drop_preview(session, preview, payload_title, window));
        }

        route_preview.map(|preview| self.render_route_drop_preview(session, preview))
    }

    fn render_target_drop_preview(
        &mut self,
        session: &DockHostRenderSession,
        preview: DockDropPreview,
        payload_title: Option<String>,
        window: &Window,
    ) -> AnyElement {
        let bounds = preview
            .target_tabs
            .and_then(|tabs| {
                preview.payload_tab.then(|| {
                    self.viewport_runtime()
                        .rendered_leaf_bounds_for_tabs(self.space(), None, tabs)
                })?
            })
            .unwrap_or(preview.bounds);
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
            .overflow_hidden();

        if let Some(title) = payload_title
            && let Some(layout) = self.drop_preview_tab_layout(session, &preview, &title, window)
        {
            let body_selector = self.record_debug_selector(
                DockDebugRegion::DropPreviewBody,
                format!("{}:drop-preview:body", session.selector_prefix()),
            );
            let tab_selector = self.record_debug_selector(
                DockDebugRegion::DropPayloadTabPreview,
                format!("{}:drop-preview:payload-tab", session.selector_prefix()),
            );
            let mut body = div()
                .id(body_selector.clone())
                .debug_selector(move || body_selector)
                .absolute()
                .left(layout.body_bounds.origin.x - bounds.origin.x)
                .top(layout.body_bounds.origin.y - bounds.origin.y)
                .w(layout.body_bounds.size.width)
                .h(layout.body_bounds.size.height)
                .border_1()
                .border_color(border)
                .bg(background);
            if layout.body_bounds.size.height > px(0.0) {
                body = body.rounded_b_sm().border_t_0();
            }
            element = element.child(body).child(
                div()
                    .id(tab_selector.clone())
                    .debug_selector(move || tab_selector)
                    .absolute()
                    .left(layout.tab_bounds.origin.x - bounds.origin.x)
                    .top(layout.tab_bounds.origin.y - bounds.origin.y)
                    .flex()
                    .items_center()
                    .justify_start()
                    .h(layout.tab_bounds.size.height)
                    .w(layout.tab_bounds.size.width)
                    .px_2()
                    .border_1()
                    .border_color(border)
                    .bg(rgb(0xf8fafc))
                    .text_color(rgb(0x334155))
                    .text_sm()
                    .shadow_sm()
                    .truncate()
                    .rounded_t_sm()
                    .rounded_br_sm()
                    .border_b_0()
                    .child(title),
            );
        } else {
            element = element.child(
                div()
                    .absolute()
                    .left(px(0.0))
                    .top(px(0.0))
                    .w(bounds.size.width)
                    .h(bounds.size.height)
                    .border_1()
                    .border_color(border)
                    .bg(background),
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
        window_id: WindowId,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        let payload = cx.active_drag_value::<DockDragPayload>()?;
        let zones = self.available_drop_guide_zones(session, node, payload, cx);
        if zones.is_empty() {
            return None;
        }
        let target_bounds = self.drop_guide_target_bounds(session, node, window_id)?;
        let active_target = self.interaction().resolved_drop_target().cloned();

        let mut overlay = div().absolute().top(px(0.0)).left(px(0.0)).size_full();
        for zone in zones {
            let Some(drop_box) = drop_guide_box_for_zone(session, node, target_bounds, zone) else {
                continue;
            };
            let active = active_target
                .as_ref()
                .is_some_and(|target| drop_target_matches_guide(target, node, zone));
            overlay = overlay.child(self.render_drop_guide(
                zone,
                session,
                node,
                target_bounds,
                drop_box,
                active,
            ));
        }
        Some(overlay.into_any_element())
    }

    fn drop_guide_target_bounds(
        &self,
        session: &DockHostRenderSession,
        node: Option<DockNodeId>,
        window_id: WindowId,
    ) -> Option<Bounds<Pixels>> {
        match node {
            Some(tabs) => self.viewport_runtime().rendered_leaf_bounds_for_tabs(
                session.space(),
                Some(window_id),
                tabs,
            ),
            None => self
                .viewport_runtime()
                .rendered_host_bounds_for_window(session.space(), Some(window_id)),
        }
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
        container_bounds: Bounds<Pixels>,
        drop_box: geometry::DockDropBox,
        active: bool,
    ) -> AnyElement {
        let selector_suffix = match node {
            Some(node) => format!("{}:{zone:?}", node.as_u64()),
            None => format!("{zone:?}"),
        };
        let selector = self.record_debug_selector(
            DockDebugRegion::DropGuide { node, zone },
            format!("{}:drop-guide:{selector_suffix}", session.selector_prefix()),
        );
        let local_bounds = localize_bounds(drop_box.hit_bounds, container_bounds.origin);
        let palette = drop_guide_palette(drop_box.kind, active);
        let cue = guide_directional_cue(zone, local_bounds.size, palette.cue);
        let inset = guide_inset_outline(local_bounds.size, palette.inset);

        let mut guide = div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .absolute()
            .left(local_bounds.origin.x)
            .top(local_bounds.origin.y)
            .w(local_bounds.size.width)
            .h(local_bounds.size.height)
            .flex()
            .items_center()
            .justify_center()
            .border_1()
            .border_color(palette.border)
            .rounded_sm()
            .bg(palette.background);
        if let Some(inset) = inset {
            guide = guide.child(inset);
        }
        if let Some(cue) = cue {
            guide = guide.child(cue);
        }

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

#[derive(Debug, Clone, Copy)]
struct DockGuidePalette {
    border: Rgba,
    background: Rgba,
    cue: Rgba,
    inset: Rgba,
}

fn drop_guide_box_for_zone(
    session: &DockHostRenderSession,
    node: Option<DockNodeId>,
    target_bounds: Bounds<Pixels>,
    zone: DropZone,
) -> Option<geometry::DockDropBox> {
    let kind = match node {
        Some(_) => match zone {
            DropZone::Center => geometry::DockDropBoxKind::Center,
            DropZone::Left | DropZone::Right | DropZone::Top | DropZone::Bottom => {
                geometry::DockDropBoxKind::InnerEdge(zone)
            }
        },
        None if session.root().is_some() => match zone {
            DropZone::Center => return None,
            DropZone::Left | DropZone::Right | DropZone::Top | DropZone::Bottom => {
                geometry::DockDropBoxKind::OuterEdge(zone)
            }
        },
        None => {
            if zone != DropZone::Center {
                return None;
            }
            geometry::DockDropBoxKind::Center
        }
    };
    let set = if matches!(kind, geometry::DockDropBoxKind::OuterEdge(_)) {
        geometry::DockDropBoxSet::Outer
    } else {
        geometry::DockDropBoxSet::Inner
    };
    geometry::drop_boxes_with_style(target_bounds, set, session.drop_guide_style())
        .into_iter()
        .find(|drop_box| drop_box.kind == kind)
}

fn drop_guide_palette(kind: geometry::DockDropBoxKind, active: bool) -> DockGuidePalette {
    match (kind.is_center(), active) {
        (true, true) => DockGuidePalette {
            border: rgb(0x2563eb),
            background: rgba(0x93c5fd59),
            cue: rgb(0x1d4ed8),
            inset: rgba(0xffffff73),
        },
        (true, false) => DockGuidePalette {
            border: rgba(0x3b82f680),
            background: rgba(0xdbeafe45),
            cue: rgba(0x2563ebad),
            inset: rgba(0xffffff52),
        },
        (false, true) => DockGuidePalette {
            border: rgb(0x1d4ed8),
            background: rgba(0x60a5fa52),
            cue: rgb(0x1e40af),
            inset: rgba(0xffffff6b),
        },
        (false, false) => DockGuidePalette {
            border: rgba(0x3b82f666),
            background: rgba(0xbfdbfe33),
            cue: rgba(0x2563eb94),
            inset: rgba(0xffffff40),
        },
    }
}

fn guide_directional_cue(
    zone: DropZone,
    box_size: open_gpui::Size<Pixels>,
    cue: Rgba,
) -> Option<AnyElement> {
    match zone {
        DropZone::Center => Some(
            div()
                .w((box_size.width * 0.48).max(px(10.0)))
                .h(px(2.0))
                .bg(cue)
                .into_any_element(),
        ),
        DropZone::Left | DropZone::Right => Some(
            div()
                .w(px(2.0))
                .h((box_size.height * 0.62).max(px(10.0)))
                .bg(cue)
                .into_any_element(),
        ),
        DropZone::Top | DropZone::Bottom => Some(
            div()
                .w((box_size.width * 0.62).max(px(10.0)))
                .h(px(2.0))
                .bg(cue)
                .into_any_element(),
        ),
    }
}

fn guide_inset_outline(box_size: open_gpui::Size<Pixels>, color: Rgba) -> Option<AnyElement> {
    if box_size.width <= px(10.0) || box_size.height <= px(10.0) {
        return None;
    }
    Some(
        div()
            .absolute()
            .left(px(3.0))
            .top(px(3.0))
            .w((box_size.width - px(6.0)).max(px(1.0)))
            .h((box_size.height - px(6.0)).max(px(1.0)))
            .border_1()
            .border_color(color)
            .rounded_sm()
            .into_any_element(),
    )
}

fn localize_bounds(bounds: Bounds<Pixels>, origin: open_gpui::Point<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        point(bounds.origin.x - origin.x, bounds.origin.y - origin.y),
        bounds.size,
    )
}

fn drop_target_matches_guide(
    target: &DockResolvedDropTarget,
    node: Option<DockNodeId>,
    zone: DropZone,
) -> bool {
    match (&target.kind, node, zone) {
        (DockResolvedDropTargetKind::EmptyDockSpace { .. }, None, DropZone::Center) => true,
        (
            DockResolvedDropTargetKind::RootEdge {
                zone: active_zone, ..
            },
            None,
            zone,
        ) => *active_zone == zone,
        (
            DockResolvedDropTargetKind::TabBar { target_tabs, .. }
            | DockResolvedDropTargetKind::LeafCenter { target_tabs, .. }
            | DockResolvedDropTargetKind::FloatingTitleBar { target_tabs, .. },
            Some(node),
            DropZone::Center,
        ) => *target_tabs == node,
        (
            DockResolvedDropTargetKind::InnerEdge {
                target_tabs,
                zone: active_zone,
                ..
            },
            Some(node),
            zone,
        ) => *target_tabs == node && *active_zone == zone,
        _ => false,
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

    // Match ImGui: only the root central leaf suppresses inner side splits in favor of
    // host/root outer guides. Nested central leaves still expose inner side guides.
    if is_central_region && root == tabs {
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

fn preview_tab_width(text_width: Pixels) -> Pixels {
    (text_width + px(DROP_PREVIEW_TAB_TEXT_PADDING))
        .max(px(DROP_PREVIEW_TAB_MIN_WIDTH))
        .min(px(DROP_PREVIEW_TAB_MAX_WIDTH))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drop_preview::DockDropRoutePreviewKind;
    use open_gpui::{point, size};

    fn preview(rejected: bool, payload_tab: bool) -> DockDropPreview {
        DockDropPreview {
            bounds: Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(80.0))),
            rejected,
            payload_tab,
            target_tabs: None,
            insert_index: None,
        }
    }

    fn route_preview(kind: DockDropRoutePreviewKind, rejected: bool) -> DockDropRoutePreview {
        DockDropRoutePreview {
            kind,
            bounds: Bounds::new(point(px(0.0), px(0.0)), size(px(56.0), px(40.0))),
            rejected,
        }
    }

    #[test]
    fn active_center_guides_have_stronger_palette_than_inactive_edge_guides() {
        let active_center = drop_guide_palette(geometry::DockDropBoxKind::Center, true);
        let inactive_edge =
            drop_guide_palette(geometry::DockDropBoxKind::InnerEdge(DropZone::Left), false);

        assert_ne!(active_center.border, inactive_edge.border);
        assert_ne!(active_center.background, inactive_edge.background);
        assert!(active_center.background.a > inactive_edge.background.a);
    }

    #[test]
    fn rejected_drop_preview_uses_rejected_palette() {
        let accepted = drop_preview_colors(&preview(false, false));
        let rejected = drop_preview_colors(&preview(true, false));

        assert_ne!(accepted, rejected);
        assert_eq!(rejected.0, rgb(0xdc2626));
    }

    #[test]
    fn route_preview_kinds_keep_distinct_palettes() {
        let known = drop_route_preview_colors(&route_preview(
            DockDropRoutePreviewKind::KnownViewport,
            false,
        ));
        let tear_off =
            drop_route_preview_colors(&route_preview(DockDropRoutePreviewKind::TearOff, false));
        let rejected =
            drop_route_preview_colors(&route_preview(DockDropRoutePreviewKind::Rejected, true));

        assert_ne!(known, tear_off);
        assert_ne!(known, rejected);
        assert_ne!(tear_off, rejected);
    }

    #[test]
    fn preview_tab_width_stays_within_bounds() {
        assert_eq!(preview_tab_width(px(8.0)), px(DROP_PREVIEW_TAB_MIN_WIDTH));
        assert_eq!(preview_tab_width(px(240.0)), px(DROP_PREVIEW_TAB_MAX_WIDTH));
        assert_eq!(
            preview_tab_width(px(90.0)),
            px(90.0 + DROP_PREVIEW_TAB_TEXT_PADDING)
        );
    }
}
