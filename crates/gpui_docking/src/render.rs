use crate::{
    DockHost, DockNode, DockNodeId,
    debug::DockDebugRegion,
    drag::DockDragPayload,
    drop_preview::{DockDropPreview, DockDropPreviewKind},
    drop_runtime::DockHostDropSceneFact,
    drop_scene_fact,
    host_render_session::DockHostRenderSession,
    interaction::{
        DockPayloadDropRelease, DockRenderedOutsideReleaseDecision,
        DockRenderedOutsideReleaseRequest,
    },
    render_split::DockRenderSplitInput,
    viewport_drop_scene::DockViewportHostSceneFrame,
};
use open_gpui::{
    AnyElement, Bounds, Context, DragMoveEvent, InteractiveElement, IntoElement, MouseButton,
    MouseUpEvent, ParentElement, Pixels, Render, Rgba, Styled, Window, black, canvas, div, point,
    px, rgb, rgba,
};
use std::{cell::RefCell, rc::Rc};

pub(crate) type DockViewportHostSceneFrameSlot = Rc<RefCell<Option<DockViewportHostSceneFrame>>>;

impl Render for DockHost {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.clear_debug_selectors();
        self.ensure_viewport_activation_subscription(window, cx);
        let session = self.render_session(cx);
        self.focus_pending_panel_from_render(&session, window, cx);
        let drop_host_space = session.space().clone();
        let outside_release_host_space = session.space().clone();
        let viewport_host_scene_frame =
            self.viewport_runtime().map(|_| Rc::new(RefCell::new(None)));

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
                    this.drop_payload_release_from_render(
                        DockPayloadDropRelease::hovered_host_with_session(
                            payload.clone(),
                            drop_host_space.clone(),
                            window.mouse_position(),
                            drag_session,
                        ),
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
                    let request = DockRenderedOutsideReleaseRequest::new(
                        this.viewport_runtime().is_some(),
                        payload,
                        cx.mouse_button_is_pressed(MouseButton::Left),
                        outside_release_host_space.clone(),
                        event.position,
                    )
                    .with_drag_session(drag_session);
                    match this.interaction_mut().rendered_outside_release(request) {
                        DockRenderedOutsideReleaseDecision::Inactive => {}
                        DockRenderedOutsideReleaseDecision::StopDragSession(drag_session) => {
                            this.finish_payload_drag_session(&drag_session);
                            this.clear_drop_preview_interaction();
                            if let Some(runtime) = this.viewport_runtime().cloned() {
                                runtime.clear_routed_drop_preview(cx);
                            }
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

        if let Some(frame_slot) = viewport_host_scene_frame.as_ref()
            && let Some(probe) = self.render_viewport_host_scene_probe(frame_slot)
        {
            host = host.child(probe);
        }

        if let Some(root) = session.root() {
            host = host.child(self.render_root_node(
                root,
                &session,
                viewport_host_scene_frame.as_ref(),
                cx,
            ));
        } else if session.empty_central_passthrough() {
            host = host.child(self.render_passthrough_empty_central_space(
                &session,
                viewport_host_scene_frame.as_ref(),
                cx,
            ));
        } else {
            host = host.child(self.render_empty_space(
                &session,
                viewport_host_scene_frame.as_ref(),
                cx,
            ));
        }

        for floating in session.floating_containers() {
            host = host.child(self.render_floating_container(
                *floating,
                &session,
                viewport_host_scene_frame.as_ref(),
                cx,
            ));
        }

        if let Some(preview) = self.render_host_drop_preview(&session, window, cx) {
            host = host.child(preview);
        }

        host
    }
}

impl DockHost {
    fn focus_pending_panel_from_render(
        &mut self,
        session: &DockHostRenderSession,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(item) = self.take_pending_panel_focus() else {
            return;
        };
        session.request_panel_focus(&item, window, cx);
    }

    pub(crate) fn render_node(
        &mut self,
        node_id: DockNodeId,
        session: &DockHostRenderSession,
        viewport_host_scene_frame: Option<&DockViewportHostSceneFrameSlot>,
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
                cx,
            ),
            DockNode::Tabs { items, active } => self.render_tabs(
                node_id,
                items,
                active,
                session,
                viewport_host_scene_frame,
                cx,
            ),
            DockNode::Floating { child } => {
                self.render_floating_node(node_id, child, session, viewport_host_scene_frame, cx)
            }
        }
    }

    fn render_root_node(
        &mut self,
        root: DockNodeId,
        session: &DockHostRenderSession,
        viewport_host_scene_frame: Option<&DockViewportHostSceneFrameSlot>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let root_child = self.render_node(root, session, viewport_host_scene_frame, cx);
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
        if let Some(probe) = self
            .render_viewport_drop_scene_fact_probe(viewport_host_scene_frame, move |bounds| {
                drop_scene_fact::root(root, bounds)
            })
        {
            root_container = root_container.child(probe);
        }
        root_container.child(root_child).into_any_element()
    }

