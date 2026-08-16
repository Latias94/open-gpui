mod native;
mod topology;

use self::native::{MacNativeDisplayBatch, MacNativeDisplayRow};
use cocoa::base::id;
use open_gpui::{Bounds, DisplayId, Pixels, PlatformDisplay, PlatformDisplaySnapshot};
use smallvec::SmallVec;
use std::{rc::Rc, sync::Arc};
use uuid::Uuid;

pub(crate) use self::topology::{
    MacDisplayTopologyAuthority, MacDisplayTopologyFailure, MacDisplayTopologyHandle,
    MacDisplayTopologyListener, MacDisplayTopologyRefresh, MacDisplayTopologyRefreshRequest,
    MacDisplayTopologyRetry, MacDisplayTopologySubscription, MacDisplayTopologyUnavailable,
    MacDisplayTopologyWeak,
};

/// Detached facts for one display in a committed macOS topology publication.
///
/// `CGDirectDisplayID` and `NSScreen` are deliberately absent because both are native adapter
/// inputs whose lifetime is shorter than a display-topology publication.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MacDisplay {
    display_id: DisplayId,
    uuid: Uuid,
    scale_factor: f32,
    bounds: Bounds<Pixels>,
    visible_bounds: Bounds<Pixels>,
}

// Native construction rejects non-finite scale factors and geometry, so equality is reflexive.
impl Eq for MacDisplay {}

impl MacDisplay {
    pub(crate) fn scale_factor(self) -> f32 {
        self.scale_factor
    }
}

impl PlatformDisplay for MacDisplay {
    fn id(&self) -> DisplayId {
        self.display_id
    }

    fn uuid(&self) -> anyhow::Result<Uuid> {
        Ok(self.uuid)
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }

    fn visible_bounds(&self) -> Bounds<Pixels> {
        self.visible_bounds
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MacDisplayTopologyCandidate {
    displays: SmallVec<[MacDisplay; 4]>,
    primary_display_id: DisplayId,
}

impl MacDisplayTopologyCandidate {
    fn from_native() -> Result<Self, MacDisplayTopologyFailure> {
        Ok(MacNativeDisplayBatch::stable_from_native()?.candidate)
    }

    fn try_new(
        mut rows: SmallVec<[MacNativeDisplayRow; 4]>,
    ) -> Result<Self, MacDisplayTopologyFailure> {
        if rows.is_empty() {
            return Err(invalid_candidate("display topology is empty"));
        }

        let mut primary_display_id = None;
        for (index, row) in rows.iter().enumerate() {
            if row.screen.is_null() {
                return Err(invalid_candidate(
                    "display topology contains a null NSScreen object",
                ));
            }
            let display = row.display;
            if !native::display_facts_are_coherent(display) {
                return Err(invalid_candidate(format!(
                    "display {:?} has incoherent detached facts",
                    display.display_id
                )));
            }
            for previous in &rows[..index] {
                if previous.display.display_id == display.display_id {
                    return Err(invalid_candidate(format!(
                        "display topology contains duplicate identity {:?}",
                        display.display_id
                    )));
                }
                if previous.display.uuid == display.uuid {
                    return Err(invalid_candidate(format!(
                        "display topology contains duplicate provenance {}",
                        display.uuid
                    )));
                }
                if previous.native_display_id == row.native_display_id {
                    return Err(invalid_candidate(
                        "display topology contains duplicate native display identities",
                    ));
                }
                if previous.screen == row.screen {
                    return Err(invalid_candidate(
                        "display topology contains duplicate NSScreen objects",
                    ));
                }
            }
            if row.is_primary && primary_display_id.replace(display.display_id).is_some() {
                return Err(invalid_candidate(
                    "display topology contains more than one primary display",
                ));
            }
        }
        let Some(primary_display_id) = primary_display_id else {
            return Err(invalid_candidate(
                "display topology has no proven primary display",
            ));
        };

        rows.sort_unstable_by_key(|row| u64::from(row.display.display_id));
        Ok(Self {
            displays: rows.into_iter().map(|row| row.display).collect(),
            primary_display_id,
        })
    }
}

#[derive(Clone, Debug)]
pub(crate) struct MacDisplayTopologySnapshot {
    generation: u64,
    displays: Arc<[MacDisplay]>,
    primary_display_id: DisplayId,
}

impl PartialEq for MacDisplayTopologySnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.generation == other.generation
            && self.displays == other.displays
            && self.primary_display_id == other.primary_display_id
    }
}

