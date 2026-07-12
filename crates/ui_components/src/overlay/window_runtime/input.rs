//! Window input interception and topmost overlay arbitration.

use super::*;

impl WindowOverlayRuntime {
    pub(super) fn install_interceptors(&self, window: &mut Window, cx: &mut App) {
        if self.state.read(cx).interceptors_installed() {
            return;
        }
        let weak_key_state = self.state.downgrade();
        let key_subscription = window.intercept_key_down(move |event, window, cx| {
            let Some(state) = weak_key_state.upgrade() else {
                return;
            };
            let ambient_parent_layers = state.read(cx).ambient_parent_layers.clone();
            let runtime = WindowOverlayRuntime {
                window_id: window.window_handle().window_id(),
                state,
                ambient_parent_layers,
            };
            runtime.handle_key_down(event, window, cx);
        });
        let weak_mouse_state = self.state.downgrade();
        let mouse_subscription = window.intercept_mouse_events(move |event, window, cx| {
            let Some(state) = weak_mouse_state.upgrade() else {
                return;
            };
            let ambient_parent_layers = state.read(cx).ambient_parent_layers.clone();
            let runtime = WindowOverlayRuntime {
                window_id: window.window_handle().window_id(),
                state,
                ambient_parent_layers,
            };
            runtime.handle_mouse_event(event, window, cx);
        });
        let activation_subscription = self.state.update(cx, |_, cx| {
            cx.observe_window_activation(window, |state, window, _| {
                if !window.is_window_active() {
                    state.mouse_routes.clear();
                }
            })
        });
        self.state.update(cx, |state, _| {
            if state.key_subscription.is_none() {
                state.key_subscription = Some(key_subscription);
                state.mouse_subscription = Some(mouse_subscription);
                state.activation_subscription = Some(activation_subscription);
            }
        });
    }

    pub(super) fn handle_key_down(&self, event: &KeyDownEvent, window: &mut Window, cx: &mut App) {
        let modifiers = event.keystroke.modifiers;
        if event.keystroke.key.as_str() == "tab"
            && !modifiers.control
            && !modifiers.alt
            && !modifiers.platform
            && !modifiers.function
        {
            let dismiss_target = self.state.read(cx).tab_dismiss_target();
            if let Some(layer_id) = dismiss_target {
                if self
                    .request_open_change_by_id(
                        layer_id,
                        None,
                        false,
                        DismissReason::Programmatic,
                        window,
                        cx,
                    )
                    .is_ok()
                {
                    cx.stop_propagation();
                    window.prevent_default();
                    return;
                }
            }
            let focus_runtime = self.state.read(cx).focus_runtime.clone();
            let _ = focus_runtime.handle_key_down(event, window, cx);
            return;
        }

        if event.keystroke.key.as_str() != "escape" {
            return;
        }
        match self.state.read(cx).resolve_escape() {
            EscapeKeyResolution::Dismiss { layer_id, reason } => {
                let _ = self.request_open_change_by_id(layer_id, None, false, reason, window, cx);
                cx.stop_propagation();
                window.prevent_default();
            }
            EscapeKeyResolution::IgnoredByTopLayer { .. } => {
                cx.stop_propagation();
                window.prevent_default();
            }
            EscapeKeyResolution::NoInteractiveLayer => {}
        }
    }

    pub(super) fn handle_mouse_down(
        &self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(capture) = window.captured_pointer() {
            if let Some((route, outcome)) = self
                .state
                .update(cx, |state, _| state.inherit_captured_mouse_route(capture))
            {
                self.state.update(cx, |state, _| {
                    state.mouse_routes.insert(event.button, route);
                });
                if outcome.apply(window, cx) {
                    return;
                }
            } else if self.state.read(cx).has_modal_pointer_barrier() {
                MouseRouteOutcome::Block.apply(window, cx);
                return;
            }
        }

        let frame_revision = window.rendered_frame_revision();
        let decision =
            self.state
                .read(cx)
                .resolve_outside(event.position, event.button, frame_revision);
        let mut consume = decision.consumes();
        match decision {
            MouseDecision::None => {}
            MouseDecision::Consume => {}
            MouseDecision::Dismiss {
                layer_id,
                reason,
                consume: _,
            } => {
                let _ = self.request_open_change_by_id(layer_id, None, false, reason, window, cx);
            }
        }
        self.state.update(cx, |state, _| {
            let hit_test =
                state.pointer_hit_test(event.position, Some(event.button), frame_revision);
            consume |= state.modal_barrier_consumes_hit_test(&hit_test);
            let route = state.new_mouse_gesture_route(consume, &hit_test);
            state.mouse_routes.insert(event.button, route);
        });
        if consume {
            cx.stop_propagation();
            window.prevent_default();
        }
    }

