use open_gpui::{Bounds, DevicePixels, Pixels, Size};
use std::{error::Error, fmt, time::Duration};

const RETRY_DELAYS: [Duration; 5] = [
    Duration::from_millis(16),
    Duration::from_millis(64),
    Duration::from_millis(250),
    Duration::from_secs(1),
    Duration::from_secs(2),
];
const SYNC_OBSERVATION_DELAY: Duration = Duration::from_millis(16);

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct X11ConfigureGeometry {
    pub(super) bounds: Bounds<Pixels>,
    pub(super) device_size: Size<DevicePixels>,
    pub(super) resized: bool,
}

pub(super) fn reconcile_x11_configure_geometry(
    current_bounds: Bounds<Pixels>,
    native_bounds: Bounds<DevicePixels>,
    scale_factor: f32,
) -> X11ConfigureGeometry {
    let logical_bounds = native_bounds.to_pixels(scale_factor);
    let resized = logical_bounds.size != current_bounds.size;
    let bounds = if resized {
        Bounds {
            origin: current_bounds.origin,
            size: logical_bounds.size,
        }
    } else {
        logical_bounds
    };

    X11ConfigureGeometry {
        bounds,
        device_size: native_bounds.size,
        resized,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum X11PropertyDomain {
    WindowState,
    EdgeConstraints,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct X11PropertyAttempt {
    pub(super) domain: X11PropertyDomain,
    epoch: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct X11PropertyRetry {
    domain: X11PropertyDomain,
    timer_epoch: u64,
    pub(super) delay: Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct X11RestoreSettlement {
    epoch: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct X11SyncCounterValue {
    pub(super) lo: u32,
    pub(super) hi: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct X11SyncRetry {
    epoch: u64,
    pub(super) delay: Duration,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct X11SyncAcknowledgementSubmission {
    pub(super) value: X11SyncCounterValue,
    pub(super) set_counter_sequence: u64,
    pub(super) fence_sequence: u64,
    pub(super) set_counter_terminal: bool,
    pub(super) fence_terminal: bool,
}

impl X11SyncAcknowledgementSubmission {
    pub(super) fn pending(
        value: X11SyncCounterValue,
        set_counter_sequence: u64,
        fence_sequence: u64,
    ) -> Self {
        Self {
            value,
            set_counter_sequence,
            fence_sequence,
            set_counter_terminal: false,
            fence_terminal: false,
        }
    }

    fn can_remain_pending_as(self, next: Self) -> bool {
        self.value == next.value
            && self.set_counter_sequence == next.set_counter_sequence
            && self.fence_sequence == next.fence_sequence
            && !self.set_counter_terminal
            && !next.set_counter_terminal
            && (!self.fence_terminal || next.fence_terminal)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct X11SyncAcknowledgementObservation {
    epoch: u64,
    pub(super) delay: Duration,
}

#[derive(Default)]
struct X11PropertyDomainState {
    epoch: u64,
    dirty: bool,
    retry_timer_epoch: u64,
    retry_scheduled: bool,
    retry_attempt: usize,
}

impl X11PropertyDomainState {
    fn invalidate(&mut self, domain: X11PropertyDomain) -> X11PropertyAttempt {
        self.epoch = self.epoch.wrapping_add(1);
        self.dirty = true;
        X11PropertyAttempt {
            domain,
            epoch: self.epoch,
        }
    }

    fn is_current(&self, attempt: X11PropertyAttempt) -> bool {
        self.dirty && self.epoch == attempt.epoch
    }

    fn retryable_failure(&mut self, attempt: X11PropertyAttempt) -> Option<X11PropertyRetry> {
        if !self.is_current(attempt) || self.retry_scheduled {
            return None;
        }

        let delay = RETRY_DELAYS[self.retry_attempt.min(RETRY_DELAYS.len() - 1)];
        self.retry_attempt = self.retry_attempt.saturating_add(1);
        self.retry_timer_epoch = self.retry_timer_epoch.wrapping_add(1);
        self.retry_scheduled = true;
        Some(X11PropertyRetry {
            domain: attempt.domain,
            timer_epoch: self.retry_timer_epoch,
            delay,
        })
    }

    fn claim_retry(&mut self, retry: X11PropertyRetry) -> Option<X11PropertyAttempt> {
        if !self.retry_scheduled || self.retry_timer_epoch != retry.timer_epoch {
            return None;
        }
        self.retry_scheduled = false;
        self.dirty.then_some(X11PropertyAttempt {
            domain: retry.domain,
            epoch: self.epoch,
        })
    }

    fn commit(&mut self, attempt: X11PropertyAttempt) -> bool {
        if !self.is_current(attempt) {
            return false;
        }
        self.dirty = false;
        self.retry_scheduled = false;
        self.retry_attempt = 0;
        self.retry_timer_epoch = self.retry_timer_epoch.wrapping_add(1);
        true
    }

    fn malformed(&mut self, attempt: X11PropertyAttempt) -> bool {
        if !self.is_current(attempt) {
            return false;
        }
        self.retry_scheduled = false;
        self.retry_timer_epoch = self.retry_timer_epoch.wrapping_add(1);
        true
    }
}

#[derive(Default)]
pub(super) struct X11WindowObservationCoordinator {
    window_state: X11PropertyDomainState,
    edge_constraints: X11PropertyDomainState,
    restore_candidate: Option<Bounds<Pixels>>,
    restore_settlement_epoch: u64,
    restore_settlement_scheduled: bool,
    pending_sync_request: Option<X11SyncCounterValue>,
    configured_sync_acknowledgement: Option<X11SyncCounterValue>,
    ready_sync_acknowledgement: Option<X11SyncCounterValue>,
    observed_sync_acknowledgement: Option<X11SyncAcknowledgementSubmission>,
    sync_wake_epoch: u64,
    sync_wake_scheduled: bool,
    sync_retry_attempt: usize,
    sync_acknowledgements_disabled: bool,
    closed: bool,
}

impl X11WindowObservationCoordinator {
    pub(super) fn invalidate_property(
        &mut self,
        domain: X11PropertyDomain,
    ) -> Option<X11PropertyAttempt> {
        if self.closed {
            return None;
        }
        Some(self.property_state_mut(domain).invalidate(domain))
    }

    pub(super) fn property_retryable_failure(
        &mut self,
        attempt: X11PropertyAttempt,
    ) -> Option<X11PropertyRetry> {
        if self.closed {
            return None;
        }
        self.property_state_mut(attempt.domain)
            .retryable_failure(attempt)
    }

    pub(super) fn claim_property_retry(
        &mut self,
        retry: X11PropertyRetry,
    ) -> Option<X11PropertyAttempt> {
        if self.closed {
            return None;
        }
        self.property_state_mut(retry.domain).claim_retry(retry)
    }

    pub(super) fn commit_property(&mut self, attempt: X11PropertyAttempt) -> bool {
        if self.closed {
            return false;
        }
        self.property_state_mut(attempt.domain).commit(attempt)
    }

    pub(super) fn retain_malformed_property(&mut self, attempt: X11PropertyAttempt) -> bool {
        if self.closed {
            return false;
        }
        self.property_state_mut(attempt.domain).malformed(attempt)
    }

    #[cfg(test)]
    fn property_is_dirty(&self, domain: X11PropertyDomain) -> bool {
        self.property_state(domain).dirty
    }

    pub(super) fn stage_restore_candidate(
        &mut self,
        bounds: Bounds<Pixels>,
    ) -> Option<X11RestoreSettlement> {
        if self.closed {
            return None;
        }

        self.restore_candidate = Some(bounds);
        if self.restore_settlement_scheduled {
            return None;
        }

        self.restore_settlement_epoch = self.restore_settlement_epoch.wrapping_add(1);
        self.restore_settlement_scheduled = true;
        Some(X11RestoreSettlement {
            epoch: self.restore_settlement_epoch,
        })
    }

    pub(super) fn claim_restore_settlement(&mut self, settlement: X11RestoreSettlement) -> bool {
        if self.closed
            || !self.restore_settlement_scheduled
            || self.restore_settlement_epoch != settlement.epoch
        {
            return false;
        }
        self.restore_settlement_scheduled = false;
        true
    }

    pub(super) fn settle_restore_candidate(&mut self, windowed: bool) -> Option<Bounds<Pixels>> {
        if self.closed || self.window_state.dirty {
            return None;
        }

        let candidate = self.restore_candidate.take();
        windowed.then_some(candidate).flatten()
    }

    pub(super) fn enqueue_sync_request(&mut self, value: X11SyncCounterValue) {
        if !self.closed && !self.sync_acknowledgements_disabled {
            self.pending_sync_request = Some(value);
        }
    }

    pub(super) fn configure_committed(&mut self) {
        if self.closed || self.sync_acknowledgements_disabled {
            return;
        }
        if let Some(request) = self.pending_sync_request.take() {
            self.configured_sync_acknowledgement = Some(request);
        }
    }

    pub(super) fn presentation_submitted(&mut self) {
        if self.closed || self.sync_acknowledgements_disabled {
            return;
        }
        // A ConfigureNotify starts the WM sync repaint obligation. The counter may only advance
        // after the renderer accepts a frame that includes the configured geometry.
        if let Some(value) = self.configured_sync_acknowledgement.take() {
            self.ready_sync_acknowledgement = Some(value);
            if self.observed_sync_acknowledgement.is_none() {
                self.invalidate_sync_wake();
            }
        }
    }

    pub(super) fn ready_sync_acknowledgement(&self) -> Option<X11SyncCounterValue> {
        if self.closed
            || self.sync_acknowledgements_disabled
            || self.sync_wake_scheduled
            || self.observed_sync_acknowledgement.is_some()
        {
            None
        } else {
            self.ready_sync_acknowledgement
        }
    }

    pub(super) fn sync_acknowledgement_submitted(
        &mut self,
        value: X11SyncCounterValue,
        set_counter_sequence: u64,
        fence_sequence: u64,
    ) -> Option<X11SyncAcknowledgementObservation> {
        if self.closed
            || self.sync_acknowledgements_disabled
            || self.sync_wake_scheduled
            || self.observed_sync_acknowledgement.is_some()
            || self.ready_sync_acknowledgement != Some(value)
        {
            return None;
        }

        self.ready_sync_acknowledgement = None;
        self.observed_sync_acknowledgement = Some(X11SyncAcknowledgementSubmission::pending(
            value,
            set_counter_sequence,
            fence_sequence,
        ));
        Some(self.schedule_sync_observation())
    }

    pub(super) fn claim_sync_acknowledgement_observation(
        &mut self,
        observation: X11SyncAcknowledgementObservation,
    ) -> Option<X11SyncAcknowledgementSubmission> {
        if self.closed
            || self.sync_acknowledgements_disabled
            || !self.sync_wake_scheduled
            || self.sync_wake_epoch != observation.epoch
        {
            return None;
        }

        self.sync_wake_scheduled = false;
        self.observed_sync_acknowledgement
    }

    pub(super) fn sync_acknowledgement_observation_pending(
        &mut self,
        claimed_submission: X11SyncAcknowledgementSubmission,
        pending_submission: X11SyncAcknowledgementSubmission,
    ) -> Option<X11SyncAcknowledgementObservation> {
        if self.closed
            || self.sync_acknowledgements_disabled
            || self.sync_wake_scheduled
            || self.observed_sync_acknowledgement != Some(claimed_submission)
            || !claimed_submission.can_remain_pending_as(pending_submission)
        {
            return None;
        }

        self.observed_sync_acknowledgement = Some(pending_submission);
        Some(self.schedule_sync_observation())
    }

    pub(super) fn sync_acknowledgement_succeeded(
        &mut self,
        submission: X11SyncAcknowledgementSubmission,
    ) -> bool {
        if self.closed
            || self.sync_acknowledgements_disabled
            || self.observed_sync_acknowledgement != Some(submission)
        {
            return false;
        }

        self.observed_sync_acknowledgement = None;
        self.sync_retry_attempt = 0;
        self.invalidate_sync_wake();
        true
    }

    pub(super) fn sync_acknowledgement_retryable_submission_failure(
        &mut self,
        value: X11SyncCounterValue,
    ) -> Option<X11SyncRetry> {
        if self.closed
            || self.sync_acknowledgements_disabled
            || self.sync_wake_scheduled
            || self.observed_sync_acknowledgement.is_some()
            || self.ready_sync_acknowledgement != Some(value)
        {
            return None;
        }

        Some(self.schedule_sync_retry())
    }

    pub(super) fn sync_acknowledgement_retryable_observation_failure(
        &mut self,
        submission: X11SyncAcknowledgementSubmission,
    ) -> Option<X11SyncRetry> {
        if self.closed
            || self.sync_acknowledgements_disabled
            || self.sync_wake_scheduled
            || self.observed_sync_acknowledgement != Some(submission)
        {
            return None;
        }

        self.observed_sync_acknowledgement = None;
        if self.ready_sync_acknowledgement.is_none() {
            self.ready_sync_acknowledgement = Some(submission.value);
        }
        Some(self.schedule_sync_retry())
    }

    pub(super) fn sync_acknowledgement_submission_permanently_failed(
        &mut self,
        value: X11SyncCounterValue,
    ) -> bool {
        if self.closed
            || self.sync_acknowledgements_disabled
            || self.observed_sync_acknowledgement.is_some()
            || self.ready_sync_acknowledgement != Some(value)
        {
            return false;
        }

        self.disable_sync_acknowledgements();
        true
    }

    pub(super) fn sync_acknowledgement_permanently_failed(
        &mut self,
        submission: X11SyncAcknowledgementSubmission,
    ) -> bool {
        if self.closed
            || self.sync_acknowledgements_disabled
            || self.observed_sync_acknowledgement != Some(submission)
        {
            return false;
        }

        self.disable_sync_acknowledgements();
        true
    }

    fn schedule_sync_retry(&mut self) -> X11SyncRetry {
        let delay = RETRY_DELAYS[self.sync_retry_attempt.min(RETRY_DELAYS.len() - 1)];
        self.sync_retry_attempt = self.sync_retry_attempt.saturating_add(1);
        self.sync_wake_epoch = self.sync_wake_epoch.wrapping_add(1);
        self.sync_wake_scheduled = true;
        X11SyncRetry {
            epoch: self.sync_wake_epoch,
            delay,
        }
    }

    pub(super) fn claim_sync_retry(&mut self, retry: X11SyncRetry) -> bool {
        if self.closed
            || self.sync_acknowledgements_disabled
            || !self.sync_wake_scheduled
            || self.sync_wake_epoch != retry.epoch
            || self.observed_sync_acknowledgement.is_some()
        {
            return false;
        }
        self.sync_wake_scheduled = false;
        self.ready_sync_acknowledgement.is_some()
    }

    pub(super) fn close(&mut self) -> Option<X11SyncAcknowledgementSubmission> {
        if self.closed {
            return None;
        }

        let abandoned_submission = self.observed_sync_acknowledgement;
        self.closed = true;
        self.window_state = X11PropertyDomainState::default();
        self.edge_constraints = X11PropertyDomainState::default();
        self.restore_candidate = None;
        self.restore_settlement_scheduled = false;
        self.disable_sync_acknowledgements();
        abandoned_submission
    }

    fn schedule_sync_observation(&mut self) -> X11SyncAcknowledgementObservation {
        self.sync_wake_epoch = self.sync_wake_epoch.wrapping_add(1);
        self.sync_wake_scheduled = true;
        X11SyncAcknowledgementObservation {
            epoch: self.sync_wake_epoch,
            delay: SYNC_OBSERVATION_DELAY,
        }
    }

    fn invalidate_sync_wake(&mut self) {
        self.sync_wake_epoch = self.sync_wake_epoch.wrapping_add(1);
        self.sync_wake_scheduled = false;
    }

    fn disable_sync_acknowledgements(&mut self) {
        self.sync_acknowledgements_disabled = true;
        self.pending_sync_request = None;
        self.configured_sync_acknowledgement = None;
        self.ready_sync_acknowledgement = None;
        self.observed_sync_acknowledgement = None;
        self.sync_retry_attempt = 0;
        self.invalidate_sync_wake();
    }

    fn property_state(&self, domain: X11PropertyDomain) -> &X11PropertyDomainState {
        match domain {
            X11PropertyDomain::WindowState => &self.window_state,
            X11PropertyDomain::EdgeConstraints => &self.edge_constraints,
        }
    }

    fn property_state_mut(&mut self, domain: X11PropertyDomain) -> &mut X11PropertyDomainState {
        match domain {
            X11PropertyDomain::WindowState => &mut self.window_state,
            X11PropertyDomain::EdgeConstraints => &mut self.edge_constraints,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct X11WindowStateFacts {
    pub(super) fullscreen: bool,
    pub(super) maximized_vertical: bool,
    pub(super) maximized_horizontal: bool,
    pub(super) hidden: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct X11WindowStateAtoms {
    pub(super) fullscreen: u32,
    pub(super) maximized_vertical: u32,
    pub(super) maximized_horizontal: u32,
    pub(super) hidden: u32,
}

#[derive(Clone, Copy)]
pub(super) struct X11PropertyPayload<'a> {
    pub(super) type_atom: u32,
    pub(super) format: u8,
    pub(super) value_len: u32,
    pub(super) bytes_after: u32,
    pub(super) value: &'a [u8],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum X11PropertyPayloadError {
    UnexpectedType { expected: u32, actual: u32 },
    UnexpectedFormat { expected: u8, actual: u8 },
    UnexpectedValueLength { expected: u32, actual: u32 },
    ValueLengthOverflow(u32),
    IncompleteValue { expected: usize, actual: usize },
    TrailingBytes(u32),
}

impl fmt::Display for X11PropertyPayloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedType { expected, actual } => {
                write!(formatter, "expected property type {expected}, got {actual}")
            }
            Self::UnexpectedFormat { expected, actual } => {
                write!(
                    formatter,
                    "expected property format {expected}, got {actual}"
                )
            }
            Self::UnexpectedValueLength { expected, actual } => {
                write!(
                    formatter,
                    "expected {expected} property items, got {actual}"
                )
            }
            Self::ValueLengthOverflow(value_len) => {
                write!(
                    formatter,
                    "property item count {value_len} cannot be represented as a byte length"
                )
            }
            Self::IncompleteValue { expected, actual } => {
                write!(
                    formatter,
                    "expected {expected} property bytes, got {actual}"
                )
            }
            Self::TrailingBytes(bytes) => {
                write!(formatter, "property reply retained {bytes} trailing bytes")
            }
        }
    }
}

impl Error for X11PropertyPayloadError {}

pub(super) fn parse_x11_window_state_property(
    payload: X11PropertyPayload<'_>,
    atom_type: u32,
    atoms: X11WindowStateAtoms,
) -> Result<X11WindowStateFacts, X11PropertyPayloadError> {
    if property_is_deleted(payload) {
        return Ok(X11WindowStateFacts::default());
    }
    let expected_bytes = usize::try_from(payload.value_len)
        .ok()
        .and_then(|value_len| value_len.checked_mul(4))
        .ok_or(X11PropertyPayloadError::ValueLengthOverflow(
            payload.value_len,
        ))?;
    validate_property_payload(payload, atom_type, 32, expected_bytes)?;

    let mut facts = X11WindowStateFacts::default();
    for chunk in payload.value.chunks_exact(4) {
        let atom = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
        if atom == atoms.fullscreen {
            facts.fullscreen = true;
        } else if atom == atoms.maximized_vertical {
            facts.maximized_vertical = true;
        } else if atom == atoms.maximized_horizontal {
            facts.maximized_horizontal = true;
        } else if atom == atoms.hidden {
            facts.hidden = true;
        }
    }
    Ok(facts)
}

pub(super) fn parse_x11_edge_constraints_property(
    payload: X11PropertyPayload<'_>,
    cardinal_type: u32,
) -> Result<Option<u32>, X11PropertyPayloadError> {
    if property_is_deleted(payload) {
        return Ok(None);
    }
    if payload.type_atom == cardinal_type
        && payload.format == 32
        && payload.value_len == 0
        && payload.bytes_after == 0
        && payload.value.is_empty()
    {
        return Ok(None);
    }
    if payload.value_len != 1 {
        return Err(X11PropertyPayloadError::UnexpectedValueLength {
            expected: 1,
            actual: payload.value_len,
        });
    }
    validate_property_payload(payload, cardinal_type, 32, 4)?;
    Ok(Some(u32::from_ne_bytes([
        payload.value[0],
        payload.value[1],
        payload.value[2],
        payload.value[3],
    ])))
}

fn property_is_deleted(payload: X11PropertyPayload<'_>) -> bool {
    payload.type_atom == 0
        && payload.format == 0
        && payload.value_len == 0
        && payload.bytes_after == 0
        && payload.value.is_empty()
}

fn validate_property_payload(
    payload: X11PropertyPayload<'_>,
    expected_type: u32,
    expected_format: u8,
    expected_bytes: usize,
) -> Result<(), X11PropertyPayloadError> {
    if payload.type_atom != expected_type {
        return Err(X11PropertyPayloadError::UnexpectedType {
            expected: expected_type,
            actual: payload.type_atom,
        });
    }
    if payload.format != expected_format {
        return Err(X11PropertyPayloadError::UnexpectedFormat {
            expected: expected_format,
            actual: payload.format,
        });
    }
    if payload.bytes_after != 0 {
        return Err(X11PropertyPayloadError::TrailingBytes(payload.bytes_after));
    }
    if payload.value.len() != expected_bytes {
        return Err(X11PropertyPayloadError::IncompleteValue {
            expected: expected_bytes,
            actual: payload.value.len(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui::{DevicePixels, Point, Size, point, px, size};

    fn window_state_atoms() -> X11WindowStateAtoms {
        X11WindowStateAtoms {
            fullscreen: 2,
            maximized_vertical: 3,
            maximized_horizontal: 4,
            hidden: 5,
        }
    }

    #[test]
    fn configure_geometry_commits_the_native_size_without_a_second_query() {
        let current = Bounds::new(point(px(10.0), px(20.0)), size(px(300.0), px(200.0)));
        let native = Bounds::new(
            Point::new(DevicePixels(400), DevicePixels(500)),
            Size::new(DevicePixels(800), DevicePixels(600)),
        );

        let observation = reconcile_x11_configure_geometry(current, native, 2.0);

        assert!(observation.resized);
        assert_eq!(observation.bounds.origin, current.origin);
        assert_eq!(observation.bounds.size, size(px(400.0), px(300.0)));
        assert_eq!(observation.device_size, native.size);
    }

    #[test]
    fn configure_geometry_accepts_a_native_move_when_the_size_is_unchanged() {
        let current = Bounds::new(point(px(10.0), px(20.0)), size(px(400.0), px(300.0)));
        let native = Bounds::new(
            Point::new(DevicePixels(-200), DevicePixels(100)),
            Size::new(DevicePixels(800), DevicePixels(600)),
        );

        let observation = reconcile_x11_configure_geometry(current, native, 2.0);

        assert!(!observation.resized);
        assert_eq!(
            observation.bounds,
            Bounds::new(point(px(-100.0), px(50.0)), current.size)
        );
    }

    #[test]
    fn retryable_property_failure_retains_dirty_state_and_coalesces_retry() {
        let mut coordinator = X11WindowObservationCoordinator::default();
        let attempt = coordinator
            .invalidate_property(X11PropertyDomain::WindowState)
            .unwrap();

        let retry = coordinator.property_retryable_failure(attempt).unwrap();

        assert!(coordinator.property_is_dirty(X11PropertyDomain::WindowState));
        assert!(coordinator.property_retryable_failure(attempt).is_none());
        assert_eq!(coordinator.claim_property_retry(retry), Some(attempt));
        assert!(coordinator.property_retryable_failure(attempt).is_some());
    }

    #[test]
    fn newer_property_notification_invalidates_an_older_retry_epoch() {
        let mut coordinator = X11WindowObservationCoordinator::default();
        let old_attempt = coordinator
            .invalidate_property(X11PropertyDomain::WindowState)
            .unwrap();
        let old_retry = coordinator.property_retryable_failure(old_attempt).unwrap();
        let new_attempt = coordinator
            .invalidate_property(X11PropertyDomain::WindowState)
            .unwrap();

        assert!(coordinator.commit_property(new_attempt));
        assert_eq!(coordinator.claim_property_retry(old_retry), None);
        assert!(!coordinator.property_is_dirty(X11PropertyDomain::WindowState));
    }

    #[test]
    fn newer_property_notification_reuses_the_existing_retry_timer() {
        let mut coordinator = X11WindowObservationCoordinator::default();
        let old_attempt = coordinator
            .invalidate_property(X11PropertyDomain::WindowState)
            .unwrap();
        let old_retry = coordinator.property_retryable_failure(old_attempt).unwrap();
        let new_attempt = coordinator
            .invalidate_property(X11PropertyDomain::WindowState)
            .unwrap();

        assert!(
            coordinator
                .property_retryable_failure(new_attempt)
                .is_none(),
            "a newer notification must reuse the already scheduled domain timer"
        );
        assert_eq!(
            coordinator.claim_property_retry(old_retry),
            Some(new_attempt)
        );
    }

    #[test]
    fn malformed_property_waits_for_a_new_notification_without_hot_retrying() {
        let mut coordinator = X11WindowObservationCoordinator::default();
        let attempt = coordinator
            .invalidate_property(X11PropertyDomain::EdgeConstraints)
            .unwrap();

        assert!(coordinator.retain_malformed_property(attempt));
        assert!(coordinator.property_is_dirty(X11PropertyDomain::EdgeConstraints));
        assert_eq!(
            coordinator.claim_property_retry(X11PropertyRetry {
                domain: attempt.domain,
                timer_epoch: 0,
                delay: Duration::ZERO,
            }),
            None
        );
    }

    #[test]
    fn dirty_window_state_retains_restore_bounds_until_a_coherent_windowed_commit() {
        let mut coordinator = X11WindowObservationCoordinator::default();
        let candidate = Bounds::new(point(px(20.0), px(30.0)), size(px(900.0), px(700.0)));
        let attempt = coordinator
            .invalidate_property(X11PropertyDomain::WindowState)
            .unwrap();
        coordinator.stage_restore_candidate(candidate);
        coordinator.property_retryable_failure(attempt).unwrap();

        assert_eq!(coordinator.settle_restore_candidate(true), None);
        assert!(coordinator.commit_property(attempt));
        assert_eq!(coordinator.settle_restore_candidate(true), Some(candidate));
    }

    #[test]
    fn confirmed_fullscreen_discards_a_pending_restore_candidate() {
        let mut coordinator = X11WindowObservationCoordinator::default();
        let fullscreen = Bounds::new(point(px(0.0), px(0.0)), size(px(1920.0), px(1080.0)));
        let attempt = coordinator
            .invalidate_property(X11PropertyDomain::WindowState)
            .unwrap();
        coordinator.stage_restore_candidate(fullscreen);

        assert!(coordinator.commit_property(attempt));
        assert_eq!(coordinator.settle_restore_candidate(false), None);
        assert_eq!(coordinator.settle_restore_candidate(true), None);
    }

    #[test]
    fn restore_settlement_requires_a_fresh_window_state_observation() {
        let mut coordinator = X11WindowObservationCoordinator::default();
        let candidate = Bounds::new(point(px(20.0), px(30.0)), size(px(900.0), px(700.0)));
        let settlement = coordinator.stage_restore_candidate(candidate).unwrap();

        assert!(coordinator.claim_restore_settlement(settlement));
        let attempt = coordinator
            .invalidate_property(X11PropertyDomain::WindowState)
            .unwrap();
        assert_eq!(coordinator.settle_restore_candidate(true), None);

        assert!(coordinator.commit_property(attempt));
        assert_eq!(coordinator.settle_restore_candidate(true), Some(candidate));
    }

    #[test]
    fn sync_acknowledgement_remains_owned_until_nonblocking_observation_succeeds() {
        let mut coordinator = X11WindowObservationCoordinator::default();
        let value = X11SyncCounterValue { lo: 7, hi: 2 };
        coordinator.enqueue_sync_request(value);

        coordinator.configure_committed();
        assert_eq!(coordinator.ready_sync_acknowledgement(), None);
        coordinator.presentation_submitted();
        assert_eq!(coordinator.ready_sync_acknowledgement(), Some(value));
        let observation = coordinator
            .sync_acknowledgement_submitted(value, 37, 38)
            .unwrap();
        assert_eq!(coordinator.ready_sync_acknowledgement(), None);

        let submission = coordinator
            .claim_sync_acknowledgement_observation(observation)
            .unwrap();
        assert_eq!(submission.value, value);
        assert_eq!(submission.set_counter_sequence, 37);
        assert_eq!(submission.fence_sequence, 38);
        let mut fence_terminal_submission = submission;
        fence_terminal_submission.fence_terminal = true;
        let observation = coordinator
            .sync_acknowledgement_observation_pending(submission, fence_terminal_submission)
            .unwrap();
        let submission = coordinator
            .claim_sync_acknowledgement_observation(observation)
            .unwrap();
        assert!(submission.fence_terminal);

        assert!(coordinator.sync_acknowledgement_succeeded(submission));
        assert_eq!(coordinator.ready_sync_acknowledgement(), None);
    }

    #[test]
    fn retryable_sync_observation_failure_restores_the_latest_obligation() {
        let mut coordinator = X11WindowObservationCoordinator::default();
        let first = X11SyncCounterValue { lo: 7, hi: 0 };
        let latest = X11SyncCounterValue { lo: 8, hi: 0 };
        coordinator.enqueue_sync_request(first);
        coordinator.configure_committed();
        coordinator.presentation_submitted();
        let observation = coordinator
            .sync_acknowledgement_submitted(first, 41, 42)
            .unwrap();
        let submission = coordinator
            .claim_sync_acknowledgement_observation(observation)
            .unwrap();

        coordinator.enqueue_sync_request(latest);
        coordinator.configure_committed();
        coordinator.presentation_submitted();
        let retry = coordinator
            .sync_acknowledgement_retryable_observation_failure(submission)
            .unwrap();

        assert_eq!(coordinator.ready_sync_acknowledgement(), None);
        assert!(coordinator.claim_sync_retry(retry));
        assert_eq!(coordinator.ready_sync_acknowledgement(), Some(latest));
    }

    #[test]
    fn permanent_sync_failure_terminates_the_lane_without_retrying() {
        let mut coordinator = X11WindowObservationCoordinator::default();
        let value = X11SyncCounterValue { lo: 7, hi: 0 };
        coordinator.enqueue_sync_request(value);
        coordinator.configure_committed();
        coordinator.presentation_submitted();
        let observation = coordinator
            .sync_acknowledgement_submitted(value, 43, 44)
            .unwrap();
        let submission = coordinator
            .claim_sync_acknowledgement_observation(observation)
            .unwrap();

        assert!(coordinator.sync_acknowledgement_permanently_failed(submission));
        coordinator.enqueue_sync_request(X11SyncCounterValue { lo: 8, hi: 0 });
        coordinator.configure_committed();
        coordinator.presentation_submitted();

        assert_eq!(coordinator.ready_sync_acknowledgement(), None);
    }

    #[test]
    fn sync_requests_coalesce_to_the_latest_configured_value() {
        let mut coordinator = X11WindowObservationCoordinator::default();
        let first = X11SyncCounterValue { lo: 1, hi: 0 };
        let second = X11SyncCounterValue { lo: 2, hi: 0 };
        coordinator.enqueue_sync_request(first);
        coordinator.enqueue_sync_request(second);

        coordinator.configure_committed();
        coordinator.configure_committed();
        assert_eq!(coordinator.ready_sync_acknowledgement(), None);
        coordinator.presentation_submitted();
        assert_eq!(coordinator.ready_sync_acknowledgement(), Some(second));
        let observation = coordinator
            .sync_acknowledgement_submitted(second, 47, 48)
            .unwrap();
        let submission = coordinator
            .claim_sync_acknowledgement_observation(observation)
            .unwrap();
        assert!(coordinator.sync_acknowledgement_succeeded(submission));
    }

    #[test]
    fn property_and_sync_retry_lanes_progress_independently() {
        let mut coordinator = X11WindowObservationCoordinator::default();
        let property_attempt = coordinator
            .invalidate_property(X11PropertyDomain::WindowState)
            .unwrap();
        let property_retry = coordinator
            .property_retryable_failure(property_attempt)
            .unwrap();
        let sync_value = X11SyncCounterValue { lo: 9, hi: 0 };
        coordinator.enqueue_sync_request(sync_value);
        coordinator.configure_committed();
        coordinator.presentation_submitted();
        assert_eq!(coordinator.ready_sync_acknowledgement(), Some(sync_value));
        let sync_retry = coordinator
            .sync_acknowledgement_retryable_submission_failure(sync_value)
            .unwrap();

        assert_eq!(
            coordinator.claim_property_retry(property_retry),
            Some(property_attempt)
        );
        assert!(coordinator.claim_sync_retry(sync_retry));
    }

    #[test]
    fn retry_backoff_caps_without_abandoning_the_obligation() {
        let mut coordinator = X11WindowObservationCoordinator::default();
        let attempt = coordinator
            .invalidate_property(X11PropertyDomain::WindowState)
            .unwrap();
        let mut delays = Vec::new();

        for _ in 0..7 {
            let retry = coordinator.property_retryable_failure(attempt).unwrap();
            delays.push(retry.delay);
            assert_eq!(coordinator.claim_property_retry(retry), Some(attempt));
        }

        assert_eq!(
            delays,
            [
                Duration::from_millis(16),
                Duration::from_millis(64),
                Duration::from_millis(250),
                Duration::from_secs(1),
                Duration::from_secs(2),
                Duration::from_secs(2),
                Duration::from_secs(2),
            ]
        );
    }

    #[test]
    fn close_invalidates_every_outstanding_observation_ticket() {
        let mut coordinator = X11WindowObservationCoordinator::default();
        let property_attempt = coordinator
            .invalidate_property(X11PropertyDomain::WindowState)
            .unwrap();
        let property_retry = coordinator
            .property_retryable_failure(property_attempt)
            .unwrap();
        let restore = coordinator
            .stage_restore_candidate(Bounds::new(
                point(px(10.0), px(20.0)),
                size(px(300.0), px(200.0)),
            ))
            .unwrap();
        let sync_value = X11SyncCounterValue { lo: 11, hi: 0 };
        coordinator.enqueue_sync_request(sync_value);
        coordinator.configure_committed();
        coordinator.presentation_submitted();
        let observation = coordinator
            .sync_acknowledgement_submitted(sync_value, 53, 54)
            .unwrap();

        let abandoned_submission = coordinator.close();

        assert_eq!(coordinator.claim_property_retry(property_retry), None);
        assert!(!coordinator.claim_restore_settlement(restore));
        assert_eq!(
            coordinator.claim_sync_acknowledgement_observation(observation),
            None
        );
        assert!(!coordinator.commit_property(property_attempt));
        assert_eq!(coordinator.ready_sync_acknowledgement(), None);
        let abandoned_submission = abandoned_submission.unwrap();
        assert_eq!(abandoned_submission.set_counter_sequence, 53);
        assert_eq!(abandoned_submission.fence_sequence, 54);
        assert!(!abandoned_submission.set_counter_terminal);
        assert!(!abandoned_submission.fence_terminal);
    }

    #[test]
    fn property_deletion_clears_window_state_and_edge_constraints() {
        let payload = X11PropertyPayload {
            type_atom: 0,
            format: 0,
            value_len: 0,
            bytes_after: 0,
            value: &[],
        };

        assert_eq!(
            parse_x11_window_state_property(payload, 10, window_state_atoms()).unwrap(),
            X11WindowStateFacts::default()
        );
        assert_eq!(
            parse_x11_edge_constraints_property(payload, 11).unwrap(),
            None
        );
    }

    #[test]
    fn window_state_focus_atoms_do_not_override_focus_event_authority() {
        let focused_atom = 1u32.to_ne_bytes();
        let payload = X11PropertyPayload {
            type_atom: 10,
            format: 32,
            value_len: 1,
            bytes_after: 0,
            value: &focused_atom,
        };

        assert_eq!(
            parse_x11_window_state_property(payload, 10, window_state_atoms()).unwrap(),
            X11WindowStateFacts::default()
        );
    }

    #[test]
    fn malformed_property_payloads_are_rejected_without_indexing_past_the_value() {
        let short_edge = X11PropertyPayload {
            type_atom: 11,
            format: 32,
            value_len: 1,
            bytes_after: 0,
            value: &[1, 2, 3],
        };
        let trailing_window_state = X11PropertyPayload {
            type_atom: 10,
            format: 32,
            value_len: 1,
            bytes_after: 0,
            value: &[1, 2, 3, 4, 5],
        };

        assert!(parse_x11_edge_constraints_property(short_edge, 11).is_err());
        assert!(
            parse_x11_window_state_property(trailing_window_state, 10, window_state_atoms())
                .is_err()
        );
    }

    #[test]
    fn edge_constraints_rejects_a_mismatched_item_count() {
        let mismatched_item_count = X11PropertyPayload {
            type_atom: 11,
            format: 32,
            value_len: 2,
            bytes_after: 0,
            value: &[1, 0, 0, 0],
        };

        assert!(parse_x11_edge_constraints_property(mismatched_item_count, 11).is_err());
    }
}
