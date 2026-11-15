//! Complete platform topology and event-time routing into core-issued work-area bindings.

use std::rc::Rc;

use dockspace::{
    geometry::ScaleFactor,
    runtime::{
        DockspaceSession, HostWorkAreaToken, NativeWorkAreaBinding, NativeWorkAreaFacts,
        NativeWorkAreaRoster,
    },
};
use open_gpui::{
    PlatformDisplaySnapshot, PlatformNativePointerPhysicalFrame, PlatformPhysicalDisplayObservation,
};

#[derive(Clone, Default, PartialEq)]
pub(super) struct DesktopObservation(Option<Rc<[PlatformPhysicalDisplayObservation]>>);

impl DesktopObservation {
    pub(super) fn from_snapshot(snapshot: &PlatformDisplaySnapshot) -> Self {
        Self(snapshot.physical_displays().map(Rc::from))
    }

    pub(super) fn roster(&self) -> NativeWorkAreaRoster {
        self.0
            .as_ref()
            .and_then(|displays| {
                displays
                    .iter()
                    .map(|display| {
                        Some(NativeWorkAreaFacts::new(
                            HostWorkAreaToken::new(u64::from(display.display_id())),
                            super::input::physical_rect(display.visible_bounds())?,
                            ScaleFactor::new(f64::from(display.scale_factor())).ok()?,
                        ))
                    })
                    .collect::<Option<Vec<_>>>()
            })
            .map_or(NativeWorkAreaRoster::Unknown, NativeWorkAreaRoster::Exact)
    }

    pub(super) fn work_area(
        &self,
        session: &DockspaceSession,
        frame: PlatformNativePointerPhysicalFrame,
    ) -> Option<NativeWorkAreaBinding> {
        let target = frame.target_display()?;
        let display = self
            .0
            .as_ref()?
            .iter()
            .find(|display| **display == target)?;
        session.native_work_area(HostWorkAreaToken::new(u64::from(display.display_id())))
    }
}
