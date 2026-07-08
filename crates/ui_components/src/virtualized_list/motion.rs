use open_gpui_motion::{
    MotionFrameDemand, MotionFrameHostResetReason,
    advanced::{MotionModel, MotionPreset, MotionScalarController},
};
use open_gpui_ui_core::UiPx;
use std::time::{Duration, Instant};

use super::render_plan::VirtualizedListRenderPlan;
use super::style::nonnegative_px;

const ACTIVE_INDICATOR_EPSILON: f32 = 0.001;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VirtualizedListActiveIndicatorAxis {
    Top,
    Height,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct VirtualizedListActiveIndicatorBounds {
    top: UiPx,
    height: UiPx,
}

impl VirtualizedListActiveIndicatorBounds {
    const fn new(top: UiPx, height: UiPx) -> Self {
        Self {
            top,
            height: nonnegative_px(height),
        }
    }

    fn approximately_equals(self, other: Self) -> bool {
        (self.top.as_f32() - other.top.as_f32()).abs() <= ACTIVE_INDICATOR_EPSILON
            && (self.height.as_f32() - other.height.as_f32()).abs() <= ACTIVE_INDICATOR_EPSILON
    }
}

#[derive(Debug, Clone, PartialEq)]
struct VirtualizedListActiveIndicatorTarget {
    key: String,
    bounds: VirtualizedListActiveIndicatorBounds,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct VirtualizedListActiveIndicatorSnapshot {
    top: UiPx,
    height: UiPx,
    frame_demand: MotionFrameDemand,
}

impl VirtualizedListActiveIndicatorSnapshot {
    const fn new(top: UiPx, height: UiPx, frame_demand: MotionFrameDemand) -> Self {
        Self {
            top,
            height,
            frame_demand,
        }
    }

    pub(super) const fn top(self) -> UiPx {
        self.top
    }

    pub(super) const fn height(self) -> UiPx {
        self.height
    }

    pub(super) const fn frame_demand(self) -> MotionFrameDemand {
        self.frame_demand
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct VirtualizedListActiveIndicatorUpdate {
    frame_demand: MotionFrameDemand,
    reset_reason: Option<MotionFrameHostResetReason>,
}

impl VirtualizedListActiveIndicatorUpdate {
    const fn new(
        frame_demand: MotionFrameDemand,
        reset_reason: Option<MotionFrameHostResetReason>,
    ) -> Self {
        Self {
            frame_demand,
            reset_reason,
        }
    }

    const fn idle() -> Self {
        Self::new(MotionFrameDemand::Idle, None)
    }

    pub(super) const fn frame_demand(self) -> MotionFrameDemand {
        self.frame_demand
    }

    pub(super) const fn reset_reason(self) -> Option<MotionFrameHostResetReason> {
        self.reset_reason
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct VirtualizedListActiveIndicatorState {
    pub(super) key: String,
    target: VirtualizedListActiveIndicatorBounds,
    sampled: VirtualizedListActiveIndicatorBounds,
    started_at: Instant,
    controller: MotionScalarController<VirtualizedListActiveIndicatorAxis>,
    frame_demand: MotionFrameDemand,
}

impl VirtualizedListActiveIndicatorState {
    fn immediate(key: String, bounds: VirtualizedListActiveIndicatorBounds, now: Instant) -> Self {
        Self {
            key,
            target: bounds,
            sampled: bounds,
            started_at: now,
            controller: active_indicator_controller_for_bounds(
                bounds,
                bounds,
                MotionPreset::immediate().resolve_model(),
            ),
            frame_demand: MotionFrameDemand::Idle,
        }
    }

    fn sample_motion(&mut self, now: Instant) -> MotionFrameDemand {
        let sample = self
            .controller
            .sample_at(now.saturating_duration_since(self.started_at));
        self.sampled = active_indicator_bounds_from_sample(&sample, self.target);
        self.frame_demand = sample.frame_demand();
        if sample.complete() {
            self.sampled = self.target;
            self.frame_demand = MotionFrameDemand::Idle;
        }
        self.frame_demand
    }

    fn sample_at(&mut self, now: Instant) -> VirtualizedListActiveIndicatorUpdate {
        let frame_demand = self.sample_motion(now);
        let reset_reason =
            (!frame_demand.needs_frame()).then_some(MotionFrameHostResetReason::Finish);
        VirtualizedListActiveIndicatorUpdate::new(frame_demand, reset_reason)
    }

    fn retarget(
        &mut self,
        target: VirtualizedListActiveIndicatorTarget,
        now: Instant,
        model: MotionModel,
    ) -> VirtualizedListActiveIndicatorUpdate {
        let sampled = self.sampled_after_update(now);
        let reset_reason = if self.key == target.key {
            MotionFrameHostResetReason::Retarget
        } else {
            MotionFrameHostResetReason::MotionIdentityChanged
        };
        if model.is_immediate() || sampled.approximately_equals(target.bounds) {
            *self = Self::immediate(target.key, target.bounds, now);
            return VirtualizedListActiveIndicatorUpdate::new(
                MotionFrameDemand::Idle,
                Some(reset_reason),
            );
        }

        self.key = target.key;
        self.target = target.bounds;
        self.sampled = sampled;
        self.started_at = now;
        self.controller = active_indicator_controller_for_bounds(sampled, target.bounds, model);
        let frame_demand = self.sample_motion(now);
        VirtualizedListActiveIndicatorUpdate::new(frame_demand, Some(reset_reason))
    }

    fn sampled_after_update(&mut self, now: Instant) -> VirtualizedListActiveIndicatorBounds {
        self.sample_motion(now);
        self.sampled
    }

    fn cancel_at(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.started_at);
        self.controller
            .cancel(&VirtualizedListActiveIndicatorAxis::Top, elapsed);
        self.controller
            .cancel(&VirtualizedListActiveIndicatorAxis::Height, elapsed);
        self.controller.prune_terminal_at(elapsed);
        self.frame_demand = MotionFrameDemand::Idle;
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct VirtualizedListActiveIndicatorRuntime {
    pub(super) state: Option<VirtualizedListActiveIndicatorState>,
}

impl VirtualizedListActiveIndicatorRuntime {
    pub(super) fn sync(
        &mut self,
        plan: &VirtualizedListRenderPlan,
        now: Instant,
        model: MotionModel,
    ) -> VirtualizedListActiveIndicatorUpdate {
        let Some(target) = active_indicator_target(plan) else {
            return self.hide_at(now);
        };

        let Some(state) = self.state.as_mut() else {
            self.state = Some(VirtualizedListActiveIndicatorState::immediate(
                target.key,
                target.bounds,
                now,
            ));
            return VirtualizedListActiveIndicatorUpdate::new(
                MotionFrameDemand::Idle,
                Some(MotionFrameHostResetReason::MotionIdentityChanged),
            );
        };

        if state.key == target.key && state.target.approximately_equals(target.bounds) {
            return state.sample_at(now);
        }

        state.retarget(target, now, model)
    }

    fn hide_at(&mut self, now: Instant) -> VirtualizedListActiveIndicatorUpdate {
        if let Some(state) = self.state.as_mut() {
            state.cancel_at(now);
            self.state = None;
            return VirtualizedListActiveIndicatorUpdate::new(
                MotionFrameDemand::Idle,
                Some(MotionFrameHostResetReason::Cancel),
            );
        }
        VirtualizedListActiveIndicatorUpdate::idle()
    }

    pub(super) fn snapshot(&self) -> Option<VirtualizedListActiveIndicatorSnapshot> {
        self.state.as_ref().map(|state| {
            VirtualizedListActiveIndicatorSnapshot::new(
                state.sampled.top,
                state.sampled.height,
                state.frame_demand,
            )
        })
    }
}

fn active_indicator_target(
    plan: &VirtualizedListRenderPlan,
) -> Option<VirtualizedListActiveIndicatorTarget> {
    plan.rows()
        .iter()
        .find(|row| row.active() && !row.disabled())
        .map(|row| VirtualizedListActiveIndicatorTarget {
            key: row.key().to_owned(),
            bounds: VirtualizedListActiveIndicatorBounds::new(
                row.virtual_start(),
                row.virtual_size(),
            ),
        })
}

fn active_indicator_controller_for_bounds(
    from: VirtualizedListActiveIndicatorBounds,
    to: VirtualizedListActiveIndicatorBounds,
    model: MotionModel,
) -> MotionScalarController<VirtualizedListActiveIndicatorAxis> {
    let mut controller = MotionScalarController::new();
    controller.start(
        VirtualizedListActiveIndicatorAxis::Top,
        model,
        from.top.as_f32(),
        to.top.as_f32(),
        0.0,
        Duration::ZERO,
    );
    controller.start(
        VirtualizedListActiveIndicatorAxis::Height,
        model,
        from.height.as_f32(),
        to.height.as_f32(),
        0.0,
        Duration::ZERO,
    );
    controller
}

fn active_indicator_bounds_from_sample(
    sample: &open_gpui_motion::advanced::MotionScalarControllerSample<
        VirtualizedListActiveIndicatorAxis,
    >,
    target: VirtualizedListActiveIndicatorBounds,
) -> VirtualizedListActiveIndicatorBounds {
    let top = sample
        .track(&VirtualizedListActiveIndicatorAxis::Top)
        .map(|track| UiPx::new(track.sample().value()))
        .unwrap_or(target.top);
    let height = sample
        .track(&VirtualizedListActiveIndicatorAxis::Height)
        .map(|track| UiPx::new(track.sample().value()))
        .unwrap_or(target.height);
    VirtualizedListActiveIndicatorBounds::new(top, height)
}
