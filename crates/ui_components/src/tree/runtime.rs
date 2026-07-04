use std::{collections::BTreeMap, time::Instant};

use open_gpui::{Context, FocusHandle};

use crate::scroll_surface::ScrollSurfaceRuntime;

use super::{TREE_TYPEAHEAD_RESET, TreeState};
#[derive(Debug, Clone, Default)]
pub(super) struct TreeRuntime {
    pub(super) scroll_surface: ScrollSurfaceRuntime,
    pub(super) selected_value: Option<String>,
    pub(super) focused_value: Option<String>,
    pub(super) expanded_values: BTreeMap<String, bool>,
    pub(super) focus_handles: BTreeMap<String, FocusHandle>,
    pub(super) typeahead_buffer: String,
    pub(super) last_typeahead_at: Option<Instant>,
}

impl TreeRuntime {
    pub(super) fn sync(&mut self, state: &TreeState, cx: &mut Context<Self>) {
        self.focus_handles
            .retain(|value, _| state.items().iter().any(|item| item.value() == value));

        for item in state.items().iter().filter(|item| item.focusable()) {
            self.focus_handles
                .entry(item.value().to_owned())
                .or_insert_with(|| cx.focus_handle());
        }

        self.selected_value = state.selected_value().map(str::to_owned);
        self.focused_value = state.focused_value().map(str::to_owned);
    }

    pub(super) fn set_focused(
        &mut self,
        value: &str,
        cx: &mut Context<Self>,
    ) -> Option<FocusHandle> {
        let value = value.to_owned();
        let changed = self.focused_value.as_deref() != Some(value.as_str());
        self.focused_value = Some(value.clone());
        if changed {
            cx.notify();
        }
        self.focus_handles.get(&value).cloned()
    }

    pub(super) fn set_selected(&mut self, value: &str, cx: &mut Context<Self>) {
        let changed = self.selected_value.as_deref() != Some(value);
        self.selected_value = Some(value.to_owned());
        if changed {
            cx.notify();
        }
    }

    pub(super) fn set_expanded(&mut self, value: &str, expanded: bool, cx: &mut Context<Self>) {
        let changed = self.expanded_values.get(value).copied() != Some(expanded);
        self.expanded_values.insert(value.to_owned(), expanded);
        if changed {
            cx.notify();
        }
    }

    pub(super) fn push_typeahead_key(&mut self, key: &str) -> String {
        let now = Instant::now();
        if self
            .last_typeahead_at
            .map_or(true, |last| now.duration_since(last) > TREE_TYPEAHEAD_RESET)
        {
            self.typeahead_buffer.clear();
        }

        self.typeahead_buffer.push_str(&key.to_lowercase());
        self.last_typeahead_at = Some(now);
        self.typeahead_buffer.clone()
    }
}
