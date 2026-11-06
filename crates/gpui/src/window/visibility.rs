use crate::{App, Subscription, Window, WindowVisibility};

impl Window {
    pub(crate) fn update_visibility(&mut self, visibility: WindowVisibility, cx: &mut App) {
        if self.visibility == visibility {
            return;
        }
        self.visibility = visibility;
        self.visibility_observers
            .clone()
            .retain(&(), |callback| callback(visibility, self, cx));
    }

    /// Returns whether the platform is presenting this window's frames.
    ///
    /// See [`WindowVisibility`] for the signal and limits of each platform.
    pub fn visibility(&self) -> WindowVisibility {
        self.visibility
    }

    /// Returns whether the platform reports visible presentation for this window.
    ///
    /// A shown or mapped window may still be hidden by minimization or occlusion,
    /// depending on the platform's signal. See [`WindowVisibility`].
    pub fn is_visible(&self) -> bool {
        self.visibility.is_visible()
    }

    /// Registers a callback for distinct changes to this window's visibility.
    ///
    /// The cached visibility is updated before the callback runs. Registration does
    /// not emit the current value; use [`Self::visibility`] to read the initial state.
    /// Visibility notifications do not request a frame.
    pub fn observe_window_visibility(
        &self,
        mut callback: impl FnMut(WindowVisibility, &mut Window, &mut App) + 'static,
    ) -> Subscription {
        let (subscription, activate) = self.visibility_observers.insert(
            (),
            Box::new(move |visibility, window, cx| {
                callback(visibility, window, cx);
                true
            }),
        );
        activate();
        subscription
    }
}
