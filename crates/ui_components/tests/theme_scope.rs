use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
    time::Duration,
};

use open_gpui::{
    AnyView, App, AppContext, Bounds, Context, Element, Empty, Entity, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, ParentElement, Pixels, Render, RenderOnce, Style,
    StyleRefinement, Styled, Window, deferred, div, point, px, size,
};
use open_gpui_ui_components::theme::{
    DARK_THEME_ID, LIGHT_THEME_ID, ThemeColor, ThemeContext, ThemeDefinition, ThemeMode,
    ThemeRegistry, ThemeResolver, ThemeScope, ThemeSelectionError, ThemeSnapshot,
    clear_window_theme, install_theme_registry, override_window_theme, register_theme,
    set_app_theme, set_window_theme,
};
use open_gpui_ui_components::{Button, ColorIntent, ColorState, IconButton, Popover};
use open_gpui_ui_core::semantic;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbePhase {
    Builder,
    Render,
    RequestLayout,
    Prepaint,
    Paint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ThemeObservation {
    label: &'static str,
    phase: ProbePhase,
    mode: ThemeMode,
    revision: u64,
}

type Observations = Rc<RefCell<Vec<ThemeObservation>>>;

fn record_theme(
    observations: &Observations,
    label: &'static str,
    phase: ProbePhase,
    theme: &ThemeContext,
) {
    observations.borrow_mut().push(ThemeObservation {
        label,
        phase,
        mode: theme.mode(),
        revision: theme.revision(),
    });
}

#[derive(IntoElement)]
struct RenderThemeProbe {
    label: &'static str,
    observations: Observations,
}

impl RenderOnce for RenderThemeProbe {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(window, cx);
        record_theme(&self.observations, self.label, ProbePhase::Render, &theme);
        Empty
    }
}

struct PhaseThemeProbe {
    label: &'static str,
    observations: Observations,
}

impl Element for PhaseThemeProbe {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<open_gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let theme = ThemeResolver::current(window, cx);
        record_theme(
            &self.observations,
            self.label,
            ProbePhase::RequestLayout,
            &theme,
        );
        (window.request_layout(Style::default(), [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let theme = ThemeResolver::current(window, cx);
        record_theme(&self.observations, self.label, ProbePhase::Prepaint, &theme);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let theme = ThemeResolver::current(window, cx);
        record_theme(&self.observations, self.label, ProbePhase::Paint, &theme);
    }
}

impl IntoElement for PhaseThemeProbe {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

#[derive(IntoElement)]
struct DeferredOpeningProbe {
    observations: Observations,
}

impl RenderOnce for DeferredOpeningProbe {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let opening_theme = ThemeResolver::current(window, cx);
        deferred(ThemeScope::new(
            "deferred-opening-theme",
            opening_theme,
            PhaseThemeProbe {
                label: "deferred",
                observations: self.observations,
            },
        ))
    }
}

struct NestedScopeView {
    observations: Observations,
}

impl Render for NestedScopeView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .child(RenderThemeProbe {
                label: "root-before",
                observations: self.observations.clone(),
            })
            .child(ThemeScope::new(
                "outer-theme",
                ThemeContext::dark(),
                div()
                    .child(RenderThemeProbe {
                        label: "outer-before",
                        observations: self.observations.clone(),
                    })
                    .child(ThemeScope::new(
                        "nested-theme",
                        ThemeContext::high_contrast(),
                        RenderThemeProbe {
                            label: "nested",
                            observations: self.observations.clone(),
                        },
                    ))
                    .child(RenderThemeProbe {
                        label: "outer-after",
                        observations: self.observations.clone(),
                    })
                    .child(DeferredOpeningProbe {
                        observations: self.observations.clone(),
                    }),
            ))
            .child(RenderThemeProbe {
                label: "root-after",
                observations: self.observations.clone(),
            })
    }
}

