use std::{cell::Cell, fmt, rc::Rc};

use open_gpui::{App, BoxShadow, Rgba, Window, point, px, rgb, rgba};

/// Complete semantic inputs used to derive a Dock visual style.
///
/// Applications that already own a theme system can map its current immutable snapshot into this
/// palette and then call [`DockVisualStyle::from_palette`]. Every field is required; Docking does
/// not merge partial application palettes with its built-in fallback.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockVisualPalette {
    /// Primary panel and floating-container surface.
    pub surface: Rgba,
    /// Secondary surface used by chrome and the root dock background.
    pub surface_muted: Rgba,
    /// Hovered secondary surface.
    pub surface_hovered: Rgba,
    /// Disabled secondary surface.
    pub surface_disabled: Rgba,
    /// Default structural border.
    pub border: Rgba,
    /// Primary foreground text.
    pub text: Rgba,
    /// Secondary foreground text.
    pub text_muted: Rgba,
    /// Disabled foreground text.
    pub text_disabled: Rgba,
    /// Primary interaction accent.
    pub accent: Rgba,
    /// Hovered or active interaction accent.
    pub accent_hovered: Rgba,
    /// Foreground placed on the accent.
    pub accent_foreground: Rgba,
    /// Focus-visible ring color.
    pub focus_ring: Rgba,
    /// Destructive or rejected-state color.
    pub destructive: Rgba,
    /// Foreground placed on a destructive surface.
    pub destructive_foreground: Rgba,
    /// Base shadow color used by floating and drag surfaces.
    pub shadow: Rgba,
}

impl DockVisualPalette {
    /// Returns the deterministic palette used by Docking when no resolver is installed.
    pub fn built_in() -> Self {
        Self {
            surface: rgb(0xffffff),
            surface_muted: rgb(0xf7f8fa),
            surface_hovered: rgb(0xf0f3f7),
            surface_disabled: rgb(0xe7ebf0),
            border: rgb(0xd8dde6),
            text: rgb(0x111827),
            text_muted: rgb(0x657083),
            text_disabled: rgb(0x94a3b8),
            accent: rgb(0x2563eb),
            accent_hovered: rgb(0x1d4ed8),
            accent_foreground: rgb(0xffffff),
            focus_ring: rgb(0x2f80ed),
            destructive: rgb(0xb42318),
            destructive_foreground: rgb(0xffffff),
            shadow: rgba(0x1118273d),
        }
    }
}

/// Background and diagnostic colors for the root host surface.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockHostVisualStyle {
    /// Normal root background.
    pub background: Rgba,
    /// Default inherited foreground.
    pub foreground: Rgba,
    /// Empty-space border.
    pub empty_border: Rgba,
    /// Empty-space message text.
    pub empty_text: Rgba,
    /// Missing-node or missing-panel border.
    pub missing_border: Rgba,
    /// Missing-node or missing-panel text.
    pub missing_text: Rgba,
    /// Opaque background used to occlude panes during transitions.
    pub transition_occlusion: Rgba,
}

/// Colors for one tab or tab action state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockTabPalette {
    /// Tab background.
    pub background: Rgba,
    /// Tab border.
    pub border: Rgba,
    /// Tab foreground.
    pub text: Rgba,
}

/// Complete tab-stack chrome style.
#[derive(Debug, Clone, PartialEq)]
pub struct DockTabsVisualStyle {
    /// Tab-stack frame background.
    pub frame_background: Rgba,
    /// Tab-stack frame border.
    pub frame_border: Rgba,
    /// Tab-strip background.
    pub strip_background: Rgba,
    /// Idle tab palette.
    pub idle: DockTabPalette,
    /// Hovered tab palette.
    pub hovered: DockTabPalette,
    /// Selected tab palette.
    pub selected: DockTabPalette,
    /// Selected and hovered tab palette.
    pub selected_hovered: DockTabPalette,
    /// Idle close-action palette.
    pub close_idle: DockTabPalette,
    /// Hovered close-action palette.
    pub close_hovered: DockTabPalette,
}

/// Interaction states resolved by [`DockTabsVisualStyle::tab`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockTabVisualState {
    /// Unselected and not hovered.
    Idle,
    /// Unselected and hovered.
    Hovered,
    /// Selected and not hovered.
    Selected,
    /// Selected and hovered.
    SelectedHovered,
}

