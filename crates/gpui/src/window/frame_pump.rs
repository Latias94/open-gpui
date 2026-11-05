use crate::ThermalState;
use open_gpui_scheduler::Instant;
use std::time::Duration;

const INACTIVE_FRAME_INTERVAL: Duration = Duration::from_micros(33333);
const THERMAL_THROTTLE_FRAME_INTERVAL: Duration = Duration::from_micros(16667);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FrameThrottleFacts {
    pub(crate) force_render: bool,
    pub(crate) require_presentation: bool,
    pub(crate) has_next_frame_callbacks: bool,
    pub(crate) active: bool,
    pub(crate) thermal_state: Option<ThermalState>,
}

impl FrameThrottleFacts {
    pub(crate) fn min_frame_interval(self) -> Option<Duration> {
        if !self.force_render && !self.require_presentation && !self.has_next_frame_callbacks {
            None
        } else if !self.active {
            Some(INACTIVE_FRAME_INTERVAL)
        } else if let Some(ThermalState::Critical | ThermalState::Serious) = self.thermal_state {
            Some(THERMAL_THROTTLE_FRAME_INTERVAL)
        } else {
            None
        }
    }
}

pub(crate) fn frame_should_wait(
    now: Instant,
    last_frame: Option<Instant>,
    min_frame_interval: Option<Duration>,
) -> bool {
    let (Some(last_frame), Some(min_frame_interval)) = (last_frame, min_frame_interval) else {
        return false;
    };
    now.duration_since(last_frame) < min_frame_interval
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PresentFacts {
    pub(crate) require_presentation: bool,
    pub(crate) needs_present: bool,
    pub(crate) active: bool,
    pub(crate) high_rate_input: bool,
}

impl PresentFacts {
    pub(crate) fn needs_present(self) -> bool {
        self.require_presentation || self.needs_present || (self.active && self.high_rate_input)
    }
}

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
            window.last_frame_time.set(Some(Instant::now()));
            window.native_frame_requested(
                RequestFrameOptions {
                    force_render: true,
                    require_presentation: false,
                },
                cx,
            );
            assert_eq!(renders.get(), initial_renders);
            assert!(window.deferred_force_render);

            window.last_frame_time.set(None);
            window.native_frame_requested(
                RequestFrameOptions {
                    force_render: false,
                    require_presentation: false,
                },
                cx,
            );
            assert_eq!(renders.get(), initial_renders + 1);
            assert!(!window.deferred_force_render);

            window.last_frame_time.set(None);
            window.native_frame_requested(
                RequestFrameOptions {
                    force_render: false,
                    require_presentation: false,
                },
                cx,
            );
            assert_eq!(renders.get(), initial_renders + 1);
        });
    }

    #[test]
    fn frame_throttle_is_disabled_without_work() {
        assert_eq!(
            FrameThrottleFacts {
                force_render: false,
                require_presentation: false,
                has_next_frame_callbacks: false,
                active: false,
                thermal_state: Some(ThermalState::Critical),
            }
            .min_frame_interval(),
            None
        );
    }

    #[test]
    fn frame_throttle_prefers_inactive_over_thermal_pressure() {
        assert_eq!(
            FrameThrottleFacts {
                force_render: true,
                require_presentation: false,
                has_next_frame_callbacks: false,
                active: false,
                thermal_state: Some(ThermalState::Critical),
            }
            .min_frame_interval(),
            Some(INACTIVE_FRAME_INTERVAL)
        );
    }

    #[test]
    fn frame_throttle_limits_active_thermal_pressure() {
        assert_eq!(
            FrameThrottleFacts {
                force_render: false,
                require_presentation: true,
                has_next_frame_callbacks: false,
                active: true,
                thermal_state: Some(ThermalState::Serious),
            }
            .min_frame_interval(),
            Some(THERMAL_THROTTLE_FRAME_INTERVAL)
        );
    }

    #[test]
    fn frame_wait_requires_previous_frame_and_interval() {
        let now = Instant::now();
        assert!(!frame_should_wait(now, None, Some(INACTIVE_FRAME_INTERVAL)));
        assert!(!frame_should_wait(now, Some(now), None));
        assert!(frame_should_wait(
            now,
            Some(now),
            Some(INACTIVE_FRAME_INTERVAL)
        ));
    }

    #[test]
    fn present_decision_uses_active_high_rate_input() {
        assert!(
            PresentFacts {
                require_presentation: false,
                needs_present: false,
                active: true,
                high_rate_input: true,
            }
            .needs_present()
        );
        assert!(
            !PresentFacts {
                require_presentation: false,
                needs_present: false,
                active: false,
                high_rate_input: true,
            }
            .needs_present()
        );
    }
}