    fn render_empty_space(
        &mut self,
        session: &DockHostRenderSession,
        viewport_host_scene_frame: Option<&DockViewportHostSceneFrameSlot>,
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
                        window,
                        cx,
                    );
                },
            ));
        if let Some(probe) = self
            .render_viewport_drop_scene_fact_probe(viewport_host_scene_frame, move |bounds| {
                drop_scene_fact::empty_space(space, bounds)
            })
        {
            empty = empty.child(probe);
        }
        empty
            .child(session.empty_message().to_string())
            .into_any_element()
    }

    fn render_passthrough_empty_central_space(
        &mut self,
        session: &DockHostRenderSession,
        viewport_host_scene_frame: Option<&DockViewportHostSceneFrameSlot>,
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
                        window,
                        cx,
                    );
                },
            ));
        if let Some(probe) = self
            .render_viewport_drop_scene_fact_probe(viewport_host_scene_frame, move |bounds| {
                drop_scene_fact::empty_space(space, bounds)
            })
        {
            empty = empty.child(probe);
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
        let routed_preview = self.viewport_runtime().and_then(|runtime| {
            runtime.routed_drop_preview_for(self.space(), window.window_handle().window_id())
        });
        let (preview, payload_title) = self
            .interaction()
            .drop_preview()
            .map(|preview| (preview, active_payload_title))
            .or_else(|| {
                routed_preview.map(|preview| (preview.preview, Some(preview.payload_title)))
            })?;
        let bounds = preview.bounds;
        let region = if preview.is_route() {
            DockDebugRegion::DropRoutePreview { kind: preview.kind }
        } else {
            DockDebugRegion::DropPreview
        };
        let selector = self.record_debug_selector(
            region,
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

        Some(element.into_any_element())
    }

    /// Publishes viewport bounds during prepaint so cross-window releases can resolve even when
    /// the target window did not receive the drag-move event.
    pub(crate) fn render_viewport_host_scene_probe(
        &self,
        frame_slot: &DockViewportHostSceneFrameSlot,
    ) -> Option<AnyElement> {
        let runtime = self.viewport_runtime()?.clone();
        let space = self.space().clone();
        let frame_slot = frame_slot.clone();
        Some(
            canvas(
                move |bounds, window, _| {
                    let mouse_position = window.mouse_position();
                    let host_position = point(
                        mouse_position.x - bounds.origin.x,
                        mouse_position.y - bounds.origin.y,
                    );
                    let registration = runtime.begin_viewport_host_scene_frame(
                        space,
                        window.window_handle().window_id(),
                        window.window_bounds(),
                        bounds,
                        host_position,
                    );
                    *frame_slot.borrow_mut() = registration.map(|registration| registration.frame);
                },
                |_, _, _, _| (),
            )
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .size_full()
            .into_any_element(),
        )
    }

    /// Publishes target bounds during prepaint for runtime-routed drops.
    pub(crate) fn render_viewport_drop_scene_fact_probe(
        &self,
        frame_slot: Option<&DockViewportHostSceneFrameSlot>,
        fact_for_bounds: impl FnOnce(Bounds<Pixels>) -> DockHostDropSceneFact + 'static,
    ) -> Option<AnyElement> {
        let runtime = self.viewport_runtime()?.clone();
        let frame_slot = frame_slot?.clone();
        Some(
            canvas(
                move |bounds, _window, _| {
                    let Some(frame) = frame_slot.borrow().as_ref().cloned() else {
                        return;
                    };
                    runtime.push_viewport_host_scene_frame_fact(&frame, fact_for_bounds(bounds));
                },
                |_, _, _, _| (),
            )
            .absolute()
            .top(px(0.0))
            .left(px(0.0))
            .size_full()
            .into_any_element(),
        )
    }
}

fn drop_preview_colors(preview: &DockDropPreview) -> (Rgba, Rgba) {
    if preview.rejected {
        return (rgb(0xdc2626), rgba(0xfca5a547));
    }

    match preview.kind {
        DockDropPreviewKind::Local => (rgb(0x2563eb), rgba(0x60a5fa47)),
        DockDropPreviewKind::KnownViewportRoute => (rgb(0x059669), rgba(0x6ee7b747)),
        DockDropPreviewKind::TearOffRoute => (rgb(0x7c3aed), rgba(0xc4b5fd47)),
        DockDropPreviewKind::RejectedRoute => (rgb(0xdc2626), rgba(0xfca5a547)),
    }
}
