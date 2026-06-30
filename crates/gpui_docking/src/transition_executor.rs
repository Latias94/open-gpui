use crate::transition_geometry::DockTransitionPlan;
use open_gpui::Window;
use open_gpui_ui_core::MotionSpec;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockTransitionExecution {
    pub(crate) plan: DockTransitionPlan,
    pub(crate) spec: MotionSpec,
    pub(crate) state: DockTransitionExecutionState,
}

/// Execution state returned by the docking transition executor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockTransitionExecutionState {
    /// Transition reached the final scene immediately.
    Immediate,
    /// Transition requested an animation frame and kept the final scene as the semantic target.
    Scheduled,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub(crate) struct DockTransitionExecutor {
    current: Option<DockTransitionExecution>,
}

impl DockTransitionExecutor {
    pub(crate) fn execute(
        &mut self,
        plan: DockTransitionPlan,
        spec: MotionSpec,
        window: Option<&Window>,
    ) -> &DockTransitionExecution {
        let state = if plan.is_immediate() || spec.is_immediate() {
            DockTransitionExecutionState::Immediate
        } else {
            if let Some(window) = window {
                window.request_animation_frame();
            }
            DockTransitionExecutionState::Scheduled
        };

        self.current = Some(DockTransitionExecution { plan, spec, state });
        self.current.as_ref().expect("execution should be stored")
    }

    #[cfg(test)]
    pub(crate) fn clear(&mut self) -> Option<DockTransitionExecution> {
        self.current.take()
    }
}
