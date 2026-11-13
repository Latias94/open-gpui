use std::rc::Rc;

use open_gpui::prelude::*;
use open_gpui::{
    AnyElement, App, ElementId, Entity, Focusable, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, PortalAnchorExt as _, RenderOnce, SharedString, StatefulInteractiveElement,
    Styled, Subscription, Window, div,
};
use open_gpui_ui_core::{
    AccessibleAction, DismissReason, EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent,
    OutsidePressPolicy, OverlayPlacementAlignment, OverlayPlacementInput, OverlayPlacementSide,
    Role, SemanticDescriptor, Sizable, Size, ThemeTokens, ui_px, ui_size,
};

use crate::a11y::UiA11yElementExt;
use crate::choice::{ChoiceSelectionOwnership, SingleChoiceSelectionControl};
use crate::choice_overlay_runtime::request_registered_choice_selection;
use crate::focus::focus_ring_shadow_with_theme;
use crate::geometry::gpui_px_from_ui;
use crate::listbox::{
    Listbox, ListboxGroup, ListboxOption, ListboxOptionState, ListboxRowContentRenderer,
};
use crate::overlay::{
    OverlayInsideRegionId, OverlayLayerBinding, OverlayLayerRegistration, OverlayOpenIntent,
    OverlayOwnership, WindowOverlayRuntime, gpui_overlay_state, gpui_portal_anchor_overlay_layer,
    resolve_overlay_open_state,
};
use crate::scroll_area::ScrollArea;
use crate::text_editing::TextEditingPolicy;
use crate::text_input::TextInput;
use crate::text_input::adapter::TextInputController;
use crate::theme::{ThemeContext, ThemeResolver, gpui_elevation_shadow};

use super::descriptor::{ComboboxGroup, ComboboxOption};
use super::model::{
    ComboboxKeyboardAction, ComboboxSelection, ComboboxState, ComboboxStateRequest,
    combobox_keyboard_action_from_value,
};
use super::render_plan::ComboboxRenderPlan;
use super::{ComboboxFilterMode, ComboboxResults, ComboboxResultsStatus};

type ComboboxOpenChangeHandler = Rc<dyn Fn(OverlayOpenIntent, &mut Window, &mut App)>;
type ComboboxSelectionHandler = Rc<dyn Fn(ComboboxSelection, &mut Window, &mut App)>;
type ComboboxQueryChangeHandler = Rc<dyn Fn(String, &mut Window, &mut App)>;
type ComboboxResultsContent =
    Box<dyn FnOnce(&ComboboxState, AnyElement, &mut Window, &mut App) -> AnyElement>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ComboboxInputValueSource {
    SelectedLabelProjection,
    UserQuery,
}

#[derive(Clone)]
struct ComboboxRuntime {
    open: bool,
    active_value: Option<String>,
    selected_value: Option<String>,
    selection_ownership: ChoiceSelectionOwnership,
    retained_selection: Option<ComboboxSelection>,
    observed_selected_item: Option<ComboboxSelection>,
    observed_selected_label: Option<Option<String>>,
    observed_input_user_edit_revision: u64,
    input_value_source: ComboboxInputValueSource,
    query_controlled: bool,
    pending_selection_projection: bool,
    pending_user_query: Option<String>,
    on_query_change: Option<ComboboxQueryChangeHandler>,
    input_subscription: Option<Rc<Subscription>>,
    overlay_binding: Option<OverlayLayerBinding>,
}

impl ComboboxRuntime {
    fn set_active(&mut self, value: String, cx: &mut Context<Self>) {
        if self.active_value.as_ref() != Some(&value) {
            self.active_value = Some(value);
            cx.notify();
        }
    }

    fn sync_selection(
        &mut self,
        selection: &SingleChoiceSelectionControl,
        selected_item: Option<&ComboboxSelection>,
    ) {
        let selection_ownership =
            ChoiceSelectionOwnership::from_controlled(selection.is_controlled());
        let mut changed = self.selection_ownership != selection_ownership;
        if selection.is_controlled() && self.selected_value.as_ref() != selection.value().as_ref() {
            self.selected_value = selection.value().clone();
            self.retained_selection = None;
            changed = true;
        }
        self.selection_ownership = selection_ownership;
        if self.observed_selected_item.as_ref() != selected_item {
            self.observed_selected_item = selected_item.cloned();
            if selected_item
                .is_some_and(|item| Some(item.value()) == self.selected_value.as_deref())
            {
                self.retained_selection = selected_item.cloned();
            }
        }
        if changed && selection.is_controlled() {
            self.pending_selection_projection = true;
        }
    }

