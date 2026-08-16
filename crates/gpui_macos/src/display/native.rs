use super::{
    MacDisplay, MacDisplayTopologyCandidate, MacDisplayTopologyFailure, display_id_from_uuid,
};
use crate::ns_string;
use cocoa::{
    appkit::NSScreen,
    base::{id, nil},
    foundation::{NSArray, NSDictionary, NSRect},
};
use core_foundation::base::CFRelease;
use core_foundation::uuid::{CFUUIDGetUUIDBytes, CFUUIDRef};
use core_graphics::{
    display::{CGDirectDisplayID, CGDisplay, CGDisplayBounds, CGDisplayIsMain},
    geometry::CGRect,
};
use objc::{msg_send, sel, sel_impl};
use open_gpui::{Bounds, Pixels, point, px, size};
use smallvec::SmallVec;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Copy, Debug)]
pub(super) struct MacNativeDisplayRow {
    pub(super) screen: id,
    pub(super) native_display_id: CGDirectDisplayID,
    pub(super) display: MacDisplay,
    pub(super) is_primary: bool,
}

impl MacNativeDisplayRow {
    pub(super) unsafe fn observe(screen: id) -> Result<Self, MacDisplayTopologyFailure> {
        if screen == nil {
            return Err(native_collection_failure("AppKit returned a null NSScreen"));
        }

        let native_display_id = unsafe { native_display_id_for_screen(screen)? };
        if native_display_id == 0 {
            return Err(native_collection_failure(
                "AppKit returned the null CoreGraphics display identity",
            ));
        }
        let is_primary = unsafe { CGDisplayIsMain(native_display_id) != 0 };

        let uuid = native_display_uuid(native_display_id)?;
        let display_id = display_id_from_uuid(uuid);
        let native_bounds = unsafe { CGDisplayBounds(native_display_id) };
        let bounds = checked_bounds(native_bounds).ok_or_else(|| {
            native_collection_failure("CoreGraphics returned invalid display bounds")
        })?;
        let screen_frame = unsafe { NSScreen::frame(screen) };
        let visible_frame = unsafe { NSScreen::visibleFrame(screen) };
        validate_appkit_frames(screen_frame, visible_frame, bounds)?;
        let visible_bounds = visible_bounds_from_appkit_frames(bounds, screen_frame, visible_frame);
        let scale_factor = unsafe { NSScreen::backingScaleFactor(screen) as f32 };
        if !scale_factor.is_finite() || scale_factor <= 0.0 {
            return Err(native_collection_failure(
                "AppKit returned an invalid display backing scale factor",
            ));
        }

        Ok(Self {
            screen,
            native_display_id,
            display: MacDisplay {
                display_id,
                uuid,
                scale_factor,
                bounds,
                visible_bounds,
            },
            is_primary,
        })
    }
}

#[derive(Clone, Debug)]
pub(super) struct MacNativeDisplayBatch {
    pub(super) rows: SmallVec<[MacNativeDisplayRow; 4]>,
    pub(super) candidate: MacDisplayTopologyCandidate,
}

impl MacNativeDisplayBatch {
    pub(super) fn stable_from_native() -> Result<Self, MacDisplayTopologyFailure> {
        let first = Self::collect_once()?;
        let second = Self::collect_once()?;
        if first.candidate != second.candidate || !first.has_same_native_mapping(&second) {
            return Err(MacDisplayTopologyFailure::UnstableDuringCollection);
        }
        Ok(second)
    }

    fn collect_once() -> Result<Self, MacDisplayTopologyFailure> {
        let mut active_display_ids = CGDisplay::active_displays().map_err(|error| {
            native_collection_failure(format!(
                "CoreGraphics could not enumerate active displays ({error})"
            ))
        })?;
        if active_display_ids.is_empty() {
            return Err(native_collection_failure(
                "CoreGraphics returned no active displays",
            ));
        }
        active_display_ids.sort_unstable();
        if active_display_ids.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(native_collection_failure(
                "CoreGraphics returned duplicate active displays",
            ));
        }

        let screens = unsafe { NSScreen::screens(nil) };
        if screens == nil {
            return Err(native_collection_failure(
                "AppKit returned a null screen collection",
            ));
        }
        let screen_count = unsafe { NSArray::count(screens) };
        if screen_count as usize != active_display_ids.len() {
            return Err(native_collection_failure(
                "AppKit and CoreGraphics returned incomplete display sets",
            ));
        }

