use std::{sync::Arc, time::Duration};

use open_gpui::{
    DevicePixels, PlatformWindowMutationTerminal, PlatformWindowMutationUnobservedTerminal,
    PlatformWindowPhysicalGeometry, Point,
};
use windows::Win32::{Foundation::HWND, UI::WindowsAndMessaging::HWND_TOP};

use crate::platform::RegisteredWindow;

use super::WeakRegisteredWindowAuthority;

const MAX_AUTOMATIC_RECOVERY_RETRIES: u8 = 4;

fn recovery_retry_delay(attempt: u8) -> Duration {
    match attempt {
        0 => Duration::from_millis(2),
        1 => Duration::from_millis(8),
        2 => Duration::from_millis(32),
        3 => Duration::from_millis(128),
        _ => Duration::from_millis(500),
    }
}

fn activation_geometry_can_refresh(
    previous: PlatformWindowPhysicalGeometry,
    current: PlatformWindowPhysicalGeometry,
) -> bool {
    if previous == current {
        return true;
    }
    if previous.client_bounds() != current.client_bounds()
        || previous.scale_factor() != current.scale_factor()
    {
        return false;
    }
    let (Some(previous_display), Some(current_display)) = (
        previous.display_observation(),
        current.display_observation(),
    ) else {
        return false;
    };
    previous_display.display_id() == current_display.display_id()
        && previous_display.bounds() == current_display.bounds()
        && previous_display.scale_factor() == current_display.scale_factor()
}

#[derive(Clone, Copy, Debug)]
pub(super) struct NativeZOrderWindowIdentity {
    hwnd: HWND,
    thread_id: u32,
    process_id: u32,
    registered: Option<RegisteredWindow>,
}

impl NativeZOrderWindowIdentity {
    pub(super) fn new(
        hwnd: HWND,
        thread_id: u32,
        process_id: u32,
        registered: Option<RegisteredWindow>,
    ) -> Self {
        Self {
            hwnd,
            thread_id,
            process_id,
            registered,
        }
    }

    pub(super) fn hwnd(self) -> HWND {
        self.hwnd
    }

    pub(super) fn registered(self) -> Option<RegisteredWindow> {
        self.registered
    }
}

impl PartialEq for NativeZOrderWindowIdentity {
    fn eq(&self, other: &Self) -> bool {
        self.hwnd == other.hwnd
            && self.thread_id == other.thread_id
            && self.process_id == other.process_id
            && match (self.registered, other.registered) {
                (Some(left), Some(right)) => left.matches(right),
                (None, None) => true,
                (Some(_), None) | (None, Some(_)) => false,
            }
    }
}

impl Eq for NativeZOrderWindowIdentity {}

#[derive(Clone, Debug)]
pub(super) struct PreparedProvisionalZOrderBand {
    point: Point<DevicePixels>,
    current: RegisteredWindow,
    peers: Arc<[RegisteredWindow]>,
    barrier: Option<NativeZOrderWindowIdentity>,
}

impl PreparedProvisionalZOrderBand {
    pub(super) fn new(
        point: Point<DevicePixels>,
        current: RegisteredWindow,
        peers: Arc<[RegisteredWindow]>,
        barrier: Option<NativeZOrderWindowIdentity>,
    ) -> Self {
        Self {
            point,
            current,
            peers,
            barrier,
        }
    }

    pub(super) fn point(&self) -> Point<DevicePixels> {
        self.point
    }

    pub(super) fn current(&self) -> RegisteredWindow {
        self.current
    }

    pub(super) fn peers(&self) -> &[RegisteredWindow] {
        &self.peers
    }

    pub(super) fn barrier(&self) -> Option<NativeZOrderWindowIdentity> {
        self.barrier
    }

    pub(super) fn insert_after(&self) -> HWND {
        self.barrier
            .map_or(HWND_TOP, NativeZOrderWindowIdentity::hwnd)
    }

    pub(super) fn is_peer(&self, identity: NativeZOrderWindowIdentity) -> bool {
        identity
            .registered()
            .is_some_and(|registered| self.peers.iter().any(|peer| peer.matches(registered)))
    }
}

pub(super) fn retain_current_registered_windows(
    expected: &[RegisteredWindow],
    current: &[RegisteredWindow],
) -> Arc<[RegisteredWindow]> {
    expected
        .iter()
        .copied()
        .filter(|expected| current.contains(expected))
        .collect()
}