#[open_gpui::test]
fn nested_scopes_restore_siblings_and_deferred_phases_keep_the_opening_snapshot(
    cx: &mut open_gpui::TestAppContext,
) {
    let observations = Observations::default();
    let (_, cx) = cx.add_window_view({
        let observations = observations.clone();
        move |_, _| NestedScopeView { observations }
    });

    observations.borrow_mut().clear();
    cx.update(|window, cx| window.draw(cx).clear());

    let observations = observations.borrow();
    let expected = [
        ("root-before", ProbePhase::Render, ThemeMode::Light),
        ("outer-before", ProbePhase::Render, ThemeMode::Dark),
        ("nested", ProbePhase::Render, ThemeMode::HighContrast),
        ("outer-after", ProbePhase::Render, ThemeMode::Dark),
        ("deferred", ProbePhase::RequestLayout, ThemeMode::Dark),
        ("root-after", ProbePhase::Render, ThemeMode::Light),
        ("deferred", ProbePhase::Prepaint, ThemeMode::Dark),
        ("deferred", ProbePhase::Paint, ThemeMode::Dark),
    ];

    assert_eq!(observations.len(), expected.len());
    for (actual, (label, phase, mode)) in observations.iter().zip(expected) {
        assert_eq!(
            (actual.label, actual.phase, actual.mode),
            (label, phase, mode)
        );
    }
}

struct WindowThemeProbe {
    observations: Observations,
}

impl Render for WindowThemeProbe {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = ThemeResolver::current(window, cx);
        record_theme(&self.observations, "window", ProbePhase::Render, &theme);
        Empty
    }
}

#[open_gpui::test]
fn app_window_and_explicit_override_precedence_is_isolated_between_windows(
    cx: &mut open_gpui::TestAppContext,
) {
    cx.update(|app| set_app_theme(app, DARK_THEME_ID).expect("built-in app theme should resolve"));

    let first_observations = Observations::default();
    let second_observations = Observations::default();
    let inherited_observations = Observations::default();
    let first = cx
        .open_window(size(px(320.0), px(200.0)), {
            let observations = first_observations.clone();
            move |_, _| WindowThemeProbe { observations }
        })
        .into();
    let second = cx
        .open_window(size(px(320.0), px(200.0)), {
            let observations = second_observations.clone();
            move |_, _| WindowThemeProbe { observations }
        })
        .into();
    let inherited = cx
        .open_window(size(px(320.0), px(200.0)), {
            let observations = inherited_observations.clone();
            move |_, _| WindowThemeProbe { observations }
        })
        .into();

    cx.update_window(first, |_, window, cx| {
        set_window_theme(window, cx, LIGHT_THEME_ID)
            .expect("built-in window selection should resolve");
        window.draw(cx).clear();
    })
    .expect("first window should remain open");
    cx.update_window(second, |_, window, cx| {
        override_window_theme(window, cx, ThemeContext::high_contrast());
        window.draw(cx).clear();
    })
    .expect("second window should remain open");
    cx.update_window(inherited, |_, window, cx| window.draw(cx).clear())
        .expect("inherited window should remain open");

    assert_eq!(
        first_observations.borrow().last().unwrap().mode,
        ThemeMode::Light
    );
    assert_eq!(
        second_observations.borrow().last().unwrap().mode,
        ThemeMode::HighContrast
    );
    assert_eq!(
        inherited_observations.borrow().last().unwrap().mode,
        ThemeMode::Dark
    );

    first_observations.borrow_mut().clear();
    second_observations.borrow_mut().clear();
    inherited_observations.borrow_mut().clear();
    cx.update(|app| set_app_theme(app, LIGHT_THEME_ID).expect("built-in app theme should resolve"));
    assert!(
        first_observations.borrow().is_empty(),
        "an app selection must not refresh a window with an explicit selection"
    );
    assert!(
        second_observations.borrow().is_empty(),
        "an app selection must not refresh a window with an explicit override"
    );
    assert_eq!(
        inherited_observations.borrow().as_slice(),
        &[ThemeObservation {
            label: "window",
            phase: ProbePhase::Render,
            mode: ThemeMode::Light,
            revision: ThemeSnapshot::light().revision(),
        }],
        "only the inheriting window should rerender for an app selection"
    );

    cx.update(|app| set_app_theme(app, DARK_THEME_ID).expect("built-in app theme should resolve"));

    assert!(cx.simulate_window_close(first));
    let replacement_observations = Observations::default();
    let replacement = cx
        .open_window(size(px(320.0), px(200.0)), {
            let observations = replacement_observations.clone();
            move |_, _| WindowThemeProbe { observations }
        })
        .into();
    cx.update_window(replacement, |_, window, cx| window.draw(cx).clear())
        .expect("replacement window should remain open");
    assert_eq!(
        replacement_observations.borrow().last().unwrap().mode,
        ThemeMode::Dark,
        "closed-window selection must not leak into a new window"
    );
}

