use super::{
    MacDisplayTopologyCandidate, MacDisplayTopologySnapshot, empty_platform_display_snapshot,
};
use open_gpui::{DisplayId, PlatformDisplaySnapshot};
use parking_lot::Mutex;
use std::{
    collections::BTreeMap,
    error::Error,
    fmt, mem,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Weak},
    time::Duration,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MacDisplayTopologyFailure {
    NativeCollection(Arc<str>),
    InvalidCandidate(Arc<str>),
    UnstableDuringCollection,
    SnapshotChanged(u64),
    UnknownTarget(DisplayId),
    GenerationExhausted,
    RequestEpochExhausted,
    ListenerIdentityExhausted,
}

impl fmt::Display for MacDisplayTopologyFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NativeCollection(message) | Self::InvalidCandidate(message) => {
                formatter.write_str(message)
            }
            Self::UnstableDuringCollection => {
                formatter.write_str("display topology changed during native collection")
            }
            Self::SnapshotChanged(generation) => write!(
                formatter,
                "native display topology no longer matches publication generation {generation}"
            ),
            Self::UnknownTarget(display_id) => {
                write!(
                    formatter,
                    "display {display_id:?} is not in the publication"
                )
            }
            Self::GenerationExhausted => {
                formatter.write_str("display topology generation exhausted")
            }
            Self::RequestEpochExhausted => {
                formatter.write_str("display topology refresh request epoch exhausted")
            }
            Self::ListenerIdentityExhausted => {
                formatter.write_str("display topology listener identity exhausted")
            }
        }
    }
}

impl Error for MacDisplayTopologyFailure {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MacDisplayTopologyUnavailable {
    RefreshPending {
        request_epoch: u64,
        retained_generation: Option<u64>,
    },
    Degraded {
        request_epoch: u64,
        retained_generation: Option<u64>,
        failure: MacDisplayTopologyFailure,
    },
}

impl fmt::Display for MacDisplayTopologyUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RefreshPending {
                request_epoch,
                retained_generation,
            } => write!(
                formatter,
                "display topology refresh {request_epoch} is pending; retained generation: {retained_generation:?}"
            ),
            Self::Degraded {
                request_epoch,
                retained_generation,
                failure,
            } => write!(
                formatter,
                "display topology refresh {request_epoch} failed while retaining generation {retained_generation:?}: {failure}"
            ),
        }
    }
}

