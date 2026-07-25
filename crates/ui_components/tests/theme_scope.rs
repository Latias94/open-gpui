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
use open_gpui_motion::MotionPreference;
use open_gpui_ui_components::theme::{
    DARK_THEME_ID, LIGHT_THEME_ID, ThemeColor, ThemeContext, ThemeDefinition, ThemeMode,
    ThemeRegistry, ThemeResolver, ThemeScope, ThemeSelectionError, ThemeSnapshot,
    clear_window_theme, install_theme_registry, override_window_theme, register_theme,
    set_app_theme, set_window_theme,
};
use open_gpui_ui_components::{Button, ColorIntent, ColorState, IconButton, Popover};
use open_gpui_ui_core::{Density, SizeScale, ThemeDesignScales, ThemeRadiusScale, semantic};

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
    source_revision: u64,
    effective_revision: u64,
    density: Density,
    motion_preference: MotionPreference,
    control_radius: [u16; 4],
}

type Observations = Rc<RefCell<Vec<ThemeObservation>>>;

fn complete_theme_definition(
    id: &str,
    label: &str,
    mode: ThemeMode,
    source_revision: u64,
) -> ThemeDefinition {
    let snapshot = match mode {
        ThemeMode::Light => ThemeSnapshot::light(),
        ThemeMode::Dark => ThemeSnapshot::dark(),
        ThemeMode::HighContrast => ThemeSnapshot::high_contrast(),
    };
    ThemeDefinition::from_snapshot(id, label, &snapshot).source_revision(source_revision)
}

fn complete_theme_definition_with_surface(
    id: impl Into<String>,
    label: impl Into<String>,
    source_revision: u64,
    rgb: u32,
) -> ThemeDefinition {
    let source = ThemeSnapshot::dark();
    let colors = source.colors().iter().copied().map(|color| {
        if color.token() == semantic::SURFACE && color.state() == ColorState::Default {
            ThemeColor::new(semantic::SURFACE, ColorState::Default, rgb)
        } else {
            color
        }
    });
    ThemeDefinition::new(id, label, source.mode(), source_revision)
        .design_scales(source.design_scales())
        .colors(colors)
}

fn complete_theme_context_with_surface(source_revision: u64, rgb: u32) -> ThemeContext {
    let definition = complete_theme_definition_with_surface(
        format!("surface-{rgb:06x}"),
        format!("Surface {rgb:06x}"),
        source_revision,
        rgb,
    );
    let mut registry = ThemeRegistry::new();
    let snapshot = registry
        .register(definition)
        .expect("complete surface canary should register")
        .snapshot()
        .clone();
    ThemeContext::new(snapshot)
}

fn complete_theme_context_with_design(
    id: &str,
    density: Density,
    motion_preference: MotionPreference,
    control_radius: SizeScale,
) -> ThemeContext {
    let source = ThemeSnapshot::dark();
    let defaults = source.design_scales();
    let design = ThemeDesignScales::new(
        defaults.typography(),
        defaults.spacing(),
        ThemeRadiusScale::new(control_radius),
        defaults.elevation(),
        density,
        motion_preference,
    );
    let definition = ThemeDefinition::from_snapshot(id, id, &source)
        .source_revision(77)
        .design_scales(design);
    let mut registry = ThemeRegistry::new();
    let snapshot = registry
        .register(definition)
        .expect("complete design canary should register")
        .snapshot()
        .clone();
    ThemeContext::new(snapshot)
}

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
        source_revision: theme.source_revision(),
        effective_revision: theme.effective_revision(),
        density: theme.density(),
        motion_preference: theme.motion_preference(),
        control_radius: theme.design_scales().radius().control().raw_values(),
    });
}

