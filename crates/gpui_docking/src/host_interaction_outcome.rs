use crate::{
    DockActionApplyError, DockActionOutcome, DockHost, DockViewportActivationTransaction,
    DockViewportDropRouteOutcome, viewport_activation::apply_viewport_activation_transaction,
};
use open_gpui::Context;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum DockHostInteractionOutcome {
    Idle,
    Notify {
        changed: bool,
    },
    RoutedDrop {
        outcome: DockViewportDropRouteOutcome,
        changed: bool,
    },
    Rejected(DockActionApplyError),
}

impl DockHostInteractionOutcome {
    pub(crate) fn changed(&self) -> bool {
        match self {
            Self::Notify { changed } => *changed,
            Self::RoutedDrop { outcome, changed } => {
                *changed
                    || outcome
                        .action_result()
                        .map(DockActionOutcome::changed)
                        .unwrap_or(false)
                    || outcome.has_window_effects()
                    || outcome.activation_transaction().is_some()
            }
            Self::Idle | Self::Rejected(_) => false,
        }
    }

    pub(crate) fn finish(self, cx: &mut Context<DockHost>) -> bool {
        let changed = self.changed();
        let should_notify = self.should_notify();
        let activation_changed =
            apply_viewport_activation_transaction(self.activation_transaction(), cx).changed();
        if should_notify && !activation_changed {
            cx.notify();
        }
        changed
    }

    pub(crate) fn from_changed() -> Self {
        Self::Notify { changed: true }
    }

    pub(crate) fn from_notify() -> Self {
        Self::Notify { changed: false }
    }

    pub(crate) fn from_session_changed(changed: bool) -> Self {
        if changed {
            Self::from_notify()
        } else {
            Self::Idle
        }
    }

    pub(crate) fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Rejected(error), _) | (_, Self::Rejected(error)) => Self::Rejected(error),
            (Self::RoutedDrop { outcome, changed }, other)
            | (other, Self::RoutedDrop { outcome, changed }) => Self::RoutedDrop {
                outcome,
                changed: changed || other.changed(),
            },
            (Self::Notify { changed }, other) | (other, Self::Notify { changed }) => Self::Notify {
                changed: changed || other.changed(),
            },
            (Self::Idle, Self::Idle) => Self::Idle,
        }
    }

    pub(crate) fn from_commit_result(
        result: Result<DockActionOutcome, DockActionApplyError>,
        notify_on_unchanged: bool,
    ) -> Self {
        match result {
            Ok(DockActionOutcome::Changed) => Self::from_changed(),
            Ok(DockActionOutcome::Unchanged) if notify_on_unchanged => Self::from_notify(),
            Ok(DockActionOutcome::Unchanged) => Self::Idle,
            Err(error) => Self::Rejected(error),
        }
    }

    fn should_notify(&self) -> bool {
        matches!(
            self,
            Self::Notify { .. } | Self::RoutedDrop { .. } | Self::Rejected(_)
        )
    }

    #[cfg(test)]
    pub(crate) fn routed_drop_outcome(&self) -> Option<&DockViewportDropRouteOutcome> {
        match self {
            Self::RoutedDrop { outcome, .. } => Some(outcome),
            Self::Idle | Self::Notify { .. } | Self::Rejected(_) => None,
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

    fn activation_transaction(&self) -> Option<DockViewportActivationTransaction> {
        match self {
            Self::RoutedDrop { outcome, .. } => outcome.activation_transaction(),
            Self::Idle | Self::Notify { .. } | Self::Rejected(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DockItemId, DockViewportDropActionOutcome, DockViewportFocusRequest,
        DockViewportWindowEffects, host_test_support::space, viewport_test_support::handle,
    };

    #[test]
    fn routed_drop_outcome_preserves_viewport_side_effects() {
        let window = handle(1);
        let routed = DockViewportDropRouteOutcome::Action(DockViewportDropActionOutcome::new(
            DockActionOutcome::Changed,
            Some(DockViewportActivationTransaction::new(
                space(),
                window,
                DockViewportFocusRequest::panel(DockItemId::from("a")),
            )),
        ));
        let outcome = DockHostInteractionOutcome::from_routed_drop_result(Ok(routed.clone()));

        assert!(outcome.changed());
        assert_eq!(outcome.routed_drop_outcome(), Some(&routed));
        assert_eq!(
            outcome
                .activation_transaction()
                .map(|target| target.window()),
            Some(window)
        );
        assert_eq!(
            outcome
                .activation_transaction()
                .map(|target| target.focus_request().clone()),
            Some(DockViewportFocusRequest::panel(DockItemId::from("a")))
        );
    }

    #[test]
    fn routed_drop_with_activation_counts_as_changed_even_when_graph_is_unchanged() {
        let window = handle(2);
        let routed = DockViewportDropRouteOutcome::Action(DockViewportDropActionOutcome::new(
            DockActionOutcome::Unchanged,
            Some(DockViewportActivationTransaction::new(
                space(),
                window,
                DockViewportFocusRequest::panel(DockItemId::from("a")),
            )),
        ));
        let outcome = DockHostInteractionOutcome::from_routed_drop_result(Ok(routed));

        assert!(outcome.changed());
    }

    #[test]
    fn routed_drop_with_window_effects_counts_as_changed_without_activation() {
        let window = handle(3);
        let routed = DockViewportDropRouteOutcome::Action(
            DockViewportDropActionOutcome::new(DockActionOutcome::Unchanged, None)
                .with_window_effects(DockViewportWindowEffects::new(
                    Vec::new(),
                    [window],
                    Vec::new(),
                )),
        );
        let outcome = DockHostInteractionOutcome::from_routed_drop_result(Ok(routed.clone()));

        assert!(outcome.changed());
        assert_eq!(outcome.routed_drop_outcome(), Some(&routed));
    }
}