impl DockTabsVisualStyle {
    /// Returns the complete palette for one tab interaction state.
    pub const fn tab(&self, state: DockTabVisualState) -> DockTabPalette {
        match state {
            DockTabVisualState::Idle => self.idle,
            DockTabVisualState::Hovered => self.hovered,
            DockTabVisualState::Selected => self.selected,
            DockTabVisualState::SelectedHovered => self.selected_hovered,
        }
    }
}

/// Complete splitter and divider colors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockSplitterVisualStyle {
    /// Idle splitter color.
    pub idle: Rgba,
    /// Hovered splitter color.
    pub hovered: Rgba,
    /// Actively dragged splitter color.
    pub active: Rgba,
    /// Disabled splitter color.
    pub disabled: Rgba,
    /// Border painted around synthesized corner affordances.
    pub corner_border: Rgba,
}

/// Splitter interaction states resolved by [`DockSplitterVisualStyle::color`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockSplitterVisualState {
    /// Idle.
    Idle,
    /// Hovered.
    Hovered,
    /// Actively dragged.
    Active,
    /// Disabled.
    Disabled,
}

impl DockSplitterVisualStyle {
    /// Returns the color for one splitter interaction state.
    pub const fn color(&self, state: DockSplitterVisualState) -> Rgba {
        match state {
            DockSplitterVisualState::Idle => self.idle,
            DockSplitterVisualState::Hovered => self.hovered,
            DockSplitterVisualState::Active => self.active,
            DockSplitterVisualState::Disabled => self.disabled,
        }
    }
}

/// Complete floating-container chrome style.
#[derive(Debug, Clone, PartialEq)]
pub struct DockFloatingVisualStyle {
    /// Floating body background.
    pub background: Rgba,
    /// Floating frame border.
    pub border: Rgba,
    /// Floating title-bar background.
    pub title_background: Rgba,
    /// Floating title-bar divider.
    pub title_border: Rgba,
    /// Floating title text.
    pub title_text: Rgba,
    /// Floating elevation layers.
    pub shadow: Vec<BoxShadow>,
}

/// Source-owned drag-preview style.
#[derive(Debug, Clone, PartialEq)]
pub struct DockDragVisualStyle {
    /// Drag preview background.
    pub background: Rgba,
    /// Drag preview border.
    pub border: Rgba,
    /// Drag preview text.
    pub text: Rgba,
    /// Drag preview elevation layers.
    pub shadow: Vec<BoxShadow>,
}

/// Accepted or rejected target-preview colors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockTargetPreviewPalette {
    /// Preview border.
    pub border: Rgba,
    /// Preview body background.
    pub body_background: Rgba,
    /// Payload-tab background.
    pub tab_background: Rgba,
    /// Payload-tab foreground.
    pub tab_text: Rgba,
}

/// Drop-guide colors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockDropGuidePalette {
    /// Guide border.
    pub border: Rgba,
    /// Guide background.
    pub background: Rgba,
    /// Directional cue.
    pub cue: Rgba,
    /// Inset outline.
    pub inset: Rgba,
}

/// Cross-window route-preview colors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockRoutePreviewPalette {
    /// Route preview border.
    pub border: Rgba,
    /// Route preview background.
    pub background: Rgba,
}

/// Target-preview states resolved by [`DockPreviewVisualStyle::target`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockTargetPreviewVisualState {
    /// The candidate can be committed.
    Accepted,
    /// The candidate is visible but rejected.
    Rejected,
}

/// Drop-guide states resolved by [`DockPreviewVisualStyle::guide`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockDropGuideVisualState {
    /// Inactive center guide.
    CenterIdle,
    /// Active center guide.
    CenterActive,
    /// Inactive edge guide.
    EdgeIdle,
    /// Active edge guide.
    EdgeActive,
}

/// Cross-window route states resolved by [`DockPreviewVisualStyle::route`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockRoutePreviewVisualState {
    /// A registered target viewport owns the route.
    KnownViewport,
    /// The payload will create a platform viewport.
    TearOff,
    /// Routing is rejected.
    Rejected,
}

/// Complete drop-guide, preview, and transition-affordance style.
#[derive(Debug, Clone, PartialEq)]
pub struct DockPreviewVisualStyle {
    /// Accepted target preview.
    pub accepted_target: DockTargetPreviewPalette,
    /// Rejected target preview.
    pub rejected_target: DockTargetPreviewPalette,
    /// Active center guide.
    pub center_active: DockDropGuidePalette,
    /// Inactive center guide.
    pub center_idle: DockDropGuidePalette,
    /// Active edge guide.
    pub edge_active: DockDropGuidePalette,
    /// Inactive edge guide.
    pub edge_idle: DockDropGuidePalette,
    /// Registered viewport route.
    pub route_known_viewport: DockRoutePreviewPalette,
    /// Tear-off route.
    pub route_tear_off: DockRoutePreviewPalette,
    /// Rejected route.
    pub route_rejected: DockRoutePreviewPalette,
    /// Elevation layers used by target-owned payload-tab previews.
    pub payload_tab_shadow: Vec<BoxShadow>,
    /// Divider color used while a layout transition is sampled.
    pub transition_divider: Rgba,
    /// Transition-affordance border.
    pub transition_affordance_border: Rgba,
    /// Transition-affordance background.
    pub transition_affordance_background: Rgba,
}

