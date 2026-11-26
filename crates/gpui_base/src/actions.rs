use std::rc::Rc;

use open_gpui::{
    AccessibleAction, App, ClickEvent, Div, ElementId, IntoElement, Role, SharedString, Stateful,
    StyleRefinement, Toggled, Window, div, prelude::*,
};

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

fn activate(
    element: Stateful<Div>,
    disabled: bool,
    handler: Option<ClickHandler>,
) -> Stateful<Div> {
    let element = element.aria_disabled(disabled);
    match handler.filter(|_| !disabled) {
        Some(handler) => {
            let accessible = handler.clone();
            element
                .focusable()
                .tab_stop(true)
                .cursor_pointer()
                .on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    handler(event.window_event(), window, cx);
                })
                .on_a11y_action(AccessibleAction::Click, move |_, window, cx| {
                    accessible(&ClickEvent::default(), window, cx);
                })
        }
        None => element,
    }
}

/// A labeled action with GPUI keyboard, pointer, and accessibility activation.
#[derive(IntoElement)]
pub struct Button {
    element: Stateful<Div>,
    disabled: bool,
    handler: Option<ClickHandler>,
}

impl Button {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Self {
        let label = label.into();
        Self::with_content(id, label.clone(), label)
    }

    pub fn with_content(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        content: impl IntoElement,
    ) -> Self {
        Self {
            element: div()
                .id(id)
                .role(Role::Button)
                .aria_label(label)
                .child(content),
            disabled: false,
            handler: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.element = self.element.aria_expanded(expanded);
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handler = Some(Rc::new(handler));
        self
    }
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        self.element.style()
    }
}

impl RenderOnce for Button {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        activate(self.element, self.disabled, self.handler)
    }
}

/// A button with caller-supplied icon content and an accessible label.
#[derive(IntoElement)]
pub struct IconButton(Button);

impl IconButton {
    pub fn new(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        icon: impl IntoElement,
    ) -> Self {
        Self(Button::with_content(id, label, icon))
    }

    pub fn disabled(self, disabled: bool) -> Self {
        Self(self.0.disabled(disabled))
    }

    pub fn on_click(self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        Self(self.0.on_click(handler))
    }
}

impl Styled for IconButton {
    fn style(&mut self) -> &mut StyleRefinement {
        self.0.style()
    }
}

impl RenderOnce for IconButton {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.0
    }
}

/// A controlled checkbox. The caller owns the checked value.
#[derive(IntoElement)]
pub struct Checkbox {
    element: Stateful<Div>,
    checked: bool,
    disabled: bool,
    handler: Option<ClickHandler>,
}

impl Checkbox {
    pub fn new(id: impl Into<ElementId>, label: impl Into<SharedString>, checked: bool) -> Self {
        let label = label.into();
        Self::with_content(
            id,
            label.clone(),
            checked,
            div()
                .flex()
                .gap_1()
                .child(if checked { "☑" } else { "☐" })
                .child(label),
        )
    }

    pub fn with_content(
        id: impl Into<ElementId>,
        label: impl Into<SharedString>,
        checked: bool,
        content: impl IntoElement,
    ) -> Self {
        let toggled = if checked {
            Toggled::True
        } else {
            Toggled::False
        };
        Self {
            element: div()
                .id(id)
                .role(Role::CheckBox)
                .aria_label(label)
                .aria_toggled(toggled)
                .child(content),
            checked,
            disabled: false,
            handler: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_toggle(mut self, handler: impl Fn(bool, &mut Window, &mut App) + 'static) -> Self {
        let checked = self.checked;
        self.handler = Some(Rc::new(move |_, window, cx| handler(!checked, window, cx)));
        self
    }
}

impl Styled for Checkbox {
    fn style(&mut self) -> &mut StyleRefinement {
        self.element.style()
    }
}

impl RenderOnce for Checkbox {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        activate(self.element, self.disabled, self.handler)
    }
}

/// A single row with caller-supplied content and optional selection semantics.
#[derive(IntoElement)]
pub struct Row {
    element: Stateful<Div>,
    handler: Option<ClickHandler>,
}

impl Row {
    pub fn new(id: impl Into<ElementId>, content: impl IntoElement) -> Self {
        Self {
            element: div().id(id).role(Role::ListItem).child(content),
            handler: None,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.element = self.element.aria_selected(selected);
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.handler = Some(Rc::new(handler));
        self
    }
}

impl Styled for Row {
    fn style(&mut self) -> &mut StyleRefinement {
        self.element.style()
    }
}

impl RenderOnce for Row {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        activate(self.element, false, self.handler)
    }
}
