use smallvec::SmallVec;
use std::{error::Error, fmt, rc::Rc, sync::Arc};
use uuid::Uuid;
use windows::{
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        UI::{
            HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
            WindowsAndMessaging::{
                EDD_GET_DEVICE_INTERFACE_NAME, MONITORINFOF_PRIMARY, USER_DEFAULT_SCREEN_DPI,
            },
        },
    },
    core::{BOOL, PCWSTR},
};

use open_gpui::{
    Bounds, DevicePixels, DisplayId, Pixels, PlatformDisplay, PlatformDisplaySnapshot,
    PlatformPhysicalDisplayObservation, Point, point, size,
};

/// Detached facts for one display in a committed Windows topology publication.
///
/// Native monitor handles are deliberately absent. `HMONITOR` is a short-lived query input and
/// can be reused by Windows after a topology change.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct WindowsDisplay {
    pub display_id: DisplayId,
    scale_factor: f32,
    bounds: Bounds<Pixels>,
    visible_bounds: Bounds<Pixels>,
    physical_bounds: Bounds<DevicePixels>,
    physical_visible_bounds: Bounds<DevicePixels>,
    uuid: Uuid,
}

// Native construction rejects non-finite scale factors, so equality remains reflexive.
impl Eq for WindowsDisplay {}

#[derive(Clone, Copy, Debug)]
struct WindowsNativeDisplayRow {
    handle: HMONITOR,
    display: WindowsDisplay,
    is_primary: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct WindowsDisplayTopologyCandidate {
    displays: SmallVec<[WindowsDisplay; 4]>,
    primary_display_id: DisplayId,
}

impl WindowsDisplayTopologyCandidate {
    fn from_native() -> Result<Self, WindowsDisplayTopologyFailure> {
        let first = Self::collect_once()?;
        let second = Self::collect_once()?;
        if first != second {
            return Err(WindowsDisplayTopologyFailure::UnstableDuringCollection);
        }
        Ok(first)
    }

    fn collect_once() -> Result<Self, WindowsDisplayTopologyFailure> {
        let rows = available_monitors()?
            .into_iter()
            .map(WindowsNativeDisplayRow::observe)
            .collect::<Result<SmallVec<[_; 4]>, _>>()?;
        Self::try_new(rows)
    }