        let mut rows = SmallVec::<[MacNativeDisplayRow; 4]>::with_capacity(screen_count as usize);
        for index in 0..screen_count {
            let screen = unsafe { NSArray::objectAtIndex(screens, index) };
            let row = unsafe { MacNativeDisplayRow::observe(screen)? };
            if (index == 0) != row.is_primary {
                return Err(native_collection_failure(
                    "AppKit and CoreGraphics disagree about the primary display",
                ));
            }
            rows.push(row);
        }

        let mut appkit_display_ids = rows
            .iter()
            .map(|row| row.native_display_id)
            .collect::<SmallVec<[_; 4]>>();
        appkit_display_ids.sort_unstable();
        if appkit_display_ids.as_slice() != active_display_ids.as_slice() {
            return Err(native_collection_failure(
                "AppKit and CoreGraphics returned different active display sets",
            ));
        }

        let candidate = MacDisplayTopologyCandidate::try_new(rows.clone())?;
        rows.sort_unstable_by_key(|row| u64::from(row.display.display_id));
        Ok(Self { rows, candidate })
    }

    fn has_same_native_mapping(&self, other: &Self) -> bool {
        self.rows.len() == other.rows.len()
            && self.rows.iter().zip(&other.rows).all(|(left, right)| {
                left.display.display_id == right.display.display_id
                    && left.native_display_id == right.native_display_id
            })
    }

    pub(super) fn row(&self, display_id: open_gpui::DisplayId) -> Option<MacNativeDisplayRow> {
        self.rows
            .iter()
            .copied()
            .find(|row| row.display.display_id == display_id)
    }
}

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn CGDisplayCreateUUIDFromDisplayID(display: CGDirectDisplayID) -> CFUUIDRef;
}

fn native_display_uuid(
    native_display_id: CGDirectDisplayID,
) -> Result<Uuid, MacDisplayTopologyFailure> {
    let cfuuid = unsafe { CGDisplayCreateUUIDFromDisplayID(native_display_id) };
    if cfuuid.is_null() {
        return Err(native_collection_failure(
            "CoreGraphics returned a null display UUID",
        ));
    }

    let bytes = unsafe { CFUUIDGetUUIDBytes(cfuuid) };
    unsafe { CFRelease(cfuuid as _) };
    Ok(Uuid::from_bytes([
        bytes.byte0,
        bytes.byte1,
        bytes.byte2,
        bytes.byte3,
        bytes.byte4,
        bytes.byte5,
        bytes.byte6,
        bytes.byte7,
        bytes.byte8,
        bytes.byte9,
        bytes.byte10,
        bytes.byte11,
        bytes.byte12,
        bytes.byte13,
        bytes.byte14,
        bytes.byte15,
    ]))
}

unsafe fn native_display_id_for_screen(
    screen: id,
) -> Result<CGDirectDisplayID, MacDisplayTopologyFailure> {
    let device_description = unsafe { NSScreen::deviceDescription(screen) };
    if device_description == nil {
        return Err(native_collection_failure(
            "AppKit returned no device description for NSScreen",
        ));
    }
    let screen_number_key = unsafe { ns_string("NSScreenNumber") };
    let screen_number = unsafe { device_description.objectForKey_(screen_number_key) };
    if screen_number == nil {
        return Err(native_collection_failure(
            "NSScreen device description has no CoreGraphics display identity",
        ));
    }
    Ok(unsafe { msg_send![screen_number, unsignedIntegerValue] })
}

fn checked_bounds(rect: CGRect) -> Option<Bounds<Pixels>> {
    let values = [
        rect.origin.x,
        rect.origin.y,
        rect.size.width,
        rect.size.height,
    ];
    if values.iter().any(|value| !value.is_finite())
        || rect.size.width <= 0.0
        || rect.size.height <= 0.0
    {
        return None;
    }
    let values = values.map(|value| value as f32);
    if values.iter().any(|value| !value.is_finite()) {
        return None;
    }
    Some(Bounds::new(
        point(px(values[0]), px(values[1])),
        size(px(values[2]), px(values[3])),
    ))
}

