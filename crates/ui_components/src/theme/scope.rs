use std::{cell::RefCell, rc::Rc};

use open_gpui::{
    AnyElement, AnyView, App, AppContext, Bounds, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Render, Window,
};

use super::runtime::{ThemeContext, theme_scope_stack};

/// An explicit immutable theme override for one GPUI element subtree.
pub struct ThemeScope {
    id: ElementId,
    context: ThemeContext,
    child: Option<AnyElement>,
    source: &'static core::panic::Location<'static>,
}

impl ThemeScope {
    /// Creates a nearest-provider theme scope around one child subtree.
    #[track_caller]
    pub fn new(
        id: impl Into<ElementId>,
        context: impl Into<ThemeContext>,
        child: impl IntoElement,
    ) -> Self {
        Self {
            id: id.into(),
            context: context.into(),
            child: Some(child.into_any_element()),
            source: core::panic::Location::caller(),
        }
    }
}

impl Element for ThemeScope {
    type RequestLayoutState = (AnyElement, bool);
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        Some(self.source)
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let id = id.expect("theme scopes always provide a stable element id");
        let context_changed =
            window.with_element_state(id, |previous: Option<ThemeScopeElementState>, _| {
                let changed = previous
                    .as_ref()
                    .is_none_or(|previous| previous.context != self.context);
                (
                    changed,
                    ThemeScopeElementState {
                        context: self.context.clone(),
                    },
                )
            });
        let mut child = self.child.take().expect("theme scope child missing");
        let layout_id = with_theme_scope(self.context.clone(), window, cx, |window, cx| {
            child.request_layout(window, cx)
        });
        (layout_id, (child, context_changed))
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        with_theme_scope(self.context.clone(), window, cx, |window, cx| {
            window.with_cached_view_refresh(state.1, |window| state.0.prepaint(window, cx))
        });
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        with_theme_scope(self.context.clone(), window, cx, |window, cx| {
            state.0.paint(window, cx)
        });
    }
}

struct ThemeScopeElementState {
    context: ThemeContext,
}

impl IntoElement for ThemeScope {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

struct ThemeScopeGuard {
    stack: Rc<RefCell<Vec<ThemeContext>>>,
    entered_depth: usize,
}

impl Drop for ThemeScopeGuard {
    fn drop(&mut self) {
        let mut stack = self.stack.borrow_mut();
        if !std::thread::panicking() {
            debug_assert_eq!(stack.len(), self.entered_depth + 1);
        }
        stack.truncate(self.entered_depth);
    }
}

fn with_theme_scope<R>(
    context: ThemeContext,
    window: &mut Window,
    cx: &mut App,
    f: impl FnOnce(&mut Window, &mut App) -> R,
) -> R {
    let stack = theme_scope_stack(window, cx);
    let entered_depth = stack.borrow().len();
    stack.borrow_mut().push(context);
    let _guard = ThemeScopeGuard {
        stack,
        entered_depth,
    };
    f(window, cx)
}

struct ScopedThemeView {
    context: ThemeContext,
    child: AnyView,
}

impl Render for ScopedThemeView {
    fn render(
        &mut self,
        _window: &mut Window,
        _cx: &mut open_gpui::Context<Self>,
    ) -> impl IntoElement {
        ThemeScope::new(
            "scoped-theme-view",
            self.context.clone(),
            self.child.clone(),
        )
    }
}

pub(crate) fn scoped_theme_view(context: ThemeContext, child: AnyView, cx: &mut App) -> AnyView {
    cx.new(|_| ScopedThemeView { context, child }).into()
}

pub(crate) fn scoped_theme_view_builder(
    context: ThemeContext,
    build: impl Fn(&mut Window, &mut App) -> AnyView + 'static,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    move |window, cx| {
        let child = with_theme_scope(context.clone(), window, cx, |window, cx| build(window, cx));
        scoped_theme_view(context.clone(), child, cx)
    }
}