    fn try_new(
        mut rows: SmallVec<[WindowsNativeDisplayRow; 4]>,
    ) -> Result<Self, WindowsDisplayTopologyFailure> {
        if rows.is_empty() {
            return Err(WindowsDisplayTopologyFailure::InvalidCandidate(
                "display topology is empty".into(),
            ));
        }

        let mut primary_display_id = None;
        for (index, row) in rows.iter().enumerate() {
            let display = row.display;
            if display.physical_observation(1).is_none() {
                return Err(WindowsDisplayTopologyFailure::InvalidCandidate(
                    format!(
                        "display {:?} has incoherent physical facts",
                        display.display_id
                    )
                    .into(),
                ));
            }
            for previous in &rows[..index] {
                if previous.display.display_id == display.display_id {
                    return Err(WindowsDisplayTopologyFailure::InvalidCandidate(
                        format!(
                            "display topology contains duplicate display identity {:?}",
                            display.display_id
                        )
                        .into(),
                    ));
                }
                if previous.handle == row.handle {
                    return Err(WindowsDisplayTopologyFailure::InvalidCandidate(
                        "display topology contains duplicate native monitor handles".into(),
                    ));
                }
                if previous.display.uuid == display.uuid {
                    return Err(WindowsDisplayTopologyFailure::InvalidCandidate(
                        format!(
                            "display topology contains duplicate display provenance {}",
                            display.uuid
                        )
                        .into(),
                    ));
                }
            }
            if row.is_primary && primary_display_id.replace(display.display_id).is_some() {
                return Err(WindowsDisplayTopologyFailure::InvalidCandidate(
                    "display topology contains more than one primary display".into(),
                ));
            }
        }
        let Some(primary_display_id) = primary_display_id else {
            return Err(WindowsDisplayTopologyFailure::InvalidCandidate(
                "display topology has no proven primary display".into(),
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
pub(crate) struct WindowsDisplayTopologySnapshot {
    generation: u64,
    displays: Rc<[WindowsDisplay]>,
    primary_display_id: DisplayId,
    physical_observations: Rc<[PlatformPhysicalDisplayObservation]>,
    platform_snapshot: PlatformDisplaySnapshot,
}

impl PartialEq for WindowsDisplayTopologySnapshot {
    fn eq(&self, other: &Self) -> bool {
        self.generation == other.generation
            && self.displays == other.displays
            && self.primary_display_id == other.primary_display_id
    }
}

impl Eq for WindowsDisplayTopologySnapshot {}

impl WindowsDisplayTopologySnapshot {
    fn new(generation: u64, candidate: WindowsDisplayTopologyCandidate) -> Self {
        debug_assert_ne!(generation, 0);
        let displays: Rc<[WindowsDisplay]> = Rc::from(candidate.displays.into_vec());
        let physical_observations: Rc<[PlatformPhysicalDisplayObservation]> = Rc::from(
            displays
                .iter()
                .copied()
                .map(|display| {
                    display
                        .physical_observation(generation)
                        .expect("validated Windows display facts must remain coherent")
                })
                .collect::<Vec<_>>(),
        );
        let platform_snapshot = PlatformDisplaySnapshot::try_new(
            Some(generation),
            displays
                .iter()
                .copied()
                .map(|display| Rc::new(display) as Rc<dyn PlatformDisplay>)
                .collect(),
            Some(candidate.primary_display_id),
        )
        .expect("validated Windows topology must project to a valid platform snapshot");
        Self {
            generation,
            displays,
            primary_display_id: candidate.primary_display_id,
            physical_observations,
            platform_snapshot,
        }
    }

    pub(crate) fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn platform_snapshot(&self) -> PlatformDisplaySnapshot {
        self.platform_snapshot.clone()
    }

    pub(crate) fn primary_display(&self) -> WindowsDisplay {
        self.display(self.primary_display_id)
            .expect("validated display snapshots retain their primary display")
    }

    pub(crate) fn display(&self, display_id: DisplayId) -> Option<WindowsDisplay> {
        self.displays
            .iter()
            .copied()
            .find(|display| display.display_id == display_id)
    }

    pub(crate) fn physical_observation(
        &self,
        display_id: DisplayId,
    ) -> Option<PlatformPhysicalDisplayObservation> {
        self.physical_observations
            .iter()
            .copied()
            .find(|observation| observation.display_id() == display_id)
    }

    pub(crate) fn physical_observation_at(
        &self,
        point: Point<DevicePixels>,
    ) -> Option<PlatformPhysicalDisplayObservation> {
        self.physical_observations
            .iter()
            .copied()
            .find(|observation| observation.contains(point))
    }

    pub(crate) fn display_for_native_monitor(&self, monitor: HMONITOR) -> Option<WindowsDisplay> {
        self.validated_native_display(monitor)
            .map(ValidatedWindowsNativeDisplay::display)
    }

    pub(crate) fn validated_native_display(
        &self,
        monitor: HMONITOR,
    ) -> Option<ValidatedWindowsNativeDisplay> {
        let row = WindowsNativeDisplayRow::observe(monitor).ok()?;
        let display = self.display(row.display.display_id)?;
        if display != row.display {
            return None;
        }
        Some(ValidatedWindowsNativeDisplay {
            monitor,
            display,
            observation: self.physical_observation(display.display_id)?,
        })
    }

    pub(crate) fn native_monitor_for_display(&self, display_id: DisplayId) -> Option<HMONITOR> {
        let observation = self.physical_observation(display_id)?;
        self.validate_target(observation, observation.bounds().center())
            .map(|target| target.monitor)
    }

    pub(crate) fn validate_target(
        &self,
        observation: PlatformPhysicalDisplayObservation,
        point: Point<DevicePixels>,
    ) -> Option<ValidatedWindowsDisplayTarget> {
        let monitor = unsafe {
            MonitorFromPoint(
                POINT {
                    x: point.x.0,
                    y: point.y.0,
                },
                MONITOR_DEFAULTTONULL,
            )
        };
        if monitor.is_invalid() {
            return None;
        }
        self.validate_target_with_native_display(
            observation,
            point,
            self.validated_native_display(monitor)?,
        )
    }

    pub(crate) fn validate_target_with_native_display(
        &self,
        observation: PlatformPhysicalDisplayObservation,
        point: Point<DevicePixels>,
        native_display: ValidatedWindowsNativeDisplay,
    ) -> Option<ValidatedWindowsDisplayTarget> {
        if observation.topology_generation() != self.generation
            || !observation.contains(point)
            || self.physical_observation(observation.display_id()) != Some(observation)
            || native_display.observation != observation
        {
            return None;
        }
        let monitor = unsafe {
            MonitorFromPoint(
                POINT {
                    x: point.x.0,
                    y: point.y.0,
                },
                MONITOR_DEFAULTTONULL,
            )
        };
        (monitor == native_display.monitor).then_some(ValidatedWindowsDisplayTarget { monitor })
    }

    fn has_same_facts(&self, candidate: &WindowsDisplayTopologyCandidate) -> bool {
        self.primary_display_id == candidate.primary_display_id
            && self.displays.as_ref() == candidate.displays.as_slice()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ValidatedWindowsDisplayTarget {
    pub(crate) monitor: HMONITOR,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ValidatedWindowsNativeDisplay {
    monitor: HMONITOR,
    display: WindowsDisplay,
    observation: PlatformPhysicalDisplayObservation,
}

impl ValidatedWindowsNativeDisplay {
    pub(crate) fn display(self) -> WindowsDisplay {
        self.display
    }

    pub(crate) fn observation(self) -> PlatformPhysicalDisplayObservation {
        self.observation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WindowsDisplayTopologyFailure {
    NativeCollection(Arc<str>),
    InvalidCandidate(Arc<str>),
    UnstableDuringCollection,
    GenerationExhausted,
    RequestEpochExhausted,
    RefreshMessageRejected(u32),
}

impl fmt::Display for WindowsDisplayTopologyFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NativeCollection(message) | Self::InvalidCandidate(message) => {
                formatter.write_str(message)
            }
            Self::UnstableDuringCollection => {
                formatter.write_str("display topology changed during native collection")
            }
            Self::GenerationExhausted => {
                formatter.write_str("display topology generation exhausted")
            }
            Self::RequestEpochExhausted => {
                formatter.write_str("display topology refresh request epoch exhausted")
            }
            Self::RefreshMessageRejected(error) => {
                write!(
                    formatter,
                    "display topology refresh message was rejected ({error})"
                )
            }
        }
    }
}

impl Error for WindowsDisplayTopologyFailure {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WindowsDisplayTopologyUnavailable {
    RefreshPending {
        request_epoch: u64,
    },
    Degraded {
        request_epoch: u64,
        failure: WindowsDisplayTopologyFailure,
    },
}

impl fmt::Display for WindowsDisplayTopologyUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RefreshPending { request_epoch } => write!(
                formatter,
                "display topology refresh {request_epoch} is still pending"
            ),
            Self::Degraded {
                request_epoch,
                failure,
            } => write!(
                formatter,
                "display topology refresh {request_epoch} failed: {failure}"
            ),
        }
    }
}

impl Error for WindowsDisplayTopologyUnavailable {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::RefreshPending { .. } => None,
            Self::Degraded { failure, .. } => Some(failure),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum WindowsDisplayTopologyStatus {
    Complete,
    RefreshPending,
    Degraded(WindowsDisplayTopologyFailure),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WindowsDisplayTopologyRefreshRequest {
    pub(crate) request_epoch: u64,
    pub(crate) should_post_message: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum WindowsDisplayTopologyRefresh {
    Unchanged {
        generation: u64,
    },
    Published {
        previous_generation: u64,
        generation: u64,
    },
    RetainedAfterFailure {
        generation: u64,
        failure: WindowsDisplayTopologyFailure,
    },
    Superseded {
        generation: u64,
        request_epoch: u64,
    },
}

pub(crate) struct WindowsDisplayTopologyAuthority {
    current: WindowsDisplayTopologySnapshot,
    status: WindowsDisplayTopologyStatus,
    request_epoch: u64,
    refresh_message_scheduled: bool,
}

impl WindowsDisplayTopologyAuthority {
    pub(crate) fn from_native() -> Result<Self, WindowsDisplayTopologyFailure> {
        Ok(Self::new(WindowsDisplayTopologyCandidate::from_native()?))
    }

    fn new(candidate: WindowsDisplayTopologyCandidate) -> Self {
        Self {
            current: WindowsDisplayTopologySnapshot::new(1, candidate),
            status: WindowsDisplayTopologyStatus::Complete,
            request_epoch: 0,
            refresh_message_scheduled: false,
        }
    }

    pub(crate) fn retained_snapshot(&self) -> WindowsDisplayTopologySnapshot {
        self.current.clone()
    }

    pub(crate) fn is_degraded(&self) -> bool {
        matches!(self.status, WindowsDisplayTopologyStatus::Degraded(_))
    }

    pub(crate) fn exact_snapshot(
        &self,
    ) -> Result<WindowsDisplayTopologySnapshot, WindowsDisplayTopologyUnavailable> {
        match &self.status {
            WindowsDisplayTopologyStatus::Complete => Ok(self.current.clone()),
            WindowsDisplayTopologyStatus::RefreshPending => {
                Err(WindowsDisplayTopologyUnavailable::RefreshPending {
                    request_epoch: self.request_epoch,
                })
            }
            WindowsDisplayTopologyStatus::Degraded(failure) => {
                Err(WindowsDisplayTopologyUnavailable::Degraded {
                    request_epoch: self.request_epoch,
                    failure: failure.clone(),
                })
            }
        }
    }

    pub(crate) fn request_refresh(&mut self) -> WindowsDisplayTopologyRefreshRequest {
        let Some(request_epoch) = self.request_epoch.checked_add(1) else {
            let failure = WindowsDisplayTopologyFailure::RequestEpochExhausted;
            self.status = WindowsDisplayTopologyStatus::Degraded(failure);
            return WindowsDisplayTopologyRefreshRequest {
                request_epoch: self.request_epoch,
                should_post_message: false,
            };
        };
        self.request_epoch = request_epoch;
        self.status = WindowsDisplayTopologyStatus::RefreshPending;
        let should_post_message = !self.refresh_message_scheduled;
        self.refresh_message_scheduled = true;
        WindowsDisplayTopologyRefreshRequest {
            request_epoch,
            should_post_message,
        }
    }

    pub(crate) fn begin_scheduled_refresh(&mut self) -> Option<u64> {
        if !self.refresh_message_scheduled {
            return None;
        }
        self.refresh_message_scheduled = false;
        match self.status {
            WindowsDisplayTopologyStatus::RefreshPending => Some(self.request_epoch),
            WindowsDisplayTopologyStatus::Complete | WindowsDisplayTopologyStatus::Degraded(_) => {
                None
            }
        }
    }

    pub(crate) fn refresh_candidate_from_native()
    -> Result<WindowsDisplayTopologyCandidate, WindowsDisplayTopologyFailure> {
        WindowsDisplayTopologyCandidate::from_native()
    }

    pub(crate) fn finish_refresh(
        &mut self,
        request_epoch: u64,
        candidate: Result<WindowsDisplayTopologyCandidate, WindowsDisplayTopologyFailure>,
    ) -> WindowsDisplayTopologyRefresh {
        if self.request_epoch != request_epoch
            || !matches!(self.status, WindowsDisplayTopologyStatus::RefreshPending)
        {
            return WindowsDisplayTopologyRefresh::Superseded {
                generation: self.current.generation,
                request_epoch: self.request_epoch,
            };
        }

        let candidate = match candidate {
            Ok(candidate) => candidate,
            Err(failure) => {
                self.status = WindowsDisplayTopologyStatus::Degraded(failure.clone());
                return WindowsDisplayTopologyRefresh::RetainedAfterFailure {
                    generation: self.current.generation,
                    failure,
                };
            }
        };
        if self.current.has_same_facts(&candidate) {
            self.status = WindowsDisplayTopologyStatus::Complete;
            return WindowsDisplayTopologyRefresh::Unchanged {
                generation: self.current.generation,
            };
        }
        let Some(generation) = self.current.generation.checked_add(1) else {
            let failure = WindowsDisplayTopologyFailure::GenerationExhausted;
            self.status = WindowsDisplayTopologyStatus::Degraded(failure.clone());
            return WindowsDisplayTopologyRefresh::RetainedAfterFailure {
                generation: self.current.generation,
                failure,
            };
        };
        let previous_generation = self.current.generation;
        self.current = WindowsDisplayTopologySnapshot::new(generation, candidate);
        self.status = WindowsDisplayTopologyStatus::Complete;
        WindowsDisplayTopologyRefresh::Published {
            previous_generation,
            generation,
        }
    }

    pub(crate) fn fail_scheduled_refresh(
        &mut self,
        request_epoch: u64,
        failure: WindowsDisplayTopologyFailure,
    ) {
        if self.request_epoch == request_epoch {
            self.refresh_message_scheduled = false;
            self.status = WindowsDisplayTopologyStatus::Degraded(failure);
        }
    }
}

impl WindowsNativeDisplayRow {
    fn observe(handle: HMONITOR) -> Result<Self, WindowsDisplayTopologyFailure> {
        if handle.is_invalid() {
            return Err(WindowsDisplayTopologyFailure::NativeCollection(
                "native monitor handle is invalid".into(),
            ));
        }
        let info = get_monitor_info(handle).map_err(native_collection_failure)?;
        let uuid = monitor_provenance_uuid(&info).map_err(native_collection_failure)?;
        let display_id = display_id_from_uuid(uuid);
        let scale_factor =
            get_scale_factor_for_monitor(handle).map_err(native_collection_failure)?;
        if !scale_factor.is_finite() || scale_factor <= 0.0 {
            return Err(WindowsDisplayTopologyFailure::NativeCollection(
                "native monitor scale factor is invalid".into(),
            ));
        }

        let monitor = info.monitorInfo.rcMonitor;
        let work = info.monitorInfo.rcWork;
        let physical_size = checked_size(monitor).ok_or_else(|| {
            WindowsDisplayTopologyFailure::NativeCollection(
                "native monitor bounds are invalid".into(),
            )
        })?;
        let physical_work_size = checked_size(work).ok_or_else(|| {
            WindowsDisplayTopologyFailure::NativeCollection(
                "native monitor work area is invalid".into(),
            )
        })?;
        let physical_bounds = Bounds {
            origin: point(monitor.left.into(), monitor.top.into()),
            size: physical_size,
        };
        let physical_visible_bounds = Bounds {
            origin: point(work.left.into(), work.top.into()),
            size: physical_work_size,
        };
        let display = WindowsDisplay {
            display_id,
            scale_factor,
            bounds: physical_bounds.to_pixels(scale_factor),
            visible_bounds: physical_visible_bounds.to_pixels(scale_factor),
            physical_bounds,
            physical_visible_bounds,
            uuid,
        };
        if display.physical_observation(1).is_none() {
            return Err(WindowsDisplayTopologyFailure::NativeCollection(
                "native monitor geometry is incoherent".into(),
            ));
        }
        Ok(Self {
            handle,
            display,
            is_primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
        })
    }
}

impl WindowsDisplay {
    pub(crate) fn physical_observation(
        self,
        topology_generation: u64,
    ) -> Option<PlatformPhysicalDisplayObservation> {
        PlatformPhysicalDisplayObservation::try_new(
            topology_generation,
            self.display_id,
            self.physical_bounds,
            self.physical_visible_bounds,
            self.scale_factor,
        )
    }

    /// Checks whether the center point of the logical bounds belongs to this detached display.
    pub fn check_given_bounds(&self, bounds: Bounds<Pixels>) -> bool {
        let center = bounds.center();
        let point = point(
            DevicePixels((center.x.as_f32() * self.scale_factor) as i32),
            DevicePixels((center.y.as_f32() * self.scale_factor) as i32),
        );
        self.contains_physical_point(point)
    }

    fn contains_physical_point(self, point: Point<DevicePixels>) -> bool {
        let left = i64::from(self.physical_bounds.origin.x.0);
        let top = i64::from(self.physical_bounds.origin.y.0);
        let right = left + i64::from(self.physical_bounds.size.width.0);
        let bottom = top + i64::from(self.physical_bounds.size.height.0);
        let x = i64::from(point.x.0);
        let y = i64::from(point.y.0);
        x >= left && x < right && y >= top && y < bottom
    }

    pub fn physical_bounds(&self) -> Bounds<DevicePixels> {
        self.physical_bounds
    }

    pub(crate) fn scale_factor(&self) -> f32 {
        self.scale_factor
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn physical_visible_bounds(&self) -> Bounds<DevicePixels> {
        self.physical_visible_bounds
    }

    #[cfg(feature = "test-support")]
    pub(crate) fn available_for_native_test() -> Vec<Self> {
        WindowsDisplayTopologyCandidate::from_native()
            .map(|candidate| candidate.displays.into_vec())
            .unwrap_or_else(|error| {
                log::error!("cannot collect a complete Windows display topology: {error}");
                Vec::new()
            })
    }
}

impl PlatformDisplay for WindowsDisplay {
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

fn available_monitors() -> Result<SmallVec<[HMONITOR; 4]>, WindowsDisplayTopologyFailure> {
    let mut monitors: SmallVec<[HMONITOR; 4]> = SmallVec::new();
    unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(monitor_enum_proc),
            LPARAM(&mut monitors as *mut _ as _),
        )
        .ok()
        .map_err(native_collection_failure)?;
    }
    if monitors.is_empty() {
        return Err(WindowsDisplayTopologyFailure::NativeCollection(
            "Windows returned an empty monitor enumeration".into(),
        ));
    }
    Ok(monitors)
}

unsafe extern "system" fn monitor_enum_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _place: *mut RECT,
    data: LPARAM,
) -> BOOL {
    let monitors = data.0 as *mut SmallVec<[HMONITOR; 4]>;
    unsafe { (*monitors).push(hmonitor) };
    BOOL(1)
}

fn get_monitor_info(hmonitor: HMONITOR) -> anyhow::Result<MONITORINFOEXW> {
    let mut monitor_info = MONITORINFOEXW::default();
    monitor_info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    let status = unsafe {
        GetMonitorInfoW(
            hmonitor,
            &mut monitor_info as *mut MONITORINFOEXW as *mut MONITORINFO,
        )
    };
    if status.as_bool() {
        Ok(monitor_info)
    } else {
        Err(anyhow::anyhow!(std::io::Error::last_os_error()))
    }
}

fn monitor_provenance_uuid(info: &MONITORINFOEXW) -> anyhow::Result<Uuid> {
    let mut device = DISPLAY_DEVICEW::default();
    device.cb = std::mem::size_of::<DISPLAY_DEVICEW>() as u32;
    let status = unsafe {
        EnumDisplayDevicesW(
            PCWSTR(info.szDevice.as_ptr()),
            0,
            &mut device,
            EDD_GET_DEVICE_INTERFACE_NAME,
        )
    };
    anyhow::ensure!(
        status.as_bool(),
        "cannot resolve stable display provenance for {:?}: {}",
        String::from_utf16_lossy(trim_wide(&info.szDevice)),
        std::io::Error::last_os_error()
    );
    let provenance = trim_wide(&device.DeviceID);
    anyhow::ensure!(
        !provenance.is_empty(),
        "Windows returned an empty display device interface"
    );
    let mut name = SmallVec::<[u8; 256]>::with_capacity(provenance.len() * 2);
    for character in provenance {
        name.extend_from_slice(&character.to_be_bytes());
    }
    Ok(Uuid::new_v5(&Uuid::NAMESPACE_OID, &name))
}

fn trim_wide(value: &[u16]) -> &[u16] {
    let length = value
        .iter()
        .position(|character| *character == 0)
        .unwrap_or(value.len());
    &value[..length]
}

fn checked_size(rect: RECT) -> Option<open_gpui::Size<DevicePixels>> {
    let width = rect.right.checked_sub(rect.left)?;
    let height = rect.bottom.checked_sub(rect.top)?;
    if width <= 0 || height <= 0 {
        return None;
    }
    Some(size(DevicePixels(width), DevicePixels(height)))
}

fn display_id_from_uuid(uuid: Uuid) -> DisplayId {
    DisplayId::new(uuid.as_u64_pair().0)
}

fn get_scale_factor_for_monitor(monitor: HMONITOR) -> anyhow::Result<f32> {
    let mut dpi_x = 0;
    let mut dpi_y = 0;
    unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) }?;
    anyhow::ensure!(dpi_x == dpi_y, "monitor DPI axes disagree");
    Ok(dpi_x as f32 / USER_DEFAULT_SCREEN_DPI as f32)
}

fn native_collection_failure(error: impl fmt::Display) -> WindowsDisplayTopologyFailure {
    WindowsDisplayTopologyFailure::NativeCollection(error.to_string().into())
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;

    use super::{
        WindowsDisplay, WindowsDisplayTopologyAuthority, WindowsDisplayTopologyCandidate,
        WindowsDisplayTopologyFailure, WindowsDisplayTopologyRefresh, WindowsNativeDisplayRow,
    };
    use open_gpui::{Bounds, DevicePixels, DisplayId, point, px, size};
    use smallvec::smallvec;
    use uuid::Uuid;
    use windows::Win32::Graphics::Gdi::HMONITOR;

    fn display(
        display_id: u64,
        origin_x: i32,
        work_width: i32,
        scale_factor: f32,
        provenance: u128,
    ) -> WindowsDisplay {
        WindowsDisplay {
            display_id: DisplayId::new(display_id),
            scale_factor,
            bounds: Bounds::new(
                point(px(origin_x as f32 / scale_factor), px(0.0)),
                size(px(1_920.0 / scale_factor), px(1_080.0 / scale_factor)),
            ),
            visible_bounds: Bounds::new(
                point(px(origin_x as f32 / scale_factor), px(0.0)),
                size(
                    px(work_width as f32 / scale_factor),
                    px(1_040.0 / scale_factor),
                ),
            ),
            physical_bounds: Bounds::new(
                point(DevicePixels(origin_x), DevicePixels(0)),
                size(DevicePixels(1_920), DevicePixels(1_080)),
            ),
            physical_visible_bounds: Bounds::new(
                point(DevicePixels(origin_x), DevicePixels(0)),
                size(DevicePixels(work_width), DevicePixels(1_040)),
            ),
            uuid: Uuid::from_u128(provenance),
        }
    }

    fn row(handle: usize, display: WindowsDisplay, is_primary: bool) -> WindowsNativeDisplayRow {
        WindowsNativeDisplayRow {
            handle: HMONITOR(handle as _),
            display,
            is_primary,
        }
    }

    fn candidate(
        primary: WindowsDisplay,
        secondary: WindowsDisplay,
    ) -> WindowsDisplayTopologyCandidate {
        WindowsDisplayTopologyCandidate::try_new(smallvec![
            row(2, secondary, false),
            row(1, primary, true),
        ])
        .expect("the complete synthetic topology should be valid")
    }

    fn refresh(
        authority: &mut WindowsDisplayTopologyAuthority,
        candidate: Result<WindowsDisplayTopologyCandidate, WindowsDisplayTopologyFailure>,
    ) -> WindowsDisplayTopologyRefresh {
        let request = authority.request_refresh();
        assert!(request.should_post_message);
        assert_eq!(
            authority.begin_scheduled_refresh(),
            Some(request.request_epoch)
        );
        authority.finish_refresh(request.request_epoch, candidate)
    }

    #[test]
    fn identical_display_broadcasts_do_not_advance_the_publication_generation() {
        let primary = display(1, 0, 1_920, 1.0, 1);
        let secondary = display(2, -1_920, 1_920, 1.5, 2);
        let initial = candidate(primary, secondary);
        let mut authority = WindowsDisplayTopologyAuthority::new(initial.clone());

        assert_eq!(
            refresh(&mut authority, Ok(initial)),
            WindowsDisplayTopologyRefresh::Unchanged { generation: 1 }
        );
        assert_eq!(authority.retained_snapshot().generation(), 1);
    }

    #[test]
    fn native_enumeration_order_does_not_change_candidate_identity() {
        let primary = display(1, 0, 1_920, 1.0, 1);
        let secondary = display(2, -1_920, 1_920, 1.5, 2);
        let forward = WindowsDisplayTopologyCandidate::try_new(smallvec![
            row(1, primary, true),
            row(2, secondary, false),
        ])
        .unwrap();
        let reverse = WindowsDisplayTopologyCandidate::try_new(smallvec![
            row(2, secondary, false),
            row(1, primary, true),
        ])
        .unwrap();

        assert_eq!(forward, reverse);
    }

    #[test]
    fn failed_display_refresh_retains_the_last_complete_publication() {
        let primary = display(1, 0, 1_920, 1.0, 1);
        let secondary = display(2, -1_920, 1_920, 1.5, 2);
        let mut authority = WindowsDisplayTopologyAuthority::new(candidate(primary, secondary));
        let before = authority.retained_snapshot();

        assert!(matches!(
            refresh(
                &mut authority,
                Err(WindowsDisplayTopologyFailure::UnstableDuringCollection)
            ),
            WindowsDisplayTopologyRefresh::RetainedAfterFailure { generation: 1, .. }
        ));
        assert_eq!(authority.retained_snapshot(), before);
        assert!(authority.exact_snapshot().is_err());
    }

    #[test]
    fn one_changed_display_fact_publishes_exactly_one_new_generation() {
        let primary = display(1, 0, 1_920, 1.0, 1);
        let secondary = display(2, -1_920, 1_920, 1.5, 2);
        let mut authority = WindowsDisplayTopologyAuthority::new(candidate(primary, secondary));
        let changed_secondary = display(2, -1_920, 1_840, 1.5, 2);
        let changed = candidate(primary, changed_secondary);

        assert_eq!(
            refresh(&mut authority, Ok(changed.clone())),
            WindowsDisplayTopologyRefresh::Published {
                previous_generation: 1,
                generation: 2,
            }
        );
        assert_eq!(
            refresh(&mut authority, Ok(changed)),
            WindowsDisplayTopologyRefresh::Unchanged { generation: 2 }
        );
        let snapshot = authority.retained_snapshot();
        assert_eq!(snapshot.generation(), 2);
        assert_eq!(
            snapshot
                .display(DisplayId::new(2))
                .expect("the secondary display should remain published")
                .physical_visible_bounds,
            changed_secondary.physical_visible_bounds
        );
    }

    #[test]
    fn candidate_rejects_duplicate_identity_and_ambiguous_primary() {
        let primary = display(1, 0, 1_920, 1.0, 1);
        let replacement = display(1, 1_920, 1_920, 1.0, 2);

        assert!(
            WindowsDisplayTopologyCandidate::try_new(smallvec![
                row(1, primary, true),
                row(2, replacement, false),
            ])
            .is_err()
        );
        assert!(
            WindowsDisplayTopologyCandidate::try_new(smallvec![
                row(1, primary, true),
                row(2, display(2, 1_920, 1_920, 1.0, 2), true),
            ])
            .is_err()
        );
        assert!(
            WindowsDisplayTopologyCandidate::try_new(smallvec![
                row(1, primary, false),
                row(2, display(2, 1_920, 1_920, 1.0, 2), false),
            ])
            .is_err()
        );
        assert!(
            WindowsDisplayTopologyCandidate::try_new(smallvec![
                row(1, primary, true),
                row(1, display(2, 1_920, 1_920, 1.0, 2), false),
            ])
            .is_err()
        );
        assert!(
            WindowsDisplayTopologyCandidate::try_new(smallvec![
                row(1, primary, true),
                row(2, display(2, 1_920, 1_920, 1.0, 1), false),
            ])
            .is_err()
        );
    }

    #[test]
    fn snapshot_projects_primary_and_physical_facts_from_one_generation() {
        let primary = display(1, 0, 1_920, 1.0, 1);
        let secondary = display(2, -1_920, 1_920, 1.5, 2);
        let authority = WindowsDisplayTopologyAuthority::new(candidate(primary, secondary));
        let snapshot = authority.retained_snapshot();

        assert_eq!(snapshot.primary_display(), primary);
        let observation = snapshot
            .physical_observation(secondary.display_id)
            .expect("the secondary physical observation should be published");
        assert_eq!(observation.topology_generation(), snapshot.generation());
        assert_eq!(observation.display_id(), secondary.display_id);
        assert_eq!(observation.bounds(), secondary.physical_bounds());
        assert_eq!(
            snapshot
                .physical_observation_at(point(DevicePixels(-1), DevicePixels(100)))
                .expect("the negative-coordinate display should own the point"),
            observation
        );
        assert_eq!(
            snapshot
                .physical_observation_at(point(DevicePixels(0), DevicePixels(100)))
                .expect("the half-open boundary should belong to the primary display")
                .display_id(),
            primary.display_id
        );

        let first_projection = snapshot.platform_snapshot().displays();
        let second_projection = snapshot.platform_snapshot().displays();
        assert_eq!(first_projection.len(), second_projection.len());
        assert!(
            first_projection
                .iter()
                .zip(&second_projection)
                .all(|(first, second)| Rc::ptr_eq(first, second))
        );
    }

    #[test]
    fn native_handle_reuse_cannot_alias_old_display_identity() {
        let primary = display(1, 0, 1_920, 1.0, 1);
        let old_secondary = display(2, -1_920, 1_920, 1.5, 2);
        let replacement = display(3, -1_920, 1_920, 1.5, 3);
        let old = WindowsDisplayTopologyCandidate::try_new(smallvec![
            row(1, primary, true),
            row(2, old_secondary, false),
        ])
        .unwrap();
        let new = WindowsDisplayTopologyCandidate::try_new(smallvec![
            row(1, primary, true),
            row(2, replacement, false),
        ])
        .unwrap();
        let mut authority = WindowsDisplayTopologyAuthority::new(old);

        assert!(matches!(
            refresh(&mut authority, Ok(new)),
            WindowsDisplayTopologyRefresh::Published { generation: 2, .. }
        ));
        let snapshot = authority.retained_snapshot();
        assert!(snapshot.display(old_secondary.display_id).is_none());
        assert_eq!(snapshot.display(replacement.display_id), Some(replacement));
    }

    #[test]
    fn a_refresh_requested_during_collection_supersedes_the_old_candidate() {
        let primary = display(1, 0, 1_920, 1.0, 1);
        let secondary = display(2, -1_920, 1_920, 1.5, 2);
        let initial = candidate(primary, secondary);
        let mut authority = WindowsDisplayTopologyAuthority::new(initial.clone());
        let first = authority.request_refresh();
        assert_eq!(
            authority.begin_scheduled_refresh(),
            Some(first.request_epoch)
        );
        let second = authority.request_refresh();

        assert_eq!(
            authority.finish_refresh(first.request_epoch, Ok(initial)),
            WindowsDisplayTopologyRefresh::Superseded {
                generation: 1,
                request_epoch: second.request_epoch,
            }
        );
        assert_eq!(
            authority.begin_scheduled_refresh(),
            Some(second.request_epoch)
        );
    }

    #[test]
    fn repeated_window_broadcasts_coalesce_into_one_scheduled_refresh() {
        let primary = display(1, 0, 1_920, 1.0, 1);
        let secondary = display(2, -1_920, 1_920, 1.5, 2);
        let initial = candidate(primary, secondary);
        let mut authority = WindowsDisplayTopologyAuthority::new(initial.clone());

        let first = authority.request_refresh();
        let second = authority.request_refresh();
        let third = authority.request_refresh();
        assert!(first.should_post_message);
        assert!(!second.should_post_message);
        assert!(!third.should_post_message);
        assert!(authority.exact_snapshot().is_err());
        assert_eq!(
            authority.begin_scheduled_refresh(),
            Some(third.request_epoch)
        );
        assert_eq!(
            authority.finish_refresh(third.request_epoch, Ok(initial)),
            WindowsDisplayTopologyRefresh::Unchanged { generation: 1 }
        );
        assert!(authority.exact_snapshot().is_ok());
    }

    #[test]
    fn generation_exhaustion_retains_the_last_complete_publication() {
        let primary = display(1, 0, 1_920, 1.0, 1);
        let secondary = display(2, -1_920, 1_920, 1.5, 2);
        let mut authority = WindowsDisplayTopologyAuthority::new(candidate(primary, secondary));
        authority.current.generation = u64::MAX;
        let before = authority.retained_snapshot();
        let changed_secondary = display(2, -1_920, 1_840, 1.5, 2);

        assert!(matches!(
            refresh(&mut authority, Ok(candidate(primary, changed_secondary))),
            WindowsDisplayTopologyRefresh::RetainedAfterFailure {
                generation: u64::MAX,
                failure: WindowsDisplayTopologyFailure::GenerationExhausted,
            }
        ));
        assert_eq!(authority.retained_snapshot(), before);
        assert!(authority.exact_snapshot().is_err());
    }
}
