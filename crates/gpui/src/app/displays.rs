use std::time::Duration;

use open_gpui_collections::HashMap;

use crate::{
    App, AppContext as _, Bounds, DisplayChanges, DisplayEvent, DisplayId, Pixels, Platform,
    Subscription,
};

pub(super) type DisplayHandler = Box<dyn FnMut(DisplayEvent, &mut App) -> bool + 'static>;

#[derive(Clone, Copy, PartialEq)]
pub(super) struct DisplayState {
    bounds: Bounds<Pixels>,
    refresh_interval: Option<Duration>,
}

impl DisplayState {
    fn changes_from(&self, previous: &Self) -> DisplayChanges {
        let mut changes = DisplayChanges::empty();
        changes.set(DisplayChanges::BOUNDS, self.bounds != previous.bounds);
        changes.set(
            DisplayChanges::REFRESH_INTERVAL,
            self.refresh_interval != previous.refresh_interval,
        );
        changes
    }
}

pub(super) fn read_displays(platform: &dyn Platform) -> HashMap<DisplayId, DisplayState> {
    platform
        .display_snapshot()
        .displays()
        .into_iter()
        .map(|display| {
            (
                display.id(),
                DisplayState {
                    bounds: display.bounds(),
                    refresh_interval: display.refresh_interval(),
                },
            )
        })
        .collect()
}

fn display_events(
    previous: &HashMap<DisplayId, DisplayState>,
    current: &HashMap<DisplayId, DisplayState>,
) -> Vec<DisplayEvent> {
    let removed = previous
        .keys()
        .filter(|id| !current.contains_key(id))
        .map(|id| DisplayEvent::Removed(*id));
    let added_or_changed = current.iter().filter_map(|(id, state)| {
        let Some(previous_state) = previous.get(id) else {
            return Some(DisplayEvent::Added(*id));
        };
        let changes = state.changes_from(previous_state);
        (!changes.is_empty()).then_some(DisplayEvent::Changed { id: *id, changes })
    });
    removed.chain(added_or_changed).collect()
}

impl App {
    pub(crate) fn displays_changed(&mut self) {
        let current = read_displays(self.platform.as_ref());
        let events = display_events(&self.displays, &current);
        self.displays = current;
        for event in &events {
            let DisplayEvent::Changed { id, .. } = *event else {
                continue;
            };
            for handle in self.windows() {
                let _ = self.update_window(handle, |_, window, cx| {
                    if window.display_id == Some(id) {
                        window.notify_display_observers(cx);
                    }
                });
            }
        }
        for event in events {
            self.display_observers
                .clone()
                .retain(&(), |callback| callback(event, self));
        }
    }

    pub(crate) fn knows_display(&self, id: DisplayId) -> bool {
        self.displays.contains_key(&id)
    }

    pub(crate) fn display_refresh_interval(&self, id: DisplayId) -> Option<Duration> {
        self.displays.get(&id)?.refresh_interval
    }

    /// Registers a callback when a display connects, disconnects or changes its properties.
    /// App's display state is ready before any window or App display observer runs.
    pub fn observe_displays(
        &self,
        mut callback: impl FnMut(DisplayEvent, &mut App) + 'static,
    ) -> Subscription {
        let (subscription, activate) = self.display_observers.insert(
            (),
            Box::new(move |event, cx| {
                callback(event, cx);
                true
            }),
        );
        activate();
        subscription
    }
}
