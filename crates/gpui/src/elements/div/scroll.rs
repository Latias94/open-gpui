use super::*;

impl Interactivity {
    /// Bind the given callback to scroll wheel events during the bubble phase.
    /// The imperative API equivalent to [`InteractiveElement::on_scroll_wheel`].
    ///
    /// See [`Context::listener`](crate::Context::listener) to get access to a view's state from this callback.
    pub fn on_scroll_wheel(
        &mut self,
        listener: impl Fn(&TargetedEvent<ScrollWheelEvent>, &mut Window, &mut App) -> ScrollWheelIntent
        + 'static,
    ) {
        self.scroll_wheel_listeners.push(Box::new(
            move |event, phase, hitbox, focus_handle, window, cx| {
                if phase == DispatchPhase::Bubble && hitbox.should_handle_scroll(window) {
                    (listener)(&TargetedEvent::new(event, hitbox), window, cx).apply(
                        focus_handle,
                        window,
                        cx,
                    );
                }
            },
        ));
    }

    /// Bind a raw callback to scroll wheel events during the bubble phase.
    ///
    /// Prefer [`Self::on_scroll_wheel`] for product code. Raw callbacks are an
    /// advanced escape hatch for integrations that must manipulate dispatch
    /// state directly.
    pub fn on_raw_scroll_wheel(
        &mut self,
        listener: impl Fn(&ScrollWheelEvent, &mut Window, &mut App) + 'static,
    ) {
        self.scroll_wheel_listeners
            .push(Box::new(move |event, phase, hitbox, _, window, cx| {
                if phase == DispatchPhase::Bubble && hitbox.should_handle_scroll(window) {
                    (listener)(event, window, cx);
                }
            }));
    }

    /// Bind the given callback to scroll wheel events during the capture phase.
    /// This runs before GPUI's default scroll handling for scrollable elements.
    ///
    /// See [`Context::listener`](crate::Context::listener) to get access to a view's state from this callback.
    pub fn capture_scroll_wheel(
        &mut self,
        listener: impl Fn(&TargetedEvent<ScrollWheelEvent>, &mut Window, &mut App) -> ScrollWheelIntent
        + 'static,
    ) {
        self.scroll_wheel_listeners.push(Box::new(
            move |event, phase, hitbox, focus_handle, window, cx| {
                if phase == DispatchPhase::Capture && hitbox.should_handle_scroll(window) {
                    (listener)(&TargetedEvent::new(event, hitbox), window, cx).apply(
                        focus_handle,
                        window,
                        cx,
                    );
                }
            },
        ));
    }

    /// Bind a raw callback to scroll wheel events during the capture phase.
    ///
    /// Prefer [`Self::capture_scroll_wheel`] for product code. Raw callbacks are
    /// an advanced escape hatch for integrations that must manipulate dispatch
    /// state directly.
    pub fn capture_raw_scroll_wheel(
        &mut self,
        listener: impl Fn(&ScrollWheelEvent, &mut Window, &mut App) + 'static,
    ) {
        self.scroll_wheel_listeners
            .push(Box::new(move |event, phase, hitbox, _, window, cx| {
                if phase == DispatchPhase::Capture && hitbox.should_handle_scroll(window) {
                    (listener)(event, window, cx);
                }
            }));
    }

    /// Bind the given callback to committed tracked-scroll viewport changes.
    /// The callback runs after GPUI has clamped the scroll offset against the
    /// element's final bounds and content size for the frame.
    pub fn on_scroll_viewport_changed(
        &mut self,
        listener: impl Fn(&ScrollViewportChangedEvent, &mut Window, &mut App) + 'static,
    ) {
        self.scroll_viewport_changed_listeners
            .push(Box::new(listener));
    }
}