    pub(super) fn handle_mouse_event(
        &self,
        event: WindowMouseEvent<'_>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let position = match event {
            WindowMouseEvent::Down(event) => {
                self.handle_mouse_down(event, window, cx);
                return;
            }
            WindowMouseEvent::Up(event) => {
                let capture = window.captured_pointer();
                let mut resolved_capture_route = false;
                if let Some(outcome) = self.state.update(cx, |state, _| {
                    state.resolve_mouse_gesture_route(event.button, capture, true)
                }) {
                    resolved_capture_route = true;
                    if outcome.apply(window, cx) {
                        return;
                    }
                }
                if let Some(capture) = capture
                    && capture.button() != event.button
                    && let Some(outcome) = self.state.update(cx, |state, _| {
                        state.resolve_mouse_gesture_route(capture.button(), Some(capture), false)
                    })
                {
                    resolved_capture_route = true;
                    if outcome.apply(window, cx) {
                        return;
                    }
                }
                if capture.is_some()
                    && !resolved_capture_route
                    && self.state.read(cx).has_modal_pointer_barrier()
                {
                    MouseRouteOutcome::Block.apply(window, cx);
                    return;
                }
                event.position
            }
            WindowMouseEvent::Move(event) => {
                let capture = window.captured_pointer();
                let route_button = capture
                    .map(|capture| capture.button())
                    .or(event.pressed_button);
                if self.apply_pointer_capture_route(capture, route_button, window, cx) {
                    return;
                }
                event.position
            }
            WindowMouseEvent::Exit(event) => {
                let capture = window.captured_pointer();
                let route_button = capture
                    .map(|capture| capture.button())
                    .or(event.pressed_button);
                if self.apply_pointer_capture_route(capture, route_button, window, cx) {
                    return;
                }
                event.position
            }
            WindowMouseEvent::Cancel(_) => {
                self.state.update(cx, |state, _| {
                    for route in state.mouse_routes.values_mut() {
                        *route = MouseGestureRoute::Blocked;
                    }
                });
                return;
            }
            WindowMouseEvent::Pressure(event) => {
                let capture = window.captured_pointer();
                if self.apply_pointer_capture_route(
                    capture,
                    capture.map(|capture| capture.button()),
                    window,
                    cx,
                ) {
                    return;
                }
                event.position
            }
            WindowMouseEvent::Scroll(event) => event.position,
            WindowMouseEvent::Pinch(event) => event.position,
            WindowMouseEvent::FileDrop(_) => window.mouse_position(),
            _ => window.mouse_position(),
        };
        let frame_revision = window.rendered_frame_revision();
        if self
            .state
            .read(cx)
            .modal_barrier_consumes(position, None, frame_revision)
        {
            cx.stop_propagation();
            window.prevent_default();
        }
    }

    fn apply_pointer_capture_route(
        &self,
        capture: Option<PointerCapture>,
        route_button: Option<MouseButton>,
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        if let Some(button) = route_button
            && let Some(outcome) = self.state.update(cx, |state, _| {
                state.resolve_mouse_gesture_route(button, capture, false)
            })
            && outcome.apply(window, cx)
        {
            return true;
        }
        if capture.is_some() && self.state.read(cx).has_modal_pointer_barrier() {
            MouseRouteOutcome::Block.apply(window, cx);
            return true;
        }
        false
    }
}

impl WindowOverlayRuntimeState {
    pub(super) fn interceptors_installed(&self) -> bool {
        self.key_subscription.is_some()
            && self.mouse_subscription.is_some()
            && self.activation_subscription.is_some()
    }

    pub(super) fn resolve_escape(&self) -> EscapeKeyResolution {
        resolve_escape_key(&self.policy_stack_from(0))
    }

