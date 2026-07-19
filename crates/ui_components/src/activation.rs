//! Semantic activation input normalization for official components.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::rc::Weak;

use open_gpui::{
    App, ClickEvent, ElementId, Entity, FocusHandle, KeyDownEvent, KeyUpEvent, MouseButton,
    StatefulInteractiveElement, Window, WindowId,
};
use open_gpui_ui_core::AccessibleAction;

use crate::a11y::UiA11yElementExt;

/// Keyboard key that produced a semantic activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActivationKey {
    /// Enter activated the control.
    Enter,
    /// Space activated the control.
    Space,
}

/// Normalized source of a semantic activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActivationSource {
    /// A primary pointer gesture activated the control.
    Pointer,
    /// An allowed keyboard key activated the control.
    Keyboard(ActivationKey),
    /// An accessibility action activated the control directly.
    Accessibility,
    /// Framework or application code requested activation without synthetic input.
    Programmatic,
}

/// Semantic activation delivered to an official control callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Activation {
    source: ActivationSource,
}

impl Activation {
    pub(crate) const fn new(source: ActivationSource) -> Self {
        Self { source }
    }

    /// Returns the normalized activation source.
    pub const fn source(self) -> ActivationSource {
        self.source
    }
}

/// Result of requesting semantic activation through an [`ActivationHandle`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActivationRequestResult {
    /// The live control accepted and dispatched the request.
    Dispatched,
    /// The control is live but its disabled or read-only gate rejected the request.
    Blocked,
    /// No live control is currently bound to the handle.
    Unavailable,
    /// The request used a window other than the control's owning window.
    WrongWindow,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActivationKeyPolicy {
    Enter,
    Space,
    EnterOrSpace,
}

impl ActivationKeyPolicy {
    fn resolve(self, key: &str) -> Option<ActivationKey> {
        match (self, key) {
            (Self::Enter | Self::EnterOrSpace, "enter") => Some(ActivationKey::Enter),
            (Self::Space | Self::EnterOrSpace, "space") => Some(ActivationKey::Space),
            _ => None,
        }
    }

    pub(crate) fn accepts(self, key: &str) -> bool {
        self.resolve(key).is_some()
    }
}

type ActivationTransaction = dyn Fn(Activation, &mut Window, &mut App);

struct ActivationDispatcher {
    enabled: bool,
    transaction: Box<ActivationTransaction>,
}

impl ActivationDispatcher {
    fn new(
        enabled: bool,
        transaction: impl Fn(Activation, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            enabled,
            transaction: Box::new(transaction),
        }
    }

    const fn enabled(&self) -> bool {
        self.enabled
    }

    fn dispatch(&self, source: ActivationSource, window: &mut Window, cx: &mut App) -> bool {
        if !self.enabled {
            return false;
        }

        (self.transaction)(Activation::new(source), window, cx);
        true
    }
}

type SharedActivationDispatcher = Rc<ActivationDispatcher>;
type WeakActivationDispatcher = Weak<ActivationDispatcher>;

struct ProgrammaticActivationBinding {
    window_id: WindowId,
    dispatcher: WeakActivationDispatcher,
}

/// Stable application-owned request seam for one rendered semantic control.
///
/// Reusing a handle across simultaneously rendered controls replaces the prior binding with the
/// most recently rendered target; create one handle per independently addressable control.
#[derive(Clone, Default)]
pub struct ActivationHandle {
    binding: Rc<RefCell<Option<ProgrammaticActivationBinding>>>,
}

impl ActivationHandle {
    /// Creates an unbound activation handle.
    pub fn new() -> Self {
        Self::default()
    }

    /// Requests activation from the currently bound control.
    pub fn request(&self, window: &mut Window, cx: &mut App) -> ActivationRequestResult {
        let binding = self
            .binding
            .borrow()
            .as_ref()
            .map(|binding| (binding.window_id, binding.dispatcher.clone()));
        let Some((window_id, dispatcher)) = binding else {
            return ActivationRequestResult::Unavailable;
        };
        let Some(dispatcher) = dispatcher.upgrade() else {
            self.binding.borrow_mut().take();
            return ActivationRequestResult::Unavailable;
        };
        if window.window_handle().window_id() != window_id {
            return ActivationRequestResult::WrongWindow;
        }

        if dispatcher.dispatch(ActivationSource::Programmatic, window, cx) {
            ActivationRequestResult::Dispatched
        } else {
            ActivationRequestResult::Blocked
        }
    }

    fn bind(&self, window_id: WindowId, dispatcher: &SharedActivationDispatcher) {
        *self.binding.borrow_mut() = Some(ProgrammaticActivationBinding {
            window_id,
            dispatcher: Rc::downgrade(dispatcher),
        });
    }
}

struct ArmedKeyActivation {
    key: ActivationKey,
    focus: Option<FocusHandle>,
    focus_claim_revision: u64,
    key_event_revision: u64,
}