#[open_gpui::test]
fn unknown_and_noop_selections_preserve_context_without_refresh(
    cx: &mut open_gpui::TestAppContext,
) {
    cx.update(|app| set_app_theme(app, DARK_THEME_ID).expect("built-in app theme should resolve"));
    let observations = Observations::default();
    let (_, cx) = cx.add_window_view({
        let observations = observations.clone();
        move |_, _| WindowThemeProbe { observations }
    });
    cx.update(|window, cx| window.draw(cx).clear());
    observations.borrow_mut().clear();

    cx.update(|window, cx| {
        let before = ThemeResolver::current(window, cx);
        set_app_theme(cx, DARK_THEME_ID).expect("reselecting the app theme should succeed");
        assert_eq!(
            set_app_theme(cx, "missing-app-theme").unwrap_err(),
            ThemeSelectionError::UnknownThemeId("missing-app-theme".to_owned())
        );
        assert_eq!(ThemeResolver::current(window, cx), before);
    });
    assert!(
        observations.borrow().is_empty(),
        "app no-op and failed selections must not refresh the window"
    );

    cx.update(|window, cx| {
        set_window_theme(window, cx, LIGHT_THEME_ID)
            .expect("built-in window selection should resolve");
    });
    observations.borrow_mut().clear();
    cx.update(|window, cx| {
        let before = ThemeResolver::current(window, cx);
        set_window_theme(window, cx, LIGHT_THEME_ID)
            .expect("reselecting the window theme should succeed");
        assert_eq!(
            set_window_theme(window, cx, "missing-window-theme").unwrap_err(),
            ThemeSelectionError::UnknownThemeId("missing-window-theme".to_owned())
        );
        assert_eq!(ThemeResolver::current(window, cx), before);
    });
    assert!(
        observations.borrow().is_empty(),
        "window no-op and failed selections must not refresh the window"
    );
}

#[open_gpui::test]
fn unknown_window_selection_is_atomic_and_clear_returns_to_app_selection(
    cx: &mut open_gpui::TestAppContext,
) {
    cx.update(|app| set_app_theme(app, DARK_THEME_ID).expect("built-in app theme should resolve"));
    let (_, cx) = cx.add_window_view(|_, _| Empty);

    cx.update(|window, cx| {
        set_window_theme(window, cx, LIGHT_THEME_ID)
            .expect("built-in window selection should resolve");
        let before = ThemeResolver::current(window, cx);
        assert_eq!(before.mode(), ThemeMode::Light);

        assert_eq!(
            set_window_theme(window, cx, "missing-theme").unwrap_err(),
            ThemeSelectionError::UnknownThemeId("missing-theme".to_owned())
        );
        let after_failure = ThemeResolver::current(window, cx);
        assert_eq!(after_failure, before);

        clear_window_theme(window, cx);
        assert_eq!(ThemeResolver::current(window, cx).mode(), ThemeMode::Dark);
    });
}

#[open_gpui::test]
fn app_selection_is_visible_in_the_current_window_transaction(cx: &mut open_gpui::TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| Empty);

    cx.update(|window, cx| {
        assert_eq!(ThemeResolver::current(window, cx).mode(), ThemeMode::Light);
        set_app_theme(cx, DARK_THEME_ID).expect("built-in app theme should resolve");
        assert_eq!(
            ThemeResolver::current(window, cx).mode(),
            ThemeMode::Dark,
            "the effective app selection must be readable before global observers flush"
        );
    });
}

