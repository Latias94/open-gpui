//! GPUI focus-handle runtime for stable-value roving-focus collections.

use std::collections::BTreeMap;

use open_gpui::{Context, FocusHandle};

/// Shared GPUI runtime for stable-value roving-focus collections.
#[derive(Debug, Default)]
pub(crate) struct StableValueFocusRuntime {
    focused_value: Option<String>,
    focus_handles: BTreeMap<String, FocusHandle>,
}

impl StableValueFocusRuntime {
    pub(crate) fn new(focused_value: Option<String>) -> Self {
        Self {
            focused_value,
            focus_handles: BTreeMap::new(),
        }
    }

    /// Resolves the next logical value, adopting a physically focused owned handle first.
    pub(crate) fn resolved_value<'a>(
        &'a self,
        physically_focused: Option<&FocusHandle>,
    ) -> Option<&'a str> {
        physically_focused
            .and_then(|focused| self.value_for_handle(focused))
            .or(self.focused_value.as_deref())
    }

    pub(crate) fn focus_handle(&self, value: &str) -> Option<FocusHandle> {
        self.focus_handles.get(value).cloned()
    }

    pub(crate) fn sync<'a>(
        &mut self,
        focusable_values: impl IntoIterator<Item = &'a str>,
        resolved_focused_value: Option<&str>,
        physically_focused: Option<&FocusHandle>,
        cx: &mut Context<Self>,
    ) -> Option<FocusHandle> {
        let focusable_values = focusable_values.into_iter().collect::<Vec<_>>();
        let focused_owned_item_became_unavailable = physically_focused
            .and_then(|focused| self.value_for_handle(focused))
            .is_some_and(|value| !focusable_values.contains(&value));

        self.focus_handles
            .retain(|value, _| focusable_values.contains(&value.as_str()));
        for value in focusable_values {
            self.focus_handles
                .entry(value.to_owned())
                .or_insert_with(|| cx.focus_handle());
        }

        self.focused_value = resolved_focused_value.map(str::to_owned);

        focused_owned_item_became_unavailable
            .then_some(resolved_focused_value)
            .flatten()
            .and_then(|value| self.focus_handle(value))
    }

    pub(crate) fn set_focused(
        &mut self,
        value: &str,
        cx: &mut Context<Self>,
    ) -> Option<FocusHandle> {
        if self.focused_value.as_deref() != Some(value) {
            self.focused_value = Some(value.to_owned());
            cx.notify();
        }
        self.focus_handle(value)
    }

    fn value_for_handle(&self, focused: &FocusHandle) -> Option<&str> {
        self.focus_handles
            .iter()
            .find_map(|(value, handle)| (handle == focused).then_some(value.as_str()))
    }
}
