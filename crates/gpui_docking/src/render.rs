use crate::{
    DockHost, DockNode, DockNodeId, DropZone,
    debug::DockDebugRegion,
    drag::DockDragPayload,
    drop_preview::{
        DockDropPreview, DockDropRoutePreview, DockPreviewDropBox, DockPreviewTabInsertionIndex,
    },
    drop_runtime::DockHostDropSceneFact,
    drop_scene_fact, geometry,
    host_render_session::{DockHostRenderSession, selected_index},
    interaction::{
        DockPayloadDropRelease, DockRenderedOutsideReleaseDecision,
        DockRenderedOutsideReleaseRequest,
    },
    overlay_scene::{DockOverlayLayer, DockOverlayScene},
    render_split::DockRenderSplitInput,
    viewport_drop_scene::DockViewportHostSceneFrame,
};
use open_gpui::{
    AnyElement, Bounds, Context, DragMoveEvent, InteractiveElement, IntoElement, MouseButton,
    MouseUpEvent, ParentElement, Pixels, Render, Rgba, SharedString, Styled, Window, black, canvas,
    div, point, px, rgb, rgba,
};
use std::{cell::RefCell, rc::Rc};

pub(crate) type DockViewportHostSceneFrameSlot = Rc<RefCell<Option<DockViewportHostSceneFrame>>>;

const DROP_PREVIEW_TAB_HEIGHT: f32 = 26.0;
const DROP_PREVIEW_TAB_HORIZONTAL_INSET: f32 = 8.0;
const DROP_PREVIEW_TAB_GAP: f32 = 6.0;
const DROP_PREVIEW_TAB_MIN_WIDTH: f32 = 72.0;
const DROP_PREVIEW_TAB_MAX_WIDTH: f32 = 180.0;
const DROP_PREVIEW_TAB_TEXT_PADDING: f32 = 22.0;
const DROP_PREVIEW_TAB_MIN_VISIBLE_WIDTH: f32 = 18.0;

#[derive(Debug, Clone, PartialEq)]
struct DockDropPreviewTabLayout {
    body_bounds: Bounds<Pixels>,
    insertion_bounds: Bounds<Pixels>,
    tab_bounds: Vec<DockDropPreviewTabPlacement>,
}

#[derive(Debug, Clone, PartialEq)]
struct DockDropPreviewTabPlacement {
    index: usize,
    title: String,
    tab_bounds: Bounds<Pixels>,
}