#[open_gpui::test]
fn a_missing_window_selection_retains_its_last_known_snapshot_until_the_id_returns(
    cx: &mut open_gpui::TestAppContext,
) {
    cx.update(|app| {
        register_theme(
            app,
            ThemeDefinition::new("brand", "Brand", ThemeMode::HighContrast, 1),
        )
        .expect("brand theme should register");
    });
    let observations = Observations::default();
    let (_, cx) = cx.add_window_view({
        let observations = observations.clone();
        move |_, _| WindowThemeProbe { observations }
    });
    cx.update(|window, cx| {
        set_window_theme(window, cx, "brand").expect("brand window selection should resolve");
        window.draw(cx).clear();
    });
    observations.borrow_mut().clear();

    cx.update(|_, cx| {
        install_theme_registry(cx, ThemeRegistry::with_builtins(), LIGHT_THEME_ID)
            .expect("replacement registry should install");
    });
    assert!(
        observations.borrow().is_empty(),
        "temporarily removing a selected id must not refresh or demote its window authority"
    );
    cx.update(|window, cx| {
        assert_eq!(
            ThemeResolver::current(window, cx).mode(),
            ThemeMode::HighContrast
        );
    });

    cx.update(|_, cx| set_app_theme(cx, DARK_THEME_ID).expect("app theme should change"));
    assert!(
        observations.borrow().is_empty(),
        "app selection changes must not affect a window retaining an explicit selection"
    );

    cx.update(|_, cx| {
        register_theme(
            cx,
            ThemeDefinition::new("brand", "Brand v2", ThemeMode::Dark, 2),
        )
        .expect("replacement brand theme should register");
    });
    assert_eq!(
        observations.borrow().as_slice(),
        &[ThemeObservation {
            label: "window",
            phase: ProbePhase::Render,
            mode: ThemeMode::Dark,
            revision: 2,
        }],
        "the selected window should refresh exactly once when its id returns with new content"
    );
}

struct EarlyReturnThemeProbe {
    observations: Observations,
}

impl Element for EarlyReturnThemeProbe {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<open_gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let theme = ThemeResolver::current(window, cx);
        record_theme(
            &self.observations,
            "early-return",
            ProbePhase::RequestLayout,
            &theme,
        );
        return (window.request_layout(Style::default(), [], cx), ());
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}

impl IntoElement for EarlyReturnThemeProbe {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

struct EarlyReturnScopeView {
    observations: Observations,
}

impl Render for EarlyReturnScopeView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .child(ThemeScope::new(
                "early-return-theme",
                ThemeContext::dark(),
                EarlyReturnThemeProbe {
                    observations: self.observations.clone(),
                },
            ))
            .child(PhaseThemeProbe {
                label: "sibling-after-early-return",
                observations: self.observations.clone(),
            })
    }
}

#[open_gpui::test]
fn early_return_restores_the_parent_theme_for_the_following_sibling(
    cx: &mut open_gpui::TestAppContext,
) {
    let observations = Observations::default();
    let (_, cx) = cx.add_window_view({
        let observations = observations.clone();
        move |_, _| EarlyReturnScopeView { observations }
    });

    observations.borrow_mut().clear();
    cx.update(|window, cx| window.draw(cx).clear());

    let observations = observations.borrow();
    assert!(observations.iter().any(|observation| {
        observation.label == "early-return"
            && observation.phase == ProbePhase::RequestLayout
            && observation.mode == ThemeMode::Dark
    }));
    assert!(observations.iter().any(|observation| {
        observation.label == "sibling-after-early-return"
            && observation.phase == ProbePhase::RequestLayout
            && observation.mode == ThemeMode::Light
    }));
    assert!(
        observations
            .iter()
            .filter(|observation| observation.label == "sibling-after-early-return")
            .all(|observation| observation.mode == ThemeMode::Light),
        "the completed scope must not leak into later sibling phases: {observations:?}"
    );
}

