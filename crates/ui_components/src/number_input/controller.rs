use super::*;
use crate::a11y::AccessibleTextInputHandler;
use open_gpui::{Context, Entity, FocusHandle, Focusable, PointerCaptureHandle, Subscription};

type ChangeHandler = Rc<dyn Fn(NumberInputChange, &mut Window, &mut App)>;
type PreviewHandler = Rc<dyn Fn(NumberInputPreview, &mut Window, &mut App)>;
type CommitHandler = Rc<dyn Fn(NumberInputCommit, &mut Window, &mut App)>;
type CancelHandler = Rc<dyn Fn(NumberInputCancel, &mut Window, &mut App)>;

#[derive(Default, Clone)]
pub(super) struct NumberInputHandlers {
    pub(super) change: Option<ChangeHandler>,
    pub(super) preview: Option<PreviewHandler>,
    pub(super) commit: Option<CommitHandler>,
    pub(super) cancel: Option<CancelHandler>,
}

pub(super) struct NumberInputMount {
    controller: Entity<NumberInputController>,
    _observation: Subscription,
}

impl NumberInputMount {
    pub(super) fn new(controller: Entity<NumberInputController>, cx: &mut Context<Self>) -> Self {
        let observation = cx.observe(&controller, |_, _, cx| cx.notify());
        Self {
            controller,
            _observation: observation,
        }
    }

    pub(super) fn controller(
        &mut self,
        external: Option<Entity<NumberInputController>>,
        cx: &mut Context<Self>,
    ) -> Entity<NumberInputController> {
        if let Some(external) = external
            && external != self.controller
        {
            *self = Self::new(external, cx);
        }
        self.controller.clone()
    }
}

struct ScrubSession {
    origin: f32,
    value: f32,
    draft: NumberInputDraft,
    selection: NumberInputSelection,
}

#[derive(Clone, Copy)]
struct NumberInputSelection {
    anchor: usize,
    focus: usize,
}

impl NumberInputSelection {
    fn read(input: &TextInputController) -> Self {
        let range = input.selected_range();
        if AccessibleTextInputHandler::selection_reversed(input) {
            Self {
                anchor: range.end,
                focus: range.start,
            }
        } else {
            Self {
                anchor: range.start,
                focus: range.end,
            }
        }
    }

    fn restore(self, input: &mut TextInputController, cx: &mut Context<TextInputController>) {
        input.select_range(self.anchor..self.focus, cx);
    }
}

struct NumberInputSubmission {
    result: NumberInputCommit,
    identity: Rc<()>,
    delivered: bool,
}

/// Persistent scalar editing state for one number input in one window.
///
/// Construct this entity once with `cx.new(|cx| NumberInputController::new(value, window, cx))`
/// and pass it to `NumberInput::controller` on each render. An input without an explicit
/// controller uses GPUI's keyed element state. Selection, composition, and clipboard behavior
/// remain owned by the contained `TextInputController`.
pub struct NumberInputController {
    text_input: Entity<TextInputController>,
    draft: NumberInputDraft,
    state: NumberInputState,
    controlled: bool,
    composing: bool,
    pending: Option<NumberInputSubmission>,
    scrub: Option<ScrubSession>,
    capture: PointerCaptureHandle,
    handlers: NumberInputHandlers,
    _subscriptions: Vec<Subscription>,
}

impl NumberInputController {
    /// Creates editing state with an initial value and the default numeric constraints.
    ///
    /// The initial value is kept finite; the component applies its bounds and step when mounted.
    pub fn new(value: f32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let value = if value.is_finite() { value } else { 0.0 };
        let text_input = cx.new(|cx| TextInputController::with_value(value.to_string(), cx));
        let focus = text_input.focus_handle(cx);
        let observe_text = cx.observe_in(&text_input, window, |this, _, window, cx| {
            this.read_text(window, cx);
        });
        let blur = cx.on_blur(&focus, window, |this, window, cx| {
            if this.scrub.is_some() || this.is_composing(cx) {
                this.cancel(NumberInputCancelReason::FocusLost, window, cx);
            } else {
                this.commit(window, cx);
            }
        });
        Self {
            text_input,
            draft: NumberInputDraft::new(value),
            state: NumberInput::new("number-controller", "Number").state(),
            controlled: false,
            composing: false,
            pending: None,
            scrub: None,
            capture: window.new_pointer_capture_handle(),
            handlers: NumberInputHandlers::default(),
            _subscriptions: vec![observe_text, blur],
        }
    }