    fn commit_selection(
        &mut self,
        selection: ComboboxSelection,
        input_user_edit_revision: u64,
    ) -> bool {
        let value = selection.value().to_owned();
        let adapter_owned = !self.selection_ownership.caller_owned();
        if adapter_owned {
            self.selected_value = Some(value.clone());
            self.retained_selection = Some(selection);
            self.observed_input_user_edit_revision = input_user_edit_revision;
            if !self.query_controlled {
                self.input_value_source = ComboboxInputValueSource::SelectedLabelProjection;
            }
        }
        self.active_value = Some(value);
        adapter_owned && !self.query_controlled
    }

    fn sync_selected_label(&mut self, selected_label: Option<&str>) -> bool {
        let current = selected_label.map(str::to_owned);
        match self.observed_selected_label.as_ref() {
            None => {
                self.observed_selected_label = Some(current);
                false
            }
            Some(previous) if previous.as_deref() == selected_label => false,
            Some(_) => {
                self.observed_selected_label = Some(current);
                true
            }
        }
    }

    fn sync_input_user_edit_revision(&mut self, revision: u64, query: &str) {
        if self.observed_input_user_edit_revision != revision {
            self.observed_input_user_edit_revision = revision;
            self.input_value_source = ComboboxInputValueSource::UserQuery;
            self.pending_user_query = Some(query.to_owned());
            self.active_value = None;
        }
    }
}

/// A concrete GPUI combobox component.
#[derive(IntoElement)]
pub struct Combobox {
    id: ElementId,
    label: SharedString,
    placeholder: SharedString,
    options: Vec<ComboboxOption>,
    groups: Vec<ComboboxGroup>,
    size: Size,
    disabled: bool,
    required: bool,
    open: Option<bool>,
    default_open: bool,
    default_query: String,
    query: Option<String>,
    filter_mode: ComboboxFilterMode,
    results_status: ComboboxResultsStatus,
    selection: SingleChoiceSelectionControl,
    selected_item: Option<ComboboxSelection>,
    default_active_value: Option<String>,
    empty_label: SharedString,
    placement_side: OverlayPlacementSide,
    placement_alignment: OverlayPlacementAlignment,
    outside_press_policy: OutsidePressPolicy,
    initial_focus_intent: InitialFocusIntent,
    focus_restore_intent: FocusRestoreIntent,
    tokens: ThemeTokens,
    on_open_change: Option<ComboboxOpenChangeHandler>,
    on_select: Option<ComboboxSelectionHandler>,
    on_query_change: Option<ComboboxQueryChangeHandler>,
    results_content: Option<ComboboxResultsContent>,
    option_content: Option<ListboxRowContentRenderer>,
}