impl DockPreviewVisualStyle {
    /// Returns target-preview colors for an accepted or rejected decision.
    pub const fn target(&self, state: DockTargetPreviewVisualState) -> DockTargetPreviewPalette {
        match state {
            DockTargetPreviewVisualState::Accepted => self.accepted_target,
            DockTargetPreviewVisualState::Rejected => self.rejected_target,
        }
    }

    /// Returns drop-guide colors for one explicit guide state.
    pub const fn guide(&self, state: DockDropGuideVisualState) -> DockDropGuidePalette {
        match state {
            DockDropGuideVisualState::CenterIdle => self.center_idle,
            DockDropGuideVisualState::CenterActive => self.center_active,
            DockDropGuideVisualState::EdgeIdle => self.edge_idle,
            DockDropGuideVisualState::EdgeActive => self.edge_active,
        }
    }

    /// Returns route-preview colors for one route state.
    pub const fn route(&self, state: DockRoutePreviewVisualState) -> DockRoutePreviewPalette {
        match state {
            DockRoutePreviewVisualState::KnownViewport => self.route_known_viewport,
            DockRoutePreviewVisualState::TearOff => self.route_tear_off,
            DockRoutePreviewVisualState::Rejected => self.route_rejected,
        }
    }
}

/// One complete immutable visual input for every Dock rendering path.
#[derive(Debug, Clone, PartialEq)]
pub struct DockVisualStyle {
    /// Root host surfaces and diagnostic states.
    pub host: DockHostVisualStyle,
    /// Tab-stack chrome and actions.
    pub tabs: DockTabsVisualStyle,
    /// Splitters and synthesized divider affordances.
    pub splitters: DockSplitterVisualStyle,
    /// In-window floating containers.
    pub floating: DockFloatingVisualStyle,
    /// Source-owned drag preview.
    pub drag: DockDragVisualStyle,
    /// Target-owned guides, previews, and transition affordances.
    pub previews: DockPreviewVisualStyle,
    /// Focus-visible ring applied to the active host surface.
    pub focus_ring: Vec<BoxShadow>,
}

impl DockVisualStyle {
    /// Returns the deterministic complete fallback used when no resolver is installed.
    pub fn built_in() -> Self {
        Self::from_palette(DockVisualPalette::built_in())
    }

