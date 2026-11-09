use open_gpui::{
    Context, IntoElement, ParentElement, Render, Styled, TestAppContext, VisualTestContext, Window,
    div, px,
};
use open_gpui_motion::MotionPreference;
use open_gpui_ui_components::{
    Splitter, SplitterPanel, SplitterPanelDescriptor,
    theme::{ThemeContext, ThemeDefinition, ThemeRegistry, ThemeScope, ThemeSnapshot},
};
use open_gpui_ui_core::ThemeDesignScales;

struct SplitterView {
    left_fraction: f32,
    extra_panel: bool,
    preference: MotionPreference,
    theme: ThemeContext,
}

impl SplitterView {
    fn new() -> Self {
        Self {
            left_fraction: 0.25,
            extra_panel: false,
            preference: MotionPreference::Animated,
            theme: ThemeContext::light(),
        }
    }
}

impl Render for SplitterView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let mut splitter = Splitter::new("app-motion-splitter")
            .motion_preference(self.preference)
            .panel(SplitterPanel::new(
                SplitterPanelDescriptor::new("left", self.left_fraction),
                div(),
            ))
            .panel(SplitterPanel::new(
                SplitterPanelDescriptor::new("right", 1.0 - self.left_fraction),
                div(),
            ));
        if self.extra_panel {
            splitter = splitter.panel(SplitterPanel::new(
                SplitterPanelDescriptor::new("extra", 1.0),
                div(),
            ));
        }
        div().size_full().child(ThemeScope::new(
            "app-motion-theme",
            self.theme.clone(),
            div().w(px(412.0)).h(px(120.0)).child(splitter),
        ))
    }
}

fn draw(cx: &mut VisualTestContext) {
    cx.update(|window, cx| {
        window.refresh();
        window.draw(cx).clear(cx);
    });
}

fn drain(cx: &mut VisualTestContext) -> usize {
    cx.update(|window, cx| window.drain_next_frame_callbacks_for_test(cx))
}

fn assert_left_width(cx: &mut VisualTestContext, expected: f32) {
    let panel = cx
        .debug_bounds("splitter-panel:left")
        .expect("left panel should render");
    assert!(
        (panel.size.width.as_f32() - expected).abs() < 0.05,
        "expected left panel width {expected}, got {panel:?}"
    );
}

fn assert_idle(cx: &mut VisualTestContext) {
    assert!(
        drain(cx) <= 1,
        "only an already queued animation may remain"
    );
    draw(cx);
    assert_eq!(drain(cx), 0, "the final layout must not renew frame demand");
}

#[open_gpui::test]
fn app_reduced_motion_snaps_mounted_splitter_and_only_new_targets_resume(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, _| SplitterView::new());
    draw(cx);
    assert_left_width(cx, 100.0);
    assert_eq!(drain(cx), 0);

    let requested = cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.left_fraction = 0.75;
            cx.notify();
        });
        window.draw(cx).clear(cx);
        window.drain_next_frame_callbacks_for_test(cx)
    });
    assert_eq!(requested, 1, "changing a mounted layout should animate");

    cx.update(|_, cx| cx.set_reduce_motion(true));
    draw(cx);
    assert_left_width(cx, 300.0);
    assert_idle(cx);

    cx.update(|_, cx| cx.set_reduce_motion(false));
    draw(cx);
    assert_left_width(cx, 300.0);
    assert_eq!(drain(cx), 0, "the completed target must not replay");

    let requested = cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.left_fraction = 0.5;
            cx.notify();
        });
        window.draw(cx).clear(cx);
        window.drain_next_frame_callbacks_for_test(cx)
    });
    assert_eq!(
        requested, 1,
        "a new target should animate after restoration"
    );
    cx.update(|_, cx| cx.set_reduce_motion(true));
    draw(cx);
    assert_left_width(cx, 200.0);
    assert_idle(cx);

    view.update(cx, |view, cx| {
        view.left_fraction = 0.625;
        cx.notify();
    });
    draw(cx);
    assert_left_width(cx, 250.0);
    assert_eq!(drain(cx), 0, "new targets must also honor the App floor");
}