struct PanickingThemeProbe;

impl Element for PanickingThemeProbe {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<open_gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        assert_eq!(ThemeResolver::current(window, cx).mode(), ThemeMode::Dark);
        panic!("intentional theme-scope unwind")
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }
}

impl IntoElement for PanickingThemeProbe {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

#[open_gpui::test]
fn panic_unwind_restores_the_window_theme_scope_stack(cx: &mut open_gpui::TestAppContext) {
    let (_, cx) = cx.add_window_view(|_, _| Empty);

    cx.update(|window, cx| {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut scoped =
                ThemeScope::new("panic-theme", ThemeContext::dark(), PanickingThemeProbe)
                    .into_any_element();
            let _ = scoped.request_layout(window, cx);
        }));
        assert!(result.is_err());
        assert_eq!(
            ThemeResolver::current(window, cx).mode(),
            ThemeMode::Light,
            "unwinding a child must restore the previous ambient theme"
        );
    });
}

struct CachedThemeChild {
    observations: Observations,
}

impl Render for CachedThemeChild {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = ThemeResolver::current(window, cx);
        record_theme(
            &self.observations,
            "cached-child",
            ProbePhase::Render,
            &theme,
        );
        Empty
    }
}

struct CachedScopeView {
    context: ThemeContext,
    child: Entity<CachedThemeChild>,
}

impl Render for CachedScopeView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        ThemeScope::new(
            "cached-theme",
            self.context.clone(),
            AnyView::from(self.child.clone()).cached(StyleRefinement::default()),
        )
    }
}

#[open_gpui::test]
fn changing_a_subtree_scope_invalidates_a_cached_child_view(cx: &mut open_gpui::TestAppContext) {
    let observations = Observations::default();
    let (view, cx) = cx.add_window_view({
        let observations = observations.clone();
        move |_, cx| CachedScopeView {
            context: ThemeContext::dark(),
            child: cx.new(|_| CachedThemeChild { observations }),
        }
    });

    observations.borrow_mut().clear();
    view.update(cx, |view, cx| {
        view.context = ThemeContext::high_contrast();
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());

    assert_eq!(
        observations.borrow().as_slice(),
        &[ThemeObservation {
            label: "cached-child",
            phase: ProbePhase::Render,
            mode: ThemeMode::HighContrast,
            revision: ThemeSnapshot::high_contrast().revision(),
        }]
    );
}

struct PopoverThemeScopeView {
    context: ThemeContext,
    open: bool,
    observations: Observations,
}

impl Render for PopoverThemeScopeView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        ThemeScope::new(
            "popover-theme",
            self.context.clone(),
            Popover::element(
                "theme-scope-popover",
                "Open",
                PhaseThemeProbe {
                    label: "popover-surface",
                    observations: self.observations.clone(),
                },
            )
            .open(self.open),
        )
    }
}

fn assert_only_observed_mode(observations: &Observations, mode: ThemeMode) {
    let observations = observations.borrow();
    assert!(
        !observations.is_empty(),
        "the deferred popover surface should traverse at least one render phase"
    );
    assert!(
        observations
            .iter()
            .all(|observation| observation.mode == mode),
        "expected only {mode:?}, observed {observations:?}"
    );
}

