use std::time::Duration;

use crate::{App, DisplayId, Subscription, Window};

impl Window {
    pub(crate) fn update_display_association(&mut self) -> Option<Option<DisplayId>> {
        self.refresh_platform_facts();
        let display_id = self.platform_window.display().map(|display| display.id());
        let moved = self.display_id != display_id;
        self.display_id = display_id;
        moved.then_some(display_id)
    }

    pub(crate) fn notify_display_observers(&mut self, cx: &mut App) {
        self.display_observers
            .clone()
            .retain(&(), |callback| callback(self, cx));
    }

    /// Returns the connected display's cached refresh interval, or `None` when unavailable.
    /// Variable refresh rate displays report their maximum rate.
    pub fn refresh_interval(&self, cx: &App) -> Option<Duration> {
        cx.display_refresh_interval(self.display_id?)
    }

    /// Registers a callback when this window moves to another display or its display properties
    /// change. [`Self::refresh_interval`] returns the updated value before the callback runs.
    pub fn observe_window_display(
        &self,
        mut callback: impl FnMut(&mut Window, &mut App) + 'static,
    ) -> Subscription {
        let (subscription, activate) = self.display_observers.insert(
            (),
            Box::new(move |window, cx| {
                callback(window, cx);
                true
            }),
        );
        activate();
        subscription
    }
}