    /// Derives a complete Dock style from complete semantic palette inputs.
    pub fn from_palette(palette: DockVisualPalette) -> Self {
        let idle_tab = DockTabPalette {
            background: palette.surface_muted,
            border: palette.border,
            text: palette.text_muted,
        };
        let hovered_tab = DockTabPalette {
            background: palette.surface_hovered,
            border: palette.border,
            text: palette.text,
        };
        let selected_tab = DockTabPalette {
            background: palette.surface,
            border: palette.accent,
            text: palette.text,
        };
        let selected_hovered_tab = DockTabPalette {
            background: palette.surface_hovered,
            border: palette.accent_hovered,
            text: palette.text,
        };
        let close_hovered = DockTabPalette {
            background: palette.surface_hovered,
            border: palette.accent,
            text: palette.text,
        };

        Self {
            host: DockHostVisualStyle {
                background: palette.surface_muted,
                foreground: palette.text,
                empty_border: palette.border,
                empty_text: palette.text_muted,
                missing_border: palette.destructive,
                missing_text: palette.destructive,
                transition_occlusion: palette.surface_muted,
            },
            tabs: DockTabsVisualStyle {
                frame_background: palette.surface,
                frame_border: palette.border,
                strip_background: palette.surface_disabled,
                idle: idle_tab,
                hovered: hovered_tab,
                selected: selected_tab,
                selected_hovered: selected_hovered_tab,
                close_idle: idle_tab,
                close_hovered,
            },
            splitters: DockSplitterVisualStyle {
                idle: palette.border,
                hovered: with_alpha(palette.accent, 0.72),
                active: with_alpha(palette.accent_hovered, 0.88),
                disabled: with_alpha(palette.text_disabled, 0.32),
                corner_border: with_alpha(palette.accent_foreground, 0.70),
            },
            floating: DockFloatingVisualStyle {
                background: palette.surface,
                border: palette.text_muted,
                title_background: palette.surface_disabled,
                title_border: palette.border,
                title_text: palette.text_muted,
                shadow: vec![
                    shadow_layer(palette.shadow, 0.0, 8.0, 24.0, -4.0),
                    shadow_layer(with_alpha(palette.shadow, 0.62), 0.0, 2.0, 8.0, -2.0),
                ],
            },
            drag: DockDragVisualStyle {
                background: palette.accent,
                border: palette.accent_hovered,
                text: palette.accent_foreground,
                shadow: vec![shadow_layer(palette.shadow, 0.0, 4.0, 14.0, -2.0)],
            },
            previews: DockPreviewVisualStyle {
                accepted_target: DockTargetPreviewPalette {
                    border: palette.accent,
                    body_background: with_alpha(palette.accent, 0.28),
                    tab_background: with_alpha(palette.accent, 0.85),
                    tab_text: palette.accent_foreground,
                },
                rejected_target: DockTargetPreviewPalette {
                    border: palette.destructive,
                    body_background: with_alpha(palette.destructive, 0.28),
                    tab_background: with_alpha(palette.destructive, 0.87),
                    tab_text: palette.destructive_foreground,
                },
                center_active: DockDropGuidePalette {
                    border: palette.accent,
                    background: with_alpha(palette.accent, 0.35),
                    cue: palette.accent_hovered,
                    inset: with_alpha(palette.accent_foreground, 0.45),
                },
                center_idle: DockDropGuidePalette {
                    border: with_alpha(palette.accent, 0.50),
                    background: with_alpha(palette.accent, 0.20),
                    cue: with_alpha(palette.accent, 0.68),
                    inset: with_alpha(palette.accent_foreground, 0.32),
                },
                edge_active: DockDropGuidePalette {
                    border: palette.accent_hovered,
                    background: with_alpha(palette.accent, 0.32),
                    cue: palette.accent_hovered,
                    inset: with_alpha(palette.accent_foreground, 0.42),
                },
                edge_idle: DockDropGuidePalette {
                    border: with_alpha(palette.accent, 0.40),
                    background: with_alpha(palette.accent, 0.16),
                    cue: with_alpha(palette.accent, 0.58),
                    inset: with_alpha(palette.accent_foreground, 0.25),
                },
                route_known_viewport: DockRoutePreviewPalette {
                    border: palette.accent,
                    background: with_alpha(palette.accent, 0.31),
                },
                route_tear_off: DockRoutePreviewPalette {
                    border: palette.text_muted,
                    background: with_alpha(palette.text_disabled, 0.28),
                },
                route_rejected: DockRoutePreviewPalette {
                    border: palette.destructive,
                    background: with_alpha(palette.destructive, 0.28),
                },
                payload_tab_shadow: vec![shadow_layer(
                    with_alpha(palette.shadow, 0.58),
                    0.0,
                    1.0,
                    3.0,
                    0.0,
                )],
                transition_divider: with_alpha(palette.accent, 0.80),
                transition_affordance_border: palette.accent,
                transition_affordance_background: with_alpha(palette.accent, 0.20),
            },
            focus_ring: vec![shadow_layer(
                with_alpha(palette.focus_ring, 0.88),
                0.0,
                0.0,
                0.0,
                2.0,
            )],
        }
    }
}

type DockVisualStyleCallback = dyn Fn(&Window, &App) -> DockVisualStyle;

#[derive(Clone)]
enum DockVisualStyleSource {
    Dynamic(Rc<DockVisualStyleCallback>),
    Fixed(Rc<DockVisualStyle>),
}

/// Named render-time resolver for a complete [`DockVisualStyle`].
///
/// The resolver is an immutable per-surface or per-runtime value. It is evaluated synchronously in
/// the active host's window and subtree render context, which lets application code adapt a local
/// theme without a Docking dependency on that theme system. Its read-only arguments prevent entity
/// updates, notifications, event dispatch, registration changes, and window refreshes. Reentrant
/// Dock style resolution is rejected separately at runtime.
#[derive(Clone)]
pub struct DockVisualStyleResolver {
    source: DockVisualStyleSource,
}