#[derive(Clone, Debug)]
pub(super) struct ProvisionalActivationZOrderAuthority {
    mutation_generation: u64,
    physical_geometry: PlatformWindowPhysicalGeometry,
    prepared: PreparedProvisionalZOrderBand,
    native_authority: WeakRegisteredWindowAuthority,
}

impl ProvisionalActivationZOrderAuthority {
    pub(super) fn new(
        mutation_generation: u64,
        physical_geometry: PlatformWindowPhysicalGeometry,
        prepared: PreparedProvisionalZOrderBand,
        native_authority: WeakRegisteredWindowAuthority,
    ) -> Self {
        Self {
            mutation_generation,
            physical_geometry,
            prepared,
            native_authority,
        }
    }

    pub(super) fn mutation_generation(&self) -> u64 {
        self.mutation_generation
    }

    pub(super) fn physical_geometry(&self) -> PlatformWindowPhysicalGeometry {
        self.physical_geometry
    }

    pub(super) fn prepared(&self) -> &PreparedProvisionalZOrderBand {
        &self.prepared
    }

    pub(super) fn native_authority(&self) -> &WeakRegisteredWindowAuthority {
        &self.native_authority
    }

    #[cfg(test)]
    pub(super) fn replace_physical_geometry_for_test(
        &mut self,
        physical_geometry: PlatformWindowPhysicalGeometry,
    ) {
        self.physical_geometry = physical_geometry;
    }

    fn same_lineage_as(&self, other: &Self) -> bool {
        self.mutation_generation == other.mutation_generation
            && self
                .native_authority
                .same_lineage_as(&other.native_authority)
    }

    fn rebound(&self, mutation_generation: u64) -> Self {
        let mut rebound = self.clone();
        rebound.mutation_generation = mutation_generation;
        rebound
    }