fn assert_single_default_scale_observation(
    observations: &Observations,
    label: &'static str,
    phase: ProbePhase,
    mode: ThemeMode,
    source_revision: u64,
) {
    let observations = observations.borrow();
    assert_eq!(observations.len(), 1);
    let observation = &observations[0];
    assert_eq!(observation.label, label);
    assert_eq!(observation.phase, phase);
    assert_eq!(observation.mode, mode);
    assert_eq!(observation.source_revision, source_revision);
    assert!(observation.effective_revision > 0);
    assert_eq!(observation.density, Density::Comfortable);
    assert_eq!(observation.motion_preference, MotionPreference::Animated);
    assert_eq!(
        observation.control_radius,
        ThemeDesignScales::default().radius().control().raw_values()
    );
}

#[derive(IntoElement)]
struct RenderThemeProbe {
    label: &'static str,
    observations: Observations,
}

impl RenderOnce for RenderThemeProbe {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let read_only_snapshot = ThemeResolver::current_snapshot(window, cx);
        let theme = ThemeResolver::current(window, cx);
        assert_eq!(
            read_only_snapshot,
            *theme.snapshot(),
            "read-only theme resolution must match the mutable runtime authority"
        );
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
    let _: fn(&Window, &App) -> ThemeSnapshot = ThemeResolver::current_snapshot;
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
    assert_single_default_scale_observation(
        &inherited_observations,
        "window",
        ProbePhase::Render,
        ThemeMode::Light,
        ThemeSnapshot::light().source_revision(),
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
fn unselected_invalid_and_metadata_only_registration_do_not_refresh_the_window(
    cx: &mut open_gpui::TestAppContext,
) {
    cx.update(|app| {
        let mut registry = ThemeRegistry::with_builtins();
        registry
            .register(complete_theme_definition(
                "brand",
                "Brand",
                ThemeMode::Dark,
                1,
            ))
            .expect("active brand should register");
        install_theme_registry(app, registry, "brand").expect("active brand should install");
    });
    let observations = Observations::default();
    let (_, cx) = cx.add_window_view({
        let observations = observations.clone();
        move |_, _| WindowThemeProbe { observations }
    });
    cx.update(|window, cx| window.draw(cx).clear());
    observations.borrow_mut().clear();

    cx.update(|_, cx| {
        register_theme(
            cx,
            complete_theme_definition("unused", "Unused", ThemeMode::HighContrast, 1),
        )
        .expect("unselected theme should register");
    });
    assert!(observations.borrow().is_empty());

    let before = cx.update(|window, cx| ThemeResolver::current(window, cx));
    cx.update(|_, cx| {
        register_theme(
            cx,
            complete_theme_definition("brand", "Brand metadata", ThemeMode::Dark, 2),
        )
        .expect("metadata-only active replacement should register");
    });
    assert!(observations.borrow().is_empty());
    let metadata_only = cx.update(|window, cx| ThemeResolver::current(window, cx));
    assert_eq!(
        metadata_only.effective_revision(),
        before.effective_revision()
    );
    assert_eq!(metadata_only.source_revision(), 2);

    cx.update(|_, cx| {
        let error = register_theme(
            cx,
            ThemeDefinition::new("brand", "Invalid brand", ThemeMode::Dark, 3)
                .design_scales(ThemeDesignScales::default()),
        )
        .expect_err("incomplete active replacement must fail");
        assert!(matches!(
            error,
            open_gpui_ui_components::theme::ThemeValidationError::MissingColor { .. }
        ));
    });
    assert!(observations.borrow().is_empty());
    cx.update(|window, cx| {
        assert_eq!(ThemeResolver::current(window, cx), metadata_only);
    });
}

#[open_gpui::test]
fn unknown_window_selection_is_atomic_and_clear_returns_to_app_selection(
    cx: &mut open_gpui::TestAppContext,
) {
    cx.update(|app| set_app_theme(app, DARK_THEME_ID).expect("built-in app theme should resolve"));
    let (_, cx) = cx.add_window_view(|_, _| Empty);

    cx.update(|window, cx| {
        let inherited_revision = ThemeResolver::current(window, cx).effective_revision();
        set_window_theme(window, cx, LIGHT_THEME_ID)
            .expect("built-in window selection should resolve");
        let before = ThemeResolver::current(window, cx);
        assert_eq!(before.mode(), ThemeMode::Light);
        assert!(before.effective_revision() > inherited_revision);

        assert_eq!(
            set_window_theme(window, cx, "missing-theme").unwrap_err(),
            ThemeSelectionError::UnknownThemeId("missing-theme".to_owned())
        );
        let after_failure = ThemeResolver::current(window, cx);
        assert_eq!(after_failure, before);

        clear_window_theme(window, cx);
        let cleared = ThemeResolver::current(window, cx);
        assert_eq!(cleared.mode(), ThemeMode::Dark);
        assert!(cleared.effective_revision() > before.effective_revision());
    });
}

#[open_gpui::test]
fn repeated_clear_window_theme_is_an_exact_noop(cx: &mut open_gpui::TestAppContext) {
    cx.update(|app| set_app_theme(app, DARK_THEME_ID).expect("built-in app theme should resolve"));
    let observations = Observations::default();
    let (_, cx) = cx.add_window_view({
        let observations = observations.clone();
        move |_, _| WindowThemeProbe { observations }
    });
    cx.update(|window, cx| {
        set_window_theme(window, cx, LIGHT_THEME_ID).expect("explicit window theme should resolve");
        window.draw(cx).clear();
    });

    let first_clear = cx.update(|window, cx| {
        clear_window_theme(window, cx);
        ThemeResolver::current(window, cx)
    });
    observations.borrow_mut().clear();
    cx.update(|window, cx| {
        clear_window_theme(window, cx);
        assert_eq!(ThemeResolver::current(window, cx), first_clear);
    });

    assert!(
        observations.borrow().is_empty(),
        "repeating clear on an inherited window must not refresh it"
    );
}

#[open_gpui::test]
fn prebuilt_registry_replacement_rebinds_selected_window_monotonically(
    cx: &mut open_gpui::TestAppContext,
) {
    let mut replacement = ThemeRegistry::with_builtins();
    replacement
        .register(complete_theme_definition_with_surface(
            "brand",
            "Brand replacement",
            2,
            0x223344,
        ))
        .expect("prebuilt replacement should register");
    cx.update(|app| {
        register_theme(
            app,
            complete_theme_definition_with_surface("brand", "Brand", 1, 0x112233),
        )
        .expect("current brand should register");
    });
    let (_, cx) = cx.add_window_view(|_, _| Empty);
    let before = cx.update(|window, cx| {
        set_window_theme(window, cx, "brand").expect("window brand should resolve");
        ThemeResolver::current(window, cx)
    });

    cx.update(|window, cx| {
        install_theme_registry(cx, replacement, "brand")
            .expect("prebuilt replacement should install");
        let after = ThemeResolver::current(window, cx);
        assert_eq!(after.source_revision(), 2);
        assert_eq!(
            after
                .snapshot()
                .color_rgb(semantic::SURFACE, ColorState::Default),
            Some(0x223344)
        );
        assert!(after.effective_revision() > before.effective_revision());
    });
}

#[open_gpui::test]
fn selected_window_reads_active_replacement_in_the_same_transaction(
    cx: &mut open_gpui::TestAppContext,
) {
    cx.update(|app| {
        register_theme(
            app,
            complete_theme_definition_with_surface("brand", "Brand", 1, 0x112233),
        )
        .expect("current brand should register");
    });
    let (_, cx) = cx.add_window_view(|_, _| Empty);
    cx.update(|window, cx| {
        set_window_theme(window, cx, "brand").expect("window brand should resolve");
        let before = ThemeResolver::current(window, cx);
        register_theme(
            cx,
            complete_theme_definition_with_surface("brand", "Brand replacement", 2, 0x334455),
        )
        .expect("active replacement should register");

        let after = ThemeResolver::current(window, cx);
        assert_eq!(after.source_revision(), 2);
        assert_eq!(
            after
                .snapshot()
                .color_rgb(semantic::SURFACE, ColorState::Default),
            Some(0x334455)
        );
        assert!(after.effective_revision() > before.effective_revision());
    });
}

#[open_gpui::test]
fn app_selection_is_visible_in_the_current_window_transaction(cx: &mut open_gpui::TestAppContext) {
    let observations = Observations::default();
    let (_, cx) = cx.add_window_view({
        let observations = observations.clone();
        move |_, _| WindowThemeProbe { observations }
    });
    cx.update(|window, cx| window.draw(cx).clear());
    observations.borrow_mut().clear();

    cx.update(|window, cx| {
        assert_eq!(ThemeResolver::current(window, cx).mode(), ThemeMode::Light);
        set_app_theme(cx, DARK_THEME_ID).expect("built-in app theme should resolve");
        assert_eq!(
            ThemeResolver::current(window, cx).mode(),
            ThemeMode::Dark,
            "the effective app selection must be readable before global observers flush"
        );
    });
    cx.run_until_parked();
    assert_eq!(
        observations
            .borrow()
            .last()
            .expect("read-through synchronization should refresh the rendered window")
            .mode,
        ThemeMode::Dark
    );
}

#[open_gpui::test]
fn a_missing_window_selection_retains_its_last_known_snapshot_until_the_id_returns(
    cx: &mut open_gpui::TestAppContext,
) {
    cx.update(|app| {
        register_theme(
            app,
            complete_theme_definition("brand", "Brand", ThemeMode::HighContrast, 1),
        )
        .expect("brand theme should register");
    });
    let observations = Observations::default();
    let (_, cx) = cx.add_window_view({
        let observations = observations.clone();
        move |_, _| WindowThemeProbe { observations }
    });
    let selected_before = cx.update(|window, cx| {
        set_window_theme(window, cx, "brand").expect("brand window selection should resolve");
        window.draw(cx).clear();
        ThemeResolver::current(window, cx)
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
        assert_eq!(ThemeResolver::current(window, cx), selected_before);
    });

    cx.update(|_, cx| set_app_theme(cx, DARK_THEME_ID).expect("app theme should change"));
    assert!(
        observations.borrow().is_empty(),
        "app selection changes must not affect a window retaining an explicit selection"
    );
    cx.update(|window, cx| {
        assert_eq!(ThemeResolver::current(window, cx), selected_before);
    });

    cx.update(|_, cx| {
        register_theme(
            cx,
            complete_theme_definition("brand", "Brand v2", ThemeMode::Dark, 2),
        )
        .expect("replacement brand theme should register");
    });
    assert_single_default_scale_observation(
        &observations,
        "window",
        ProbePhase::Render,
        ThemeMode::Dark,
        2,
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
    let opening_context = complete_theme_context_with_design(
        "cached-compact",
        Density::Compact,
        MotionPreference::Animated,
        SizeScale::new(1, 2, 3, 4),
    );
    let changed_context = complete_theme_context_with_design(
        "cached-spacious",
        Density::Spacious,
        MotionPreference::Reduced,
        SizeScale::new(5, 6, 7, 8),
    );
    let (view, cx) = cx.add_window_view({
        let observations = observations.clone();
        move |_, cx| CachedScopeView {
            context: opening_context,
            child: cx.new(|_| CachedThemeChild { observations }),
        }
    });

    observations.borrow_mut().clear();
    view.update(cx, move |view, cx| {
        view.context = changed_context;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let observations = observations.borrow();
    assert_eq!(observations.len(), 1);
    let observation = &observations[0];
    assert_eq!(observation.label, "cached-child");
    assert_eq!(observation.phase, ProbePhase::Render);
    assert_eq!(observation.mode, ThemeMode::Dark);
    assert_eq!(observation.source_revision, 77);
    assert_eq!(observation.density, Density::Spacious);
    assert_eq!(observation.motion_preference, MotionPreference::Reduced);
    assert_eq!(observation.control_radius, [5, 6, 7, 8]);
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

fn assert_only_observed_design(
    observations: &Observations,
    density: Density,
    motion_preference: MotionPreference,
    control_radius: [u16; 4],
) {
    let observations = observations.borrow();
    assert!(!observations.is_empty());
    let effective_revision = observations[0].effective_revision;
    assert!(observations.iter().all(|observation| {
        observation.mode == ThemeMode::Dark
            && observation.source_revision == 77
            && observation.effective_revision == effective_revision
            && observation.density == density
            && observation.motion_preference == motion_preference
            && observation.control_radius == control_radius
    }));
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

#[open_gpui::test]
fn deferred_overlay_freezes_non_color_scales_for_one_open_generation(
    cx: &mut open_gpui::TestAppContext,
) {
    let opening_context = complete_theme_context_with_design(
        "deferred-compact",
        Density::Compact,
        MotionPreference::Animated,
        SizeScale::new(1, 2, 3, 4),
    );
    let reopened_context = complete_theme_context_with_design(
        "deferred-spacious",
        Density::Spacious,
        MotionPreference::Reduced,
        SizeScale::new(5, 6, 7, 8),
    );
    let observations = Observations::default();
    let (view, cx) = cx.add_window_view({
        let observations = observations.clone();
        move |_, _| PopoverThemeScopeView {
            context: opening_context,
            open: true,
            observations,
        }
    });

    observations.borrow_mut().clear();
    cx.update(|window, cx| window.draw(cx).clear());
    assert_only_observed_design(
        &observations,
        Density::Compact,
        MotionPreference::Animated,
        [1, 2, 3, 4],
    );

    observations.borrow_mut().clear();
    view.update(cx, move |view, cx| {
        view.context = reopened_context;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    assert_only_observed_design(
        &observations,
        Density::Compact,
        MotionPreference::Animated,
        [1, 2, 3, 4],
    );

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
    assert_only_observed_design(
        &observations,
        Density::Spacious,
        MotionPreference::Reduced,
        [5, 6, 7, 8],
    );
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
    let opening_context = complete_theme_context_with_surface(77, OPENING_COLOR >> 8);
    let reopened_context = complete_theme_context_with_surface(77, REOPENED_COLOR >> 8);
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
    assert_all_tooltip_phases(observations);
}

fn assert_observed_tooltip_design_phases(
    observations: &Observations,
    density: Density,
    motion_preference: MotionPreference,
    control_radius: [u16; 4],
) {
    assert_only_observed_design(observations, density, motion_preference, control_radius);
    assert_all_tooltip_phases(observations);
}

fn assert_all_tooltip_phases(observations: &Observations) {
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
    let opening_context = complete_theme_context_with_design(
        "tooltip-compact",
        Density::Compact,
        MotionPreference::Animated,
        SizeScale::new(1, 2, 3, 4),
    );
    let reopened_context = complete_theme_context_with_design(
        "tooltip-spacious",
        Density::Spacious,
        MotionPreference::Reduced,
        SizeScale::new(5, 6, 7, 8),
    );
    let observations = Observations::default();
    let (view, cx) = cx.add_window_view({
        let observations = observations.clone();
        move |_, _| NativeTooltipScopeView {
            context: opening_context,
            trigger: NativeTooltipTrigger::Button,
            observations,
        }
    });
    cx.update(|window, cx| window.draw(cx).clear());

    let trigger = cx
        .debug_bounds("button:scoped-tooltip-button:root")
        .expect("scoped button should render");
    cx.simulate_mouse_move(trigger.center(), None, Default::default());
    view.update(cx, move |view, cx| {
        view.context = reopened_context;
        cx.notify();
    });
    cx.update(|window, cx| window.draw(cx).clear());
    cx.executor().advance_clock(Duration::from_millis(500));
    cx.run_until_parked();
    cx.update(|window, cx| window.draw(cx).clear());
    assert_observed_tooltip_design_phases(
        &observations,
        Density::Compact,
        MotionPreference::Animated,
        [1, 2, 3, 4],
    );

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
    assert_observed_tooltip_design_phases(
        &observations,
        Density::Spacious,
        MotionPreference::Reduced,
        [5, 6, 7, 8],
    );
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