impl DockVisualStyleResolver {
    /// Creates a resolver from one complete synchronous render-context mapping.
    pub fn new(callback: impl Fn(&Window, &App) -> DockVisualStyle + 'static) -> Self {
        Self {
            source: DockVisualStyleSource::Dynamic(Rc::new(callback)),
        }
    }

    /// Creates a resolver that always returns the same immutable style.
    pub fn fixed(style: DockVisualStyle) -> Self {
        Self {
            source: DockVisualStyleSource::Fixed(Rc::new(style)),
        }
    }

    pub(crate) fn resolve(&self, window: &mut Window, cx: &mut App) -> Rc<DockVisualStyle> {
        match &self.source {
            DockVisualStyleSource::Dynamic(callback) => {
                Rc::new(with_dock_visual_style_resolution(|| callback(window, cx)))
            }
            DockVisualStyleSource::Fixed(style) => style.clone(),
        }
    }
}

impl fmt::Debug for DockVisualStyleResolver {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DockVisualStyleResolver")
            .finish_non_exhaustive()
    }
}

thread_local! {
    static DOCK_VISUAL_STYLE_RESOLUTION_DEPTH: Cell<usize> = const { Cell::new(0) };
}

fn with_dock_visual_style_resolution<T>(resolve: impl FnOnce() -> T) -> T {
    DOCK_VISUAL_STYLE_RESOLUTION_DEPTH.with(|depth| {
        let entered_depth = depth.get();
        assert_eq!(
            entered_depth, 0,
            "DockVisualStyleResolver callbacks must not reenter Dock style resolution"
        );
        depth.set(entered_depth + 1);
        let _guard = DockVisualStyleResolutionGuard { depth };
        resolve()
    })
}

struct DockVisualStyleResolutionGuard<'a> {
    depth: &'a Cell<usize>,
}

impl Drop for DockVisualStyleResolutionGuard<'_> {
    fn drop(&mut self) {
        self.depth.set(self.depth.get().saturating_sub(1));
    }
}

fn with_alpha(color: Rgba, alpha: f32) -> Rgba {
    Rgba { a: alpha, ..color }
}

fn shadow_layer(
    color: Rgba,
    offset_x: f32,
    offset_y: f32,
    blur_radius: f32,
    spread_radius: f32,
) -> BoxShadow {
    BoxShadow {
        color: color.into(),
        offset: point(px(offset_x), px(offset_y)),
        blur_radius: px(blur_radius),
        spread_radius: px(spread_radius),
        inset: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_style_is_complete_and_deterministic() {
        assert_eq!(
            DockVisualStyle::from_palette(DockVisualPalette::built_in()),
            DockVisualStyle::built_in()
        );
    }

    #[test]
    fn every_public_interaction_state_has_an_explicit_palette() {
        let style = DockVisualStyle::built_in();
        for state in [
            DockTabVisualState::Idle,
            DockTabVisualState::Hovered,
            DockTabVisualState::Selected,
            DockTabVisualState::SelectedHovered,
        ] {
            assert!(style.tabs.tab(state).background.a > 0.0);
        }
        for state in [
            DockSplitterVisualState::Idle,
            DockSplitterVisualState::Hovered,
            DockSplitterVisualState::Active,
            DockSplitterVisualState::Disabled,
        ] {
            assert!(style.splitters.color(state).a > 0.0);
        }
        for state in [
            DockTargetPreviewVisualState::Accepted,
            DockTargetPreviewVisualState::Rejected,
        ] {
            assert!(style.previews.target(state).border.a > 0.0);
        }
        for state in [
            DockDropGuideVisualState::CenterIdle,
            DockDropGuideVisualState::CenterActive,
            DockDropGuideVisualState::EdgeIdle,
            DockDropGuideVisualState::EdgeActive,
        ] {
            assert!(style.previews.guide(state).cue.a > 0.0);
        }
        for state in [
            DockRoutePreviewVisualState::KnownViewport,
            DockRoutePreviewVisualState::TearOff,
            DockRoutePreviewVisualState::Rejected,
        ] {
            assert!(style.previews.route(state).border.a > 0.0);
        }
    }

    #[test]
    fn resolution_scope_rejects_reentry_and_recovers_after_unwind() {
        let panic = std::panic::catch_unwind(|| {
            with_dock_visual_style_resolution(|| {
                with_dock_visual_style_resolution(|| DockVisualStyle::built_in())
            })
        });
        assert!(panic.is_err(), "nested resolution must be rejected");
        assert_eq!(
            with_dock_visual_style_resolution(DockVisualStyle::built_in),
            DockVisualStyle::built_in(),
            "the RAII guard must restore resolution state after unwind"
        );
    }
}