impl Eq for MacDisplayTopologySnapshot {}

impl MacDisplayTopologySnapshot {
    fn new(generation: u64, candidate: MacDisplayTopologyCandidate) -> Self {
        debug_assert_ne!(generation, 0);
        let displays: Arc<[MacDisplay]> = Arc::from(candidate.displays.into_vec());
        Self {
            generation,
            displays,
            primary_display_id: candidate.primary_display_id,
        }
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn platform_snapshot(&self) -> PlatformDisplaySnapshot {
        PlatformDisplaySnapshot::try_new(
            Some(self.generation),
            self.displays
                .iter()
                .copied()
                .map(|display| Rc::new(display) as Rc<dyn PlatformDisplay>)
                .collect(),
            Some(self.primary_display_id),
        )
        .expect("validated macOS topology must project to a valid platform snapshot")
    }

    pub(crate) fn primary_display(&self) -> MacDisplay {
        self.display(self.primary_display_id)
            .expect("validated display snapshots retain their primary display")
    }

    pub(crate) fn display(&self, display_id: DisplayId) -> Option<MacDisplay> {
        self.displays
            .iter()
            .copied()
            .find(|display| display.display_id == display_id)
    }

    /// Resolves a detached display identity to one callback-local `NSScreen` after proving that the
    /// complete native topology still matches this publication.
    pub(crate) fn resolve_native_target(
        &self,
        requested_display_id: Option<DisplayId>,
    ) -> Result<ValidatedMacDisplayTarget, MacDisplayTopologyFailure> {
        let display_id = requested_display_id.unwrap_or(self.primary_display_id);
        let expected_display = self
            .display(display_id)
            .ok_or(MacDisplayTopologyFailure::UnknownTarget(display_id))?;
        let batch = MacNativeDisplayBatch::stable_from_native()?;
        if !self.has_same_facts(&batch.candidate) {
            return Err(MacDisplayTopologyFailure::SnapshotChanged(self.generation));
        }
        let row = batch
            .row(display_id)
            .ok_or(MacDisplayTopologyFailure::UnknownTarget(display_id))?;
        if row.display != expected_display {
            return Err(MacDisplayTopologyFailure::SnapshotChanged(self.generation));
        }
        Ok(ValidatedMacDisplayTarget {
            generation: self.generation,
            screen: row.screen,
            display: row.display,
        })
    }

    /// Validates one callback-provided `NSScreen` against this immutable publication without
    /// re-enumerating the complete desktop topology.
    pub(crate) fn validate_native_screen(
        &self,
        screen: id,
    ) -> Result<ValidatedMacDisplayTarget, MacDisplayTopologyFailure> {
        let row = unsafe { MacNativeDisplayRow::observe(screen)? };
        self.validate_native_row(row)
    }

    fn validate_native_row(
        &self,
        row: MacNativeDisplayRow,
    ) -> Result<ValidatedMacDisplayTarget, MacDisplayTopologyFailure> {
        let expected_display = self
            .display(row.display.display_id)
            .ok_or(MacDisplayTopologyFailure::SnapshotChanged(self.generation))?;
        if row.display != expected_display
            || row.is_primary != (row.display.display_id == self.primary_display_id)
        {
            return Err(MacDisplayTopologyFailure::SnapshotChanged(self.generation));
        }
        Ok(ValidatedMacDisplayTarget {
            generation: self.generation,
            screen: row.screen,
            display: row.display,
        })
    }

    fn has_same_facts(&self, candidate: &MacDisplayTopologyCandidate) -> bool {
        self.primary_display_id == candidate.primary_display_id
            && self.displays.as_ref() == candidate.displays.as_slice()
    }
}

/// Ephemeral native target validated against one display publication.
///
/// Callers must consume the `NSScreen` during the current native operation and retain only
/// [`MacDisplay`] plus [`Self::generation`] across callbacks.
#[derive(Debug)]
pub(crate) struct ValidatedMacDisplayTarget {
    generation: u64,
    screen: id,
    display: MacDisplay,
}

impl ValidatedMacDisplayTarget {
    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn screen(&self) -> id {
        self.screen
    }