impl Combobox {
    /// Creates an empty combobox.
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            placeholder: "Search".into(),
            options: Vec::new(),
            groups: Vec::new(),
            size: Size::Medium,
            disabled: false,
            required: false,
            open: None,
            default_open: false,
            default_query: String::new(),
            query: None,
            filter_mode: ComboboxFilterMode::Local,
            results_status: ComboboxResultsStatus::Ready,
            selection: SingleChoiceSelectionControl::uncontrolled(None),
            selected_item: None,
            default_active_value: None,
            empty_label: "No results".into(),
            placement_side: OverlayPlacementSide::Bottom,
            placement_alignment: OverlayPlacementAlignment::Start,
            outside_press_policy: OutsidePressPolicy::DismissAndConsume,
            initial_focus_intent: InitialFocusIntent::None,
            focus_restore_intent: FocusRestoreIntent::Trigger,
            tokens: ThemeTokens::default(),
            on_open_change: None,
            on_select: None,
            on_query_change: None,
            results_content: None,
            option_content: None,
        }
    }

    /// Applies placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Adds one standalone option.
    pub fn option(mut self, option: ComboboxOption) -> Self {
        self.options.push(option);
        self
    }

    /// Adds many standalone options.
    pub fn options(mut self, options: impl IntoIterator<Item = ComboboxOption>) -> Self {
        self.options.extend(options);
        self
    }

    /// Adds one option group.
    pub fn group(mut self, group: ComboboxGroup) -> Self {
        self.groups.push(group);
        self
    }

    /// Adds many option groups.
    pub fn groups(mut self, groups: impl IntoIterator<Item = ComboboxGroup>) -> Self {
        self.groups.extend(groups);
        self
    }

    /// Marks the combobox as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Marks the combobox as required.
    pub fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    /// Applies controlled open state.
    pub fn open(mut self, open: bool) -> Self {
        self.open = Some(open);
        self
    }

    /// Applies uncontrolled initial open state.
    pub fn default_open(mut self, default_open: bool) -> Self {
        self.default_open = default_open;
        self
    }

    /// Applies the default query text for adapter-owned input state.
    pub fn default_query(mut self, query: impl Into<String>) -> Self {
        let query = query.into();
        self.default_query = TextEditingPolicy::single_line().normalize_text(query.as_str());
        self
    }

    /// Applies caller-owned query text, independently of selection and open state.
    ///
    /// User edits propose a value through [`Self::on_query_change`]. Rejected
    /// proposals are reconciled through the existing text controller. Active IME
    /// composition remains controller-owned until the marked text is committed.
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = Some(TextEditingPolicy::single_line().normalize_text(&query.into()));
        self
    }

    /// Applies ownership of filtering for options supplied with the option builders.
    pub fn filter_mode(mut self, mode: ComboboxFilterMode) -> Self {
        self.filter_mode = mode;
        self
    }

    /// Replaces the result page and disables local filtering.
    ///
    /// The caller must reject stale replies before supplying this snapshot, using
    /// its resource owner or [`super::ComboboxQuerySession`]. This builder never
    /// starts work or changes selected identity, query text, or open state.
    pub fn results(mut self, results: ComboboxResults) -> Self {
        let (status, options, groups) = results.into_parts();
        self.filter_mode = ComboboxFilterMode::External;
        self.results_status = status;
        self.options = options
            .into_iter()
            .map(ComboboxOption::from_descriptor)
            .collect();
        self.groups = groups
            .into_iter()
            .map(ComboboxGroup::from_descriptor)
            .collect();
        self
    }

    /// Applies the caller-owned selected option value.
    pub fn selected(mut self, value: Option<String>) -> Self {
        self.selection = SingleChoiceSelectionControl::controlled(value);
        self.selected_item = None;
        self
    }

    /// Applies caller-owned selection metadata independently of result membership.
    ///
    /// A unique result with the same value supplies its latest label. Otherwise
    /// this payload supplies the label, including before the first page arrives.
    /// Duplicate or disabled current results remain ineligible for activation.
    pub fn selected_item(mut self, selection: Option<ComboboxSelection>) -> Self {
        self.selection = SingleChoiceSelectionControl::controlled(
            selection
                .as_ref()
                .map(|selection| selection.value().to_owned()),
        );
        self.selected_item = selection;
        self
    }

    /// Applies the default selected option value for adapter-owned runtime state.
    pub fn default_selected(mut self, value: impl Into<String>) -> Self {
        let value = value.into();
        if !self.selection.is_controlled() {
            self.selection = SingleChoiceSelectionControl::uncontrolled(Some(value));
        }
        self
    }

    /// Applies the initial active option for adapter-owned runtime state.
    pub fn default_active(mut self, value: impl Into<String>) -> Self {
        self.default_active_value = Some(value.into());
        self
    }

    /// Applies empty-state label.
    pub fn empty_label(mut self, label: impl Into<SharedString>) -> Self {
        self.empty_label = label.into();
        self
    }

    /// Applies preferred placement.
    pub fn placement(
        mut self,
        side: OverlayPlacementSide,
        alignment: OverlayPlacementAlignment,
    ) -> Self {
        self.placement_side = side;
        self.placement_alignment = alignment;
        self
    }

    /// Applies outside-press policy.
    pub fn outside_press_policy(mut self, policy: OutsidePressPolicy) -> Self {
        self.outside_press_policy = policy;
        self
    }

    /// Applies a token bundle.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Registers an open-change handler with the runtime-issued intent.
    pub fn on_open_change(
        mut self,
        handler: impl Fn(OverlayOpenIntent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(handler));
        self
    }

    /// Registers a combobox selection handler.
    pub fn on_select(
        mut self,
        handler: impl Fn(ComboboxSelection, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_select = Some(Rc::new(handler));
        self
    }

    /// Observes user query edits without changing query ownership.
    ///
    /// Marked IME edits are published after composition ends. Selection label
    /// projection and caller-provided query updates do not emit this callback.
    pub fn on_query_change(
        mut self,
        handler: impl Fn(String, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_query_change = Some(Rc::new(handler));
        self
    }

    /// Composes result content inside the owned popup surface.
    ///
    /// Render the supplied listbox viewport exactly once. It retains navigation,
    /// selection transactions, accessibility, scrolling, and editor-owned focus.
    /// The callback may wrap it with application content or status presentation;
    /// it must not replace the listbox's interaction or create a second popup owner.
    pub fn results_content<E: IntoElement>(
        mut self,
        render: impl FnOnce(&ComboboxState, AnyElement, &mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.results_content = Some(Box::new(move |state, listbox, window, cx| {
            render(state, listbox, window, cx).into_any_element()
        }));
        self
    }

    /// Renders each visible result's content through the original listbox row.
    ///
    /// Use the stable option value to look up application thumbnails and metadata.
    /// The descriptor label remains the filtering and accessibility label. The
    /// original row owns selection, disabled gates, identity, focus, and scrolling;
    /// query text and external result ownership are unaffected by this callback.
    ///
    /// Passive images and text remain clickable as part of the result. Wrap
    /// interactive children in [`crate::listbox::ListboxRowAction`] and apply the
    /// option's disabled state to those children when appropriate. The callback
    /// runs under the original row identity and captured popup theme.
    pub fn option_content<E: IntoElement>(
        mut self,
        render: impl Fn(&ListboxOptionState, &mut Window, &mut App) -> E + 'static,
    ) -> Self {
        self.option_content = Some(Rc::new(move |option, window, cx| {
            render(option, window, cx).into_any_element()
        }));
        self
    }

    /// Returns resolved combobox state.
    pub fn state(&self) -> ComboboxState {
        ComboboxState::resolve(ComboboxStateRequest {
            size: self.size,
            disabled: self.disabled,
            required: self.required,
            open: self.open,
            default_open: self.default_open,
            label: self.label.to_string(),
            placeholder: self.placeholder.to_string(),
            query: self.query.as_ref().unwrap_or(&self.default_query).clone(),
            filter_mode: self.filter_mode,
            results_status: self.results_status.clone(),
            selected_value: self.selection.value().clone(),
            selected_label: self
                .selected_item
                .as_ref()
                .map(|item| item.label().to_owned()),
            active_value: self.default_active_value.clone(),
            empty_label: self.empty_label.to_string(),
            groups: self.groups.iter().map(ComboboxGroup::descriptor).collect(),
            options: self
                .options
                .iter()
                .map(ComboboxOption::descriptor)
                .collect(),
            placement_side: self.placement_side,
            placement_alignment: self.placement_alignment,
            outside_press_policy: self.outside_press_policy,
            initial_focus_intent: self.initial_focus_intent.clone(),
            focus_restore_intent: self.focus_restore_intent.clone(),
            tokens: self.tokens,
        })
    }
}

impl Sizable for Combobox {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }
}

impl RenderOnce for Combobox {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(window, cx);
        let runtime = window.use_keyed_state(self.id.clone(), cx, |_, _| ComboboxRuntime {
            open: self.default_open,
            active_value: self.default_active_value.clone(),
            selected_value: self.selection.value().clone(),
            selection_ownership: ChoiceSelectionOwnership::from_controlled(
                self.selection.is_controlled(),
            ),
            retained_selection: self.selected_item.clone(),
            observed_selected_item: self.selected_item.clone(),
            observed_selected_label: None,
            observed_input_user_edit_revision: 0,
            input_value_source: if self.default_query.is_empty() {
                ComboboxInputValueSource::SelectedLabelProjection
            } else {
                ComboboxInputValueSource::UserQuery
            },
            query_controlled: self.query.is_some(),
            pending_selection_projection: false,
            pending_user_query: None,
            on_query_change: self.on_query_change.clone(),
            input_subscription: None,
            overlay_binding: None,
        });
        let input_state_key: ElementId = (self.id.clone(), "input-state").into();
        let input_controller = window.use_keyed_state(input_state_key, cx, |_, cx| {
            let initial_value = if let Some(query) = self.query.as_ref() {
                query.clone()
            } else if self.default_query.is_empty() {
                self.state().selected_label().unwrap_or_default().to_owned()
            } else {
                self.default_query.clone()
            };
            let mut input = TextInputController::with_value(initial_value, cx);
            input.set_placeholder(self.placeholder.clone(), cx);
            input
        });
        runtime.update(cx, |runtime, cx| {
            runtime.query_controlled = self.query.is_some();
            runtime.on_query_change = self.on_query_change.clone();
            if runtime.input_subscription.is_none() {
                runtime.input_subscription = Some(Rc::new(cx.observe_in(
                    &input_controller,
                    window,
                    |runtime, controller, window, cx| {
                        let input = controller.read(cx);
                        runtime.sync_input_user_edit_revision(
                            input.user_edit_revision(),
                            input.value(),
                        );
                        if input.marked_range_utf16().is_none()
                            && let Some(query) = runtime.pending_user_query.take()
                            && let Some(on_query_change) = runtime.on_query_change.clone()
                        {
                            on_query_change(query, window, cx);
                        }
                        cx.notify();
                    },
                )));
            }
        });
        runtime.update(cx, |runtime, _| {
            runtime.sync_selection(&self.selection, self.selected_item.as_ref())
        });
        let (input_query, input_user_edit_revision, composing) = {
            let controller = input_controller.read(cx);
            (
                controller.value().to_owned(),
                controller.user_edit_revision(),
                controller.marked_range_utf16().is_some(),
            )
        };
        runtime.update(cx, |runtime, _| {
            runtime.sync_input_user_edit_revision(input_user_edit_revision, &input_query);
        });
        let runtime_state = runtime.read(cx).clone();
        let open_state = resolve_overlay_open_state(self.open, runtime_state.open);
        let resolved_open = open_state.open();
        let query = if composing {
            input_query
        } else {
            self.query.clone().unwrap_or(input_query)
        };

        if open_state.runtime_changed() {
            runtime.update(cx, |runtime, _| {
                runtime.open = resolved_open;
            });
        }

        let selected_value = runtime_state.selected_value.as_deref();
        let active_value = runtime_state.active_value.as_deref().or(selected_value);
        let retained_selection = (self.filter_mode == ComboboxFilterMode::External
            || self.selected_item.is_some())
        .then_some(runtime_state.retained_selection.as_ref())
        .flatten();
        let state = ComboboxState::resolve(ComboboxStateRequest {
            size: self.size,
            disabled: self.disabled,
            required: self.required,
            open: Some(resolved_open),
            default_open: self.default_open,
            label: self.label.to_string(),
            placeholder: self.placeholder.to_string(),
            query: query.clone(),
            filter_mode: self.filter_mode,
            results_status: self.results_status.clone(),
            selected_value: selected_value.map(str::to_owned),
            selected_label: retained_selection
                .filter(|item| Some(item.value()) == selected_value)
                .map(|item| item.label().to_owned()),
            active_value: active_value.map(str::to_owned),
            empty_label: self.empty_label.to_string(),
            groups: self.groups.iter().map(ComboboxGroup::descriptor).collect(),
            options: self
                .options
                .iter()
                .map(ComboboxOption::descriptor)
                .collect(),
            placement_side: self.placement_side,
            placement_alignment: self.placement_alignment,
            outside_press_policy: self.outside_press_policy,
            initial_focus_intent: self.initial_focus_intent.clone(),
            focus_restore_intent: self.focus_restore_intent.clone(),
            tokens: self.tokens,
        });
        let selected_label = state.selected_label().unwrap_or_default().to_owned();
        let selected_label_changed = runtime.update(cx, |runtime, _| {
            if let Some(value) = state.selected_value()
                && let Some(label) = state.selected_label()
            {
                runtime.retained_selection = Some(ComboboxSelection::new(value, label));
            }
            runtime.sync_selected_label(state.selected_label())
        });
        let should_project_selected_label = self.query.is_none()
            && !composing
            && (runtime_state.pending_selection_projection
                || (runtime_state.input_value_source
                    == ComboboxInputValueSource::SelectedLabelProjection
                    && selected_label_changed));
        if should_project_selected_label {
            runtime.update(cx, |runtime, _| {
                runtime.input_value_source = ComboboxInputValueSource::SelectedLabelProjection;
                runtime.pending_selection_projection = false;
            });
        } else if self.query.is_some() {
            runtime.update(cx, |runtime, _| {
                runtime.pending_selection_projection = false
            });
        }
        input_controller.update(cx, |controller, cx| {
            if should_project_selected_label && controller.value() != selected_label {
                controller.set_value(selected_label.clone(), cx);
            }
            if controller.placeholder() != self.placeholder.as_ref() {
                controller.set_placeholder(self.placeholder.clone(), cx);
            }
            if !composing && self.query.is_some() {
                controller.sync_adapter_state(
                    Some(&query),
                    None,
                    self.disabled,
                    false,
                    controller.display_mode(),
                    None,
                );
            }
        });

        let plan = ComboboxRenderPlan::from_state(self.id, &state);
        let window_overlay_runtime = WindowOverlayRuntime::for_window(window, cx);
        let ownership = if open_state.controlled() {
            OverlayOwnership::Controlled
        } else {
            OverlayOwnership::Uncontrolled
        };
        let overlay_policy = if composing {
            state
                .overlay()
                .policy()
                .clone()
                .with_escape_key_policy(EscapeKeyPolicy::Ignore)
        } else {
            state.overlay().policy().clone()
        };
        let mut registration = OverlayLayerRegistration::new(
            format!("combobox:{}", plan.debug_id),
            overlay_policy,
            ownership,
        )
        .portal_anchored();
        if let Some(on_open_change) = self.on_open_change.clone() {
            registration = registration.on_open_change(move |intent, window, cx| {
                on_open_change(intent, window, cx);
            });
        }
        if ownership == OverlayOwnership::Uncontrolled {
            let runtime = runtime.downgrade();
            registration = registration.uncontrolled_commit(move |open, _, cx| {
                let _ = runtime.update(cx, |runtime, _| {
                    runtime.open = open;
                });
            });
        }
        let existing_binding = runtime.read(cx).overlay_binding.clone();
        let editor_focus = input_controller.focus_handle(cx);
        let overlay_binding = window_overlay_runtime
            .bind_component_layer_with_trigger_focus(
                &runtime,
                existing_binding.as_ref(),
                registration,
                editor_focus,
                window,
                cx,
            )
            .expect("Combobox overlay registration should remain valid");
        if existing_binding.is_none() {
            runtime.update(cx, |runtime, _| {
                runtime.overlay_binding = Some(overlay_binding.clone());
            });
        }
        let open = plan.open;
        let disabled = plan.disabled;
        let toggle_focus_shadow = focus_ring_shadow_with_theme(plan.focus_ring, &theme);
        let overlay_adapter = gpui_overlay_state(state.overlay());
        let placement_content_size = ui_size(plan.metrics.popup_min_width(), plan.input_height);
        let placement_side = state.placement_side();
        let placement_alignment = state.placement_alignment();
        let portal_anchor = overlay_binding
            .portal_anchor()
            .expect("Combobox registration must own a portal anchor");
        let input_semantics = SemanticDescriptor::new(plan.input_role)
            .with_label(plan.label.as_ref())
            .with_expanded(open)
            .with_disabled(disabled);
        let toggle_semantics = SemanticDescriptor::new(Role::Button)
            .with_label("Toggle combobox popup")
            .with_expanded(open)
            .with_disabled(disabled)
            .with_actions(&[AccessibleAction::Click, AccessibleAction::Focus]);

        div()
            .id(plan.root_id.clone())
            .debug_selector({
                let debug_id = plan.debug_id.clone();
                move || format!("combobox:{debug_id}:root")
            })
            .relative()
            .flex()
            .flex_col()
            .items_start()
            .child(
                window_overlay_runtime
                    .inside_region(
                        &overlay_binding,
                        OverlayInsideRegionId::new("trigger"),
                        format!("combobox:{}:trigger-region", plan.debug_id),
                        div()
                            .id(plan.input_row_id.clone())
                            .debug_selector({
                                let debug_id = plan.debug_id.clone();
                                move || format!("combobox:{debug_id}:input-row")
                            })
                            .min_w(gpui_px_from_ui(plan.metrics.popup_min_width()))
                            .max_w(gpui_px_from_ui(plan.metrics.popup_max_width()))
                            .flex()
                            .items_center()
                            .gap_1()
                            .ui_semantics(&input_semantics)
                            .on_key_down({
                                let runtime = runtime.clone();
                                let input_controller = input_controller.clone();
                                let on_select = self.on_select.clone();
                                let key_state = state.clone();
                                let window_overlay_runtime = window_overlay_runtime.clone();
                                let overlay_binding = overlay_binding.clone();
                                move |event: &KeyDownEvent, window, cx| {
                                    if event.prefer_character_input
                                        || event.keystroke.modifiers.modified()
                                        || window.default_prevented()
                                        || input_controller.read(cx).marked_range_utf16().is_some()
                                    {
                                        return;
                                    }
                                    let current_value =
                                        runtime.read(cx).active_value.clone().or_else(|| {
                                            key_state.active_value().map(str::to_owned)
                                        });
                                    match combobox_keyboard_action_from_value(
                                        &key_state,
                                        event.keystroke.key.as_str(),
                                        current_value.as_deref(),
                                    ) {
                                        ComboboxKeyboardAction::Navigate(value) => {
                                            cx.stop_propagation();
                                            window.prevent_default();
                                            if !key_state.open() {
                                                let effect_runtime = runtime.clone();
                                                window_overlay_runtime
                                            .request_open_change_with_effect(
                                                &overlay_binding,
                                                true,
                                                DismissReason::Trigger,
                                                window,
                                                cx,
                                                move |_, cx| {
                                                    effect_runtime.update(cx, |runtime, cx| {
                                                        runtime.set_active(value, cx);
                                                    });
                                                },
                                            )
                                            .expect(
                                                "Combobox navigation should own its registration",
                                            );
                                            } else {
                                                runtime.update(cx, |runtime, cx| {
                                                    runtime.set_active(value, cx);
                                                });
                                            }
                                        }
                                        ComboboxKeyboardAction::Select(selection) => {
                                            cx.stop_propagation();
                                            window.prevent_default();
                                            let selected_label = selection.label().to_owned();
                                            let committed_selection = selection.clone();
                                            let input_controller = input_controller.clone();
                                            let on_select = on_select.clone();
                                            let effect_runtime = runtime.clone();
                                            request_registered_choice_selection(
                                                &window_overlay_runtime,
                                                &overlay_binding,
                                                window,
                                                cx,
                                                move |window, cx| {
                                                    let input_user_edit_revision = input_controller
                                                        .read(cx)
                                                        .user_edit_revision();
                                                    let committed =
                                                        effect_runtime.update(cx, |runtime, _| {
                                                            runtime.commit_selection(
                                                                committed_selection,
                                                                input_user_edit_revision,
                                                            )
                                                        });
                                                    if committed {
                                                        input_controller.update(
                                                            cx,
                                                            |controller, cx| {
                                                                controller
                                                                    .set_value(selected_label, cx);
                                                            },
                                                        );
                                                    }
                                                    if let Some(on_select) = on_select.as_ref() {
                                                        on_select(selection, window, cx);
                                                    }
                                                },
                                            );
                                        }
                                        ComboboxKeyboardAction::Open => {
                                            cx.stop_propagation();
                                            window.prevent_default();
                                            if !key_state.open() {
                                                window_overlay_runtime
                                                    .request_open_change(
                                                        &overlay_binding,
                                                        true,
                                                        DismissReason::Trigger,
                                                        window,
                                                        cx,
                                                    )
                                                    .expect(
                                                        "Combobox open should own its registration",
                                                    );
                                            }
                                        }
                                        ComboboxKeyboardAction::Ignore => {}
                                    }
                                }
                            })
                            .child(
                                TextInput::new(plan.input_id.clone(), plan.label.clone())
                                    .controller(input_controller.clone())
                                    .placeholder(plan.placeholder.clone())
                                    .value(query)
                                    .disabled(state.disabled())
                                    .required(state.required())
                                    .tokens(self.tokens)
                                    .with_size(state.size()),
                            )
                            .child(
                                div()
                                    .id(plan.toggle_id.clone())
                                    .debug_selector({
                                        let debug_id = plan.debug_id.clone();
                                        move || format!("combobox:{debug_id}:toggle")
                                    })
                                    .px_2()
                                    .py_1()
                                    .rounded(gpui_px_from_ui(plan.input_radius))
                                    .border_1()
                                    .border_color(theme.resolve(plan.colors.popup_border()))
                                    .text_color(theme.resolve(plan.colors.popup_foreground()))
                                    .ui_semantics(&toggle_semantics)
                                    .focus_visible(move |style| style.shadow(toggle_focus_shadow))
                                    .focusable()
                                    .tab_stop(!disabled)
                                    .when(disabled, |this| this.opacity(0.56).cursor_not_allowed())
                                    .when(!disabled, |this| {
                                        let input_controller = input_controller.clone();
                                        let window_overlay_runtime = window_overlay_runtime.clone();
                                        let overlay_binding = overlay_binding.clone();
                                        this.cursor_pointer().on_click(move |_event, window, cx| {
                                            cx.stop_propagation();
                                            window.prevent_default();
                                            input_controller.focus_handle(cx).focus(window, cx);
                                            window_overlay_runtime
                                                .request_open_change(
                                                    &overlay_binding,
                                                    !open,
                                                    DismissReason::Trigger,
                                                    window,
                                                    cx,
                                                )
                                                .expect(
                                                    "Combobox toggle should own its registration",
                                                );
                                        })
                                    })
                                    .child(if open { "^" } else { "v" }),
                            ),
                    )
                    .track_portal_anchor(&portal_anchor),
            )
            .when(open, |this| {
                let content_runtime = window_overlay_runtime.clone();
                let content_binding = overlay_binding.clone();
                this.child(gpui_portal_anchor_overlay_layer(
                    &overlay_adapter,
                    &window_overlay_runtime,
                    &overlay_binding,
                    window,
                    cx,
                    move |anchor| {
                        OverlayPlacementInput::new(anchor, placement_content_size)
                            .with_side(placement_side)
                            .with_alignment(placement_alignment)
                            .with_offset(ui_px(4.0))
                    },
                    move |opening_theme, window, cx| {
                        combobox_content_element(
                            plan.content_id.clone(),
                            plan.listbox_id.clone(),
                            plan.debug_id.clone(),
                            state.clone(),
                            content_runtime,
                            content_binding,
                            input_controller.clone(),
                            runtime.clone(),
                            self.on_select.clone(),
                            self.tokens,
                            opening_theme,
                            self.results_content,
                            self.option_content,
                            window,
                            cx,
                        )
                        .into_any_element()
                    },
                ))
            })
    }
}