#[derive(Debug, Clone, PartialEq)]
struct DockDropPreviewPayloadTab {
    index: usize,
    title: String,
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
        preview_bounds: Bounds<Pixels>,
        overlay_scene: &DockOverlayScene,
        window: &Window,
    ) -> Option<DockDropPreviewTabLayout> {
        let insertion = overlay_scene.tab_insertion()?;
        let target_tabs = insertion.target_node?;
        let DockNode::Tabs { items, .. } = session.node(target_tabs)?.clone() else {
            return None;
        };
        let payload_tabs = overlay_payload_tabs(overlay_scene);
        if payload_tabs.is_empty() {
            return None;
        }
        let tab_height = px(f32::from(preview_bounds.size.height)
            .min(DROP_PREVIEW_TAB_HEIGHT)
            .max(0.0));
        if tab_height <= px(0.0) {
            return None;
        }

        let text_style = window.text_style();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let tab_gap = px(DROP_PREVIEW_TAB_GAP);
        let insert_index = insertion
            .tab_insertion
            .as_ref()
            .map(|insertion| match insertion.index {
                DockPreviewTabInsertionIndex::At(index) => index,
                DockPreviewTabInsertionIndex::Append => items.len(),
            })
            .unwrap_or(items.len())
            .min(items.len());
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

        let tab_strip_left = f32::from(preview_bounds.origin.x);
        let tab_strip_right = f32::from(preview_bounds.origin.x + preview_bounds.size.width);
        let tab_gap = f32::from(tab_gap);
        let requested_left = f32::from(tab_left).max(tab_strip_left);
        let mut tab_widths = Vec::with_capacity(payload_tabs.len());
        for payload_tab in &payload_tabs {
            let payload_title = payload_tab.title.as_str();
            let payload_line = window.text_system().shape_line(
                SharedString::from(payload_title.to_string()),
                font_size,
                &[text_style.to_run(payload_title.len())],
                None,
            );
            tab_widths.push(f32::from(preview_tab_width(payload_line.width())));
        }
        let mut visible_count = tab_widths.len();
        while visible_count > 0 {
            let total_gap = tab_gap * visible_count.saturating_sub(1) as f32;
            if tab_strip_right - tab_strip_left
                >= (DROP_PREVIEW_TAB_MIN_VISIBLE_WIDTH * visible_count as f32) + total_gap
            {
                break;
            }
            visible_count -= 1;
        }
        if visible_count == 0 {
            return None;
        }
        tab_widths.truncate(visible_count);
        let total_gap = tab_gap * visible_count.saturating_sub(1) as f32;
        let available_width =
            (tab_strip_right - requested_left).max(tab_strip_right - tab_strip_left);
        let max_total_tab_width =
            (available_width - total_gap).max(DROP_PREVIEW_TAB_MIN_VISIBLE_WIDTH);
        let requested_total_tab_width: f32 = tab_widths.iter().sum();
        if requested_total_tab_width > max_total_tab_width {
            let compressed_width = (max_total_tab_width / visible_count as f32)
                .max(DROP_PREVIEW_TAB_MIN_VISIBLE_WIDTH);
            tab_widths.fill(compressed_width);
        }
        let tab_strip_width = tab_widths.iter().sum::<f32>() + total_gap;
        let mut tab_left =
            requested_left.min((tab_strip_right - tab_strip_width).max(tab_strip_left));
        let mut tab_bounds = Vec::with_capacity(payload_tabs.len());
        for (payload_tab, tab_width) in payload_tabs.iter().zip(tab_widths) {
            tab_bounds.push(DockDropPreviewTabPlacement {
                index: payload_tab.index,
                title: payload_tab.title.clone(),
                tab_bounds: Bounds::new(
                    point(px(tab_left), preview_bounds.origin.y),
                    open_gpui::size(px(tab_width), tab_height),
                ),
            });
            tab_left += tab_width + tab_gap;
        }

        let first_tab_bounds = tab_bounds.first()?.tab_bounds;
        let insertion_width = px(3.0);
        let insertion_bounds = Bounds::new(
            point(
                first_tab_bounds.origin.x - insertion_width / 2.0,
                first_tab_bounds.origin.y,
            ),
            open_gpui::size(insertion_width, first_tab_bounds.size.height),
        );

        let body_origin_y = first_tab_bounds.origin.y + first_tab_bounds.size.height;
        let body_height =
            (preview_bounds.origin.y + preview_bounds.size.height - body_origin_y).max(px(0.0));
        let body_bounds = Bounds::new(
            point(preview_bounds.origin.x, body_origin_y),
            open_gpui::size(preview_bounds.size.width, body_height),
        );

        Some(DockDropPreviewTabLayout {
            body_bounds,
            insertion_bounds,
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
        let rendered_root = self
            .zoom_state()
            .target(session.space())
            .filter(|target| session.node(*target).is_some())
            .unwrap_or(root);
        let root_child = self.render_node(
            rendered_root,
            session,
            viewport_host_scene_frame,
            window,
            cx,
        );
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
        let active_payload = cx.active_drag_value::<DockDragPayload>().cloned();
        let routed_preview = self
            .viewport_runtime()
            .routed_drop_preview_for(self.space(), window.window_handle().window_id());
        let local_preview = self.interaction().drop_preview();
        let route_preview = self
            .viewport_runtime()
            .routed_drop_route_preview_for(self.space(), window.window_handle().window_id());
        if let Some(mut preview) = local_preview {
            if let Some(payload) = active_payload.as_ref() {
                preview.populate_payload_tabs(payload);
            }
            return Some(self.render_target_drop_preview(session, preview, window));
        }

        if let Some(routed_preview) = routed_preview {
            return Some(self.render_target_drop_preview(session, routed_preview.preview, window));
        }

        route_preview.map(|preview| self.render_route_drop_preview(session, preview))
    }

    fn render_target_drop_preview(
        &mut self,
        session: &DockHostRenderSession,
        preview: DockDropPreview,
        window: &Window,
    ) -> AnyElement {
        let scene = &preview.scene;
        let overlay_scene = DockOverlayScene::from_preview(scene);
        let bounds = scene
            .payload_tabs
            .as_ref()
            .and_then(|payload_tabs| payload_tabs.target_tabs)
            .and_then(|tabs| {
                self.viewport_runtime()
                    .rendered_leaf_bounds_for_tabs(self.space(), None, tabs)
            })
            .unwrap_or(scene.body.future_bounds);
        let selector = self.record_debug_selector(
            DockDebugRegion::DropPreview,
            format!("{}:drop-preview", session.selector_prefix()),
        );
        let theme = dock_preview_theme();
        let palette = theme.target_preview(&scene.decision);
        let mut element = div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .absolute()
            .left(bounds.origin.x)
            .top(bounds.origin.y)
            .w(bounds.size.width)
            .h(bounds.size.height)
            .overflow_hidden();

        if overlay_scene.has_payload_tab_preview()
            && let Some(layout) =
                self.drop_preview_tab_layout(session, bounds, &overlay_scene, window)
        {
            let body_selector = self.record_debug_selector(
                DockDebugRegion::DropPreviewBody,
                format!("{}:drop-preview:body", session.selector_prefix()),
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
                .border_color(palette.border)
                .bg(palette.body_background);
            if layout.body_bounds.size.height > px(0.0) {
                body = body.rounded_b_sm().border_t_0();
            }
            element = element.child(body);
            let insertion_selector = self.record_debug_selector(
                DockDebugRegion::DropTabInsertionPreview,
                format!("{}:drop-preview:tab-insertion", session.selector_prefix()),
            );
            element = element.child(
                div()
                    .id(insertion_selector.clone())
                    .debug_selector(move || insertion_selector)
                    .absolute()
                    .left(layout.insertion_bounds.origin.x - bounds.origin.x)
                    .top(layout.insertion_bounds.origin.y - bounds.origin.y)
                    .w(layout.insertion_bounds.size.width)
                    .h(layout.insertion_bounds.size.height)
                    .rounded_sm()
                    .bg(palette.border),
            );
            for placement in layout.tab_bounds {
                let tab_selector = self.record_debug_selector(
                    DockDebugRegion::DropPayloadTabPreview {
                        index: placement.index,
                    },
                    format!(
                        "{}:drop-preview:payload-tab:{}",
                        session.selector_prefix(),
                        placement.index
                    ),
                );
                element = element.child(
                    div()
                        .id(tab_selector.clone())
                        .debug_selector(move || tab_selector)
                        .absolute()
                        .left(placement.tab_bounds.origin.x - bounds.origin.x)
                        .top(placement.tab_bounds.origin.y - bounds.origin.y)
                        .flex()
                        .items_center()
                        .justify_start()
                        .h(placement.tab_bounds.size.height)
                        .w(placement.tab_bounds.size.width)
                        .px_2()
                        .border_1()
                        .border_color(palette.border)
                        .bg(palette.tab_background)
                        .text_color(palette.tab_text)
                        .text_sm()
                        .shadow_sm()
                        .truncate()
                        .rounded_t_sm()
                        .rounded_br_sm()
                        .border_b_0()
                        .child(placement.title),
                );
            }
        } else {
            let body_selector = self.record_debug_selector(
                DockDebugRegion::DropPreviewBody,
                format!("{}:drop-preview:body", session.selector_prefix()),
            );
            let body_bounds = localize_bounds(scene.body.body_bounds, bounds.origin);
            element = element.child(
                div()
                    .id(body_selector.clone())
                    .debug_selector(move || body_selector)
                    .absolute()
                    .left(body_bounds.origin.x)
                    .top(body_bounds.origin.y)
                    .w(body_bounds.size.width)
                    .h(body_bounds.size.height)
                    .border_1()
                    .border_color(palette.border)
                    .bg(palette.body_background),
            );
        }

        for drop_box in overlay_scene.guide_drop_boxes() {
            element = element.child(self.render_scene_drop_guide(session, bounds, drop_box));
        }

        element.into_any_element()
    }

    fn render_route_drop_preview(
        &mut self,
        session: &DockHostRenderSession,
        preview: DockDropRoutePreview,
    ) -> AnyElement {
        let overlay_scene = DockOverlayScene::from_route_preview(&preview);
        let bounds = overlay_scene
            .layers
            .first()
            .map(|layer| layer.bounds)
            .unwrap_or(preview.bounds);
        let selector = self.record_debug_selector(
            DockDebugRegion::DropRoutePreview { kind: preview.kind },
            format!("{}:drop-route-preview", session.selector_prefix()),
        );
        let theme = dock_preview_theme();
        let palette = theme.route_preview(&preview);

        div()
            .id(selector.clone())
            .debug_selector(move || selector)
            .absolute()
            .left(bounds.origin.x)
            .top(bounds.origin.y)
            .w(bounds.size.width)
            .h(bounds.size.height)
            .border_1()
            .border_color(palette.border)
            .bg(palette.background)
            .into_any_element()
    }

    fn render_scene_drop_guide(
        &mut self,
        session: &DockHostRenderSession,
        container_bounds: Bounds<Pixels>,
        drop_box: DockPreviewDropBox,
    ) -> AnyElement {
        let node = drop_box.debug_node;
        let zone = drop_box.zone;
        let selector_suffix = drop_box_selector_suffix(drop_box);
        let selector = self.record_debug_selector(
            DockDebugRegion::DropGuide { node, zone },
            format!("{}:drop-guide:{selector_suffix}", session.selector_prefix()),
        );
        let local_bounds = localize_bounds(drop_box.draw_bounds, container_bounds.origin);
        let theme = dock_preview_theme();
        let palette = theme.drop_guide(drop_box.kind, drop_box.active);
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

fn overlay_payload_tabs(overlay_scene: &DockOverlayScene) -> Vec<DockDropPreviewPayloadTab> {
    let mut tabs = overlay_scene
        .payload_tabs()
        .filter_map(payload_tab_from_overlay_layer)
        .collect::<Vec<_>>();
    tabs.sort_by_key(|tab| tab.index);
    tabs
}

fn payload_tab_from_overlay_layer(layer: &DockOverlayLayer) -> Option<DockDropPreviewPayloadTab> {
    Some(DockDropPreviewPayloadTab {
        index: layer.payload_index?,
        title: layer.payload_title.clone().unwrap_or_default(),
    })
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

fn drop_box_selector_suffix(drop_box: DockPreviewDropBox) -> String {
    let layer = match drop_box.layer {
        crate::drop_preview::DockPreviewLayerKind::Inner => "inner",
        crate::drop_preview::DockPreviewLayerKind::Outer => "outer",
    };
    match drop_box.debug_node {
        Some(node) => format!("{layer}:{}:{:?}", node.as_u64(), drop_box.zone),
        None => format!("{layer}:{:?}", drop_box.zone),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DockPreviewTheme {
    accepted_target: DockTargetPreviewTokens,
    rejected_target: DockTargetPreviewTokens,
    guide_center_active: DockDropGuideTokens,
    guide_center_inactive: DockDropGuideTokens,
    guide_edge_active: DockDropGuideTokens,
    guide_edge_inactive: DockDropGuideTokens,
    route_known_viewport: DockRoutePreviewTokens,
    route_tear_off: DockRoutePreviewTokens,
    route_rejected: DockRoutePreviewTokens,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DockTargetPreviewTokens {
    border: Rgba,
    body_background: Rgba,
    tab_background: Rgba,
    tab_text: Rgba,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DockDropGuideTokens {
    border: Rgba,
    background: Rgba,
    cue: Rgba,
    inset: Rgba,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct DockRoutePreviewTokens {
    border: Rgba,
    background: Rgba,
}

impl DockPreviewTheme {
    fn default_tokens() -> Self {
        Self {
            accepted_target: DockTargetPreviewTokens {
                border: rgb(0x2563eb),
                body_background: rgba(0x3b82f647),
                tab_background: rgba(0x2563ebd9),
                tab_text: rgb(0xffffff),
            },
            rejected_target: DockTargetPreviewTokens {
                border: rgb(0xdc2626),
                body_background: rgba(0xfca5a547),
                tab_background: rgba(0xdc2626dd),
                tab_text: rgb(0xffffff),
            },
            guide_center_active: DockDropGuideTokens {
                border: rgb(0x2563eb),
                background: rgba(0x93c5fd59),
                cue: rgb(0x1d4ed8),
                inset: rgba(0xffffff73),
            },
            guide_center_inactive: DockDropGuideTokens {
                border: rgba(0x3b82f680),
                background: rgba(0xdbeafe45),
                cue: rgba(0x2563ebad),
                inset: rgba(0xffffff52),
            },
            guide_edge_active: DockDropGuideTokens {
                border: rgb(0x1d4ed8),
                background: rgba(0x60a5fa52),
                cue: rgb(0x1e40af),
                inset: rgba(0xffffff6b),
            },
            guide_edge_inactive: DockDropGuideTokens {
                border: rgba(0x3b82f666),
                background: rgba(0xbfdbfe33),
                cue: rgba(0x2563eb94),
                inset: rgba(0xffffff40),
            },
            route_known_viewport: DockRoutePreviewTokens {
                border: rgb(0x2563eb),
                background: rgba(0x3b82f64f),
            },
            route_tear_off: DockRoutePreviewTokens {
                border: rgb(0x475569),
                background: rgba(0x94a3b847),
            },
            route_rejected: DockRoutePreviewTokens {
                border: rgb(0xdc2626),
                background: rgba(0xfca5a547),
            },
        }
    }

    fn target_preview(
        &self,
        decision: &crate::drop_preview::DockPreviewDecision,
    ) -> DockTargetPreviewTokens {
        if decision.is_allowed() {
            self.accepted_target
        } else {
            self.rejected_target
        }
    }

    fn drop_guide(&self, kind: geometry::DockDropBoxKind, active: bool) -> DockDropGuideTokens {
        match (kind.is_center(), active) {
            (true, true) => self.guide_center_active,
            (true, false) => self.guide_center_inactive,
            (false, true) => self.guide_edge_active,
            (false, false) => self.guide_edge_inactive,
        }
    }

    fn route_preview(&self, preview: &DockDropRoutePreview) -> DockRoutePreviewTokens {
        if preview.rejected {
            return self.route_rejected;
        }

        match preview.kind {
            crate::drop_preview::DockDropRoutePreviewKind::KnownViewport => {
                self.route_known_viewport
            }
            crate::drop_preview::DockDropRoutePreviewKind::TearOff => self.route_tear_off,
            crate::drop_preview::DockDropRoutePreviewKind::Rejected => self.route_rejected,
        }
    }
}

fn dock_preview_theme() -> DockPreviewTheme {
    DockPreviewTheme::default_tokens()
}

fn preview_tab_width(text_width: Pixels) -> Pixels {
    (text_width + px(DROP_PREVIEW_TAB_TEXT_PADDING))
        .max(px(DROP_PREVIEW_TAB_MIN_WIDTH))
        .min(px(DROP_PREVIEW_TAB_MAX_WIDTH))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::drop_preview::DockDropRoutePreviewKind;
    use open_gpui::{point, size};

    fn preview(rejected: bool, payload_tab: bool) -> DockDropPreview {
        let bounds = Bounds::new(point(px(0.0), px(0.0)), size(px(100.0), px(80.0)));
        let target_tabs = None;
        let insert_index = None;
        let decision = if rejected {
            crate::drop_preview::DockPreviewDecision::rejected(None)
        } else {
            crate::drop_preview::DockPreviewDecision::allowed()
        };
        let payload_tabs = payload_tab.then(|| crate::drop_preview::DockPreviewPayloadTabs {
            target_tabs,
            insert_index,
            insertion: None,
            tabs: vec![crate::drop_preview::DockPreviewPayloadTab {
                title: "Panel".to_string(),
            }],
        });
        DockDropPreview {
            scene: crate::drop_preview::DockPreviewScene {
                decision,
                layers: Vec::new(),
                body: crate::drop_preview::DockPreviewBody {
                    future_bounds: bounds,
                    body_bounds: bounds,
                },
                payload_tabs,
            },
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
    fn payload_tab_render_inputs_come_from_overlay_layers() {
        let overlay = DockOverlayScene {
            layers: vec![
                DockOverlayLayer {
                    kind: crate::overlay_scene::DockOverlayLayerKind::TabInsertion,
                    bounds: Bounds::new(point(px(0.0), px(0.0)), size(px(3.0), px(26.0))),
                    target_node: None,
                    zone: Some(DropZone::Center),
                    preview_layer: None,
                    active: true,
                    payload_index: None,
                    payload_title: None,
                    drop_box: None,
                    tab_insertion: None,
                },
                DockOverlayLayer {
                    kind: crate::overlay_scene::DockOverlayLayerKind::PayloadTab,
                    bounds: Bounds::new(point(px(0.0), px(0.0)), size(px(80.0), px(26.0))),
                    target_node: None,
                    zone: Some(DropZone::Center),
                    preview_layer: None,
                    active: true,
                    payload_index: Some(1),
                    payload_title: Some("Diff".to_string()),
                    drop_box: None,
                    tab_insertion: None,
                },
                DockOverlayLayer {
                    kind: crate::overlay_scene::DockOverlayLayerKind::PayloadTab,
                    bounds: Bounds::new(point(px(0.0), px(0.0)), size(px(80.0), px(26.0))),
                    target_node: None,
                    zone: Some(DropZone::Center),
                    preview_layer: None,
                    active: true,
                    payload_index: Some(0),
                    payload_title: Some("Preview".to_string()),
                    drop_box: None,
                    tab_insertion: None,
                },
            ],
        };

        assert!(overlay.has_payload_tab_preview());
        assert_eq!(
            overlay_payload_tabs(&overlay),
            vec![
                DockDropPreviewPayloadTab {
                    index: 0,
                    title: "Preview".to_string(),
                },
                DockDropPreviewPayloadTab {
                    index: 1,
                    title: "Diff".to_string(),
                },
            ]
        );
    }

    #[test]
    fn active_center_guides_have_stronger_palette_than_inactive_edge_guides() {
        let theme = dock_preview_theme();
        let active_center = theme.drop_guide(geometry::DockDropBoxKind::Center, true);
        let inactive_edge =
            theme.drop_guide(geometry::DockDropBoxKind::InnerEdge(DropZone::Left), false);

        assert_ne!(active_center.border, inactive_edge.border);
        assert_ne!(active_center.background, inactive_edge.background);
        assert!(active_center.background.a > inactive_edge.background.a);
    }

    #[test]
    fn rejected_drop_preview_uses_rejected_palette() {
        let theme = dock_preview_theme();
        let accepted = theme.target_preview(&preview(false, false).scene.decision);
        let rejected = theme.target_preview(&preview(true, false).scene.decision);

        assert_ne!(accepted, rejected);
        assert_eq!(rejected.border, rgb(0xdc2626));
    }

    #[test]
    fn payload_tab_preview_uses_stronger_selected_tab_palette() {
        let theme = dock_preview_theme();
        let palette = theme.target_preview(&preview(false, true).scene.decision);

        assert!(palette.tab_background.a > palette.body_background.a);
        assert_eq!(palette.tab_text, rgb(0xffffff));
    }

    #[test]
    fn route_preview_kinds_keep_distinct_palettes() {
        let theme = dock_preview_theme();
        let known = theme.route_preview(&route_preview(
            DockDropRoutePreviewKind::KnownViewport,
            false,
        ));
        let tear_off =
            theme.route_preview(&route_preview(DockDropRoutePreviewKind::TearOff, false));
        let rejected =
            theme.route_preview(&route_preview(DockDropRoutePreviewKind::Rejected, true));

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