impl ArmedKeyActivation {
    fn matches_next_event(
        &self,
        key: ActivationKey,
        focus: Option<&FocusHandle>,
        focus_claim_revision: u64,
        key_event_revision: u64,
    ) -> bool {
        self.key == key
            && self.focus.as_ref() == focus
            && self.focus_claim_revision == focus_claim_revision
            && self.key_event_revision.wrapping_add(1) == key_event_revision
    }
}

#[derive(Default)]
struct ActivationRuntime {
    enabled: Cell<Option<bool>>,
    armed_key: RefCell<Option<ArmedKeyActivation>>,
    pointer_armed: Cell<bool>,
}

impl ActivationRuntime {
    fn rebind(&self, enabled: bool) {
        if self
            .enabled
            .replace(Some(enabled))
            .is_some_and(|old| old != enabled)
        {
            self.clear_armed_key();
            self.pointer_armed.set(false);
        }
    }

    fn clear_armed_key(&self) {
        self.armed_key.borrow_mut().take();
    }

    fn arm_key(
        &self,
        key: ActivationKey,
        focus: Option<FocusHandle>,
        focus_claim_revision: u64,
        key_event_revision: u64,
    ) {
        *self.armed_key.borrow_mut() = Some(ArmedKeyActivation {
            key,
            focus,
            focus_claim_revision,
            key_event_revision,
        });
    }

    fn advance_armed_key(
        &self,
        key: ActivationKey,
        focus: Option<&FocusHandle>,
        focus_claim_revision: u64,
        key_event_revision: u64,
    ) -> bool {
        let mut armed_key = self.armed_key.borrow_mut();
        let Some(armed) = armed_key.as_mut() else {
            return false;
        };
        if !armed.matches_next_event(key, focus, focus_claim_revision, key_event_revision) {
            armed_key.take();
            return false;
        }

        armed.key_event_revision = key_event_revision;
        true
    }

    fn take_armed_key(&self) -> Option<ArmedKeyActivation> {
        self.armed_key.borrow_mut().take()
    }

    fn arm_pointer(&self, armed: bool) {
        self.pointer_armed.set(armed);
    }

    fn take_armed_pointer(&self) -> bool {
        self.pointer_armed.replace(false)
    }
}

#[derive(Clone)]
pub(crate) struct ActivationBinding {
    keys: ActivationKeyPolicy,
    dispatcher: SharedActivationDispatcher,
    runtime: Entity<ActivationRuntime>,
    window_id: WindowId,
    programmatic_handle: Option<ActivationHandle>,
}

impl ActivationBinding {
    pub(crate) fn new(
        window: &mut Window,
        cx: &mut App,
        state_key: impl Into<ElementId>,
        enabled: bool,
        keys: ActivationKeyPolicy,
        transaction: impl Fn(Activation, &mut Window, &mut App) + 'static,
    ) -> Self {
        let runtime = window.use_keyed_state(state_key, cx, |_, _| ActivationRuntime::default());
        runtime.read(cx).rebind(enabled);

        Self {
            keys,
            dispatcher: Rc::new(ActivationDispatcher::new(enabled, transaction)),
            runtime,
            window_id: window.window_handle().window_id(),
            programmatic_handle: None,
        }
    }

    pub(crate) fn with_programmatic_handle(mut self, handle: Option<ActivationHandle>) -> Self {
        self.programmatic_handle = handle;
        self
    }

    fn dispatch(&self, source: ActivationSource, window: &mut Window, cx: &mut App) -> bool {
        if window.window_handle().window_id() != self.window_id {
            return false;
        }

        self.dispatcher.dispatch(source, window, cx)
    }

    pub(crate) fn programmatic(&self, window: &mut Window, cx: &mut App) -> bool {
        self.dispatch(ActivationSource::Programmatic, window, cx)
    }

    /// Binds only the programmatic request seam and retains it with the rendered element.
    pub(crate) fn bind_programmatic<E>(self, element: E) -> E
    where
        E: StatefulInteractiveElement + Sized,
    {
        if self.bind_programmatic_handle() {
            element.retain_for_frame(self.dispatcher)
        } else {
            element
        }
    }

    pub(crate) fn bind<E>(self, element: E) -> E
    where
        E: StatefulInteractiveElement + Sized,
    {
        let keyboard = self.clone();
        keyboard.bind_keyboard(self.bind_pointer_and_accessibility(element))
    }