    pub(super) fn refreshed(
        &self,
        physical_geometry: PlatformWindowPhysicalGeometry,
        prepared: PreparedProvisionalZOrderBand,
    ) -> Self {
        Self {
            mutation_generation: self.mutation_generation,
            physical_geometry,
            prepared,
            native_authority: self.native_authority.clone(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub(super) struct ProvisionalActivationZOrderState {
    inner: ProvisionalActivationZOrderStateKind,
}

#[derive(Clone, Debug, Default)]
enum ProvisionalActivationZOrderStateKind {
    #[default]
    Absent,
    Armed(ProvisionalActivationZOrderAuthority),
    PlacementPending {
        generation: u64,
        previous: ProvisionalActivationZOrderAuthority,
    },
    Invalidated(ProvisionalActivationZOrderAuthority),
}

#[derive(Clone, Debug)]
pub(super) enum ProvisionalActivationZOrderSnapshot {
    Absent,
    PlacementPending,
    Armed(ProvisionalActivationZOrderAuthority),
    Invalidated(ProvisionalActivationZOrderAuthority),
}

pub(super) enum ProvisionalActivationNativeTransition {
    Absent,
    Park(ProvisionalActivationZOrderRecoveryKey),
    Recover(ProvisionalActivationZOrderAuthority),
}

pub(super) enum ProvisionalActivationPlacementFinish {
    Superseded,
    RetainCurrent,
    Reconcile {
        previous: ProvisionalActivationZOrderAuthority,
        rebound: ProvisionalActivationZOrderAuthority,
    },
}

impl ProvisionalActivationZOrderState {
    pub(super) fn armed(authority: ProvisionalActivationZOrderAuthority) -> Self {
        Self {
            inner: ProvisionalActivationZOrderStateKind::Armed(authority),
        }
    }

    pub(super) fn invalidated(authority: ProvisionalActivationZOrderAuthority) -> Self {
        Self {
            inner: ProvisionalActivationZOrderStateKind::Invalidated(authority),
        }
    }

    pub(super) fn snapshot(&self) -> ProvisionalActivationZOrderSnapshot {
        match &self.inner {
            ProvisionalActivationZOrderStateKind::Absent => {
                ProvisionalActivationZOrderSnapshot::Absent
            }
            ProvisionalActivationZOrderStateKind::Armed(authority) => {
                ProvisionalActivationZOrderSnapshot::Armed(authority.clone())
            }
            ProvisionalActivationZOrderStateKind::PlacementPending { .. } => {
                ProvisionalActivationZOrderSnapshot::PlacementPending
            }
            ProvisionalActivationZOrderStateKind::Invalidated(authority) => {
                ProvisionalActivationZOrderSnapshot::Invalidated(authority.clone())
            }
        }
    }

    pub(super) fn clear(&mut self) {
        self.inner = ProvisionalActivationZOrderStateKind::Absent;
    }

    pub(super) fn candidate_is_current(
        &self,
        authority: &ProvisionalActivationZOrderAuthority,
    ) -> bool {
        match &self.inner {
            ProvisionalActivationZOrderStateKind::Armed(current)
            | ProvisionalActivationZOrderStateKind::Invalidated(current) => {
                current.same_lineage_as(authority)
            }
            ProvisionalActivationZOrderStateKind::Absent
            | ProvisionalActivationZOrderStateKind::PlacementPending { .. } => false,
        }
    }

    pub(super) fn invalidate(&mut self, authority: &ProvisionalActivationZOrderAuthority) {
        let current = match &self.inner {
            ProvisionalActivationZOrderStateKind::Armed(current)
            | ProvisionalActivationZOrderStateKind::Invalidated(current)
                if current.same_lineage_as(authority) =>
            {
                current.clone()
            }
            ProvisionalActivationZOrderStateKind::Absent
            | ProvisionalActivationZOrderStateKind::PlacementPending { .. }
            | ProvisionalActivationZOrderStateKind::Armed(_)
            | ProvisionalActivationZOrderStateKind::Invalidated(_) => return,
        };
        self.inner = ProvisionalActivationZOrderStateKind::Invalidated(current);
    }

    pub(super) fn replace_refreshed(
        &mut self,
        previous: &ProvisionalActivationZOrderAuthority,
        replacement: ProvisionalActivationZOrderAuthority,
    ) -> bool {
        let current_matches = matches!(
            &self.inner,
            ProvisionalActivationZOrderStateKind::Armed(current)
                | ProvisionalActivationZOrderStateKind::Invalidated(current)
                if current.same_lineage_as(previous)
        );
        if current_matches {
            self.inner = ProvisionalActivationZOrderStateKind::Armed(replacement);
        }
        current_matches
    }

    pub(super) fn observe_native_activation(&mut self) -> ProvisionalActivationNativeTransition {
        match &self.inner {
            ProvisionalActivationZOrderStateKind::Absent => {
                ProvisionalActivationNativeTransition::Absent
            }
            ProvisionalActivationZOrderStateKind::PlacementPending {
                generation,
                previous,
            } => ProvisionalActivationNativeTransition::Park(
                ProvisionalActivationZOrderRecoveryKey::from_authority(previous)
                    .rebound(*generation),
            ),
            ProvisionalActivationZOrderStateKind::Armed(authority)
            | ProvisionalActivationZOrderStateKind::Invalidated(authority) => {
                let authority = authority.clone();
                self.inner = ProvisionalActivationZOrderStateKind::Invalidated(authority.clone());
                ProvisionalActivationNativeTransition::Recover(authority)
            }
        }
    }

    pub(super) fn invalidated_authority(&self) -> Option<ProvisionalActivationZOrderAuthority> {
        match &self.inner {
            ProvisionalActivationZOrderStateKind::Invalidated(authority) => Some(authority.clone()),
            ProvisionalActivationZOrderStateKind::Absent
            | ProvisionalActivationZOrderStateKind::Armed(_)
            | ProvisionalActivationZOrderStateKind::PlacementPending { .. } => None,
        }
    }

    pub(super) fn matching_invalidated_authority(
        &self,
        key: ProvisionalActivationZOrderRecoveryKey,
    ) -> Option<ProvisionalActivationZOrderAuthority> {
        self.invalidated_authority()
            .filter(|authority| key.matches(authority))
    }

    pub(super) fn consume(&mut self, authority: &ProvisionalActivationZOrderAuthority) {
        if matches!(
            &self.inner,
            ProvisionalActivationZOrderStateKind::Armed(candidate)
                | ProvisionalActivationZOrderStateKind::Invalidated(candidate)
                if candidate.same_lineage_as(authority)
        ) {
            self.clear();
        }
    }

    pub(super) fn begin_placement(
        &mut self,
        generation: u64,
    ) -> Option<(
        ProvisionalActivationZOrderRecoveryKey,
        ProvisionalActivationZOrderRecoveryKey,
    )> {
        let previous = match std::mem::take(&mut self.inner) {
            ProvisionalActivationZOrderStateKind::Armed(previous)
            | ProvisionalActivationZOrderStateKind::Invalidated(previous) => {
                let current = ProvisionalActivationZOrderRecoveryKey::from_authority(&previous);
                Some((previous, current))
            }
            ProvisionalActivationZOrderStateKind::PlacementPending {
                generation: pending_generation,
                previous,
            } => {
                let current = ProvisionalActivationZOrderRecoveryKey::from_authority(&previous)
                    .rebound(pending_generation);
                Some((previous, current))
            }
            ProvisionalActivationZOrderStateKind::Absent => None,
        };
        let Some((previous, current)) = previous else {
            self.inner = ProvisionalActivationZOrderStateKind::Absent;
            return None;
        };
        self.inner = ProvisionalActivationZOrderStateKind::PlacementPending {
            generation,
            previous,
        };
        Some((current, current.rebound(generation)))
    }

    pub(super) fn placement_finish(&self, generation: u64) -> ProvisionalActivationPlacementFinish {
        match &self.inner {
            ProvisionalActivationZOrderStateKind::PlacementPending {
                generation: pending_generation,
                previous,
            } if *pending_generation == generation => {
                ProvisionalActivationPlacementFinish::Reconcile {
                    previous: previous.clone(),
                    rebound: previous.rebound(generation),
                }
            }
            ProvisionalActivationZOrderStateKind::Armed(current)
            | ProvisionalActivationZOrderStateKind::Invalidated(current)
                if current.mutation_generation() == generation =>
            {
                ProvisionalActivationPlacementFinish::RetainCurrent
            }
            ProvisionalActivationZOrderStateKind::Absent => {
                ProvisionalActivationPlacementFinish::RetainCurrent
            }
            ProvisionalActivationZOrderStateKind::PlacementPending { .. }
            | ProvisionalActivationZOrderStateKind::Armed(_)
            | ProvisionalActivationZOrderStateKind::Invalidated(_) => {
                ProvisionalActivationPlacementFinish::Superseded
            }
        }
    }

    pub(super) fn commit_placement_reconciliation(
        &mut self,
        generation: u64,
        previous: &ProvisionalActivationZOrderAuthority,
        next: Self,
    ) -> bool {
        let still_pending = matches!(
            &self.inner,
            ProvisionalActivationZOrderStateKind::PlacementPending {
                generation: pending_generation,
                previous: current,
            } if *pending_generation == generation && current.same_lineage_as(previous)
        );
        if still_pending {
            *self = next;
        }
        still_pending
    }

    pub(super) fn recovery_key(&self) -> Option<ProvisionalActivationZOrderRecoveryKey> {
        match &self.inner {
            ProvisionalActivationZOrderStateKind::Armed(authority)
            | ProvisionalActivationZOrderStateKind::Invalidated(authority) => Some(
                ProvisionalActivationZOrderRecoveryKey::from_authority(authority),
            ),
            ProvisionalActivationZOrderStateKind::Absent
            | ProvisionalActivationZOrderStateKind::PlacementPending { .. } => None,
        }
    }

    pub(super) fn consume_armed_after_recovery(&mut self, consume: Option<bool>) {
        if consume == Some(true)
            && matches!(&self.inner, ProvisionalActivationZOrderStateKind::Armed(_))
        {
            self.clear();
        }
    }

    #[cfg(test)]
    pub(super) fn has_authority(&self) -> bool {
        !matches!(&self.inner, ProvisionalActivationZOrderStateKind::Absent)
    }

    #[cfg(test)]
    pub(super) fn is_invalidated(&self) -> bool {
        matches!(
            &self.inner,
            ProvisionalActivationZOrderStateKind::Invalidated(_)
        )
    }

    #[cfg(test)]
    pub(super) fn is_armed(&self) -> bool {
        matches!(&self.inner, ProvisionalActivationZOrderStateKind::Armed(_))
    }

    #[cfg(test)]
    pub(super) fn settled_authority_mut(
        &mut self,
    ) -> Option<&mut ProvisionalActivationZOrderAuthority> {
        match &mut self.inner {
            ProvisionalActivationZOrderStateKind::Armed(authority)
            | ProvisionalActivationZOrderStateKind::Invalidated(authority) => Some(authority),
            ProvisionalActivationZOrderStateKind::Absent
            | ProvisionalActivationZOrderStateKind::PlacementPending { .. } => None,
        }
    }
}

pub(super) enum ProvisionalActivationZOrderRefresh {
    AuthorityEnded,
    TemporarilyUnavailable,
    GeometryChanged,
    Refreshed(ProvisionalActivationZOrderAuthority),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProvisionalActivationZOrderMutationTerminal {
    Settled,
    WindowClosed,
}

impl From<PlatformWindowMutationTerminal> for ProvisionalActivationZOrderMutationTerminal {
    fn from(terminal: PlatformWindowMutationTerminal) -> Self {
        match terminal {
            PlatformWindowMutationTerminal::WindowClosed => Self::WindowClosed,
            PlatformWindowMutationTerminal::Observed
            | PlatformWindowMutationTerminal::Rejected
            | PlatformWindowMutationTerminal::Unsupported => Self::Settled,
        }
    }
}

impl From<PlatformWindowMutationUnobservedTerminal>
    for ProvisionalActivationZOrderMutationTerminal
{
    fn from(terminal: PlatformWindowMutationUnobservedTerminal) -> Self {
        match terminal {
            PlatformWindowMutationUnobservedTerminal::WindowClosed => Self::WindowClosed,
            PlatformWindowMutationUnobservedTerminal::Unchanged
            | PlatformWindowMutationUnobservedTerminal::Rejected
            | PlatformWindowMutationUnobservedTerminal::Unsupported => Self::Settled,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProvisionalActivationWindowAuthorityStatus {
    AuthorityEnded,
    TemporarilyUnavailable,
    GeometryChanged,
    Compatible(PlatformWindowPhysicalGeometry),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProvisionalActivationWindowObservation {
    AuthorityEnded,
    Hidden,
    Minimized {
        hwnd: HWND,
    },
    Observed {
        hwnd: HWND,
        physical_geometry: PlatformWindowPhysicalGeometry,
    },
}

pub(super) fn classify_activation_window_authority(
    previous: PlatformWindowPhysicalGeometry,
    current: Option<PlatformWindowPhysicalGeometry>,
) -> ProvisionalActivationWindowAuthorityStatus {
    let Some(current) = current else {
        return ProvisionalActivationWindowAuthorityStatus::AuthorityEnded;
    };
    if activation_geometry_can_refresh(previous, current) {
        ProvisionalActivationWindowAuthorityStatus::Compatible(current)
    } else {
        ProvisionalActivationWindowAuthorityStatus::GeometryChanged
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ProvisionalActivationZOrderRecoveryKey {
    mutation_generation: u64,
    registration: RegisteredWindow,
}

impl ProvisionalActivationZOrderRecoveryKey {
    pub(super) fn from_authority(authority: &ProvisionalActivationZOrderAuthority) -> Self {
        Self {
            mutation_generation: authority.mutation_generation,
            registration: authority.native_authority.registration,
        }
    }

    fn matches(self, authority: &ProvisionalActivationZOrderAuthority) -> bool {
        self == Self::from_authority(authority)
    }

    fn rebound(self, mutation_generation: u64) -> Self {
        Self {
            mutation_generation,
            registration: self.registration,
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct ProvisionalActivationZOrderRecovery {
    scheduled: bool,
    schedule_epoch: u64,
    attempt: u8,
    key: Option<ProvisionalActivationZOrderRecoveryKey>,
    consume_after_recovery: bool,
}

impl ProvisionalActivationZOrderRecovery {
    fn advance_schedule_epoch(&mut self) {
        self.schedule_epoch = self.schedule_epoch.wrapping_add(1).max(1);
    }

    pub(super) fn schedule(
        &mut self,
        key: ProvisionalActivationZOrderRecoveryKey,
        consume_after_recovery: bool,
    ) -> Option<(u64, Duration)> {
        if self.scheduled && self.key.is_some_and(|current| current == key) {
            self.consume_after_recovery |= consume_after_recovery;
            return None;
        }
        if self.key.is_none_or(|current| current != key) {
            self.attempt = 0;
            self.consume_after_recovery = consume_after_recovery;
        } else {
            self.attempt = 0;
            self.consume_after_recovery |= consume_after_recovery;
        }
        self.scheduled = true;
        self.key = Some(key);
        self.advance_schedule_epoch();
        Some((self.schedule_epoch, recovery_retry_delay(self.attempt)))
    }

    pub(super) fn schedule_rejected_activation(
        &mut self,
        key: ProvisionalActivationZOrderRecoveryKey,
    ) -> (u64, Duration) {
        self.scheduled = true;
        self.key = Some(key);
        self.attempt = 0;
        self.consume_after_recovery = false;
        self.advance_schedule_epoch();
        (self.schedule_epoch, recovery_retry_delay(self.attempt))
    }

    pub(super) fn claim(
        &mut self,
        schedule_epoch: u64,
        key: ProvisionalActivationZOrderRecoveryKey,
    ) -> Option<bool> {
        if !self.scheduled
            || self.schedule_epoch != schedule_epoch
            || self.key.is_none_or(|current| current != key)
        {
            return None;
        }
        self.scheduled = false;
        Some(self.consume_after_recovery)
    }

    pub(super) fn schedule_retry(
        &mut self,
        key: ProvisionalActivationZOrderRecoveryKey,
    ) -> Option<(u64, Duration)> {
        if self.scheduled
            || self.key.is_none_or(|current| current != key)
            || self.attempt >= MAX_AUTOMATIC_RECOVERY_RETRIES
        {
            return None;
        }
        self.attempt += 1;
        self.scheduled = true;
        self.advance_schedule_epoch();
        Some((self.schedule_epoch, recovery_retry_delay(self.attempt)))
    }

    pub(super) fn park(
        &mut self,
        key: ProvisionalActivationZOrderRecoveryKey,
        consume_after_recovery: bool,
    ) {
        if self.key.is_none_or(|current| current != key) {
            self.attempt = 0;
            self.consume_after_recovery = consume_after_recovery;
        } else {
            self.consume_after_recovery |= consume_after_recovery;
        }
        self.scheduled = false;
        self.key = Some(key);
        self.advance_schedule_epoch();
    }

    pub(super) fn park_rejected_activation(&mut self, key: ProvisionalActivationZOrderRecoveryKey) {
        self.scheduled = false;
        self.key = Some(key);
        self.attempt = 0;
        self.consume_after_recovery = false;
        self.advance_schedule_epoch();
    }

    pub(super) fn rebind_for_placement(
        &mut self,
        current: ProvisionalActivationZOrderRecoveryKey,
        replacement: ProvisionalActivationZOrderRecoveryKey,
    ) {
        if self.key.is_none_or(|key| key != current) {
            return;
        }
        self.scheduled = false;
        self.key = Some(replacement);
        self.advance_schedule_epoch();
    }

    pub(super) fn consume_intent_for(
        &self,
        key: ProvisionalActivationZOrderRecoveryKey,
    ) -> Option<bool> {
        self.key
            .is_some_and(|current| current == key)
            .then_some(self.consume_after_recovery)
    }

    pub(super) fn complete(&mut self, key: ProvisionalActivationZOrderRecoveryKey) {
        if self.key.is_some_and(|current| current == key) {
            self.scheduled = false;
            self.advance_schedule_epoch();
            self.attempt = 0;
            self.key = None;
            self.consume_after_recovery = false;
        }
    }

    pub(super) fn complete_claimed(
        &mut self,
        schedule_epoch: u64,
        key: ProvisionalActivationZOrderRecoveryKey,
    ) {
        if self.schedule_epoch == schedule_epoch && !self.scheduled {
            self.complete(key);
        }
    }

    pub(super) fn complete_generation(&mut self, mutation_generation: u64) {
        if let Some(key) = self
            .key
            .filter(|key| key.mutation_generation == mutation_generation)
        {
            self.complete(key);
        }
    }

    pub(super) fn cancel(&mut self) {
        if self.key.is_none() && !self.scheduled {
            return;
        }
        self.scheduled = false;
        self.advance_schedule_epoch();
        self.attempt = 0;
        self.key = None;
        self.consume_after_recovery = false;
    }

    #[cfg(test)]
    pub(super) fn retry_is_scheduled(&self) -> bool {
        self.scheduled && self.attempt > 0 && self.key.is_some()
    }

    #[cfg(test)]
    pub(super) fn is_parked(&self) -> bool {
        !self.scheduled && self.key.is_some()
    }
}

#[cfg(test)]
mod tests {
    use open_gpui::{
        Bounds, DevicePixels, DisplayId, PlatformPhysicalDisplayObservation,
        PlatformWindowPhysicalGeometry, WindowId, point, size,
    };
    use windows::Win32::Foundation::HWND;

    use super::*;

    fn recovery_key(
        mutation_generation: u64,
        nonce: usize,
    ) -> ProvisionalActivationZOrderRecoveryKey {
        ProvisionalActivationZOrderRecoveryKey {
            mutation_generation,
            registration: RegisteredWindow::new(HWND::default(), nonce, WindowId::from(1_u64)),
        }
    }

    #[test]
    fn provisional_activation_peers_retain_only_exact_live_incarnations() {
        let retained = RegisteredWindow::new(HWND::default(), 7, WindowId::from(1_u64));
        let closed = RegisteredWindow::new(HWND::default(), 8, WindowId::from(2_u64));
        let replacement = RegisteredWindow::new(HWND::default(), 9, closed.window_id());

        let current =
            retain_current_registered_windows(&[retained, closed], &[retained, replacement]);

        assert_eq!(&*current, &[retained]);
    }

    #[test]
    fn provisional_activation_recovery_does_not_inherit_consumption_across_authorities() {
        let first_key = recovery_key(3, 7);
        let replacement_key = recovery_key(4, 8);
        let mut recovery = ProvisionalActivationZOrderRecovery::default();

        let (first_generation, _) = recovery
            .schedule(first_key, true)
            .expect("the first recovery should schedule");
        assert_eq!(recovery.claim(first_generation, first_key), Some(true));

        let (replacement_generation, _) = recovery
            .schedule(replacement_key, false)
            .expect("the replacement recovery should supersede the claimed authority");
        assert_eq!(
            recovery.claim(replacement_generation, replacement_key),
            Some(false),
            "a replacement authority must not inherit the prior activation's consume-on-success policy"
        );
    }

    #[test]
    fn rejected_activation_supersedes_a_consuming_recovery_for_the_same_authority() {
        let key = recovery_key(3, 7);
        let mut recovery = ProvisionalActivationZOrderRecovery::default();

        let (accepted_epoch, _) = recovery
            .schedule(key, true)
            .expect("an exact native activation should schedule consuming recovery");
        let (rejected_epoch, _) = recovery.schedule_rejected_activation(key);

        assert_eq!(
            recovery.claim(accepted_epoch, key),
            None,
            "the rejected command must invalidate an earlier consuming timer"
        );
        assert_eq!(
            recovery.claim(rejected_epoch, key),
            Some(false),
            "recovery after a rejected command must retain the activation authority"
        );

        recovery.park_rejected_activation(key);
        recovery.complete_claimed(rejected_epoch, key);
        assert_eq!(
            recovery.consume_intent_for(key),
            Some(false),
            "a claimed consuming recovery must not clear the newer retained authority"
        );
    }

    #[test]
    fn predispatch_rejection_preserves_an_existing_consuming_recovery() {
        let key = recovery_key(3, 7);
        let mut recovery = ProvisionalActivationZOrderRecovery::default();

        let (accepted_epoch, _) = recovery
            .schedule(key, true)
            .expect("an exact native activation should schedule consuming recovery");
        assert_eq!(
            recovery.schedule(key, false),
            None,
            "a pre-dispatch rejection must reuse the existing recovery schedule",
        );
        assert_eq!(
            recovery.claim(accepted_epoch, key),
            Some(true),
            "a command rejected before native activation must preserve the earlier consume intent",
        );
    }

    #[test]
    fn provisional_activation_recovery_rebinds_claimed_intent_across_placement_generation() {
        let first_key = recovery_key(3, 7);
        let rebound_key = first_key.rebound(4);
        let mut recovery = ProvisionalActivationZOrderRecovery::default();

        let (first_generation, _) = recovery
            .schedule(first_key, true)
            .expect("the first recovery should schedule");
        assert_eq!(recovery.claim(first_generation, first_key), Some(true));

        recovery.rebind_for_placement(first_key, rebound_key);
        let _ = recovery.schedule_retry(first_key);
        recovery.complete(first_key);

        assert_eq!(recovery.consume_intent_for(rebound_key), Some(true));
        let (rebound_generation, _) = recovery
            .schedule(rebound_key, true)
            .expect("the rebound recovery should schedule under the new placement generation");
        assert_eq!(recovery.claim(rebound_generation, rebound_key), Some(true));
    }

    #[test]
    fn provisional_activation_recovery_parks_after_a_bounded_retry_wave() {
        let key = recovery_key(3, 7);
        let mut recovery = ProvisionalActivationZOrderRecovery::default();

        let (initial_epoch, _) = recovery
            .schedule(key, true)
            .expect("an explicit recovery request should start one retry wave");
        assert_eq!(recovery.claim(initial_epoch, key), Some(true));

        for _ in 0..MAX_AUTOMATIC_RECOVERY_RETRIES {
            let (retry_epoch, _) = recovery
                .schedule_retry(key)
                .expect("the bounded automatic retry wave should still have budget");
            assert_eq!(recovery.claim(retry_epoch, key), Some(true));
        }
        assert!(
            recovery.schedule_retry(key).is_none(),
            "persistent native failure must park instead of scheduling forever"
        );
        assert_eq!(
            recovery.consume_intent_for(key),
            Some(true),
            "parking must retain the exact consume-on-success intent"
        );

        let (explicit_epoch, _) = recovery
            .schedule(key, true)
            .expect("a later explicit request should start a fresh bounded retry wave");
        assert_eq!(recovery.claim(explicit_epoch, key), Some(true));
    }

    #[test]
    fn provisional_activation_recovery_restarts_after_visibility_parking() {
        let key = recovery_key(3, 7);
        let mut recovery = ProvisionalActivationZOrderRecovery::default();

        let (first_epoch, _) = recovery
            .schedule(key, true)
            .expect("the first recovery should schedule");
        assert_eq!(recovery.claim(first_epoch, key), Some(true));
        recovery.park(key, true);

        assert_eq!(recovery.consume_intent_for(key), Some(true));
        let (resumed_epoch, _) = recovery
            .schedule(key, true)
            .expect("a visibility transition should start a fresh recovery wave");
        assert_eq!(recovery.attempt, 0);
        assert_eq!(recovery.claim(resumed_epoch, key), Some(true));
    }

    #[test]
    fn claimed_recovery_cannot_complete_a_reentrant_same_key_schedule() {
        let key = recovery_key(3, 7);
        let mut recovery = ProvisionalActivationZOrderRecovery::default();

        let (first_epoch, _) = recovery
            .schedule(key, true)
            .expect("the first recovery should schedule");
        assert_eq!(recovery.claim(first_epoch, key), Some(true));
        let (replacement_epoch, _) = recovery
            .schedule(key, true)
            .expect("a reentrant explicit request should supersede the claimed schedule");

        recovery.complete_claimed(first_epoch, key);

        assert_eq!(recovery.claim(replacement_epoch, key), Some(true));
    }

    #[test]
    fn provisional_activation_geometry_refresh_only_accepts_topology_metadata_drift() {
        let client_bounds = Bounds::new(
            point(DevicePixels(-1_200), DevicePixels(80)),
            size(DevicePixels(800), DevicePixels(600)),
        );
        let display_bounds = Bounds::new(
            point(DevicePixels(-1_920), DevicePixels(0)),
            size(DevicePixels(1_920), DevicePixels(1_080)),
        );
        let display = |generation, display_id, bounds, visible_bounds, scale_factor| {
            PlatformPhysicalDisplayObservation::try_new(
                generation,
                DisplayId::from(display_id),
                bounds,
                visible_bounds,
                scale_factor,
            )
            .unwrap()
        };
        let geometry = |bounds, scale_factor, display| {
            PlatformWindowPhysicalGeometry::try_new(bounds, scale_factor)
                .and_then(|geometry| geometry.with_display_observation(display))
                .unwrap()
        };
        let original = geometry(
            client_bounds,
            1.5,
            display(7, 11, display_bounds, display_bounds, 1.5),
        );
        let topology_refresh = geometry(
            client_bounds,
            1.5,
            display(
                8,
                11,
                display_bounds,
                Bounds::new(
                    display_bounds.origin,
                    size(DevicePixels(1_920), DevicePixels(1_040)),
                ),
                1.5,
            ),
        );
        assert!(activation_geometry_can_refresh(original, topology_refresh));
        assert!(matches!(
            classify_activation_window_authority(original, Some(topology_refresh)),
            ProvisionalActivationWindowAuthorityStatus::Compatible(current)
                if current == topology_refresh
        ));
        assert!(matches!(
            classify_activation_window_authority(original, None),
            ProvisionalActivationWindowAuthorityStatus::AuthorityEnded
        ));

        let moved = geometry(
            Bounds::new(
                point(DevicePixels(-1_100), DevicePixels(80)),
                client_bounds.size,
            ),
            1.5,
            display(8, 11, display_bounds, display_bounds, 1.5),
        );
        assert!(!activation_geometry_can_refresh(original, moved));
        assert!(matches!(
            classify_activation_window_authority(original, Some(moved)),
            ProvisionalActivationWindowAuthorityStatus::GeometryChanged
        ));

        let rescaled = geometry(
            client_bounds,
            2.0,
            display(8, 11, display_bounds, display_bounds, 2.0),
        );
        assert!(!activation_geometry_can_refresh(original, rescaled));

        let replaced_display = geometry(
            client_bounds,
            1.5,
            display(8, 12, display_bounds, display_bounds, 1.5),
        );
        assert!(!activation_geometry_can_refresh(original, replaced_display,));

        let resized_display = geometry(
            client_bounds,
            1.5,
            display(
                8,
                11,
                Bounds::new(
                    display_bounds.origin,
                    size(DevicePixels(1_600), DevicePixels(900)),
                ),
                Bounds::new(
                    display_bounds.origin,
                    size(DevicePixels(1_600), DevicePixels(900)),
                ),
                1.5,
            ),
        );
        assert!(!activation_geometry_can_refresh(original, resized_display,));
    }
}