#[open_gpui::test]
fn app_reduced_motion_finishes_splitter_panel_insertion_and_removal(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, _| SplitterView::new());
    draw(cx);
    let requested = cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.extra_panel = true;
            cx.notify();
        });
        window.draw(cx).clear(cx);
        window.drain_next_frame_callbacks_for_test(cx)
    });
    assert_eq!(
        requested, 1,
        "inserting a panel should start a layout transition"
    );

    cx.update(|_, cx| cx.set_reduce_motion(true));
    draw(cx);
    assert_left_width(cx, 63.0);
    let extra = cx
        .debug_bounds("splitter-panel:extra")
        .expect("the inserted panel should reach its final layout");
    let scale = cx.update(|window, _| window.scale_factor());
    let expected_end = (412.0 * scale).round() / scale;
    let expected_start = ((412.0 - 388.0 * 8.0 / 17.0) * scale).round() / scale;
    assert_eq!(extra.origin.x.as_f32(), expected_start);
    assert!(
        (extra.size.width.as_f32() - (expected_end - expected_start)).abs() < 0.05,
        "the minimum-constrained fractions must fill 388 pixels between two handles: {extra:?}"
    );
    assert!(cx.debug_bounds("splitter:transition-overlay").is_none());
    assert_idle(cx);

    cx.update(|_, cx| cx.set_reduce_motion(false));
    draw(cx);
    assert_eq!(drain(cx), 0);
    let requested = cx.update(|window, cx| {
        view.update(cx, |view, cx| {
            view.extra_panel = false;
            cx.notify();
        });
        window.draw(cx).clear(cx);
        window.drain_next_frame_callbacks_for_test(cx)
    });
    assert_eq!(requested, 1, "a later panel removal should animate");
    cx.update(|_, cx| cx.set_reduce_motion(true));
    draw(cx);
    assert_left_width(cx, 100.0);
    assert!(cx.debug_bounds("splitter-panel:extra").is_none());
    assert!(cx.debug_bounds("splitter:transition-overlay").is_none());
    assert_idle(cx);
}

fn reduced_theme() -> ThemeContext {
    let source = ThemeSnapshot::light();
    let baseline = source.design_scales();
    let design = ThemeDesignScales::new(
        baseline.typography(),
        baseline.spacing(),
        baseline.radius(),
        baseline.elevation(),
        baseline.density(),
        MotionPreference::Reduced,
    );
    let mut registry = ThemeRegistry::new();
    let snapshot = registry
        .register(
            ThemeDefinition::from_snapshot("motion-floor", "Motion floor", &source)
                .design_scales(design),
        )
        .expect("a complete reduced-motion theme should register")
        .snapshot()
        .clone();
    ThemeContext::new(snapshot)
}

#[open_gpui::test]
fn splitter_theme_and_explicit_reduced_floors_survive_app_restoration(cx: &mut TestAppContext) {
    let (view, cx) = cx.add_window_view(|_, _| SplitterView::new());
    cx.update(|_, cx| cx.set_reduce_motion(true));
    view.update(cx, |view, cx| {
        view.theme = reduced_theme();
        view.left_fraction = 0.75;
        cx.notify();
    });
    draw(cx);
    assert_left_width(cx, 300.0);
    assert_idle(cx);

    cx.update(|_, cx| cx.set_reduce_motion(false));
    view.update(cx, |view, cx| {
        view.left_fraction = 0.5;
        cx.notify();
    });
    draw(cx);
    assert_left_width(cx, 200.0);
    assert_eq!(
        drain(cx),
        0,
        "an Animated request cannot weaken the theme floor"
    );

    view.update(cx, |view, cx| {
        view.theme = ThemeContext::light();
        view.preference = MotionPreference::Reduced;
        view.left_fraction = 0.25;
        cx.notify();
    });
    draw(cx);
    assert_left_width(cx, 100.0);
    assert_eq!(
        drain(cx),
        0,
        "the explicit Reduced request remains authoritative"
    );
}