    pub(crate) fn bind_pointer_and_accessibility<E>(self, element: E) -> E
    where
        E: StatefulInteractiveElement + Sized,
    {
        let programmatic_dispatcher = self
            .bind_programmatic_handle()
            .then(|| self.dispatcher.clone());

        let pointer = self.clone();
        let pointer_down = self.clone();
        let accessibility = self;

        let element = if pointer.dispatcher.enabled() {
            let pointer_down_runtime = pointer_down.runtime.clone();
            let pointer_click_runtime = pointer.runtime.clone();
            element
                .on_mouse_down(
                    MouseButton::Left,
                    move |_, window: &mut Window, cx: &mut App| {
                        pointer_down_runtime.read(cx).arm_pointer(
                            pointer_down.dispatcher.enabled() && !window.default_prevented(),
                        );
                    },
                )
                .on_click(move |event, window, cx| {
                    let event = event.window_event();
                    if !matches!(event, ClickEvent::Mouse(_)) || !event.standard_click() {
                        return;
                    }
                    let pointer_armed = pointer_click_runtime.read(cx).take_armed_pointer();
                    if !pointer_armed || window.default_prevented() {
                        return;
                    }

                    if pointer.dispatch(ActivationSource::Pointer, window, cx) {
                        cx.stop_propagation();
                    }
                })
        } else {
            element
        };

        let element = element.on_ui_a11y_action(AccessibleAction::Click, move |_, window, cx| {
            accessibility.dispatch(ActivationSource::Accessibility, window, cx);
        });

        if let Some(dispatcher) = programmatic_dispatcher {
            element.retain_for_frame(dispatcher)
        } else {
            element
        }
    }

    pub(crate) fn bind_keyboard<E>(self, element: E) -> E
    where
        E: StatefulInteractiveElement + Sized,
    {
        let key_down = self.clone();
        let key_up = self;
        let key_down_runtime = key_down.runtime.clone();
        let key_up_runtime = key_up.runtime.clone();

        element
            .on_key_down(move |event: &KeyDownEvent, window, cx| {
                let runtime = key_down_runtime.read(cx);
                if event.keystroke.modifiers.modified()
                    || event.prefer_character_input
                    || window.default_prevented()
                {
                    runtime.clear_armed_key();
                    return;
                }
                let Some(key) = key_down.keys.resolve(event.keystroke.key.as_str()) else {
                    runtime.clear_armed_key();
                    return;
                };
                if !key_down.dispatcher.enabled() {
                    runtime.clear_armed_key();
                    return;
                }

                let focus = window.focused(cx);
                let focus_claim_revision = window.focus_claim_revision();
                let key_event_revision = window.key_event_revision();
                if event.is_held {
                    if !runtime.advance_armed_key(
                        key,
                        focus.as_ref(),
                        focus_claim_revision,
                        key_event_revision,
                    ) {
                        return;
                    }
                } else {
                    runtime.arm_key(key, focus, focus_claim_revision, key_event_revision);
                }

                cx.stop_propagation();
                if key == ActivationKey::Space {
                    window.prevent_default();
                }
            })
            .on_key_up(move |event: &KeyUpEvent, window, cx| {
                let armed = key_up_runtime.read(cx).take_armed_key();
                if event.keystroke.modifiers.modified() || window.default_prevented() {
                    return;
                }
                let Some(key) = key_up.keys.resolve(event.keystroke.key.as_str()) else {
                    return;
                };
                if !key_up.dispatcher.enabled()
                    || !armed.is_some_and(|armed| {
                        armed.matches_next_event(
                            key,
                            window.focused(cx).as_ref(),
                            window.focus_claim_revision(),
                            window.key_event_revision(),
                        )
                    })
                {
                    return;
                }

                cx.stop_propagation();
                if key == ActivationKey::Space {
                    window.prevent_default();
                }
                key_up.dispatch(ActivationSource::Keyboard(key), window, cx);
            })
    }

    fn bind_programmatic_handle(&self) -> bool {
        let Some(handle) = self.programmatic_handle.as_ref() else {
            return false;
        };
        handle.bind(self.window_id, &self.dispatcher);
        true
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use open_gpui::{Context, IntoElement, Render, div};

    use super::*;

    #[open_gpui::test]
    fn programmatic_activation_uses_the_same_enabled_gate(cx: &mut open_gpui::TestAppContext) {
        struct EmptyView;

        impl Render for EmptyView {
            fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
                div()
            }
        }

        let sources = Rc::new(RefCell::new(Vec::new()));
        let enabled_sources = sources.clone();
        let disabled_sources = sources.clone();
        let enabled = ActivationDispatcher::new(true, move |activation, _, _| {
            enabled_sources.borrow_mut().push(activation.source())
        });
        let disabled = ActivationDispatcher::new(false, move |activation, _, _| {
            disabled_sources.borrow_mut().push(activation.source())
        });

        let (_, cx) = cx.add_window_view(|_, _| EmptyView);
        cx.update(|window, cx| {
            assert!(enabled.dispatch(ActivationSource::Programmatic, window, cx));
            assert!(!disabled.dispatch(ActivationSource::Programmatic, window, cx));
        });

        assert_eq!(
            sources.borrow().as_slice(),
            &[ActivationSource::Programmatic]
        );
    }
}
