use crate::{PowerMode, ThermalState};
use open_gpui_scheduler::Instant;
use std::time::Duration;

const FRAME_INTERVAL_TOLERANCE: Duration = Duration::from_millis(1);

/// Keeps throttled frames on a stable cadence despite callback delivery jitter.
#[derive(Default)]
pub(super) struct FramePacer {
    pub(super) last_frame: Option<Instant>,
}

impl FramePacer {
    pub(super) fn should_defer(
        &mut self,
        now: Instant,
        minimum_interval: Option<Duration>,
    ) -> bool {
        if let Some(interval) = minimum_interval
            && let Some(last_frame) = self.last_frame
        {
            let Some(deadline) = last_frame.checked_add(interval) else {
                return true;
            };
            if now.saturating_duration_since(last_frame) < interval
                && deadline.saturating_duration_since(now) > FRAME_INTERVAL_TOLERANCE
            {
                return true;
            }
            self.last_frame = Some(if now.saturating_duration_since(deadline) <= interval / 4 {
                deadline
            } else {
                now
            });
        } else {
            self.last_frame = Some(now);
        }
        false
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FrameRateLimit {
    Inactive,
    LowPower,
    Thermal,
    Animation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FrameThrottleFacts {
    pub(crate) require_presentation: bool,
    pub(crate) active: bool,
    pub(crate) high_rate_input: bool,
    pub(crate) thermal_state: ThermalState,
    pub(crate) power_mode: PowerMode,
    pub(crate) inactive_frame_interval: Option<Duration>,
    pub(crate) low_power_frame_interval: Option<Duration>,
    pub(crate) thermal_frame_interval: Option<Duration>,
    pub(crate) animation_frame_interval: Option<Duration>,
}

impl FrameThrottleFacts {
    pub(crate) fn min_frame_interval(self) -> Option<(Duration, FrameRateLimit)> {
        if self.require_presentation {
            return None;
        }
        let mut longest = None;
        for candidate in [
            self.inactive_frame_interval
                .filter(|_| !self.active && !self.high_rate_input)
                .map(|interval| (interval, FrameRateLimit::Inactive)),
            self.low_power_frame_interval
                .filter(|_| self.power_mode == PowerMode::LowPower)
                .map(|interval| (interval, FrameRateLimit::LowPower)),
            self.thermal_frame_interval
                .filter(|_| {
                    matches!(
                        self.thermal_state,
                        ThermalState::Critical | ThermalState::Serious
                    )
                })
                .map(|interval| (interval, FrameRateLimit::Thermal)),
            self.animation_frame_interval
                .filter(|interval| !interval.is_zero())
                .map(|interval| (interval, FrameRateLimit::Animation)),
        ]
        .into_iter()
        .flatten()
        {
            if longest.is_none_or(|(interval, _)| candidate.0 > interval) {
                longest = Some(candidate);
            }
        }
        longest
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PresentFacts {
    pub(crate) require_presentation: bool,
    pub(crate) needs_present: bool,
    pub(crate) high_rate_input: bool,
}

impl PresentFacts {
    pub(crate) fn needs_present(self) -> bool {
        self.require_presentation || self.needs_present || self.high_rate_input
    }
}

#[cfg(test)]
mod cadence_tests;

#[cfg(test)]
mod high_rate_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AnyView, AppContext as _, Context, Entity, IntoElement, ParentElement, Render,
        RequestFrameOptions, StyleRefinement, Styled, TestAppContext, Window, div,
    };
    use std::{cell::Cell, rc::Rc};

    struct CachedFrameChild(Rc<Cell<usize>>);

    impl Render for CachedFrameChild {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            self.0.set(self.0.get() + 1);
            div().size_full()
        }
    }

    struct CachedFrameRoot(Entity<CachedFrameChild>);

    impl Render for CachedFrameRoot {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
                .size_full()
                .child(AnyView::from(self.0.clone()).cached(StyleRefinement::default().size_full()))
        }
    }

    #[test]
    fn throttled_force_render_refreshes_cached_views_on_the_next_eligible_frame_once() {
        let mut cx = TestAppContext::single();
        let renders = Rc::new(Cell::new(0));
        let (_, visual) = cx.add_window_view({
            let renders = renders.clone();
            move |_, cx| CachedFrameRoot(cx.new(|_| CachedFrameChild(renders)))
        });
        visual.update(|window, cx| {
            window.draw(cx).clear(cx);
            let initial_renders = renders.get();
            assert!(initial_renders > 0);
            window.active.set(false);
            window.frame_pacer.last_frame = Some(Instant::now());
            window.native_frame_requested(
                RequestFrameOptions {
                    force_render: true,
                    require_presentation: false,
                    ..Default::default()
                },
                cx,
            );
            assert_eq!(renders.get(), initial_renders);
            assert!(window.deferred_force_render);

            window.frame_pacer.last_frame = None;
            window.native_frame_requested(
                RequestFrameOptions {
                    force_render: false,
                    require_presentation: false,
                    ..Default::default()
                },
                cx,
            );
            assert_eq!(renders.get(), initial_renders + 1);
            assert!(!window.deferred_force_render);

            window.frame_pacer.last_frame = None;
            window.native_frame_requested(
                RequestFrameOptions {
                    force_render: false,
                    require_presentation: false,
                    ..Default::default()
                },
                cx,
            );
            assert_eq!(renders.get(), initial_renders + 1);
        });
    }

    fn default_limits() -> FrameThrottleFacts {
        let options = crate::WindowOptions::default();
        FrameThrottleFacts {
            require_presentation: false,
            active: true,
            high_rate_input: false,
            thermal_state: ThermalState::Nominal,
            power_mode: PowerMode::Unknown,
            inactive_frame_interval: options.inactive_frame_interval,
            low_power_frame_interval: options.low_power_frame_interval,
            thermal_frame_interval: options.thermal_frame_interval,
            animation_frame_interval: None,
        }
    }

    #[test]
    fn frame_throttle_prefers_inactive_over_thermal_pressure() {
        assert_eq!(
            FrameThrottleFacts {
                active: false,
                thermal_state: ThermalState::Critical,
                ..default_limits()
            }
            .min_frame_interval(),
            Some((Duration::from_micros(33_333), FrameRateLimit::Inactive))
        );
    }

    #[test]
    fn frame_throttle_limits_active_thermal_pressure() {
        assert_eq!(
            FrameThrottleFacts {
                thermal_state: ThermalState::Serious,
                ..default_limits()
            }
            .min_frame_interval(),
            Some((Duration::from_micros(16_667), FrameRateLimit::Thermal))
        );
    }

    #[test]
    fn required_presentation_bypasses_inactive_and_thermal_throttling() {
        for active in [false, true] {
            assert_eq!(
                FrameThrottleFacts {
                    require_presentation: true,
                    active,
                    thermal_state: ThermalState::Critical,
                    power_mode: PowerMode::LowPower,
                    ..default_limits()
                }
                .min_frame_interval(),
                None
            );
        }
    }

    #[test]
    fn high_rate_input_bypasses_only_the_inactive_limit() {
        for (thermal_state, expected) in [
            (ThermalState::Nominal, None),
            (
                ThermalState::Serious,
                Some((Duration::from_micros(16_667), FrameRateLimit::Thermal)),
            ),
        ] {
            assert_eq!(
                FrameThrottleFacts {
                    active: false,
                    high_rate_input: true,
                    thermal_state,
                    ..default_limits()
                }
                .min_frame_interval(),
                expected
            );
        }
    }

    #[test]
    fn frame_wait_requires_previous_frame_and_interval() {
        let now = Instant::now();
        let mut pacer = FramePacer::default();
        assert!(!pacer.should_defer(now, Some(Duration::from_millis(30))));
        assert!(pacer.should_defer(now, Some(Duration::from_millis(30))));
        assert!(!pacer.should_defer(now, None));
    }

    #[test]
    fn present_decision_keeps_high_rate_input_demand() {
        assert!(
            !PresentFacts {
                require_presentation: false,
                needs_present: false,
                high_rate_input: false,
            }
            .needs_present()
        );
        assert!(
            PresentFacts {
                require_presentation: false,
                needs_present: false,
                high_rate_input: true,
            }
            .needs_present()
        );
    }
}