pub(super) fn display_facts_are_coherent(display: MacDisplay) -> bool {
    let bounds = display.bounds;
    let visible = display.visible_bounds;
    let values = [
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height),
        f32::from(visible.origin.x),
        f32::from(visible.origin.y),
        f32::from(visible.size.width),
        f32::from(visible.size.height),
        display.scale_factor,
    ];
    if values.iter().any(|value| !value.is_finite())
        || bounds.size.width <= px(0.0)
        || bounds.size.height <= px(0.0)
        || visible.size.width <= px(0.0)
        || visible.size.height <= px(0.0)
        || display.scale_factor <= 0.0
        || display.display_id != display_id_from_uuid(display.uuid)
    {
        return false;
    }

    let bounds_max_x = bounds.origin.x + bounds.size.width;
    let bounds_max_y = bounds.origin.y + bounds.size.height;
    let visible_max_x = visible.origin.x + visible.size.width;
    let visible_max_y = visible.origin.y + visible.size.height;
    visible.origin.x >= bounds.origin.x
        && visible.origin.y >= bounds.origin.y
        && visible_max_x <= bounds_max_x
        && visible_max_y <= bounds_max_y
}

fn validate_appkit_frames(
    screen_frame: NSRect,
    visible_frame: NSRect,
    display_bounds: Bounds<Pixels>,
) -> Result<(), MacDisplayTopologyFailure> {
    let screen_values = [
        screen_frame.origin.x,
        screen_frame.origin.y,
        screen_frame.size.width,
        screen_frame.size.height,
    ];
    let visible_values = [
        visible_frame.origin.x,
        visible_frame.origin.y,
        visible_frame.size.width,
        visible_frame.size.height,
    ];
    if screen_values
        .iter()
        .chain(&visible_values)
        .any(|value| !value.is_finite())
        || screen_frame.size.width <= 0.0
        || screen_frame.size.height <= 0.0
        || visible_frame.size.width <= 0.0
        || visible_frame.size.height <= 0.0
    {
        return Err(native_collection_failure(
            "AppKit returned invalid display frame geometry",
        ));
    }
    if screen_frame.size.width as f32 != f32::from(display_bounds.size.width)
        || screen_frame.size.height as f32 != f32::from(display_bounds.size.height)
    {
        return Err(native_collection_failure(
            "AppKit and CoreGraphics disagree about display size",
        ));
    }
    let visible_max_x = visible_frame.origin.x + visible_frame.size.width;
    let visible_max_y = visible_frame.origin.y + visible_frame.size.height;
    let screen_max_x = screen_frame.origin.x + screen_frame.size.width;
    let screen_max_y = screen_frame.origin.y + screen_frame.size.height;
    if visible_frame.origin.x < screen_frame.origin.x
        || visible_frame.origin.y < screen_frame.origin.y
        || visible_max_x > screen_max_x
        || visible_max_y > screen_max_y
    {
        return Err(native_collection_failure(
            "AppKit visible frame lies outside its display frame",
        ));
    }
    Ok(())
}

fn visible_bounds_from_appkit_frames(
    display_bounds: Bounds<Pixels>,
    screen_frame: NSRect,
    visible_frame: NSRect,
) -> Bounds<Pixels> {
    let relative_x = visible_frame.origin.x - screen_frame.origin.x;
    let relative_y = screen_frame.origin.y + screen_frame.size.height
        - visible_frame.origin.y
        - visible_frame.size.height;
    Bounds {
        origin: point(
            display_bounds.origin.x + px(relative_x as f32),
            display_bounds.origin.y + px(relative_y as f32),
        ),
        size: size(
            px(visible_frame.size.width as f32),
            px(visible_frame.size.height as f32),
        ),
    }
}

fn native_collection_failure(message: impl Into<Arc<str>>) -> MacDisplayTopologyFailure {
    MacDisplayTopologyFailure::NativeCollection(message.into())
}

#[cfg(test)]
mod tests {
    use super::visible_bounds_from_appkit_frames;
    use cocoa::foundation::{NSPoint, NSRect, NSSize};
    use open_gpui::{Bounds, point, px, size};

    #[test]
    fn visible_bounds_include_the_global_vertical_display_origin() {
        let display_bounds = Bounds::new(
            point(px(-1_920.0), px(-1_080.0)),
            size(px(1_920.0), px(1_080.0)),
        );
        let screen_frame = NSRect::new(
            NSPoint::new(-1_920.0, 1_080.0),
            NSSize::new(1_920.0, 1_080.0),
        );
        let visible_frame = NSRect::new(
            NSPoint::new(-1_920.0, 1_120.0),
            NSSize::new(1_920.0, 1_040.0),
        );

        assert_eq!(
            visible_bounds_from_appkit_frames(display_bounds, screen_frame, visible_frame),
            Bounds::new(
                point(px(-1_920.0), px(-1_080.0)),
                size(px(1_920.0), px(1_040.0)),
            )
        );
    }
}