#[open_gpui::test]
fn official_deferred_overlay_freezes_theme_for_one_open_generation(
    cx: &mut open_gpui::TestAppContext,
) {
    let observations = Observations::default();
    let (view, cx) = cx.add_window_view({
        let observations = observations.clone();
        move |_, _| PopoverThemeScopeView {
            context: ThemeContext::dark(),
            open: true,
            observations,
        }
    });

    observations.borrow_mut().clear();
    cx.update(|window, cx| window.draw(cx).clear());
    assert_only_observed_mode(&observations, ThemeMode::Dark);

    observations.borrow_mut().clear();
    view.update(cx, |view, cx| {
        view.context = ThemeContext::high_contrast();
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert_only_observed_mode(&observations, ThemeMode::Dark);

    view.update(cx, |view, cx| {
        view.open = false;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());

    observations.borrow_mut().clear();
    view.update(cx, |view, cx| {
        view.open = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert_only_observed_mode(&observations, ThemeMode::HighContrast);
}

type PaletteObservations = Rc<RefCell<Vec<(ProbePhase, u32)>>>;

struct PaletteThemeProbe {
    observations: PaletteObservations,
}

impl PaletteThemeProbe {
    fn record(&self, phase: ProbePhase, window: &mut Window, cx: &mut App) {
        let theme = ThemeResolver::current(window, cx);
        let color = u32::from(theme.resolve(ColorIntent::new(semantic::SURFACE, 0)));
        self.observations.borrow_mut().push((phase, color));
    }
}

impl Element for PaletteThemeProbe {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<open_gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.record(ProbePhase::RequestLayout, window, cx);
        (window.request_layout(Style::default(), [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.record(ProbePhase::Prepaint, window, cx);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.record(ProbePhase::Paint, window, cx);
    }
}

impl IntoElement for PaletteThemeProbe {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

struct PalettePopoverScopeView {
    context: ThemeContext,
    open: bool,
    observations: PaletteObservations,
}

impl Render for PalettePopoverScopeView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        ThemeScope::new(
            "palette-popover-theme",
            self.context.clone(),
            Popover::element(
                "palette-theme-scope-popover",
                "Open",
                PaletteThemeProbe {
                    observations: self.observations.clone(),
                },
            )
            .open(self.open),
        )
    }
}

fn assert_only_observed_palette(observations: &PaletteObservations, expected: u32) {
    let observations = observations.borrow();
    assert!(
        !observations.is_empty(),
        "the deferred surface must resolve at least one palette color"
    );
    assert!(
        observations.iter().all(|(_, color)| *color == expected),
        "expected only palette color {expected:#010x}, observed {observations:?}"
    );
}

#[open_gpui::test]
fn deferred_overlay_freezes_the_complete_palette_snapshot_for_one_open_generation(
    cx: &mut open_gpui::TestAppContext,
) {
    const OPENING_COLOR: u32 = 0x13579bff;
    const REOPENED_COLOR: u32 = 0x2468acff;
    let opening_colors = [ThemeColor::new(
        semantic::SURFACE,
        ColorState::Default,
        OPENING_COLOR >> 8,
    )];
    let reopened_colors = [ThemeColor::new(
        semantic::SURFACE,
        ColorState::Default,
        REOPENED_COLOR >> 8,
    )];
    let opening_context =
        ThemeContext::new(ThemeSnapshot::new(ThemeMode::Dark, 77, &opening_colors));
    let reopened_context =
        ThemeContext::new(ThemeSnapshot::new(ThemeMode::Dark, 77, &reopened_colors));
    let observations = PaletteObservations::default();
    let (view, cx) = cx.add_window_view({
        let observations = observations.clone();
        move |_, _| PalettePopoverScopeView {
            context: opening_context,
            open: true,
            observations,
        }
    });

    observations.borrow_mut().clear();
    cx.update(|window, cx| window.draw(cx).clear());
    assert_only_observed_palette(&observations, OPENING_COLOR);

    observations.borrow_mut().clear();
    view.update(cx, |view, cx| {
        view.context = reopened_context.clone();
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert_only_observed_palette(&observations, OPENING_COLOR);

    view.update(cx, |view, cx| {
        view.open = false;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());

    observations.borrow_mut().clear();
    view.update(cx, |view, cx| {
        view.open = true;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert_only_observed_palette(&observations, REOPENED_COLOR);
}

struct TooltipThemeProbeView {
    label: &'static str,
    observations: Observations,
}

impl Render for TooltipThemeProbeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = ThemeResolver::current(window, cx);
        record_theme(&self.observations, self.label, ProbePhase::Render, &theme);
        PhaseThemeProbe {
            label: self.label,
            observations: self.observations.clone(),
        }
    }
}

#[derive(Clone, Copy)]
enum NativeTooltipTrigger {
    Button,
    IconButton,
}

struct NativeTooltipScopeView {
    context: ThemeContext,
    trigger: NativeTooltipTrigger,
    observations: Observations,
}

impl Render for NativeTooltipScopeView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let trigger = match self.trigger {
            NativeTooltipTrigger::Button => {
                let observations = self.observations.clone();
                Button::new("scoped-tooltip-button", "Button")
                    .tooltip(move |window, cx| {
                        let theme = ThemeResolver::current(window, cx);
                        record_theme(&observations, "button-tooltip", ProbePhase::Builder, &theme);
                        cx.new(|_| TooltipThemeProbeView {
                            label: "button-tooltip",
                            observations: observations.clone(),
                        })
                        .into()
                    })
                    .into_any_element()
            }
            NativeTooltipTrigger::IconButton => {
                let observations = self.observations.clone();
                IconButton::new("scoped-tooltip-icon-button", "?", "Help")
                    .tooltip(move |window, cx| {
                        let theme = ThemeResolver::current(window, cx);
                        record_theme(
                            &observations,
                            "icon-button-tooltip",
                            ProbePhase::Builder,
                            &theme,
                        );
                        cx.new(|_| TooltipThemeProbeView {
                            label: "icon-button-tooltip",
                            observations: observations.clone(),
                        })
                        .into()
                    })
                    .into_any_element()
            }
        };

        div().size_full().child(ThemeScope::new(
            "native-tooltip-theme",
            self.context.clone(),
            trigger,
        ))
    }
}

fn assert_observed_tooltip_phases(observations: &Observations, mode: ThemeMode) {
    assert_only_observed_mode(observations, mode);
    let observations = observations.borrow();
    for phase in [
        ProbePhase::Builder,
        ProbePhase::Render,
        ProbePhase::RequestLayout,
        ProbePhase::Prepaint,
        ProbePhase::Paint,
    ] {
        assert!(
            observations
                .iter()
                .any(|observation| observation.phase == phase),
            "native tooltip should observe {phase:?}: {observations:?}"
        );
    }
}

#[open_gpui::test]
fn button_delayed_tooltip_freezes_scope_until_close_and_recaptures_on_reopen(
    cx: &mut open_gpui::TestAppContext,
) {
    let observations = Observations::default();
    let (view, cx) = cx.add_window_view({
        let observations = observations.clone();
        move |_, _| NativeTooltipScopeView {
            context: ThemeContext::dark(),
            trigger: NativeTooltipTrigger::Button,
            observations,
        }
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let trigger = cx
        .debug_bounds("button:scoped-tooltip-button:root")
        .expect("scoped button should render");
    cx.simulate_mouse_move(trigger.center(), None, Default::default());
    view.update(cx, |view, cx| {
        view.context = ThemeContext::high_contrast();
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert_observed_tooltip_phases(&observations, ThemeMode::Dark);

    cx.simulate_mouse_move(point(px(300.0), px(160.0)), None, Default::default());
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());

    observations.borrow_mut().clear();
    let trigger = cx
        .debug_bounds("button:scoped-tooltip-button:root")
        .expect("scoped button should rerender");
    cx.simulate_mouse_move(trigger.center(), None, Default::default());
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert_observed_tooltip_phases(&observations, ThemeMode::HighContrast);
}

#[open_gpui::test]
fn icon_button_delayed_tooltip_builder_and_view_share_the_opening_scope(
    cx: &mut open_gpui::TestAppContext,
) {
    let observations = Observations::default();
    let (_, cx) = cx.add_window_view({
        let observations = observations.clone();
        move |_, _| NativeTooltipScopeView {
            context: ThemeContext::high_contrast(),
            trigger: NativeTooltipTrigger::IconButton,
            observations,
        }
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let trigger = cx
        .debug_bounds("icon-button:scoped-tooltip-icon-button:root")
        .expect("scoped icon button should render");
    cx.simulate_mouse_move(trigger.center(), None, Default::default());
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert_observed_tooltip_phases(&observations, ThemeMode::HighContrast);
}
