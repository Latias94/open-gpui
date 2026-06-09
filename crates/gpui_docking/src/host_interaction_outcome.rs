use crate::{
    DockActionApplyError, DockActionOutcome, DockHost, DockViewportActivationTarget,
    DockViewportDropRouteOutcome,
};
use open_gpui::Context;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockHostInteractionOutcome {
    Idle,
    Changed,
    Notify,
    RoutedDrop {
        outcome: DockViewportDropRouteOutcome,
        changed: bool,
    },
    Rejected(DockActionApplyError),
}

impl DockHostInteractionOutcome {
    pub(crate) fn changed(&self) -> bool {
        match self {
            Self::Changed => true,
            Self::RoutedDrop { outcome, changed } => {
                *changed
                    || outcome
                        .action_result()
                        .map(DockActionOutcome::changed)
                        .unwrap_or(false)
            }
            Self::Idle | Self::Notify | Self::Rejected(_) => false,
        }
    }

    pub(crate) fn finish(self, cx: &mut Context<DockHost>) -> bool {
        let changed = self.changed();
        let should_notify = self.should_notify();
        if let Some(activation) = self.activation_target() {
            let activation_space = activation.space;
            let focus_item = activation.focus_item;
            let _ = activation.window.update(cx, move |view, window, cx| {
                window.activate_window();
                if let Some(focus_item) = focus_item
                    && let Ok(host) = view.downcast::<DockHost>()
                {
                    host.update(cx, |host, cx| {
                        if host.space() == &activation_space && host.request_panel_focus(focus_item)
                        {
                            cx.notify();
                        }
                    });
                }
            });
        }
        if should_notify {
            cx.notify();
        }
        changed
    }

    pub(crate) fn from_session_changed(changed: bool) -> Self {
        if changed { Self::Notify } else { Self::Idle }
    }

    pub(crate) fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Rejected(error), _) | (_, Self::Rejected(error)) => Self::Rejected(error),
            (Self::RoutedDrop { outcome, changed }, other)
            | (other, Self::RoutedDrop { outcome, changed }) => Self::RoutedDrop {
                outcome,
                changed: changed || other.changed(),
            },
            (Self::Changed, _) | (_, Self::Changed) => Self::Changed,
            (Self::Notify, _) | (_, Self::Notify) => Self::Notify,
            (Self::Idle, Self::Idle) => Self::Idle,
        }
    }

    pub(crate) fn from_commit_result(
        result: Result<DockActionOutcome, DockActionApplyError>,
        notify_on_unchanged: bool,
    ) -> Self {
        match result {
            Ok(DockActionOutcome::Changed) => Self::Changed,
            Ok(DockActionOutcome::Unchanged) if notify_on_unchanged => Self::Notify,
            Ok(DockActionOutcome::Unchanged) => Self::Idle,
            Err(error) => Self::Rejected(error),
        }
    }

    fn should_notify(&self) -> bool {
        matches!(
            self,
            Self::Changed | Self::Notify | Self::RoutedDrop { .. } | Self::Rejected(_)
        )
    }

    #[cfg(test)]
    pub(crate) fn routed_drop_outcome(&self) -> Option<&DockViewportDropRouteOutcome> {
        match self {
            Self::RoutedDrop { outcome, .. } => Some(outcome),
            Self::Idle | Self::Changed | Self::Notify | Self::Rejected(_) => None,
        }
    }

    pub(crate) fn from_routed_drop_result(
        result: Result<DockViewportDropRouteOutcome, DockActionApplyError>,
    ) -> Self {
        match result {
            Ok(outcome) => Self::RoutedDrop {
                outcome,
                changed: false,
            },
            Err(error) => Self::Rejected(error),
        }
    }

    fn activation_target(&self) -> Option<DockViewportActivationTarget> {
        match self {
            Self::RoutedDrop { outcome, .. } => outcome.activation_target(),
            Self::Idle | Self::Changed | Self::Notify | Self::Rejected(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockItemId, DockViewportDropActionOutcome, host_test_support::space,
        viewport_test_support::handle,
    };

    #[test]
    fn routed_drop_outcome_preserves_viewport_side_effects() {
        let window = handle(1);
        let routed = DockViewportDropRouteOutcome::Action(DockViewportDropActionOutcome {
            action: DockActionOutcome::Changed,
            activation: Some(DockViewportActivationTarget {
                space: space(),
                window,
                focus_item: Some(DockItemId::from("a")),
            }),
        });
        let outcome = DockHostInteractionOutcome::from_routed_drop_result(Ok(routed.clone()));

        assert!(outcome.changed());
        assert_eq!(outcome.routed_drop_outcome(), Some(&routed));
        assert_eq!(
            outcome.activation_target().map(|target| target.window),
            Some(window)
        );
        assert_eq!(
            outcome
                .activation_target()
                .and_then(|target| target.focus_item),
            Some(DockItemId::from("a"))
        );
    }
}