    pub(super) fn new_mouse_gesture_route(
        &self,
        blocked: bool,
        hit_test: &PointerHitTest,
    ) -> MouseGestureRoute {
        if blocked {
            return MouseGestureRoute::Blocked;
        }
        MouseGestureRoute::Allowed {
            authority: self.mouse_authority_stamp(),
            owner: self.mouse_gesture_owner(hit_test),
            capture: None,
        }
    }

    pub(super) fn inherit_captured_mouse_route(
        &mut self,
        capture: PointerCapture,
    ) -> Option<(MouseGestureRoute, MouseRouteOutcome)> {
        let capture_button = capture.button();
        let mut route = self.mouse_routes.remove(&capture_button)?;
        let outcome = route.resolve(self, Some(capture));
        let inherited = route.clone();
        self.mouse_routes.insert(capture_button, route);
        Some((inherited, outcome))
    }

    pub(super) fn resolve_mouse_gesture_route(
        &mut self,
        button: MouseButton,
        capture: Option<PointerCapture>,
        remove: bool,
    ) -> Option<MouseRouteOutcome> {
        let mut route = self.mouse_routes.remove(&button)?;
        let outcome = route.resolve(self, capture);
        if !remove {
            self.mouse_routes.insert(button, route);
        }
        Some(outcome)
    }

    pub(super) fn mouse_authority_stamp(&self) -> MouseAuthorityStamp {
        MouseAuthorityStamp(self.mouse_authority_revision)
    }

    pub(super) fn bump_mouse_authority(&mut self) {
        self.mouse_authority_revision = self.mouse_authority_revision.wrapping_add(1);
    }

    pub(super) fn mouse_gesture_owner(
        &self,
        hit_test: &PointerHitTest,
    ) -> Option<MouseGestureOwner> {
        self.stack.iter().rev().find_map(|id| {
            let entry = self.entries.get(id)?;
            (entry.keyboard_eligible() && hit_test.is_inside(id)).then(|| MouseGestureOwner {
                id: id.clone(),
                lease_token: entry.lease_token,
                generation: entry.generation,
            })
        })
    }

    pub(super) fn mouse_gesture_owner_is_current(&self, owner: &MouseGestureOwner) -> bool {
        self.entries.get(&owner.id).is_some_and(|entry| {
            entry.lease_token == owner.lease_token
                && entry.generation == owner.generation
                && entry.keyboard_eligible()
                && !entry.pending_unregister
        })
    }

    pub(super) fn resolve_outside(
        &self,
        point: Point<Pixels>,
        button: MouseButton,
        frame_revision: u64,
    ) -> MouseDecision {
        let hit_test = self.pointer_hit_test(point, Some(button), frame_revision);
        let modal_barrier = hit_test.modal_barrier;
        let arbitration_floor = modal_barrier.unwrap_or(0);

        for (offset, layer_id) in self.stack[arbitration_floor..].iter().enumerate().rev() {
            let stack_index = arbitration_floor + offset;
            let Some(entry) = self.entries.get(layer_id) else {
                continue;
            };
            if !entry.keyboard_eligible() {
                continue;
            }
            if !entry.policy.outside_press_participation().participates() {
                continue;
            }
            if hit_test.is_inside(layer_id) {
                return MouseDecision::None;
            }

            let layer = OverlayLayer::new(layer_id.as_str(), entry.projected_policy());
            let OutsidePressResolution::Handled { layer_id, outcome } =
                resolve_outside_press(std::slice::from_ref(&layer))
            else {
                continue;
            };
            let barrier_consumes = modal_barrier.is_some_and(|barrier| {
                !self.stack[barrier..stack_index].iter().any(|id| {
                    self.entries.get(id).is_some_and(|entry| {
                        entry.keyboard_eligible()
                            && entry.policy.outside_press_participation().participates()
                            && hit_test.is_inside(id)
                    })
                })
            });
            let ancestor_consumes = !outcome.consumes_event()
                && self.outside_press_ancestor_consumes(&layer_id, &hit_test);
            if let Some(reason) = outcome.dismiss_reason() {
                return MouseDecision::Dismiss {
                    layer_id,
                    reason,
                    consume: barrier_consumes || ancestor_consumes || outcome.consumes_event(),
                };
            }
            return if barrier_consumes || outcome.consumes_event() {
                MouseDecision::Consume
            } else {
                MouseDecision::None
            };
        }

        if modal_barrier.is_some() {
            MouseDecision::Consume
        } else {
            MouseDecision::None
        }
    }