impl Error for MacDisplayTopologyUnavailable {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RefreshPending { .. } => None,
            Self::Degraded { failure, .. } => Some(failure),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum MacDisplayTopologyState {
    Complete(MacDisplayTopologySnapshot),
    RefreshPending {
        retained: Option<MacDisplayTopologySnapshot>,
    },
    Degraded {
        retained: Option<MacDisplayTopologySnapshot>,
        failure: MacDisplayTopologyFailure,
    },
}

impl MacDisplayTopologyState {
    fn retained_snapshot(&self) -> Option<MacDisplayTopologySnapshot> {
        match self {
            Self::Complete(snapshot) => Some(snapshot.clone()),
            Self::RefreshPending { retained } | Self::Degraded { retained, .. } => retained.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MacDisplayTopologyRefreshRequest {
    pub(crate) request_epoch: u64,
    pub(crate) should_schedule: bool,
}

// The final delay repeats until a stable double sample succeeds or a newer notification replaces
// the chain. Bounding the interval instead of the attempt count preserves eventual recovery.
const MAC_DISPLAY_TOPOLOGY_RETRY_DELAYS: [Duration; 5] = [
    Duration::from_millis(16),
    Duration::from_millis(64),
    Duration::from_millis(250),
    Duration::from_secs(1),
    Duration::from_secs(2),
];

/// One delayed, coalesced retry owned by the display-topology authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MacDisplayTopologyRetry {
    retry_epoch: u64,
    attempt: usize,
    delay: Duration,
}

impl MacDisplayTopologyRetry {
    pub(crate) fn retry_epoch(self) -> u64 {
        self.retry_epoch
    }

    pub(crate) fn attempt(self) -> usize {
        self.attempt
    }

    pub(crate) fn delay(self) -> Duration {
        self.delay
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MacDisplayTopologyRefresh {
    Unchanged {
        generation: u64,
    },
    Published {
        previous_generation: Option<u64>,
        generation: u64,
    },
    RetainedAfterFailure {
        generation: Option<u64>,
        failure: MacDisplayTopologyFailure,
        retry: Option<MacDisplayTopologyRetry>,
    },
    Superseded {
        generation: Option<u64>,
        request_epoch: u64,
    },
}

pub(crate) struct MacDisplayTopologyAuthority {
    state: MacDisplayTopologyState,
    request_epoch: u64,
    refresh_scheduled: bool,
    retry_epoch: u64,
    retry_attempts_started: usize,
    scheduled_retry: Option<MacDisplayTopologyRetry>,
}

pub(crate) type MacDisplayTopologyListener = Arc<dyn Fn(u64) + Send + Sync>;

struct MacDisplayTopologyShared {
    authority: MacDisplayTopologyAuthority,
    next_listener_id: u64,
    listeners: BTreeMap<u64, MacDisplayTopologyListener>,
}

/// Shared access to the macOS display-topology publication.
///
/// Native collection always happens outside the lock and remains UI-thread-only. The synchronized
/// handle is safe to retain inside `MacWindowState`; window callbacks clone an exact detached
/// snapshot and validate their callback-provided `NSScreen` instead of querying every display
/// getter independently.
#[derive(Clone)]
pub(crate) struct MacDisplayTopologyHandle(Arc<Mutex<MacDisplayTopologyShared>>);

#[derive(Clone)]
pub(crate) struct MacDisplayTopologyWeak(Weak<Mutex<MacDisplayTopologyShared>>);

/// Removes one display-publication listener when its owning window is destroyed.
pub(crate) struct MacDisplayTopologySubscription {
    shared: Weak<Mutex<MacDisplayTopologyShared>>,
    listener_id: u64,
}

impl Drop for MacDisplayTopologySubscription {
    fn drop(&mut self) {
        if let Some(shared) = self.shared.upgrade() {
            shared.lock().listeners.remove(&self.listener_id);
        }
    }
}

impl MacDisplayTopologyHandle {
    pub(crate) fn from_native() -> Self {
        Self::from_authority(MacDisplayTopologyAuthority::from_native())
    }

    fn from_authority(authority: MacDisplayTopologyAuthority) -> Self {
        Self(Arc::new(Mutex::new(MacDisplayTopologyShared {
            authority,
            next_listener_id: 0,
            listeners: BTreeMap::new(),
        })))
    }

    pub(crate) fn downgrade(&self) -> MacDisplayTopologyWeak {
        MacDisplayTopologyWeak(Arc::downgrade(&self.0))
    }

    pub(crate) fn retained_snapshot(&self) -> Option<MacDisplayTopologySnapshot> {
        self.0.lock().authority.retained_snapshot()
    }

    pub(crate) fn retained_platform_snapshot(&self) -> PlatformDisplaySnapshot {
        self.0.lock().authority.retained_platform_snapshot()
    }

    pub(crate) fn exact_snapshot(
        &self,
    ) -> Result<MacDisplayTopologySnapshot, MacDisplayTopologyUnavailable> {
        self.0.lock().authority.exact_snapshot()
    }

    pub(crate) fn request_refresh(&self) -> MacDisplayTopologyRefreshRequest {
        self.0.lock().authority.request_refresh()
    }

    pub(crate) fn begin_scheduled_refresh(&self) -> Option<u64> {
        self.0.lock().authority.begin_scheduled_refresh()
    }

    pub(crate) fn begin_retry(&self, retry_epoch: u64) -> Option<u64> {
        self.0.lock().authority.begin_retry(retry_epoch)
    }

    pub(crate) fn cancel_retry(&self, retry_epoch: u64) {
        self.0.lock().authority.cancel_retry(retry_epoch);
    }

    pub(crate) fn finish_refresh(
        &self,
        request_epoch: u64,
        candidate: Result<MacDisplayTopologyCandidate, MacDisplayTopologyFailure>,
    ) -> MacDisplayTopologyRefresh {
        let (refresh, publication) = {
            let mut shared = self.0.lock();
            let refresh = shared.authority.finish_refresh(request_epoch, candidate);
            let generation = match &refresh {
                MacDisplayTopologyRefresh::Unchanged { generation }
                | MacDisplayTopologyRefresh::Published { generation, .. } => Some(*generation),
                MacDisplayTopologyRefresh::RetainedAfterFailure { .. }
                | MacDisplayTopologyRefresh::Superseded { .. } => None,
            };
            let publication = generation.map(|generation| {
                (
                    generation,
                    shared.listeners.values().cloned().collect::<Vec<_>>(),
                )
            });
            (refresh, publication)
        };
        if let Some((generation, listeners)) = publication {
            for listener in listeners {
                notify_display_topology_listener(&listener, generation);
            }
        }
        refresh
    }

    pub(crate) fn refresh_candidate_from_native()
    -> Result<MacDisplayTopologyCandidate, MacDisplayTopologyFailure> {
        MacDisplayTopologyAuthority::refresh_candidate_from_native()
    }

    /// Registers one generation observer without a publication race.
    ///
    /// The listener is invoked once with the current retained generation after the registry lock is
    /// released, so registration cannot miss a same-generation refresh. Future successful
    /// refreshes invoke it for both `Published` and `Unchanged`, allowing a window whose native
    /// callback arrived first to perform one mandatory event-driven re-sample. Listeners run
    /// synchronously on the UI thread and must only enqueue that re-sample; they must not perform
    /// native observation or invoke application callbacks on the publication stack.
    pub(crate) fn subscribe_publications(
        &self,
        observed_generation: u64,
        listener: MacDisplayTopologyListener,
    ) -> Result<MacDisplayTopologySubscription, MacDisplayTopologyFailure> {
        let (subscription, notify_generation) = {
            let mut shared = self.0.lock();
            let notify_generation = shared
                .authority
                .retained_snapshot()
                .map(|snapshot| snapshot.generation());
            if notify_generation.is_none_or(|generation| generation < observed_generation) {
                return Err(MacDisplayTopologyFailure::SnapshotChanged(
                    observed_generation,
                ));
            }
            let Some(listener_id) = shared.next_listener_id.checked_add(1) else {
                return Err(MacDisplayTopologyFailure::ListenerIdentityExhausted);
            };
            shared.next_listener_id = listener_id;
            shared.listeners.insert(listener_id, listener.clone());
            (
                MacDisplayTopologySubscription {
                    shared: Arc::downgrade(&self.0),
                    listener_id,
                },
                notify_generation,
            )
        };
        if let Some(generation) = notify_generation {
            notify_display_topology_listener(&listener, generation);
        }
        Ok(subscription)
    }
}

impl MacDisplayTopologyWeak {
    pub(crate) fn upgrade(&self) -> Option<MacDisplayTopologyHandle> {
        self.0.upgrade().map(MacDisplayTopologyHandle)
    }
}

fn notify_display_topology_listener(listener: &MacDisplayTopologyListener, generation: u64) {
    if let Err(payload) = catch_unwind(AssertUnwindSafe(|| listener(generation))) {
        log::error!(
            "macOS display-topology publication listener panicked for generation {generation}"
        );
        // Panic payloads are arbitrary user values; dropping one here could panic across the
        // native callback boundary that this catch is meant to contain.
        mem::forget(payload);
    }
}

impl MacDisplayTopologyAuthority {
    pub(crate) fn from_native() -> Self {
        Self::from_initial_candidate(MacDisplayTopologyCandidate::from_native())
    }

    fn from_initial_candidate(
        candidate: Result<MacDisplayTopologyCandidate, MacDisplayTopologyFailure>,
    ) -> Self {
        let state = match candidate {
            Ok(candidate) => {
                MacDisplayTopologyState::Complete(MacDisplayTopologySnapshot::new(1, candidate))
            }
            Err(failure) => MacDisplayTopologyState::Degraded {
                retained: None,
                failure,
            },
        };
        Self {
            state,
            request_epoch: 0,
            refresh_scheduled: false,
            retry_epoch: 0,
            retry_attempts_started: 0,
            scheduled_retry: None,
        }
    }

    pub(crate) fn retained_snapshot(&self) -> Option<MacDisplayTopologySnapshot> {
        self.state.retained_snapshot()
    }

    pub(crate) fn retained_platform_snapshot(&self) -> PlatformDisplaySnapshot {
        self.retained_snapshot()
            .map(|snapshot| snapshot.platform_snapshot())
            .unwrap_or_else(empty_platform_display_snapshot)
    }

    pub(crate) fn exact_snapshot(
        &self,
    ) -> Result<MacDisplayTopologySnapshot, MacDisplayTopologyUnavailable> {
        match &self.state {
            MacDisplayTopologyState::Complete(snapshot) => Ok(snapshot.clone()),
            MacDisplayTopologyState::RefreshPending { retained } => {
                Err(MacDisplayTopologyUnavailable::RefreshPending {
                    request_epoch: self.request_epoch,
                    retained_generation: retained.as_ref().map(|snapshot| snapshot.generation),
                })
            }
            MacDisplayTopologyState::Degraded { retained, failure } => {
                Err(MacDisplayTopologyUnavailable::Degraded {
                    request_epoch: self.request_epoch,
                    retained_generation: retained.as_ref().map(|snapshot| snapshot.generation),
                    failure: failure.clone(),
                })
            }
        }
    }

    pub(crate) fn request_refresh(&mut self) -> MacDisplayTopologyRefreshRequest {
        self.cancel_retry_chain();
        self.request_refresh_without_retry_reset()
    }

    fn request_refresh_without_retry_reset(&mut self) -> MacDisplayTopologyRefreshRequest {
        let retained = self.state.retained_snapshot();
        let Some(request_epoch) = self.request_epoch.checked_add(1) else {
            self.state = MacDisplayTopologyState::Degraded {
                retained,
                failure: MacDisplayTopologyFailure::RequestEpochExhausted,
            };
            self.refresh_scheduled = false;
            self.cancel_retry_chain();
            return MacDisplayTopologyRefreshRequest {
                request_epoch: self.request_epoch,
                should_schedule: false,
            };
        };
        self.request_epoch = request_epoch;
        self.state = MacDisplayTopologyState::RefreshPending { retained };
        let should_schedule = !self.refresh_scheduled;
        self.refresh_scheduled = true;
        MacDisplayTopologyRefreshRequest {
            request_epoch,
            should_schedule,
        }
    }

    pub(crate) fn begin_scheduled_refresh(&mut self) -> Option<u64> {
        if !self.refresh_scheduled {
            return None;
        }
        self.refresh_scheduled = false;
        matches!(self.state, MacDisplayTopologyState::RefreshPending { .. })
            .then_some(self.request_epoch)
    }

    pub(crate) fn begin_retry(&mut self, retry_epoch: u64) -> Option<u64> {
        if self
            .scheduled_retry
            .is_none_or(|retry| retry.retry_epoch != retry_epoch)
        {
            return None;
        }
        self.scheduled_retry = None;
        self.request_refresh_without_retry_reset();
        self.begin_scheduled_refresh()
    }

    pub(crate) fn cancel_retry(&mut self, retry_epoch: u64) {
        if self
            .scheduled_retry
            .is_some_and(|retry| retry.retry_epoch == retry_epoch)
        {
            self.scheduled_retry = None;
            self.retry_attempts_started = 0;
        }
    }

    fn schedule_retry(&mut self) -> Option<MacDisplayTopologyRetry> {
        let delay_index = self
            .retry_attempts_started
            .min(MAC_DISPLAY_TOPOLOGY_RETRY_DELAYS.len() - 1);
        let delay = MAC_DISPLAY_TOPOLOGY_RETRY_DELAYS[delay_index];
        let retry_epoch = self.retry_epoch.checked_add(1)?;
        let attempt = self.retry_attempts_started.saturating_add(1);
        let retry = MacDisplayTopologyRetry {
            retry_epoch,
            attempt,
            delay,
        };
        self.retry_epoch = retry_epoch;
        self.retry_attempts_started = attempt;
        self.scheduled_retry = Some(retry);
        Some(retry)
    }

    fn cancel_retry_chain(&mut self) {
        self.scheduled_retry = None;
        self.retry_attempts_started = 0;
    }

    pub(crate) fn refresh_candidate_from_native()
    -> Result<MacDisplayTopologyCandidate, MacDisplayTopologyFailure> {
        MacDisplayTopologyCandidate::from_native()
    }

    pub(crate) fn finish_refresh(
        &mut self,
        request_epoch: u64,
        candidate: Result<MacDisplayTopologyCandidate, MacDisplayTopologyFailure>,
    ) -> MacDisplayTopologyRefresh {
        if self.request_epoch != request_epoch
            || !matches!(self.state, MacDisplayTopologyState::RefreshPending { .. })
        {
            return MacDisplayTopologyRefresh::Superseded {
                generation: self
                    .state
                    .retained_snapshot()
                    .map(|snapshot| snapshot.generation),
                request_epoch: self.request_epoch,
            };
        }

        let retained = self.state.retained_snapshot();
        let candidate = match candidate {
            Ok(candidate) => candidate,
            Err(failure) => {
                let generation = retained.as_ref().map(|snapshot| snapshot.generation);
                self.state = MacDisplayTopologyState::Degraded {
                    retained,
                    failure: failure.clone(),
                };
                let retry = self.schedule_retry();
                return MacDisplayTopologyRefresh::RetainedAfterFailure {
                    generation,
                    failure,
                    retry,
                };
            }
        };

        if let Some(snapshot) = retained.as_ref()
            && snapshot.has_same_facts(&candidate)
        {
            let generation = snapshot.generation;
            self.cancel_retry_chain();
            self.state = MacDisplayTopologyState::Complete(snapshot.clone());
            return MacDisplayTopologyRefresh::Unchanged { generation };
        }

        let previous_generation = retained.as_ref().map(|snapshot| snapshot.generation);
        let generation = match previous_generation {
            Some(previous_generation) => match previous_generation.checked_add(1) {
                Some(generation) => generation,
                None => {
                    let failure = MacDisplayTopologyFailure::GenerationExhausted;
                    self.cancel_retry_chain();
                    self.state = MacDisplayTopologyState::Degraded {
                        retained,
                        failure: failure.clone(),
                    };
                    return MacDisplayTopologyRefresh::RetainedAfterFailure {
                        generation: previous_generation.into(),
                        failure,
                        retry: None,
                    };
                }
            },
            None => 1,
        };
        self.cancel_retry_chain();
        self.state = MacDisplayTopologyState::Complete(MacDisplayTopologySnapshot::new(
            generation, candidate,
        ));
        MacDisplayTopologyRefresh::Published {
            previous_generation,
            generation,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        MacDisplayTopologyAuthority, MacDisplayTopologyFailure, MacDisplayTopologyHandle,
        MacDisplayTopologyListener, MacDisplayTopologyRefresh, MacDisplayTopologySubscription,
        MacDisplayTopologyUnavailable, notify_display_topology_listener,
    };
    use crate::display::{
        MacDisplay, MacDisplayTopologyCandidate, display_id_from_uuid, native::MacNativeDisplayRow,
    };
    use open_gpui::{Bounds, PlatformDisplay, point, px, size};
    use smallvec::smallvec;
    use std::{
        panic,
        sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        },
        time::Duration,
    };
    use uuid::Uuid;

    fn display(
        provenance: u128,
        origin_x: f32,
        origin_y: f32,
        scale_factor: f32,
        visible_width: f32,
    ) -> MacDisplay {
        let uuid = Uuid::from_u128(provenance);
        let bounds = Bounds::new(
            point(px(origin_x), px(origin_y)),
            size(px(1_920.0), px(1_080.0)),
        );
        MacDisplay {
            display_id: display_id_from_uuid(uuid),
            uuid,
            scale_factor,
            bounds,
            visible_bounds: Bounds::new(bounds.origin, size(px(visible_width), px(1_040.0))),
        }
    }

    fn row(native_display_id: u32, display: MacDisplay, is_primary: bool) -> MacNativeDisplayRow {
        MacNativeDisplayRow {
            screen: std::ptr::without_provenance_mut(native_display_id as usize),
            native_display_id,
            display,
            is_primary,
        }
    }

    fn candidate(primary: MacDisplay, secondary: MacDisplay) -> MacDisplayTopologyCandidate {
        MacDisplayTopologyCandidate::try_new(smallvec![
            row(1, primary, true),
            row(2, secondary, false),
        ])
        .unwrap()
    }

    fn refresh(
        authority: &mut MacDisplayTopologyAuthority,
        candidate: Result<MacDisplayTopologyCandidate, MacDisplayTopologyFailure>,
    ) -> MacDisplayTopologyRefresh {
        let request = authority.request_refresh();
        assert!(request.should_schedule);
        let request_epoch = authority.begin_scheduled_refresh().unwrap();
        authority.finish_refresh(request_epoch, candidate)
    }

    #[test]
    fn initial_failure_is_explicit_without_a_partial_publication() {
        let failure = MacDisplayTopologyFailure::NativeCollection("incomplete batch".into());
        let authority = MacDisplayTopologyAuthority::from_initial_candidate(Err(failure.clone()));

        assert!(authority.retained_snapshot().is_none());
        assert!(matches!(
            authority.exact_snapshot(),
            Err(MacDisplayTopologyUnavailable::Degraded {
                retained_generation: None,
                failure: observed,
                ..
            }) if observed == failure
        ));
        assert_eq!(authority.retained_platform_snapshot().generation(), None);
        assert!(authority.retained_platform_snapshot().displays().is_empty());
    }

    #[test]
    fn failed_refresh_retains_the_previous_complete_generation() {
        let primary = display(1, 0.0, 0.0, 2.0, 1_880.0);
        let secondary = display(2, -1_920.0, 0.0, 1.0, 1_920.0);
        let mut authority =
            MacDisplayTopologyAuthority::from_initial_candidate(Ok(candidate(primary, secondary)));

        assert!(matches!(
            refresh(
                &mut authority,
                Err(MacDisplayTopologyFailure::UnstableDuringCollection)
            ),
            MacDisplayTopologyRefresh::RetainedAfterFailure {
                generation: Some(1),
                ..
            }
        ));
        assert_eq!(authority.retained_snapshot().unwrap().generation(), 1);
        assert!(authority.exact_snapshot().is_err());
    }

    #[test]
    fn single_notification_recovers_after_one_transient_failure_via_retry() {
        let primary = display(1, 0.0, 0.0, 2.0, 1_880.0);
        let secondary = display(2, 1_920.0, 0.0, 1.0, 1_920.0);
        let stable = candidate(primary, secondary);
        let mut authority = MacDisplayTopologyAuthority::from_initial_candidate(Ok(stable.clone()));

        let notification = authority.request_refresh();
        assert_eq!(
            authority.begin_scheduled_refresh(),
            Some(notification.request_epoch)
        );
        let retry = match authority.finish_refresh(
            notification.request_epoch,
            Err(MacDisplayTopologyFailure::UnstableDuringCollection),
        ) {
            MacDisplayTopologyRefresh::RetainedAfterFailure {
                generation: Some(1),
                retry: Some(retry),
                ..
            } => retry,
            other => panic!("expected one retained retry, got {other:?}"),
        };
        assert_eq!(retry.attempt(), 1);
        assert_eq!(retry.delay(), Duration::from_millis(16));
        assert!(authority.exact_snapshot().is_err());

        let retry_request_epoch = authority
            .begin_retry(retry.retry_epoch())
            .expect("the scheduled retry should own the next refresh");
        assert!(matches!(
            authority.finish_refresh(retry_request_epoch, Ok(stable)),
            MacDisplayTopologyRefresh::Unchanged { generation: 1 }
        ));
        assert_eq!(authority.exact_snapshot().unwrap().generation(), 1);
        assert_eq!(authority.retry_attempts_started, 0);
        assert!(authority.scheduled_retry.is_none());
        assert_eq!(authority.begin_retry(retry.retry_epoch()), None);
    }

    #[test]
    fn changed_publication_cancels_the_retry_chain() {
        let primary = display(1, 0.0, 0.0, 2.0, 1_880.0);
        let secondary = display(2, 1_920.0, 0.0, 1.0, 1_920.0);
        let mut authority =
            MacDisplayTopologyAuthority::from_initial_candidate(Ok(candidate(primary, secondary)));
        let request = authority.request_refresh();
        let request_epoch = authority.begin_scheduled_refresh().unwrap();
        let retry = match authority.finish_refresh(
            request_epoch,
            Err(MacDisplayTopologyFailure::UnstableDuringCollection),
        ) {
            MacDisplayTopologyRefresh::RetainedAfterFailure {
                retry: Some(retry), ..
            } => retry,
            other => panic!("expected a queued retry, got {other:?}"),
        };
        let retry_request_epoch = authority.begin_retry(retry.retry_epoch()).unwrap();
        let moved_secondary = display(2, 2_000.0, 0.0, 1.0, 2_000.0);

        assert!(matches!(
            authority.finish_refresh(retry_request_epoch, Ok(candidate(primary, moved_secondary))),
            MacDisplayTopologyRefresh::Published {
                previous_generation: Some(1),
                generation: 2,
            }
        ));
        assert_eq!(authority.retry_attempts_started, 0);
        assert!(authority.scheduled_retry.is_none());
        assert_eq!(authority.begin_retry(retry.retry_epoch()), None);
        assert_eq!(request.request_epoch, 1);
    }

    #[test]
    fn consecutive_failures_cap_backoff_without_stopping_retries() {
        let primary = display(1, 0.0, 0.0, 2.0, 1_880.0);
        let secondary = display(2, 1_920.0, 0.0, 1.0, 1_920.0);
        let mut authority =
            MacDisplayTopologyAuthority::from_initial_candidate(Ok(candidate(primary, secondary)));
        let request = authority.request_refresh();
        let mut request_epoch = authority.begin_scheduled_refresh().unwrap();
        let expected_delays = [
            Duration::from_millis(16),
            Duration::from_millis(64),
            Duration::from_millis(250),
            Duration::from_secs(1),
            Duration::from_secs(2),
            Duration::from_secs(2),
            Duration::from_secs(2),
        ];

        for (index, expected_delay) in expected_delays.into_iter().enumerate() {
            let retry = match authority.finish_refresh(
                request_epoch,
                Err(MacDisplayTopologyFailure::UnstableDuringCollection),
            ) {
                MacDisplayTopologyRefresh::RetainedAfterFailure {
                    retry: Some(retry), ..
                } => retry,
                other => panic!("expected retry {} but got {other:?}", index + 1),
            };
            assert_eq!(retry.attempt(), index + 1);
            assert_eq!(retry.delay(), expected_delay);
            request_epoch = authority.begin_retry(retry.retry_epoch()).unwrap();
        }

        let retry = match authority.finish_refresh(
            request_epoch,
            Err(MacDisplayTopologyFailure::UnstableDuringCollection),
        ) {
            MacDisplayTopologyRefresh::RetainedAfterFailure {
                retry: Some(retry), ..
            } => retry,
            other => panic!("expected the capped retry chain to continue, got {other:?}"),
        };
        assert_eq!(retry.attempt(), expected_delays.len() + 1);
        assert_eq!(retry.delay(), Duration::from_secs(2));
        assert_eq!(request.request_epoch, 1);
    }

    #[test]
    fn newer_notification_cancels_the_queued_retry_epoch() {
        let primary = display(1, 0.0, 0.0, 2.0, 1_880.0);
        let secondary = display(2, 1_920.0, 0.0, 1.0, 1_920.0);
        let stable = candidate(primary, secondary);
        let mut authority = MacDisplayTopologyAuthority::from_initial_candidate(Ok(stable.clone()));
        let first = authority.request_refresh();
        let first_epoch = authority.begin_scheduled_refresh().unwrap();
        let retry = match authority.finish_refresh(
            first_epoch,
            Err(MacDisplayTopologyFailure::UnstableDuringCollection),
        ) {
            MacDisplayTopologyRefresh::RetainedAfterFailure {
                retry: Some(retry), ..
            } => retry,
            other => panic!("expected a queued retry, got {other:?}"),
        };

        let replacement = authority.request_refresh();
        assert!(replacement.should_schedule);
        assert_eq!(authority.begin_retry(retry.retry_epoch()), None);
        assert_eq!(
            authority.begin_scheduled_refresh(),
            Some(replacement.request_epoch)
        );
        assert!(matches!(
            authority.finish_refresh(replacement.request_epoch, Ok(stable)),
            MacDisplayTopologyRefresh::Unchanged { generation: 1 }
        ));
        assert_eq!(authority.exact_snapshot().unwrap().generation(), 1);
        assert_eq!(first.request_epoch, 1);
    }

    #[test]
    fn complete_candidate_recovers_from_initial_failure_as_generation_one() {
        let primary = display(1, 0.0, 0.0, 2.0, 1_880.0);
        let secondary = display(2, 1_920.0, 0.0, 1.0, 1_920.0);
        let mut authority = MacDisplayTopologyAuthority::from_initial_candidate(Err(
            MacDisplayTopologyFailure::NativeCollection("initial failure".into()),
        ));

        assert!(matches!(
            refresh(&mut authority, Ok(candidate(primary, secondary))),
            MacDisplayTopologyRefresh::Published {
                previous_generation: None,
                generation: 1,
            }
        ));
        assert_eq!(authority.exact_snapshot().unwrap().generation(), 1);
    }

    #[test]
    fn refresh_requests_coalesce_and_only_the_latest_epoch_can_publish() {
        let primary = display(1, 0.0, 0.0, 2.0, 1_880.0);
        let secondary = display(2, 1_920.0, 0.0, 1.0, 1_920.0);
        let initial = candidate(primary, secondary);
        let mut authority =
            MacDisplayTopologyAuthority::from_initial_candidate(Ok(initial.clone()));

        let first = authority.request_refresh();
        let second = authority.request_refresh();
        assert!(first.should_schedule);
        assert!(!second.should_schedule);
        assert_eq!(
            authority.begin_scheduled_refresh(),
            Some(second.request_epoch)
        );
        assert!(matches!(
            authority.finish_refresh(first.request_epoch, Ok(initial.clone())),
            MacDisplayTopologyRefresh::Superseded {
                generation: Some(1),
                request_epoch,
            } if request_epoch == second.request_epoch
        ));
        assert!(matches!(
            authority.finish_refresh(second.request_epoch, Ok(initial)),
            MacDisplayTopologyRefresh::Unchanged { generation: 1 }
        ));
    }

    #[test]
    fn scale_work_area_provenance_and_native_id_reuse_publish_new_generations() {
        let primary = display(1, 0.0, 0.0, 2.0, 1_880.0);
        let secondary = display(2, 1_920.0, 0.0, 1.0, 1_920.0);
        let mut authority =
            MacDisplayTopologyAuthority::from_initial_candidate(Ok(candidate(primary, secondary)));

        let changed_scale = display(2, 1_920.0, 0.0, 1.5, 1_920.0);
        assert!(matches!(
            refresh(&mut authority, Ok(candidate(primary, changed_scale))),
            MacDisplayTopologyRefresh::Published {
                previous_generation: Some(1),
                generation: 2,
            }
        ));

        let changed_work_area = display(2, 1_920.0, 0.0, 1.5, 1_800.0);
        assert!(matches!(
            refresh(&mut authority, Ok(candidate(primary, changed_work_area))),
            MacDisplayTopologyRefresh::Published { generation: 3, .. }
        ));

        let replacement = display(3, 1_920.0, 0.0, 1.5, 1_800.0);
        let replacement_candidate = MacDisplayTopologyCandidate::try_new(smallvec![
            row(1, primary, true),
            row(2, replacement, false),
        ])
        .unwrap();
        assert!(matches!(
            refresh(&mut authority, Ok(replacement_candidate)),
            MacDisplayTopologyRefresh::Published { generation: 4, .. }
        ));
        assert!(
            authority
                .retained_snapshot()
                .unwrap()
                .display(changed_work_area.id())
                .is_none()
        );
    }

    #[test]
    fn unchanged_refresh_preserves_generation_and_immutable_lookup() {
        let primary = display(1, 0.0, 0.0, 2.0, 1_880.0);
        let secondary = display(2, 1_920.0, 0.0, 1.0, 1_920.0);
        let initial = candidate(primary, secondary);
        let mut authority =
            MacDisplayTopologyAuthority::from_initial_candidate(Ok(initial.clone()));
        let retained = authority.retained_snapshot().unwrap();

        assert!(matches!(
            refresh(&mut authority, Ok(initial)),
            MacDisplayTopologyRefresh::Unchanged { generation: 1 }
        ));
        assert_eq!(retained.primary_display(), primary);
        assert_eq!(retained.display(secondary.id()), Some(secondary));
        assert_eq!(retained.platform_snapshot().generation(), Some(1));
    }

    #[test]
    fn topology_handle_is_safe_to_retain_in_window_state() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MacDisplayTopologyHandle>();
        assert_send_sync::<MacDisplayTopologySubscription>();
    }

    #[test]
    fn publication_listener_observes_registration_and_unchanged_refresh() {
        let primary = display(1, 0.0, 0.0, 2.0, 1_880.0);
        let secondary = display(2, 1_920.0, 0.0, 1.0, 1_920.0);
        let initial = candidate(primary, secondary);
        let handle = MacDisplayTopologyHandle::from_authority(
            MacDisplayTopologyAuthority::from_initial_candidate(Ok(initial.clone())),
        );
        let notifications = Arc::new(parking_lot::Mutex::new(Vec::new()));
        let listener_notifications = notifications.clone();
        let subscription = handle
            .subscribe_publications(
                1,
                Arc::new(move |generation| listener_notifications.lock().push(generation)),
            )
            .unwrap();
        assert_eq!(notifications.lock().as_slice(), &[1]);

        let request = handle.request_refresh();
        assert_eq!(
            handle.begin_scheduled_refresh(),
            Some(request.request_epoch)
        );
        assert!(matches!(
            handle.finish_refresh(request.request_epoch, Ok(initial.clone())),
            MacDisplayTopologyRefresh::Unchanged { generation: 1 }
        ));
        assert_eq!(notifications.lock().as_slice(), &[1, 1]);

        drop(subscription);
        let request = handle.request_refresh();
        assert_eq!(
            handle.begin_scheduled_refresh(),
            Some(request.request_epoch)
        );
        handle.finish_refresh(request.request_epoch, Ok(initial));
        assert_eq!(notifications.lock().as_slice(), &[1, 1]);
    }

    #[test]
    fn panicking_listener_payload_is_not_dropped_across_the_callback_boundary() {
        struct DropProbe(Arc<AtomicBool>);

        impl Drop for DropProbe {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }

        let payload_dropped = Arc::new(AtomicBool::new(false));
        let listener_payload_dropped = payload_dropped.clone();
        let listener: MacDisplayTopologyListener = Arc::new(move |_| {
            panic::panic_any(DropProbe(listener_payload_dropped.clone()));
        });

        notify_display_topology_listener(&listener, 7);

        assert!(!payload_dropped.load(Ordering::SeqCst));
    }
}