    pub(crate) fn display(&self) -> MacDisplay {
        self.display
    }
}

fn empty_platform_display_snapshot() -> PlatformDisplaySnapshot {
    PlatformDisplaySnapshot::try_new(None, Vec::new(), None)
        .expect("an empty legacy display projection is valid")
}

fn display_id_from_uuid(uuid: Uuid) -> DisplayId {
    DisplayId::new(uuid.as_u64_pair().0)
}

fn invalid_candidate(message: impl Into<Arc<str>>) -> MacDisplayTopologyFailure {
    MacDisplayTopologyFailure::InvalidCandidate(message.into())
}

#[cfg(test)]
mod tests {
    use super::{
        MacDisplay, MacDisplayTopologyCandidate, MacDisplayTopologyFailure,
        MacDisplayTopologySnapshot, display_id_from_uuid, native::MacNativeDisplayRow,
    };
    use cocoa::base::nil;
    use open_gpui::{Bounds, PlatformDisplay, point, px, size};
    use smallvec::{SmallVec, smallvec};
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

    #[test]
    fn candidate_rejects_empty_duplicate_and_ambiguous_primary_batches() {
        assert!(matches!(
            MacDisplayTopologyCandidate::try_new(SmallVec::new()),
            Err(MacDisplayTopologyFailure::InvalidCandidate(_))
        ));

        let primary = display(1, -1_920.0, -240.0, 1.0, 1_920.0);
        let secondary = display(2, 0.0, 0.0, 2.0, 1_880.0);
        assert!(matches!(
            MacDisplayTopologyCandidate::try_new(smallvec![
                row(1, primary, true),
                row(1, secondary, false),
            ]),
            Err(MacDisplayTopologyFailure::InvalidCandidate(_))
        ));
        assert!(matches!(
            MacDisplayTopologyCandidate::try_new(smallvec![
                row(1, primary, true),
                row(2, secondary, true),
            ]),
            Err(MacDisplayTopologyFailure::InvalidCandidate(_))
        ));

        let invalid_scale = display(3, 1_920.0, 0.0, f32::NAN, 1_920.0);
        assert!(matches!(
            MacDisplayTopologyCandidate::try_new(smallvec![row(3, invalid_scale, true)]),
            Err(MacDisplayTopologyFailure::InvalidCandidate(_))
        ));
    }

    #[test]
    fn candidate_is_order_independent_and_preserves_signed_origins() {
        let primary = display(1, 0.0, 0.0, 2.0, 1_880.0);
        let secondary = display(2, -1_920.0, -1_080.0, 1.0, 1_920.0);
        let forward = MacDisplayTopologyCandidate::try_new(smallvec![
            row(1, primary, true),
            row(2, secondary, false),
        ])
        .unwrap();
        let reverse = MacDisplayTopologyCandidate::try_new(smallvec![
            row(2, secondary, false),
            row(1, primary, true),
        ])
        .unwrap();

        assert_eq!(forward, reverse);
        assert_eq!(forward.displays[0].bounds().origin.y, px(-1_080.0));
    }

    #[test]
    fn callback_validation_rejects_reused_native_identity_with_new_provenance() {
        let primary = display(1, 0.0, 0.0, 2.0, 1_880.0);
        let secondary = display(2, 1_920.0, 0.0, 1.0, 1_920.0);
        let snapshot = MacDisplayTopologySnapshot::new(1, candidate(primary, secondary));

        assert_eq!(
            snapshot
                .validate_native_row(row(2, secondary, false))
                .unwrap()
                .generation(),
            1
        );
        let replacement = display(3, 1_920.0, 0.0, 1.0, 1_920.0);
        assert!(matches!(
            snapshot.validate_native_row(row(2, replacement, false)),
            Err(MacDisplayTopologyFailure::SnapshotChanged(1))
        ));
    }

    #[test]
    fn test_rows_never_publish_null_screen_identity() {
        let primary = display(1, 0.0, 0.0, 2.0, 1_880.0);
        let row = MacNativeDisplayRow {
            screen: nil,
            native_display_id: 1,
            display: primary,
            is_primary: true,
        };
        assert!(matches!(
            MacDisplayTopologyCandidate::try_new(smallvec![row]),
            Err(MacDisplayTopologyFailure::InvalidCandidate(_))
        ));
    }
}