    fn outside_press_ancestor_consumes(
        &self,
        layer_id: &OverlayLayerId,
        hit_test: &PointerHitTest,
    ) -> bool {
        let mut parent_id = self
            .entries
            .get(layer_id)
            .and_then(|entry| entry.parent.as_ref());
        while let Some(id) = parent_id {
            let Some(parent) = self.entries.get(id) else {
                return false;
            };
            if hit_test.is_inside(id) {
                return false;
            }
            if parent.keyboard_eligible()
                && parent.policy.outside_press_participation().participates()
            {
                match parent.policy.outside_press_policy() {
                    OutsidePressPolicy::Ignore => return false,
                    OutsidePressPolicy::Consume | OutsidePressPolicy::DismissAndConsume => {
                        return true;
                    }
                    OutsidePressPolicy::DismissAndPassThrough => {}
                }
            }
            parent_id = parent.parent.as_ref();
        }
        false
    }

    pub(super) fn highest_modal_barrier_index(&self) -> Option<usize> {
        self.stack.iter().enumerate().rev().find_map(|(index, id)| {
            let entry = self.entries.get(id)?;
            (entry.policy.kind() == OverlayLayerKind::Modal && entry.lifecycle.presence().present())
                .then_some(index)
        })
    }

    pub(super) fn has_modal_pointer_barrier(&self) -> bool {
        self.highest_modal_barrier_index().is_some()
    }

    pub(super) fn modal_barrier_consumes(
        &self,
        point: Point<Pixels>,
        button: Option<MouseButton>,
        frame_revision: u64,
    ) -> bool {
        if self.highest_modal_barrier_index().is_none() {
            return false;
        }
        let hit_test = self.pointer_hit_test(point, button, frame_revision);
        self.modal_barrier_consumes_hit_test(&hit_test)
    }

    fn modal_barrier_consumes_hit_test(&self, hit_test: &PointerHitTest) -> bool {
        let Some(barrier) = hit_test.modal_barrier else {
            return false;
        };
        !self.stack[barrier..].iter().any(|id| {
            self.entries.get(id).is_some_and(|entry| {
                entry.keyboard_eligible()
                    && entry.policy.outside_press_participation().participates()
                    && hit_test.is_inside(id)
            })
        })
    }

    pub(super) fn policy_stack_from(&self, floor: usize) -> Vec<OverlayLayer> {
        self.stack
            .iter()
            .skip(floor)
            .filter_map(|id| {
                let entry = self.entries.get(id)?;
                Some(OverlayLayer::new(id.as_str(), entry.projected_policy()))
            })
            .collect()
    }

    fn pointer_hit_test(
        &self,
        point: Point<Pixels>,
        button: Option<MouseButton>,
        frame_revision: u64,
    ) -> PointerHitTest {
        let mut modal_barrier = None;
        let mut inside_layers = HashSet::new();
        for (index, id) in self.stack.iter().enumerate() {
            let Some(entry) = self.entries.get(id) else {
                continue;
            };
            if entry.policy.kind() == OverlayLayerKind::Modal
                && entry.lifecycle.presence().present()
            {
                modal_barrier = Some(index);
            }
            let directly_inside = entry.keyboard_eligible()
                && entry.inside_regions.values().any(|region| {
                    frame_revision <= region.valid_through
                        && region.button.is_none_or(|inside_button| {
                            button.is_some_and(|button| button == inside_button)
                        })
                        && region.bounds.contains(&point)
                });
            if !directly_inside {
                continue;
            }

            let mut current = Some(id);
            while let Some(layer_id) = current {
                if !inside_layers.insert(layer_id.clone()) {
                    break;
                }
                current = self
                    .entries
                    .get(layer_id)
                    .and_then(|entry| entry.parent.as_ref());
            }
        }
        PointerHitTest {
            modal_barrier,
            inside_layers,
        }
    }
}
