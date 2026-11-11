use anyhow::Context as _;
use open_gpui_core_util::ResultExt;
use open_gpui_scheduler::Instant;
use smallvec::SmallVec;
use std::time::Duration;

use crate::{App, Entity, EntityId, FocusId, KeyContext, Keystroke, Subscription, Task, Window};

const PENDING_INPUT_TIMEOUT: Duration = Duration::from_secs(1);

/// Pending input for a potential multi-stroke key binding.
pub struct PendingInputStatus<'a> {
    keystrokes: &'a [Keystroke],
    timeout: Option<PendingInputTimeoutStatus>,
}

impl<'a> PendingInputStatus<'a> {
    /// Returns the keystrokes entered so far.
    pub fn keystrokes(&self) -> &'a [Keystroke] {
        self.keystrokes
    }

    /// Returns the timeout state for flushing this input, if it needs a timeout.
    pub fn timeout(&self) -> Option<PendingInputTimeoutStatus> {
        self.timeout
    }
}

/// The timeout state for pending input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PendingInputTimeoutStatus {
    remaining: Duration,
    started_at: Option<Instant>,
}

impl PendingInputTimeoutStatus {
    /// Returns the full timeout duration.
    pub fn duration(&self) -> Duration {
        PENDING_INPUT_TIMEOUT
    }

    /// Returns the duration remaining before pending input is flushed.
    pub fn remaining(&self, cx: &App) -> Duration {
        self.started_at.map_or(self.remaining, |started_at| {
            let elapsed = cx
                .background_executor()
                .now()
                .saturating_duration_since(started_at);
            self.remaining.saturating_sub(elapsed)
        })
    }

    /// Returns whether the timeout is paused.
    pub fn is_paused(&self) -> bool {
        self.started_at.is_none()
    }
}

#[derive(Debug)]
enum PendingInputTimeout {
    Running {
        started_at: Instant,
        remaining: Duration,
        _task: Task<()>,
    },
    Paused {
        remaining: Duration,
        owner_id: EntityId,
        _release_subscription: Subscription,
    },
}

impl PendingInputTimeout {
    fn status(&self) -> PendingInputTimeoutStatus {
        match *self {
            Self::Running {
                started_at,
                remaining,
                ..
            } => PendingInputTimeoutStatus {
                remaining,
                started_at: Some(started_at),
            },
            Self::Paused { remaining, .. } => PendingInputTimeoutStatus {
                remaining,
                started_at: None,
            },
        }
    }
}

#[derive(Default, Debug)]
pub(super) struct PendingInput {
    pub(super) keystrokes: SmallVec<[Keystroke; 1]>,
    pub(super) focus: Option<FocusId>,
    pub(super) context_stack: Option<Vec<KeyContext>>,
    timeout: Option<PendingInputTimeout>,
}

impl PendingInput {
    pub(super) fn update_timeout(&mut self, needs_timeout: bool, window: &Window, cx: &App) {
        match &mut self.timeout {
            Some(PendingInputTimeout::Paused { remaining, .. }) => {
                *remaining = PENDING_INPUT_TIMEOUT;
            }
            timeout if timeout.is_some() || needs_timeout => {
                drop(timeout.take());
                *timeout = Some(window.start_pending_input_timeout(PENDING_INPUT_TIMEOUT, cx));
            }
            _ => {}
        }
    }
}

impl Window {
    /// Returns pending input that can still complete a multi-stroke key binding. Input left over
    /// from a previous focus or key context can never complete one.
    pub fn pending_input(&self) -> Option<PendingInputStatus<'_>> {
        let pending = self.active_pending_input()?;
        Some(PendingInputStatus {
            keystrokes: pending.keystrokes.as_slice(),
            timeout: pending.timeout.as_ref().map(PendingInputTimeout::status),
        })
    }

    /// Pauses or resumes the current pending input timeout on behalf of `owner`.
    ///
    /// A paused timeout resumes automatically if `owner` is released. Returns whether the timeout
    /// state changed. A timeout paused by one owner cannot be resumed by another.
    /// An unaccepted frame cannot change the timeout.
    pub fn set_pending_input_timeout_paused<T: 'static>(
        &mut self,
        owner: &Entity<T>,
        paused: bool,
        cx: &mut App,
    ) -> bool {
        let owner_id = owner.entity_id();
        if !paused {
            return self.resume_pending_input_timeout(owner_id, cx);
        }

        self.replace_pending_input_timeout(cx, |window, timeout, cx| {
            if !matches!(timeout, PendingInputTimeout::Running { .. }) {
                return None;
            }
            let remaining = timeout.status().remaining(cx);
            let release_subscription = window.observe_release(owner, cx, move |_, window, cx| {
                window.resume_pending_input_timeout(owner_id, cx);
            });
            Some(PendingInputTimeout::Paused {
                remaining,
                owner_id,
                _release_subscription: release_subscription,
            })
        })
    }

    fn resume_pending_input_timeout(&mut self, owner_id: EntityId, cx: &mut App) -> bool {
        self.replace_pending_input_timeout(cx, |window, timeout, cx| match *timeout {
            PendingInputTimeout::Paused {
                remaining,
                owner_id: paused_by,
                ..
            } if paused_by == owner_id => Some(window.start_pending_input_timeout(remaining, cx)),
            _ => None,
        })
    }

    fn replace_pending_input_timeout(
        &mut self,
        cx: &mut App,
        replace: impl FnOnce(&Self, &PendingInputTimeout, &mut App) -> Option<PendingInputTimeout>,
    ) -> bool {
        if !self.user_interaction_is_admitted()
            || self
                .candidate_frame_transaction
                .as_ref()
                .is_some_and(|frame| !frame.is_accepted())
        {
            return false;
        }
        let Some(timeout) = self
            .active_pending_input()
            .and_then(|input| input.timeout.as_ref())
        else {
            return false;
        };
        let Some(timeout) = replace(self, timeout, cx) else {
            return false;
        };
        self.pending_input
            .as_mut()
            .expect("timeout belongs to the current pending input")
            .timeout = Some(timeout);
        self.defer_pending_input_changed(cx);
        true
    }

    fn start_pending_input_timeout(&self, remaining: Duration, cx: &App) -> PendingInputTimeout {
        let started_at = cx.background_executor().now();
        let timer = cx.background_executor().timer(remaining);
        let task = self.spawn(cx, async move |cx| {
            timer.await;
            cx.update_when_available(Window::flush_pending_input_timeout)
                .await
                .with_context(|| {
                    format!(
                        "handling pending input timeout for window {:?}",
                        cx.window_handle().window_id()
                    )
                })
                .log_err();
        });
        PendingInputTimeout::Running {
            started_at,
            remaining,
            _task: task,
        }
    }

    fn flush_pending_input_timeout(&mut self, cx: &mut App) {
        let Some(pending) = self.pending_input.take() else {
            return;
        };
        let Some(context) = pending.context_stack.filter(|context| {
            self.user_interaction_is_admitted()
                && self.pending_input_target_is_current(pending.focus, context)
        }) else {
            self.pending_input_changed(cx);
            return;
        };

        let node_id = self.focus_node_id_in_rendered_frame(pending.focus);
        let dispatch_tree = &mut self.rendered_frame.dispatch_tree;
        let dispatch_path = dispatch_tree.dispatch_path(node_id);
        let to_replay = dispatch_tree.flush_dispatch(pending.keystrokes, &dispatch_path);
        self.pending_input_changed(cx);
        self.replay_pending_input(to_replay, pending.focus, &context, cx);
    }
}