#[allow(clippy::too_many_arguments)]
fn combobox_content_element(
    content_id: ElementId,
    listbox_id: ElementId,
    debug_id: String,
    state: ComboboxState,
    window_overlay_runtime: WindowOverlayRuntime,
    overlay_binding: OverlayLayerBinding,
    input_controller: Entity<TextInputController>,
    runtime: Entity<ComboboxRuntime>,
    on_select: Option<ComboboxSelectionHandler>,
    tokens: ThemeTokens,
    theme: &ThemeContext,
    results_content: Option<ComboboxResultsContent>,
    option_content: Option<ListboxRowContentRenderer>,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let metrics = state.metrics();
    let colors = state.colors();
    let selected_value = state.selected_value().map(str::to_owned);
    let active_value = state.active_value().map(str::to_owned);
    let label = state.label().to_owned();
    let listbox_window_overlay_runtime = window_overlay_runtime.clone();
    let listbox_overlay_binding = overlay_binding.clone();
    let listbox_state = state.listbox();
    let standalone_count = listbox_state
        .options()
        .partition_point(|option| option.group_index().is_none());
    let (standalone_options, mut grouped_options) =
        listbox_state.options().split_at(standalone_count);
    let project_option = |option: &ListboxOptionState| {
        ListboxOption::new(option.value(), option.label().to_owned()).disabled(option.disabled())
    };
    let listbox = Listbox::new(listbox_id, label.clone())
        .options(standalone_options.iter().map(project_option))
        .groups(listbox_state.groups().iter().map(|group| {
            let count = grouped_options
                .partition_point(|option| option.group_index() == Some(group.index()));
            let (options, remaining) = grouped_options.split_at(count);
            grouped_options = remaining;
            ListboxGroup::new(group.value(), group.label().to_owned())
                .options(options.iter().map(project_option))
        }))
        .tokens(tokens)
        .with_size(state.size())
        .empty_label(state.empty_label().to_owned())
        .embedded(true)
        .editor_owned_focus()
        .selection_transaction({
            let input_controller = input_controller.clone();
            let runtime = runtime.clone();
            move |intent, window, cx| {
                let payload_value = intent.selection().value().to_owned();
                let payload_label = intent.selection().label().to_owned();
                let input_controller = input_controller.clone();
                let runtime = runtime.clone();
                request_registered_choice_selection(
                    &listbox_window_overlay_runtime,
                    &listbox_overlay_binding,
                    window,
                    cx,
                    move |window, cx| {
                        let input_user_edit_revision =
                            input_controller.read(cx).user_edit_revision();
                        let committed = runtime.update(cx, |runtime, _| {
                            runtime.commit_selection(
                                ComboboxSelection::new(payload_value, payload_label.clone()),
                                input_user_edit_revision,
                            )
                        });
                        if committed {
                            input_controller.update(cx, |controller, cx| {
                                controller.set_value(payload_label, cx);
                            });
                        }
                        intent.deliver(window, cx);
                    },
                );
            }
        });
    let listbox = if let Some(on_select) = on_select {
        listbox.on_select(move |selection, window, cx| {
            on_select(
                ComboboxSelection::new(selection.value().to_owned(), selection.label().to_owned()),
                window,
                cx,
            );
        })
    } else {
        listbox
    };
    let listbox = listbox.selected(selected_value);
    let listbox = if let Some(active_value) = active_value {
        listbox.projected_active(active_value)
    } else {
        listbox
    };
    let listbox = listbox.when_some(option_content, |listbox, render| {
        listbox.row_content(move |option, window, cx| render(option, window, cx))
    });
    let scroll_viewport_id = state.scroll_area().viewport_id().to_owned();
    let content_semantics =
        SemanticDescriptor::new(state.content_role()).with_label(label.as_ref());
    let viewport = ScrollArea::new(scroll_viewport_id, listbox)
        .vertical()
        .preserve_scroll()
        .with_size(state.size())
        .into_any_element();
    let content = if let Some(render) = results_content {
        render(&state, viewport, window, cx)
    } else {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .when_some(
                state.results_status().message().filter(|_| !state.empty()),
                |this, message| {
                    this.child(
                        div()
                            .id((content_id.clone(), "status"))
                            .ui_semantics(
                                &SemanticDescriptor::new(Role::Status).with_label(message),
                            )
                            .child(message.to_owned()),
                    )
                },
            )
            .child(viewport)
            .into_any_element()
    };

    window_overlay_runtime.surface(
        &overlay_binding,
        OverlayInsideRegionId::new("surface"),
        format!("combobox:{debug_id}:surface-region"),
        div()
            .id(content_id)
            .debug_selector(move || format!("combobox:{debug_id}:content"))
            .min_w(gpui_px_from_ui(metrics.popup_min_width()))
            .max_w(gpui_px_from_ui(metrics.popup_max_width()))
            .p(gpui_px_from_ui(metrics.popup_padding()))
            .h(gpui_px_from_ui(metrics.popup_max_height()))
            .flex()
            .flex_col()
            .rounded(gpui_px_from_ui(metrics.popup_radius()))
            .border_1()
            .border_color(theme.resolve(colors.popup_border()))
            .bg(theme.resolve(colors.popup_background()))
            .text_color(theme.resolve(colors.popup_foreground()))
            .shadow(gpui_elevation_shadow(
                ThemeResolver::overlay_surface_elevation(theme),
            ))
            .occlude()
            .ui_semantics(&content_semantics)
            .child(content),
    )
}