    /// Returns the original text controller for focus, selection, and platform text input.
    pub fn text_input(&self) -> &Entity<TextInputController> {
        &self.text_input
    }

    /// Returns the accepted numeric value.
    pub const fn value(&self) -> f32 {
        self.draft.value
    }

    /// Returns the exact local text draft and its parse state.
    pub fn draft(&self) -> &NumberInputDraft {
        &self.draft
    }

    /// Returns whether a captured scrub gesture is in progress.
    pub fn is_scrubbing(&self) -> bool {
        self.scrub.is_some()
    }

    /// Sets an application-owned value, canceling any outstanding draft.
    ///
    /// Prefer the component's `value` builder for controlled inputs. Programmatic replacement
    /// preserves focus and clamps the existing selection to the new text.
    pub fn set_value(&mut self, value: f32, window: &mut Window, cx: &mut Context<Self>) {
        if !value.is_finite() {
            return;
        }
        self.cancel(NumberInputCancelReason::ExternalValue, window, cx);
        let value =
            normalize_number_value(value, self.state.min(), self.state.max(), self.state.step());
        self.draft = NumberInputDraft::new(value);
        self.write_text(cx);
        cx.notify();
    }

    /// Submits a finite text draft. Composition Enter and clean drafts are ignored.
    ///
    /// A trailing decimal point is finalized on submission. Other incomplete, empty, and invalid
    /// drafts are canceled. Normalization happens only here; typing a finite out-of-range value
    /// leaves the exact text and preview intact.
    /// Returns whether this edit consumed the submission, including cancellation of invalid text.
    pub fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.read_text(window, cx);
        if !self.state.input_enabled() || self.is_composing(cx) || self.pending.is_some() {
            return false;
        }
        if self.scrub.is_some() || !self.draft.dirty {
            return false;
        }
        let Some(value) = self.draft.submitted_value() else {
            return self.cancel(NumberInputCancelReason::InvalidDraft, window, cx);
        };
        self.propose(
            value,
            NumberInputStepAction::SetValue,
            NumberInputEditSource::Text,
            window,
            cx,
        );
        true
    }

    /// Cancels an edit without changing the owner's value or stealing focus.
    ///
    /// A scrub restores its exact starting draft and selection. Other cancellations restore
    /// the current accepted value while retaining the selection where possible.
    pub fn cancel(
        &mut self,
        reason: NumberInputCancelReason,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let scrubbing = self.scrub.take();
        if !self.draft.dirty
            && scrubbing.is_none()
            && self.pending.is_none()
            && !self.is_composing(cx)
        {
            return false;
        }
        let event = NumberInputCancel {
            reason,
            draft: self.draft.clone(),
        };
        let previous_value = self
            .pending
            .take()
            .map(|pending| pending.result.previous_value());
        let _ = window.release_pointer(&self.capture);
        if let Some(scrub) = scrubbing {
            self.draft = scrub.draft;
            self.write_text(cx);
            self.text_input
                .update(cx, |input, cx| scrub.selection.restore(input, cx));
        } else {
            self.draft = NumberInputDraft::new(previous_value.unwrap_or(self.draft.value));
            self.write_text(cx);
        }
        if let Some(handler) = self.handlers.cancel.clone() {
            window.defer(cx, move |window, cx| handler(event, window, cx));
        }
        cx.notify();
        true
    }

    /// Applies a discrete, bounded step through the same proposal path as text and scrubbing.
    pub fn step(
        &mut self,
        action: NumberInputStepAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.step_from(action, NumberInputEditSource::Keyboard, window, cx)
    }

    pub(super) fn sync(
        &mut self,
        mut state: NumberInputState,
        controlled_value: Option<f32>,
        handlers: NumberInputHandlers,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.handlers = handlers;
        let constraints_changed = self.state.min() != state.min()
            || self.state.max() != state.max()
            || self.state.step() != state.step();
        if !state.input_enabled() && self.state.input_enabled() {
            self.cancel(NumberInputCancelReason::EditingDisabled, window, cx);
        }
        if constraints_changed {
            self.cancel(NumberInputCancelReason::ExternalValue, window, cx);
        }
        let value = controlled_value.unwrap_or(self.draft.value);
        let value = normalize_number_value(value, state.min(), state.max(), state.step());
        self.controlled = controlled_value.is_some();
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.delivered)
        {
            let Some(pending) = self.pending.take() else {
                return;
            };
            let mut result = pending.result;
            result.value = value;
            self.draft = NumberInputDraft::new(value);
            self.write_text(cx);
            self.emit_commit(result, window, cx);
        } else if self.draft.value != value {
            self.cancel(NumberInputCancelReason::ExternalValue, window, cx);
            self.draft = NumberInputDraft::new(value);
            self.write_text(cx);
        }
        state.value = self.draft.value;
        self.state = state;
        self.text_input.update(cx, |input, cx| {
            input.set_disabled(self.state.disabled(), cx);
            input.set_read_only(self.state.read_only(), cx);
        });
    }

    pub(super) fn is_composing(&self, cx: &App) -> bool {
        self.text_input.read(cx).marked_range_utf16().is_some()
    }

    pub(super) fn capture(&self) -> PointerCaptureHandle {
        self.capture
    }

    pub(super) fn pending_submission(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn step_from(
        &mut self,
        action: NumberInputStepAction,
        source: NumberInputEditSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.read_text(window, cx);
        if !self.state.input_enabled()
            || self.is_composing(cx)
            || self.pending.is_some()
            || self.scrub.is_some()
        {
            return false;
        }
        let mut state = self.state.clone();
        state.value = self.draft.parsed().value().unwrap_or(self.draft.value);
        let Some(change) = state.step_change(action) else {
            return false;
        };
        if change.value() == self.draft.value && !self.draft.dirty {
            return false;
        }
        self.propose(change.value(), action, source, window, cx);
        true
    }

    pub(super) fn set_accessible_value(
        &mut self,
        value: f64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.state.input_enabled()
            || self.is_composing(cx)
            || self.pending.is_some()
            || self.scrub.is_some()
        {
            return;
        }
        if let Some(change) = self.state.set_value_change(value)
            && change.changed()
        {
            self.propose(
                change.value(),
                change.action(),
                NumberInputEditSource::Accessibility,
                window,
                cx,
            );
        }
    }

    pub(super) fn begin_scrub(
        &mut self,
        position: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.read_text(window, cx);
        if !self.state.input_enabled()
            || self.is_composing(cx)
            || self.pending.is_some()
            || self.scrub.is_some()
        {
            return false;
        }
        if window
            .capture_pointer(&self.capture, open_gpui::MouseButton::Left)
            .is_err()
        {
            return false;
        }
        let value = self.draft.parsed().value().unwrap_or(self.draft.value);
        self.scrub = Some(ScrubSession {
            origin: position,
            value: normalize_number_value(
                value,
                self.state.min(),
                self.state.max(),
                self.state.step(),
            ),
            draft: self.draft.clone(),
            selection: NumberInputSelection::read(self.text_input.read(cx)),
        });
        cx.notify();
        true
    }

    pub(super) fn update_scrub(
        &mut self,
        position: f32,
        pixels_per_step: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(scrub) = &self.scrub else {
            return;
        };
        if !position.is_finite() {
            return;
        }
        let steps = ((position as f64 - scrub.origin as f64) / pixels_per_step as f64).round();
        if steps == 0.0 {
            if self.draft != scrub.draft {
                let selection = scrub.selection;
                self.draft = scrub.draft.clone();
                self.write_text(cx);
                self.text_input
                    .update(cx, |input, cx| selection.restore(input, cx));
                self.emit_preview(NumberInputEditSource::Scrub, window, cx);
                cx.notify();
            }
            return;
        }
        let requested = scrub.value as f64 + steps * self.state.step() as f64;
        let value = normalize_number_value(
            requested.clamp(self.state.min() as f64, self.state.max() as f64) as f32,
            self.state.min(),
            self.state.max(),
            self.state.step(),
        );
        let text = value.to_string();
        if text == self.draft.text {
            return;
        }
        self.draft.text = text;
        self.draft.dirty = true;
        self.write_text(cx);
        self.emit_preview(NumberInputEditSource::Scrub, window, cx);
        cx.notify();
    }

    pub(super) fn finish_scrub(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(scrub) = self.scrub.take() else {
            return;
        };
        let _ = window.release_pointer(&self.capture);
        if self.draft == scrub.draft {
            cx.notify();
            return;
        }
        if let Some(value) = self.draft.parsed().value() {
            self.propose(
                value,
                NumberInputStepAction::SetValue,
                NumberInputEditSource::Scrub,
                window,
                cx,
            );
        }
    }

    fn read_text(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.text_input.read(cx);
        let composing = input.marked_range_utf16().is_some();
        let composition_finished = self.composing && !composing;
        self.composing = composing;
        if input.value() == self.draft.text {
            if composition_finished && self.draft.dirty {
                self.emit_preview(NumberInputEditSource::Text, window, cx);
            }
            return;
        }
        if !self.state.input_enabled() || self.pending.is_some() || self.scrub.is_some() {
            self.write_text(cx);
            return;
        }
        self.draft.text = input.value().to_owned();
        self.draft.dirty = true;
        if !self.is_composing(cx) {
            self.emit_preview(NumberInputEditSource::Text, window, cx);
        }
        cx.notify();
    }

    fn write_text(&self, cx: &mut Context<Self>) {
        self.text_input.update(cx, |input, cx| {
            if input.value() != self.draft.text || input.marked_range_utf16().is_some() {
                let selection = NumberInputSelection::read(input);
                input.set_value(self.draft.text.clone(), cx);
                selection.restore(input, cx);
            }
        });
    }

    fn propose(
        &mut self,
        value: f32,
        action: NumberInputStepAction,
        source: NumberInputEditSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let value =
            normalize_number_value(value, self.state.min(), self.state.max(), self.state.step());
        let result = NumberInputCommit {
            source,
            previous_value: self.draft.value,
            proposed_value: value,
            value,
        };
        let change = NumberInputChange::new(action, self.draft.value, value);
        let identity = Rc::new(());
        self.pending = Some(NumberInputSubmission {
            result,
            identity: identity.clone(),
            delivered: false,
        });
        if !self.controlled {
            self.draft = NumberInputDraft::new(value);
            self.write_text(cx);
        }
        let on_change = self.handlers.change.clone();
        let on_commit = self.handlers.commit.clone();
        let controller = cx.weak_entity();
        window.defer(cx, move |window, cx| {
            let delivery = controller.update(cx, |controller, _| {
                let pending = controller.pending.as_mut()?;
                if !Rc::ptr_eq(&pending.identity, &identity) {
                    return None;
                }
                pending.delivered = true;
                let locally_owned = !controller.controlled;
                if locally_owned {
                    controller.pending = None;
                }
                Some(locally_owned)
            });
            let Ok(Some(locally_owned)) = delivery else {
                return;
            };
            if change.changed()
                && let Some(handler) = on_change
            {
                handler(change, window, cx);
            }
            if locally_owned && let Some(handler) = on_commit {
                handler(result, window, cx);
            }
            window.refresh();
        });
        cx.notify();
    }

    fn emit_preview(
        &self,
        source: NumberInputEditSource,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(handler) = self.handlers.preview.clone() {
            let event = NumberInputPreview {
                source,
                draft: self.draft.clone(),
            };
            window.defer(cx, move |window, cx| handler(event, window, cx));
        }
    }

    fn emit_commit(&self, result: NumberInputCommit, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(handler) = self.handlers.commit.clone() {
            window.defer(cx, move |window, cx| handler(result, window, cx));
        }
    }
}

impl Focusable for NumberInputController {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.text_input.focus_handle(cx)
    }
}
