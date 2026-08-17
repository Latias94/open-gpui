use crate::{
    BoolExt, DisplayLink, DisplayLinkError, MacDisplay, MacDisplayTopologyHandle,
    MacDisplayTopologySnapshot, MacDisplayTopologySubscription, NSRange, NSStringExt,
    TISCopyCurrentKeyboardInputSource, TISGetInputSourceProperty, ValidatedMacDisplayTarget,
    events::platform_input_from_native, kTISPropertyInputSourceIsASCIICapable,
    kTISPropertyInputSourceType, kTISTypeKeyboardInputMode, ns_string, renderer,
};
use anyhow::{Result, anyhow};
use block2::RcBlock;
use cocoa::{
    appkit::{
        NSAppKitVersionNumber, NSAppKitVersionNumber12_0, NSApplication, NSBackingStoreBuffered,
        NSColor, NSEvent, NSEventModifierFlags, NSFilenamesPboardType, NSPasteboard, NSScreen,
        NSView, NSViewHeightSizable, NSViewWidthSizable, NSVisualEffectMaterial,
        NSVisualEffectState, NSVisualEffectView, NSWindow, NSWindowButton,
        NSWindowCollectionBehavior, NSWindowOcclusionState, NSWindowOrderingMode,
        NSWindowStyleMask, NSWindowTabbingMode, NSWindowTitleVisibility,
    },
    base::{id, nil},
    foundation::{
        NSArray, NSAutoreleasePool, NSFastEnumeration, NSInteger, NSNotFound,
        NSOperatingSystemVersion, NSPoint, NSProcessInfo, NSRect, NSSize, NSString, NSUInteger,
        NSUserDefaults,
    },
};
#[cfg(any(test, feature = "test-support"))]
use image::RgbaImage;
use open_gpui::{
    AnyWindowHandle, BackgroundExecutor, Bounds, Capslock, CursorStyle, ExternalPaths,
    FileDropEvent, ForegroundExecutor, KeyDownEvent, Keystroke, Modifiers, ModifiersChangedEvent,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, PlatformAtlas,
    PlatformDisplay, PlatformInput, PlatformInputCallback, PlatformInputCallbackSlot,
    PlatformInputHandler, PlatformInputHandlerSlot, PlatformNativeWindowRetirementOutcome,
    PlatformPresentationShutdownOutcome, PlatformWindow, PlatformWindowActiveStatusObservation,
    PlatformWindowCommand, PlatformWindowCommandDispatcher, PlatformWindowCommandOutcome,
    PlatformWindowInteractionQuiescence, PlatformWindowPresentOutcome, Point,
    PreparedPlatformPresentationShutdown, PromptButton, PromptLevel, RequestFrameOptions,
    SharedString, Size, SystemWindowTab, WindowActivationPolicy, WindowAppearance,
    WindowBackgroundAppearance, WindowBounds, WindowControlArea, WindowCreationFacts, WindowId,
    WindowKind, WindowParams, WindowPresentationShutdownTicket, point, px, size,
};

use core_foundation::base::{CFRelease, CFTypeRef};
use core_foundation_sys::base::CFEqual;
use core_foundation_sys::number::{CFBooleanGetValue, CFBooleanRef};
use core_graphics::display::{CGDirectDisplayID, CGPoint, CGRect};
use ctor::ctor;
use dispatch2::{DispatchQueue, DispatchTime};
use futures::channel::oneshot;
use objc::{
    class,
    declare::ClassDecl,
    msg_send,
    rc::StrongPtr,
    runtime::{BOOL, Class, NO, Object, Protocol, Sel, YES},
    sel, sel_impl,
};
use objc2_app_kit::NSBeep;
use parking_lot::Mutex;
use raw_window_handle as rwh;
use smallvec::SmallVec;
use std::{
    cell::Cell,
    collections::{HashMap, VecDeque},
    ffi::{CStr, c_void},
    mem,
    ops::Range,
    path::PathBuf,
    ptr::{self, NonNull},
    rc::Rc,
    sync::{
        Arc, Weak,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

const WINDOW_STATE_IVAR: &str = "windowState";

static mut WINDOW_CLASS: *const Class = ptr::null();
static mut PANEL_CLASS: *const Class = ptr::null();
static mut VIEW_CLASS: *const Class = ptr::null();
static mut BLURRED_VIEW_CLASS: *const Class = ptr::null();
static NEXT_EMERGENCY_PRESENTATION_SHUTDOWN_GENERATION: AtomicU64 = AtomicU64::new(1);

#[allow(non_upper_case_globals)]
const NSWindowStyleMaskNonactivatingPanel: NSWindowStyleMask =
    NSWindowStyleMask::from_bits_retain(1 << 7);
// WindowLevel const value ref: https://docs.rs/core-graphics2/0.4.1/src/core_graphics2/window_level.rs.html
#[allow(non_upper_case_globals)]
const NSNormalWindowLevel: NSInteger = 0;
#[allow(non_upper_case_globals)]
const NSFloatingWindowLevel: NSInteger = 3;
#[allow(non_upper_case_globals)]
const NSPopUpWindowLevel: NSInteger = 101;
#[allow(non_upper_case_globals)]
const NSTrackingMouseEnteredAndExited: NSUInteger = 0x01;
#[allow(non_upper_case_globals)]
const NSTrackingMouseMoved: NSUInteger = 0x02;
#[allow(non_upper_case_globals)]
const NSTrackingActiveAlways: NSUInteger = 0x80;
#[allow(non_upper_case_globals)]
const NSTrackingInVisibleRect: NSUInteger = 0x200;
#[allow(non_upper_case_globals)]
const NSWindowAnimationBehaviorUtilityWindow: NSInteger = 4;
#[allow(non_upper_case_globals)]
const NSViewLayerContentsRedrawDuringViewResize: NSInteger = 2;
// https://developer.apple.com/documentation/appkit/nsdragoperation
type NSDragOperation = NSUInteger;
#[allow(non_upper_case_globals)]
const NSDragOperationNone: NSDragOperation = 0;
#[allow(non_upper_case_globals)]
const NSDragOperationCopy: NSDragOperation = 1;
#[derive(PartialEq)]
pub enum UserTabbingPreference {
    Never,
    Always,
    InFullScreen,
}

#[derive(Clone, Default)]
pub(crate) struct MacWindowRegistry(Arc<Mutex<MacWindowRegistryState>>);

#[derive(Default)]
struct MacWindowRegistryState {
    windows: HashMap<WindowId, Weak<RegisteredMacWindow>>,
}

struct RegisteredMacWindow {
    handle: AnyWindowHandle,
    native_window: StrongPtr,
    closed: Arc<AtomicBool>,
    interaction_quiesced: Arc<AtomicBool>,
}

struct MacWindowRegistration {
    registry: Weak<Mutex<MacWindowRegistryState>>,
    entry: Arc<RegisteredMacWindow>,
}

pub(crate) struct MacTransientOwnerLease {
    registry: Weak<Mutex<MacWindowRegistryState>>,
    entry: Arc<RegisteredMacWindow>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MacTransientOwnerLeaseState {
    Current,
    Retired,
}

enum MacTransientOwnerResolution {
    Current(MacTransientOwnerLease),
    Retired(MacTransientOwnerLease),
    Gone,
    Busy,
}

impl MacWindowRegistry {
    fn observe_transient_owner(&self, owner: AnyWindowHandle) -> MacTransientOwnerResolution {
        let entry = {
            let Some(mut registry) = self.0.try_lock() else {
                return MacTransientOwnerResolution::Busy;
            };
            registry
                .windows
                .retain(|_, entry| entry.upgrade().is_some());
            let Some(entry) = registry
                .windows
                .get(&owner.window_id())
                .and_then(Weak::upgrade)
            else {
                return MacTransientOwnerResolution::Gone;
            };
            entry
        };
        if entry.handle != owner {
            return MacTransientOwnerResolution::Gone;
        }
        let lease = MacTransientOwnerLease {
            registry: Arc::downgrade(&self.0),
            entry,
        };
        if lease.entry.closed.load(Ordering::Acquire)
            || lease.entry.interaction_quiesced.load(Ordering::Acquire)
        {
            return MacTransientOwnerResolution::Retired(lease);
        }
        match lease.state() {
            MacTransientOwnerLeaseState::Current => MacTransientOwnerResolution::Current(lease),
            MacTransientOwnerLeaseState::Retired => MacTransientOwnerResolution::Retired(lease),
        }
    }

    pub(crate) fn resolve_transient_owner(
        &self,
        owner: AnyWindowHandle,
    ) -> Result<MacTransientOwnerLease> {
        let lease = match self.observe_transient_owner(owner) {
            MacTransientOwnerResolution::Current(lease) => lease,
            MacTransientOwnerResolution::Gone => {
                return Err(anyhow!(
                    "transient owner is not a live macOS platform window"
                ));
            }
            MacTransientOwnerResolution::Retired(_) => {
                return Err(anyhow!("macOS transient owner is already retiring"));
            }
            MacTransientOwnerResolution::Busy => {
                return Err(anyhow!(
                    "macOS window registry is busy while resolving a transient owner"
                ));
            }
        };
        anyhow::ensure!(
            !lease.entry.interaction_quiesced.load(Ordering::Acquire),
            "macOS transient owner is already quiescing"
        );
        Ok(lease)
    }

    fn register(
        &self,
        window: &MacWindow,
        transient_owner: Option<&MacTransientOwnerLease>,
    ) -> Result<MacWindowRegistration> {
        let entry = {
            let window_state = window
                .0
                .try_lock()
                .ok_or_else(|| anyhow!("new macOS window state is busy before registry commit"))?;
            anyhow::ensure!(
                !window_state.is_closed(),
                "cannot register a closed macOS platform window"
            );
            Arc::new(RegisteredMacWindow {
                handle: window_state.handle,
                native_window: unsafe { StrongPtr::retain(window_state.native_window) },
                closed: window_state.closed.clone(),
                interaction_quiesced: window_state.interaction_quiesced.clone(),
            })
        };
        let mut registry = self
            .0
            .try_lock()
            .ok_or_else(|| anyhow!("macOS window registry is busy during window commit"))?;
        registry
            .windows
            .retain(|_, entry| entry.upgrade().is_some());
        if let Some(owner) = transient_owner {
            anyhow::ensure!(
                owner
                    .registry
                    .upgrade()
                    .is_some_and(|owner_registry| Arc::ptr_eq(&owner_registry, &self.0))
                    && registry
                        .windows
                        .get(&owner.entry.handle.window_id())
                        .and_then(Weak::upgrade)
                        .is_some_and(|current| Arc::ptr_eq(&current, &owner.entry))
                    && !owner.entry.closed.load(Ordering::Acquire)
                    && !owner.entry.interaction_quiesced.load(Ordering::Acquire),
                "macOS transient owner changed before the logical relationship was committed"
            );
        }
        if let Some(existing) = registry.windows.get(&entry.handle.window_id()) {
            anyhow::ensure!(
                existing.upgrade().is_none(),
                "macOS window registry already contains this live window generation"
            );
        }
        registry
            .windows
            .insert(entry.handle.window_id(), Arc::downgrade(&entry));
        if let Some(owner) = transient_owner {
            if owner.entry.closed.load(Ordering::Acquire)
                || owner.entry.interaction_quiesced.load(Ordering::Acquire)
            {
                registry.windows.remove(&entry.handle.window_id());
                return Err(anyhow!(
                    "macOS transient owner retired during logical relationship commit"
                ));
            }
        }
        Ok(MacWindowRegistration {
            registry: Arc::downgrade(&self.0),
            entry,
        })
    }
}

impl Drop for MacWindowRegistration {
    fn drop(&mut self) {
        let Some(registry) = self.registry.upgrade() else {
            return;
        };
        let mut registry = registry.lock();
        if registry
            .windows
            .get(&self.entry.handle.window_id())
            .and_then(Weak::upgrade)
            .is_some_and(|entry| Arc::ptr_eq(&entry, &self.entry))
        {
            registry.windows.remove(&self.entry.handle.window_id());
        }
    }
}

impl MacTransientOwnerLease {
    fn handle(&self) -> AnyWindowHandle {
        self.entry.handle
    }

    fn native_window(&self) -> id {
        *self.entry.native_window
    }

    fn state(&self) -> MacTransientOwnerLeaseState {
        if self.entry.closed.load(Ordering::Acquire)
            || self.entry.interaction_quiesced.load(Ordering::Acquire)
        {
            return MacTransientOwnerLeaseState::Retired;
        }
        let Some(registry) = self.registry.upgrade() else {
            return MacTransientOwnerLeaseState::Retired;
        };
        let registry = registry.lock();
        if !registry
            .windows
            .get(&self.entry.handle.window_id())
            .and_then(Weak::upgrade)
            .is_some_and(|entry| Arc::ptr_eq(&entry, &self.entry))
        {
            return MacTransientOwnerLeaseState::Retired;
        }
        if self.entry.closed.load(Ordering::Acquire)
            || self.entry.interaction_quiesced.load(Ordering::Acquire)
        {
            MacTransientOwnerLeaseState::Retired
        } else {
            MacTransientOwnerLeaseState::Current
        }
    }

    fn is_current(&self) -> bool {
        self.state() == MacTransientOwnerLeaseState::Current
    }

    fn is_presentable(&self) -> bool {
        if self.entry.closed.load(Ordering::Acquire)
            || self.entry.interaction_quiesced.load(Ordering::Acquire)
        {
            return false;
        }

        let native_window = self.native_window();
        let presentable = unsafe {
            let visible: BOOL = msg_send![native_window, isVisible];
            let miniaturized: BOOL = msg_send![native_window, isMiniaturized];
            let on_active_space: BOOL = msg_send![native_window, isOnActiveSpace];
            visible == YES && miniaturized == NO && on_active_space == YES
        };
        presentable && self.is_current()
    }
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    // Widely used private APIs; Apple uses them for their Terminal.app.
    fn CGSMainConnectionID() -> id;
    fn CGSSetWindowBackgroundBlurRadius(
        connection_id: id,
        window_id: NSInteger,
        radius: i64,
    ) -> i32;
}

#[ctor(unsafe)]
unsafe fn build_classes() {
    unsafe {
        WINDOW_CLASS = build_window_class("GPUIWindow", class!(NSWindow));
        PANEL_CLASS = build_window_class("GPUIPanel", class!(NSPanel));
        VIEW_CLASS = {
            let mut decl = ClassDecl::new("GPUIView", class!(NSView)).unwrap();
            decl.add_ivar::<*mut c_void>(WINDOW_STATE_IVAR);
            decl.add_method(sel!(dealloc), dealloc_view as extern "C" fn(&Object, Sel));

            decl.add_method(
                sel!(performKeyEquivalent:),
                handle_key_equivalent as extern "C" fn(&Object, Sel, id) -> BOOL,
            );
            decl.add_method(
                sel!(keyDown:),
                handle_key_down as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(keyUp:),
                handle_key_up as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(mouseDown:),
                handle_view_event as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(mouseUp:),
                handle_view_event as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(rightMouseDown:),
                handle_view_event as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(rightMouseUp:),
                handle_view_event as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(otherMouseDown:),
                handle_view_event as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(otherMouseUp:),
                handle_view_event as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(mouseMoved:),
                handle_view_event as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(resetCursorRects),
                reset_cursor_rects as extern "C" fn(&Object, Sel),
            );
            decl.add_method(
                sel!(pressureChangeWithEvent:),
                handle_view_event as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(mouseExited:),
                handle_view_event as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(magnifyWithEvent:),
                handle_view_event as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(mouseDragged:),
                handle_view_event as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(rightMouseDragged:),
                handle_view_event as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(otherMouseDragged:),
                handle_view_event as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(scrollWheel:),
                handle_view_event as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(swipeWithEvent:),
                handle_view_event as extern "C" fn(&Object, Sel, id),
            );
            decl.add_method(
                sel!(flagsChanged:),
                handle_view_event as extern "C" fn(&Object, Sel, id),
            );

            decl.add_method(
                sel!(makeBackingLayer),
                make_backing_layer as extern "C" fn(&Object, Sel) -> id,
            );

            decl.add_protocol(Protocol::get("CALayerDelegate").unwrap());
            decl.add_method(
                sel!(viewDidChangeBackingProperties),
                view_did_change_backing_properties as extern "C" fn(&Object, Sel),
            );
            decl.add_method(
                sel!(setFrameSize:),
                set_frame_size as extern "C" fn(&Object, Sel, NSSize),
            );
            decl.add_method(
                sel!(displayLayer:),
                display_layer as extern "C" fn(&Object, Sel, id),
            );

            decl.add_protocol(Protocol::get("NSTextInputClient").unwrap());
            decl.add_method(
                sel!(validAttributesForMarkedText),
                valid_attributes_for_marked_text as extern "C" fn(&Object, Sel) -> id,
            );
            decl.add_method(
                sel!(hasMarkedText),
                has_marked_text as extern "C" fn(&Object, Sel) -> BOOL,
            );
            decl.add_method(
                sel!(markedRange),
                marked_range as extern "C" fn(&Object, Sel) -> NSRange,
            );
            decl.add_method(
                sel!(selectedRange),
                selected_range as extern "C" fn(&Object, Sel) -> NSRange,
            );
            decl.add_method(
                sel!(firstRectForCharacterRange:actualRange:),
                first_rect_for_character_range
                    as extern "C" fn(&Object, Sel, NSRange, id) -> NSRect,
            );
            decl.add_method(
                sel!(insertText:replacementRange:),
                insert_text as extern "C" fn(&Object, Sel, id, NSRange),
            );
            decl.add_method(
                sel!(setMarkedText:selectedRange:replacementRange:),
                set_marked_text as extern "C" fn(&Object, Sel, id, NSRange, NSRange),
            );
            decl.add_method(sel!(unmarkText), unmark_text as extern "C" fn(&Object, Sel));
            decl.add_method(
                sel!(attributedSubstringForProposedRange:actualRange:),
                attributed_substring_for_proposed_range
                    as extern "C" fn(&Object, Sel, NSRange, *mut c_void) -> id,
            );
            decl.add_method(
                sel!(viewDidChangeEffectiveAppearance),
                view_did_change_effective_appearance as extern "C" fn(&Object, Sel),
            );

            // Suppress beep on keystrokes with modifier keys.
            decl.add_method(
                sel!(doCommandBySelector:),
                do_command_by_selector as extern "C" fn(&Object, Sel, Sel),
            );

            decl.add_method(
                sel!(acceptsFirstMouse:),
                accepts_first_mouse as extern "C" fn(&Object, Sel, id) -> BOOL,
            );

            decl.add_method(
                sel!(characterIndexForPoint:),
                character_index_for_point as extern "C" fn(&Object, Sel, NSPoint) -> u64,
            );
            decl.register()
        };
        BLURRED_VIEW_CLASS = {
            let mut decl = ClassDecl::new("BlurredView", class!(NSVisualEffectView)).unwrap();
            decl.add_method(
                sel!(initWithFrame:),
                blurred_view_init_with_frame as extern "C" fn(&Object, Sel, NSRect) -> id,
            );
            decl.add_method(
                sel!(updateLayer),
                blurred_view_update_layer as extern "C" fn(&Object, Sel),
            );
            decl.register()
        };
    }
}

pub(crate) fn convert_mouse_position(position: NSPoint, window_height: Pixels) -> Point<Pixels> {
    point(
        px(position.x as f32),
        // macOS screen coordinates are relative to bottom left
        window_height - px(position.y as f32),
    )
}

unsafe fn set_native_window_cursor_style(native_window: id, style: CursorStyle) {
    unsafe {
        let window_state = get_window_state(&*native_window);
        let mut window_state = window_state.lock();
        if window_state.cursor_style != style {
            window_state.cursor_style = style;
            let _: () = msg_send![
                window_state.native_window,
                invalidateCursorRectsForView: window_state.native_view.as_ptr()
            ];
        }
    }
}

/// Returns every visible application window in native front-to-back order, including panels.
///
/// The returned Objective-C objects are not retained. They must only be used synchronously on the
/// AppKit main thread.
unsafe fn visible_app_windows_front_to_back() -> Vec<id> {
    unsafe {
        let app = NSApplication::sharedApplication(nil);
        let windows: id = msg_send![app, windows];
        let count: NSUInteger = msg_send![windows, count];
        let mut ordered_windows = Vec::with_capacity(count as usize);

        for index in 0..count {
            let window: id = msg_send![windows, objectAtIndex: index];
            let visible: BOOL = msg_send![window, isVisible];
            let miniaturized: BOOL = msg_send![window, isMiniaturized];
            let on_active_space: BOOL = msg_send![window, isOnActiveSpace];
            if visible == YES && miniaturized == NO && on_active_space == YES {
                // NSApplication.orderedWindows excludes panels. orderedIndex covers every visible
                // application window, including policy-backed GPUI NSPanel instances.
                let ordered_index: NSInteger = msg_send![window, orderedIndex];
                ordered_windows.push((ordered_index, window));
            }
        }

        ordered_windows.sort_unstable_by_key(|(ordered_index, _)| *ordered_index);
        ordered_windows
            .into_iter()
            .map(|(_, window)| window)
            .collect()
    }
}

/// Returns the native GPUI window under the mouse, matching the public hovered-window semantics.
///
/// The returned Objective-C object is not retained. It must only be used synchronously on the
/// AppKit main thread.
unsafe fn hovered_gpui_native_window() -> Option<id> {
    unsafe {
        let mouse_location = NSEvent::mouseLocation(nil);
        for window in visible_app_windows_front_to_back() {
            let frame = NSWindow::frame(window);
            if !ns_rect_contains_point(frame, mouse_location) {
                continue;
            }

            if !is_gpui_window(window) {
                return None;
            }

            let window_state = get_window_state(&*window);
            let window_state = window_state.lock();
            if window_state.is_closed() {
                return None;
            }
            if window_state.accepts_pointer_input && !window_state.interaction_is_quiesced() {
                return Some(window);
            }
        }
        None
    }
}

unsafe fn build_window_class(name: &'static str, superclass: &Class) -> *const Class {
    unsafe {
        let mut decl = ClassDecl::new(name, superclass).unwrap();
        decl.add_ivar::<*mut c_void>(WINDOW_STATE_IVAR);
        decl.add_method(sel!(dealloc), dealloc_window as extern "C" fn(&Object, Sel));

        decl.add_method(
            sel!(canBecomeMainWindow),
            can_become_active_window as extern "C" fn(&Object, Sel) -> BOOL,
        );
        decl.add_method(
            sel!(canBecomeKeyWindow),
            can_become_active_window as extern "C" fn(&Object, Sel) -> BOOL,
        );
        decl.add_method(
            sel!(windowDidResize:),
            window_did_resize as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(windowDidChangeOcclusionState:),
            window_did_change_occlusion_state as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(windowWillEnterFullScreen:),
            window_will_enter_fullscreen as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(windowWillExitFullScreen:),
            window_will_exit_fullscreen as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(windowDidEnterFullScreen:),
            window_fullscreen_transition_did_finish as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(windowDidExitFullScreen:),
            window_fullscreen_transition_did_finish as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(windowDidFailToEnterFullScreen:),
            window_fullscreen_transition_did_finish as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(windowDidFailToExitFullScreen:),
            window_fullscreen_transition_did_finish as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(windowDidMiniaturize:),
            window_state_did_change as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(windowDidDeminiaturize:),
            window_state_did_change as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(windowDidMove:),
            window_did_move as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(windowDidChangeScreen:),
            window_did_change_screen as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(windowDidBecomeKey:),
            window_did_change_key_status as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(windowDidResignKey:),
            window_did_change_key_status as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(windowShouldClose:),
            window_should_close as extern "C" fn(&Object, Sel, id) -> BOOL,
        );

        decl.add_method(sel!(close), close_window as extern "C" fn(&Object, Sel));

        decl.add_method(
            sel!(draggingEntered:),
            dragging_entered as extern "C" fn(&Object, Sel, id) -> NSDragOperation,
        );
        decl.add_method(
            sel!(draggingUpdated:),
            dragging_updated as extern "C" fn(&Object, Sel, id) -> NSDragOperation,
        );
        decl.add_method(
            sel!(draggingExited:),
            dragging_exited as extern "C" fn(&Object, Sel, id),
        );
        decl.add_method(
            sel!(performDragOperation:),
            perform_drag_operation as extern "C" fn(&Object, Sel, id) -> BOOL,
        );
        decl.add_method(
            sel!(concludeDragOperation:),
            conclude_drag_operation as extern "C" fn(&Object, Sel, id),
        );

        decl.add_method(
            sel!(addTitlebarAccessoryViewController:),
            add_titlebar_accessory_view_controller as extern "C" fn(&Object, Sel, id),
        );

        decl.add_method(
            sel!(moveTabToNewWindow:),
            move_tab_to_new_window as extern "C" fn(&Object, Sel, id),
        );

        decl.add_method(
            sel!(mergeAllWindows:),
            merge_all_windows as extern "C" fn(&Object, Sel, id),
        );

        decl.add_method(
            sel!(selectNextTab:),
            select_next_tab as extern "C" fn(&Object, Sel, id),
        );

        decl.add_method(
            sel!(selectPreviousTab:),
            select_previous_tab as extern "C" fn(&Object, Sel, id),
        );

        decl.add_method(
            sel!(toggleTabBar:),
            toggle_tab_bar as extern "C" fn(&Object, Sel, id),
        );

        decl.register()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MacWindowCreationState {
    Windowed,
    Maximized,
    Fullscreen,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MacInitialPresentation {
    show: bool,
    allows_automatic_window_tabbing: bool,
    state: MacWindowCreationState,
    mapped: bool,
    completed: bool,
}

impl MacInitialPresentation {
    fn should_apply_automatic_tabbing(self) -> bool {
        self.allows_automatic_window_tabbing
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MacWindowCreationProjection {
    bounds: Bounds<Pixels>,
    state: MacWindowCreationState,
    restore_bounds: Bounds<Pixels>,
    accepts_pointer_input: bool,
    focus_on_appearing: bool,
    activation_policy: WindowActivationPolicy,
    topmost: bool,
    taskbar_visible: bool,
}

fn macos_click_can_activate(policy: WindowActivationPolicy) -> bool {
    policy.focus_on_click
}

fn should_defer_occluded_draw(attempted_window_draw: bool, visible: bool) -> bool {
    attempted_window_draw && !visible
}

fn global_client_bounds_to_appkit_content_rect(
    bounds: Bounds<Pixels>,
    screen_frame: NSRect,
    display_bounds: Bounds<Pixels>,
) -> NSRect {
    let relative_x = (bounds.origin.x - display_bounds.origin.x).as_f32() as f64;
    let relative_y = (bounds.origin.y - display_bounds.origin.y).as_f32() as f64;
    let content_top =
        screen_frame.origin.y + display_bounds.size.height.as_f32() as f64 - relative_y;
    NSRect::new(
        NSPoint::new(
            screen_frame.origin.x + relative_x,
            content_top - bounds.size.height.as_f32() as f64,
        ),
        NSSize::new(
            bounds.size.width.as_f32() as f64,
            bounds.size.height.as_f32() as f64,
        ),
    )
}

fn appkit_content_rect_to_global_client_bounds(
    content_rect: NSRect,
    screen_frame: NSRect,
    display_bounds: Bounds<Pixels>,
) -> Bounds<Pixels> {
    let relative_x = content_rect.origin.x - screen_frame.origin.x;
    let relative_y = screen_frame.origin.y + display_bounds.size.height.as_f32() as f64
        - content_rect.origin.y
        - content_rect.size.height;
    Bounds::new(
        point(
            display_bounds.origin.x + px(relative_x as f32),
            display_bounds.origin.y + px(relative_y as f32),
        ),
        size(
            px(content_rect.size.width as f32),
            px(content_rect.size.height as f32),
        ),
    )
}

fn native_rect_is_valid(rect: NSRect) -> bool {
    rect.origin.x.is_finite()
        && rect.origin.y.is_finite()
        && rect.size.width.is_finite()
        && rect.size.height.is_finite()
        && rect.size.width >= 0.0
        && rect.size.height >= 0.0
}

fn client_bounds_are_valid(bounds: Bounds<Pixels>) -> bool {
    let values = [
        bounds.origin.x.as_f32(),
        bounds.origin.y.as_f32(),
        bounds.size.width.as_f32(),
        bounds.size.height.as_f32(),
    ];
    values.into_iter().all(f32::is_finite)
        && bounds.size.width >= px(0.0)
        && bounds.size.height >= px(0.0)
}

fn native_scale_matches(actual: f64, expected: f32) -> bool {
    actual.is_finite() && actual > 0.0 && actual as f32 == expected
}

fn backing_scale_matches(logical: NSRect, backing: NSRect, expected: f32) -> bool {
    if logical.size.width <= 0.0 || logical.size.height <= 0.0 {
        return false;
    }
    native_scale_matches(backing.size.width / logical.size.width, expected)
        && native_scale_matches(backing.size.height / logical.size.height, expected)
}

fn commit_stable_native_observation<T: Copy + PartialEq>(
    committed: &mut Option<T>,
    first: Option<T>,
    second: Option<T>,
) -> bool {
    let (Some(first), Some(second)) = (first, second) else {
        return false;
    };
    if first != second {
        return false;
    }
    *committed = Some(second);
    true
}

impl MacWindowCreationProjection {
    fn new(
        window_bounds: WindowBounds,
        kind: &WindowKind,
        accepts_pointer_input: bool,
        focus_on_appearing: bool,
        activation_policy: WindowActivationPolicy,
    ) -> Self {
        let state = if macos_supports_toplevel_creation_state(kind) {
            match window_bounds {
                WindowBounds::Windowed(_) => MacWindowCreationState::Windowed,
                WindowBounds::Maximized(_) => MacWindowCreationState::Maximized,
                WindowBounds::Fullscreen(_) => MacWindowCreationState::Fullscreen,
            }
        } else {
            MacWindowCreationState::Windowed
        };

        Self {
            bounds: window_bounds.get_bounds(),
            state,
            restore_bounds: window_bounds.get_bounds(),
            accepts_pointer_input,
            focus_on_appearing: match kind {
                WindowKind::Normal | WindowKind::Floating => focus_on_appearing,
                WindowKind::PopUp => false,
                WindowKind::Dialog => true,
            },
            activation_policy: match kind {
                WindowKind::Normal | WindowKind::Floating => activation_policy,
                WindowKind::PopUp => WindowActivationPolicy {
                    accepts_activation: false,
                    focus_on_click: false,
                },
                WindowKind::Dialog => WindowActivationPolicy::default(),
            },
            topmost: matches!(kind, WindowKind::PopUp | WindowKind::Floating),
            taskbar_visible: matches!(kind, WindowKind::Normal),
        }
    }

    fn requires_nonactivating_panel(self) -> bool {
        !self.activation_policy.focus_on_click
    }

    fn becomes_key_only_if_needed(self) -> bool {
        !self.activation_policy.focus_on_click
    }

    fn panel_hides_on_deactivate(self) -> bool {
        !self.taskbar_visible
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MacWindowBackgroundProjection {
    appearance: WindowBackgroundAppearance,
    native_opaque: bool,
    renderer_transparent: bool,
    background_alpha: f64,
    blur_enabled: bool,
}

impl MacWindowBackgroundProjection {
    fn new(requested: WindowBackgroundAppearance) -> Self {
        let appearance = match requested {
            WindowBackgroundAppearance::MicaBackdrop
            | WindowBackgroundAppearance::MicaAltBackdrop => {
                WindowBackgroundAppearance::Transparent
            }
            appearance => appearance,
        };
        let native_opaque = appearance == WindowBackgroundAppearance::Opaque;
        Self {
            appearance,
            native_opaque,
            renderer_transparent: !native_opaque,
            background_alpha: if native_opaque { 1.0 } else { 0.0001 },
            blur_enabled: appearance == WindowBackgroundAppearance::Blurred,
        }
    }
}

pub(crate) fn macos_supports_toplevel_creation_state(kind: &WindowKind) -> bool {
    matches!(kind, WindowKind::Normal | WindowKind::Floating)
}

pub(crate) fn macos_supports_focus_on_appearing(kind: &WindowKind) -> bool {
    matches!(kind, WindowKind::Normal | WindowKind::Floating)
}

struct MacPresentationShutdownAuthority {
    window_id: WindowId,
    ticket: Option<WindowPresentationShutdownTicket>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MacRendererGeometry {
    content_size: Size<Pixels>,
    scale_factor: f32,
}

impl MacRendererGeometry {
    fn with_content_size(self, content_size: Size<Pixels>) -> Self {
        Self {
            content_size,
            ..self
        }
    }

    fn differs_from(self, previous: Self) -> bool {
        self != previous
    }
}

fn renderer_geometry_for_frame_size(
    current: MacRendererGeometry,
    content_size: Size<Pixels>,
) -> MacRendererGeometry {
    current.with_content_size(content_size)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MacFullscreenTransition {
    Entering,
    Exiting,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MacFullscreenTransitionTerminal {
    Entered,
    Exited,
    FailedToEnter,
    FailedToExit,
}

impl MacFullscreenTransitionTerminal {
    fn expected_transition(self) -> MacFullscreenTransition {
        match self {
            Self::Entered | Self::FailedToEnter => MacFullscreenTransition::Entering,
            Self::Exited | Self::FailedToExit => MacFullscreenTransition::Exiting,
        }
    }

    fn is_fullscreen(self) -> bool {
        matches!(self, Self::Entered | Self::FailedToExit)
    }

    fn finish(self, transition: &mut Option<MacFullscreenTransition>) -> bool {
        if *transition != Some(self.expected_transition()) {
            return false;
        }
        *transition = None;
        true
    }

    fn titlebar_appears_transparent(self, requested_transparent: bool) -> bool {
        requested_transparent && !self.is_fullscreen()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MacWindowActiveEvent {
    BecameKey,
    ResignedKey,
}

impl MacWindowActiveEvent {
    fn is_active(self) -> bool {
        matches!(self, Self::BecameKey)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MacWindowStateExpectation {
    is_minimized: bool,
    is_maximized: bool,
}

impl MacWindowStateExpectation {
    fn new(is_minimized: bool, is_maximized: bool) -> Self {
        Self {
            is_minimized,
            is_maximized,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MacWindowStateEventSource {
    Resized,
    Miniaturized,
    Deminiaturized,
}

impl MacWindowStateEventSource {
    fn expected_state(self, native: MacWindowStateExpectation) -> MacWindowStateExpectation {
        match self {
            Self::Resized => native,
            Self::Miniaturized => MacWindowStateExpectation::new(true, native.is_maximized),
            Self::Deminiaturized => MacWindowStateExpectation::new(false, native.is_maximized),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MacWindowStateEvent {
    source: MacWindowStateEventSource,
    expected: MacWindowStateExpectation,
}

impl MacWindowStateEvent {
    fn new(source: MacWindowStateEventSource, expected: MacWindowStateExpectation) -> Self {
        Self { source, expected }
    }

    fn is_coalescible_resize_with(self, other: Self) -> bool {
        self.source == MacWindowStateEventSource::Resized
            && other.source == MacWindowStateEventSource::Resized
            && self.expected == other.expected
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MacWindowObservationEvent {
    Active(MacWindowActiveEvent),
    Fullscreen(MacFullscreenTransitionTerminal),
    Moved,
    State(MacWindowStateEvent),
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MacWindowPendingObservationEvent<O = MacWindowNativeObservation> {
    kind: MacWindowObservationEvent,
    observation: Option<O>,
}

impl<O> MacWindowPendingObservationEvent<O> {
    fn observed(kind: MacWindowObservationEvent, observation: O) -> Self {
        Self {
            kind,
            observation: Some(observation),
        }
    }

    fn unobserved(kind: MacWindowObservationEvent) -> Self {
        Self {
            kind,
            observation: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct MacWindowPendingObservationEvents<O = MacWindowNativeObservation> {
    events: SmallVec<[MacWindowPendingObservationEvent<O>; 4]>,
}

impl<O> Default for MacWindowPendingObservationEvents<O> {
    fn default() -> Self {
        Self {
            events: SmallVec::new(),
        }
    }
}

impl<O> MacWindowPendingObservationEvents<O> {
    fn record(&mut self, event: MacWindowPendingObservationEvent<O>) {
        // An unresolved fullscreen terminal has no historical fact to expose. Retain only its
        // latest native terminal instead of replaying the final observation multiple times.
        if matches!(event.kind, MacWindowObservationEvent::Fullscreen(_)) {
            self.events.retain(|pending| {
                !matches!(pending.kind, MacWindowObservationEvent::Fullscreen(_))
                    || pending.observation.is_some()
            });
        }

        if let Some(last) = self.events.last_mut() {
            let coalesces_move = matches!(event.kind, MacWindowObservationEvent::Moved)
                && matches!(last.kind, MacWindowObservationEvent::Moved);
            let coalesces_resize = match (last.kind, event.kind) {
                (
                    MacWindowObservationEvent::State(previous),
                    MacWindowObservationEvent::State(current),
                ) => previous.is_coalescible_resize_with(current),
                _ => false,
            };
            if coalesces_move || coalesces_resize {
                if event.observation.is_some() || last.observation.is_none() {
                    *last = event;
                }
                return;
            }
        }

        self.events.push(event);
    }

    #[cfg(test)]
    fn fullscreen_terminal_count(&self) -> usize {
        self.events
            .iter()
            .filter(|event| matches!(event.kind, MacWindowObservationEvent::Fullscreen(_)))
            .count()
    }

    #[cfg(test)]
    fn has_window_state_event(&self) -> bool {
        self.events
            .iter()
            .any(|event| matches!(event.kind, MacWindowObservationEvent::State(_)))
    }

    #[cfg(test)]
    fn has_moved_event(&self) -> bool {
        self.events
            .iter()
            .any(|event| matches!(event.kind, MacWindowObservationEvent::Moved))
    }

    fn last_active_event_index(&self) -> Option<usize> {
        self.events
            .iter()
            .rposition(|event| matches!(event.kind, MacWindowObservationEvent::Active(_)))
    }
}

#[derive(Debug)]
struct MacWindowSerialEffectDrain<T> {
    pending: VecDeque<T>,
    owner: MacWindowSerialEffectDrainOwner,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MacWindowSerialEffectDrainOwner {
    Idle,
    Draining,
}

impl<T> Default for MacWindowSerialEffectDrain<T> {
    fn default() -> Self {
        Self {
            pending: VecDeque::new(),
            owner: MacWindowSerialEffectDrainOwner::Idle,
        }
    }
}

impl<T> MacWindowSerialEffectDrain<T> {
    fn enqueue(&mut self, effect: T) -> bool {
        // The current drain retains ownership across callbacks. Reentrant commits append only.
        self.pending.push_back(effect);
        match self.owner {
            MacWindowSerialEffectDrainOwner::Idle => {
                self.owner = MacWindowSerialEffectDrainOwner::Draining;
                true
            }
            MacWindowSerialEffectDrainOwner::Draining => false,
        }
    }

    fn pop_next(&mut self) -> Option<T> {
        if self.owner != MacWindowSerialEffectDrainOwner::Draining {
            return None;
        }
        let next = self.pending.pop_front();
        if next.is_none() {
            self.owner = MacWindowSerialEffectDrainOwner::Idle;
        }
        next
    }

    fn cancel(&mut self) {
        self.pending.clear();
        self.owner = MacWindowSerialEffectDrainOwner::Idle;
    }
}

#[derive(Default)]
struct MacWindowCallbackPanicBoundary {
    first_panic: Option<Box<dyn std::any::Any + Send>>,
}

impl MacWindowCallbackPanicBoundary {
    fn deliver(&mut self, effect: impl FnOnce()) {
        if let Err(panic) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(effect)) {
            self.retain(panic);
        }
    }

    fn retain(&mut self, panic: Box<dyn std::any::Any + Send>) {
        if self.first_panic.is_none() {
            self.first_panic = Some(panic);
        } else {
            // Panic payloads are arbitrary user values whose destructor may also panic. Only the
            // first payload is retained for boundary reporting; later payloads must not be dropped.
            mem::forget(panic);
        }
    }

    fn into_first_panic(self) -> Option<Box<dyn std::any::Any + Send>> {
        self.first_panic
    }

    fn isolate_at_native_boundary(self, context: &str) {
        let Some(panic) = self.into_first_panic() else {
            return;
        };
        // An arbitrary panic payload may itself panic from Drop. Keep it from unwinding through
        // the Objective-C or dispatch ABI after all recoverable effects have been delivered.
        mem::forget(panic);
        log::error!("isolated a panic from {context} at the native callback boundary");
    }
}

const MAC_WINDOW_OBSERVATION_RETRY_DELAYS: [Duration; 5] = [
    Duration::from_millis(16),
    Duration::from_millis(64),
    Duration::from_millis(250),
    Duration::from_secs(1),
    Duration::from_secs(2),
];

// Owns the one window-level observation obligation. A newer topology generation supersedes the
// in-flight job epoch while retaining typed native events for the eventual complete-fact commit.
#[derive(Clone, Debug, Default, PartialEq)]
struct MacWindowObservationCommitCoordinator {
    target_generation: Option<u64>,
    job_epoch: u64,
    job_scheduled: bool,
    retry_attempt: usize,
    pending_events: MacWindowPendingObservationEvents,
}

impl MacWindowObservationCommitCoordinator {
    fn request(
        &mut self,
        target_generation: u64,
        event: Option<MacWindowPendingObservationEvent>,
    ) -> Option<u64> {
        if let Some(event) = event {
            self.pending_events.record(event);
        }

        let target_was_replaced = self
            .target_generation
            .is_none_or(|current| target_generation > current);
        if target_was_replaced {
            self.target_generation = Some(target_generation);
            self.retry_attempt = 0;
            return Some(self.start_job());
        }

        if self.job_scheduled {
            None
        } else {
            Some(self.start_job())
        }
    }

    fn start_job(&mut self) -> u64 {
        self.job_epoch = self.job_epoch.wrapping_add(1);
        self.job_scheduled = true;
        self.job_epoch
    }

    fn target_for_job(&self, job_epoch: u64) -> Option<u64> {
        if self.job_scheduled && self.job_epoch == job_epoch {
            self.target_generation
        } else {
            None
        }
    }

    fn pause(&mut self, job_epoch: u64) {
        if self.job_scheduled && self.job_epoch == job_epoch {
            self.job_scheduled = false;
        }
    }

    fn retry_delay(&mut self, job_epoch: u64) -> Option<Duration> {
        self.target_for_job(job_epoch)?;
        let index = self
            .retry_attempt
            .min(MAC_WINDOW_OBSERVATION_RETRY_DELAYS.len() - 1);
        let delay = MAC_WINDOW_OBSERVATION_RETRY_DELAYS[index];
        self.retry_attempt = self.retry_attempt.saturating_add(1);
        Some(delay)
    }

    fn commit(
        &mut self,
        job_epoch: u64,
        observed_generation: u64,
    ) -> Option<MacWindowPendingObservationEvents> {
        let target_generation = self.target_for_job(job_epoch)?;
        if observed_generation < target_generation {
            return None;
        }

        self.target_generation = None;
        self.job_scheduled = false;
        self.retry_attempt = 0;
        Some(mem::take(&mut self.pending_events))
    }

    fn cancel(&mut self) {
        self.target_generation = None;
        self.job_epoch = self.job_epoch.wrapping_add(1);
        self.job_scheduled = false;
        self.retry_attempt = 0;
        self.pending_events = MacWindowPendingObservationEvents::default();
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MacWindowNativeObservation {
    topology_generation: u64,
    bounds: Bounds<Pixels>,
    display: MacDisplay,
    is_minimized: bool,
    is_maximized: bool,
    is_fullscreen: bool,
    is_active: bool,
    style_mask: NSUInteger,
    titlebar_appears_transparent: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MacWindowNativeObservationRefreshFailure {
    AwaitTopologyPublication,
    AwaitFullscreenTerminal,
    UnstableNativeSample,
}

impl MacWindowNativeObservation {
    fn renderer_geometry(self) -> MacRendererGeometry {
        MacRendererGeometry {
            content_size: self.bounds.size,
            scale_factor: self.display.scale_factor(),
        }
    }

    fn state_expectation(self) -> MacWindowStateExpectation {
        MacWindowStateExpectation::new(self.is_minimized, self.is_maximized)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MacWindowObservationEffectEvent {
    kind: MacWindowObservationEvent,
    is_latest_active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct MacWindowObservationEffectBatch<O = MacWindowNativeObservation> {
    observation: O,
    event: Option<MacWindowObservationEffectEvent>,
}

impl<O: Copy + PartialEq> MacWindowPendingObservationEvents<O> {
    fn into_effect_batches(
        self,
        final_observation: O,
    ) -> SmallVec<[MacWindowObservationEffectBatch<O>; 4]> {
        // Replay each event-bound complete fact in native order, then converge to the newest exact
        // topology fact. Unobserved events use that final fact and are subject to their typed
        // fallback semantics (notably latest-wins fullscreen terminals).
        let last_active_event_index = self.last_active_event_index();
        let mut batches = SmallVec::with_capacity(self.events.len() + 1);
        for (event_index, event) in self.events.into_iter().enumerate() {
            batches.push(MacWindowObservationEffectBatch {
                observation: event.observation.unwrap_or(final_observation),
                event: Some(MacWindowObservationEffectEvent {
                    kind: event.kind,
                    is_latest_active: Some(event_index) == last_active_event_index,
                }),
            });
        }
        if batches
            .last()
            .is_none_or(|batch| batch.observation != final_observation)
        {
            batches.push(MacWindowObservationEffectBatch {
                observation: final_observation,
                event: None,
            });
        }
        batches
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct MacWindowNativeObservationChanges {
    renderer_geometry_changed: bool,
    moved: bool,
    minimized_or_maximized_changed: bool,
    state_changed: bool,
}

fn moved_or_display_facts_changed<D: PartialEq>(
    previous_origin: Point<Pixels>,
    previous_display: &D,
    current_origin: Point<Pixels>,
    current_display: &D,
) -> bool {
    previous_origin != current_origin || previous_display != current_display
}

impl MacWindowNativeObservationChanges {
    fn between(previous: MacWindowNativeObservation, current: MacWindowNativeObservation) -> Self {
        let minimized_or_maximized_changed = previous.is_minimized != current.is_minimized
            || previous.is_maximized != current.is_maximized;
        Self {
            renderer_geometry_changed: current
                .renderer_geometry()
                .differs_from(previous.renderer_geometry()),
            moved: moved_or_display_facts_changed(
                previous.bounds.origin,
                &previous.display,
                current.bounds.origin,
                &current.display,
            ),
            minimized_or_maximized_changed,
            state_changed: minimized_or_maximized_changed
                || previous.is_fullscreen != current.is_fullscreen
                || previous.style_mask != current.style_mask
                || previous.titlebar_appears_transparent != current.titlebar_appears_transparent,
        }
    }
}

impl MacPresentationShutdownAuthority {
    fn new(window_id: WindowId) -> Self {
        Self {
            window_id,
            ticket: None,
        }
    }

    fn claim(
        &mut self,
        candidate: WindowPresentationShutdownTicket,
    ) -> Option<WindowPresentationShutdownTicket> {
        if candidate.snapshot().window_id() != self.window_id {
            return None;
        }
        if let Some(current) = self.ticket.as_ref() {
            return Some(current.clone());
        }
        self.ticket = Some(candidate.clone());
        Some(candidate)
    }

    fn ticket(&mut self) -> WindowPresentationShutdownTicket {
        if let Some(ticket) = self.ticket.as_ref() {
            return ticket.clone();
        }

        let generation =
            NEXT_EMERGENCY_PRESENTATION_SHUTDOWN_GENERATION.fetch_add(1, Ordering::Relaxed);
        assert_ne!(
            generation, 0,
            "emergency presentation-shutdown generation space exhausted"
        );
        let ticket = WindowPresentationShutdownTicket::new(self.window_id, generation);
        self.ticket = Some(ticket.clone());
        ticket
    }
}

struct MacWindowState {
    handle: AnyWindowHandle,
    window_registry: MacWindowRegistry,
    foreground_executor: ForegroundExecutor,
    background_executor: BackgroundExecutor,
    native_window: id,
    native_view: NonNull<Object>,
    blurred_view: Option<id>,
    background_appearance: WindowBackgroundAppearance,
    cursor_style: CursorStyle,
    cursor_visible: Arc<AtomicBool>,
    display_link: Option<DisplayLink>,
    renderer: renderer::Renderer,
    presentation_shutdown_authority: Arc<Mutex<MacPresentationShutdownAuthority>>,
    request_frame_callback: Option<Box<dyn FnMut(RequestFrameOptions)>>,
    event_callback: PlatformInputCallbackSlot,
    activate_callback: Option<Box<dyn FnMut(PlatformWindowActiveStatusObservation)>>,
    resize_callback: Option<Box<dyn FnMut(Size<Pixels>, f32)>>,
    moved_callback: Option<Box<dyn FnMut()>>,
    window_state_change_callback: Option<Box<dyn FnMut()>>,
    should_close_callback: Option<Box<dyn FnMut() -> bool>>,
    close_callback: Option<Box<dyn FnOnce()>>,
    appearance_changed_callback: Option<Box<dyn FnMut()>>,
    input_handler: PlatformInputHandlerSlot,
    last_key_equivalent: Option<KeyDownEvent>,
    synthetic_drag_counter: usize,
    traffic_light_position: Option<Point<Pixels>>,
    transparent_titlebar: bool,
    accepts_pointer_input: bool,
    activation_policy: WindowActivationPolicy,
    topmost: bool,
    taskbar_visible: bool,
    previous_modifiers_changed_event: Option<PlatformInput>,
    keystroke_for_do_command: Option<Keystroke>,
    do_command_handled: Option<bool>,
    external_files_dragged: bool,
    // Whether the next left-mouse click is also the focusing click.
    first_mouse: bool,
    display_topology: MacDisplayTopologyHandle,
    display_topology_subscription: Option<MacDisplayTopologySubscription>,
    observation_commit: MacWindowObservationCommitCoordinator,
    observation_effects: MacWindowSerialEffectDrain<MacWindowObservationEffectBatch>,
    renderer_geometry: MacRendererGeometry,
    renderer_geometry_initialized: bool,
    native_observation: Option<MacWindowNativeObservation>,
    windowed_restore_bounds: Bounds<Pixels>,
    fullscreen_restore_bounds: Bounds<Pixels>,
    pending_fullscreen_restore_bounds: Option<Bounds<Pixels>>,
    fullscreen_transition: Option<MacFullscreenTransition>,
    move_tab_to_new_window_callback: Option<Box<dyn FnMut()>>,
    merge_all_windows_callback: Option<Box<dyn FnMut()>>,
    select_next_tab_callback: Option<Box<dyn FnMut()>>,
    select_previous_tab_callback: Option<Box<dyn FnMut()>>,
    toggle_tab_bar_callback: Option<Box<dyn FnMut()>>,
    activated_least_once: bool,
    closed: Arc<AtomicBool>,
    accesskit_adapter: Option<accesskit_macos::SubclassingAdapter>,
    creation_facts: WindowCreationFacts,
    initial_presentation: MacInitialPresentation,
    transient_ordering_in_progress: bool,
    attempted_window_draw: bool,
    interaction_quiesced: Arc<AtomicBool>,
}

impl MacWindowState {
    fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }

    fn interaction_is_quiesced(&self) -> bool {
        self.interaction_quiesced.load(Ordering::Acquire)
    }

    fn admits_native_tabbing(&self) -> bool {
        !self.is_closed()
            && !self.interaction_is_quiesced()
            && self.creation_facts.transient_for.is_none()
    }

    fn mark_closed(&mut self) -> (PlatformInputCallbackSlot, PlatformInputHandlerSlot) {
        self.closed.store(true, Ordering::Release);
        self.interaction_quiesced.store(true, Ordering::Release);
        self.stop_display_link();
        self.synthetic_drag_counter += 1;
        self.fullscreen_transition = None;
        self.observation_commit.cancel();
        self.observation_effects.cancel();
        self.request_frame_callback = None;
        self.activate_callback = None;
        self.resize_callback = None;
        self.moved_callback = None;
        self.window_state_change_callback = None;
        self.should_close_callback = None;
        self.appearance_changed_callback = None;
        self.display_topology_subscription.take();
        (self.event_callback.clone(), self.input_handler.clone())
    }

    fn move_traffic_light(&self) {
        if let Some(traffic_light_position) = self.traffic_light_position {
            if self.is_fullscreen() {
                // Moving traffic lights while fullscreen doesn't work,
                // see https://github.com/zed-industries/zed/issues/4712
                return;
            }

            let titlebar_height = self.titlebar_height();

            unsafe {
                let close_button: id = msg_send![
                    self.native_window,
                    standardWindowButton: NSWindowButton::NSWindowCloseButton
                ];
                let min_button: id = msg_send![
                    self.native_window,
                    standardWindowButton: NSWindowButton::NSWindowMiniaturizeButton
                ];
                let zoom_button: id = msg_send![
                    self.native_window,
                    standardWindowButton: NSWindowButton::NSWindowZoomButton
                ];

                let mut close_button_frame: CGRect = msg_send![close_button, frame];
                let mut min_button_frame: CGRect = msg_send![min_button, frame];
                let mut zoom_button_frame: CGRect = msg_send![zoom_button, frame];
                let mut origin = point(
                    traffic_light_position.x,
                    titlebar_height
                        - traffic_light_position.y
                        - px(close_button_frame.size.height as f32),
                );
                let button_spacing =
                    px((min_button_frame.origin.x - close_button_frame.origin.x) as f32);

                close_button_frame.origin = CGPoint::new(origin.x.into(), origin.y.into());
                let _: () = msg_send![close_button, setFrame: close_button_frame];
                origin.x += button_spacing;

                min_button_frame.origin = CGPoint::new(origin.x.into(), origin.y.into());
                let _: () = msg_send![min_button, setFrame: min_button_frame];
                origin.x += button_spacing;

                zoom_button_frame.origin = CGPoint::new(origin.x.into(), origin.y.into());
                let _: () = msg_send![zoom_button, setFrame: zoom_button_frame];
                origin.x += button_spacing;
            }
        }
    }

    fn start_display_link(&mut self) {
        if self.is_closed() {
            self.stop_display_link();
            return;
        }

        let (visible, screen) = unsafe {
            (
                self.native_window
                    .occlusionState()
                    .contains(NSWindowOcclusionState::NSWindowOcclusionStateVisible),
                self.native_window.screen(),
            )
        };
        if !visible {
            self.stop_display_link();
            return;
        }

        self.stop_display_link();

        if screen == nil {
            log::debug!(
                "skipping display link start for {:?}: window has no screen",
                self.handle.window_id()
            );
            self.request_layer_display();
            return;
        }

        let display_id = unsafe { display_id_for_screen(screen) };
        if display_id == 0 {
            log::debug!(
                "skipping display link start for {:?}: screen returned display id 0",
                self.handle.window_id()
            );
            self.request_layer_display();
            return;
        }

        let display_link =
            DisplayLink::new(display_id, self.native_view.as_ptr() as *mut c_void, step);
        let Some(mut display_link) = display_link
            .inspect_err(|error| self.log_display_link_start_failure(display_id, *error))
            .ok()
        else {
            self.request_layer_display();
            return;
        };

        match display_link.start() {
            Ok(()) => {
                self.display_link = Some(display_link);
            }
            Err(error) => {
                self.log_display_link_start_failure(display_id, error);
                self.request_layer_display();
            }
        }
    }

    fn request_layer_display(&self) {
        unsafe {
            let _: () = msg_send![self.native_view.as_ptr(), setNeedsDisplay:YES];
        }
    }

    fn log_display_link_start_failure(
        &self,
        display_id: CGDirectDisplayID,
        error: DisplayLinkError,
    ) {
        let message = format!(
            "{error}; window_id={:?}, display_id={display_id}, closed={}, visible={}",
            self.handle.window_id(),
            self.is_closed(),
            unsafe {
                self.native_window
                    .occlusionState()
                    .contains(NSWindowOcclusionState::NSWindowOcclusionStateVisible)
            }
        );
        if error.is_transient_create_failure() {
            log::debug!("{message}");
        } else {
            log::warn!("{message}");
        }
    }

    fn stop_display_link(&mut self) {
        self.display_link = None;
    }

    fn committed_native_observation(&self) -> MacWindowNativeObservation {
        self.native_observation
            .expect("an open macOS window must have a committed native observation")
    }

    fn observation_target_generation(&self) -> u64 {
        let committed_generation = self
            .native_observation
            .map(|observation| observation.topology_generation)
            .unwrap_or(0);
        self.display_topology
            .retained_snapshot()
            .map(|snapshot| snapshot.generation())
            .unwrap_or(committed_generation)
            .max(committed_generation)
    }

    fn native_state_expectation(&self) -> MacWindowStateExpectation {
        unsafe {
            let style_mask = NSWindow::styleMask(self.native_window);
            let is_fullscreen = style_mask.contains(NSWindowStyleMask::NSFullScreenWindowMask);
            let is_minimized: BOOL = msg_send![self.native_window, isMiniaturized];
            let is_maximized: BOOL = msg_send![self.native_window, isZoomed];
            MacWindowStateExpectation::new(
                is_minimized == YES,
                !is_fullscreen && is_maximized == YES,
            )
        }
    }

    fn pending_observation_event(
        &self,
        kind: MacWindowObservationEvent,
    ) -> MacWindowPendingObservationEvent {
        match self.retained_native_observation() {
            Some(observation) => MacWindowPendingObservationEvent::observed(kind, observation),
            None => MacWindowPendingObservationEvent::unobserved(kind),
        }
    }

    fn sample_native_observation(
        &self,
        topology: &MacDisplayTopologySnapshot,
    ) -> Option<MacWindowNativeObservation> {
        unsafe {
            let screen_before = NSWindow::screen(self.native_window);
            let target_before = topology.validate_native_screen(screen_before).ok()?;
            let display = target_before.display();
            let screen_frame = NSScreen::frame(screen_before);
            let window_frame = NSWindow::frame(self.native_window);
            let content_rect = NSWindow::contentRectForFrameRect_(self.native_window, window_frame);
            let content_view = NSWindow::contentView(self.native_window);
            if content_view == nil {
                return None;
            }
            let content_view_bounds = NSView::bounds(content_view);
            let content_view_backing_bounds: NSRect =
                msg_send![content_view, convertRectToBacking: content_view_bounds];
            let window_scale_factor: f64 = msg_send![self.native_window, backingScaleFactor];
            let style_mask = NSWindow::styleMask(self.native_window);
            let titlebar_appears_transparent: BOOL =
                msg_send![self.native_window, titlebarAppearsTransparent];
            let is_minimized: BOOL = msg_send![self.native_window, isMiniaturized];
            let is_maximized: BOOL = msg_send![self.native_window, isZoomed];
            let is_active = NSWindow::isKeyWindow(self.native_window) == YES;
            let screen_after = NSWindow::screen(self.native_window);
            let target_after = topology.validate_native_screen(screen_after).ok()?;

            if screen_before != screen_after
                || target_before.generation() != topology.generation()
                || target_after.generation() != topology.generation()
                || target_before.display() != target_after.display()
                || !native_rect_is_valid(window_frame)
                || !native_rect_is_valid(content_rect)
                || !native_rect_is_valid(content_view_bounds)
                || !native_rect_is_valid(content_view_backing_bounds)
                || content_rect.size.width != content_view_bounds.size.width
                || content_rect.size.height != content_view_bounds.size.height
                || !backing_scale_matches(
                    content_view_bounds,
                    content_view_backing_bounds,
                    display.scale_factor(),
                )
                || !native_scale_matches(window_scale_factor, display.scale_factor())
            {
                return None;
            }

            let is_fullscreen = style_mask.contains(NSWindowStyleMask::NSFullScreenWindowMask);
            let bounds = appkit_content_rect_to_global_client_bounds(
                content_rect,
                screen_frame,
                display.bounds(),
            );
            if !client_bounds_are_valid(bounds) {
                return None;
            }

            Some(MacWindowNativeObservation {
                topology_generation: topology.generation(),
                bounds,
                display,
                is_minimized: is_minimized == YES,
                is_maximized: !is_fullscreen && is_maximized == YES,
                is_fullscreen,
                is_active,
                style_mask: style_mask.bits(),
                titlebar_appears_transparent: titlebar_appears_transparent == YES,
            })
        }
    }

    fn stable_native_observation(
        &self,
        topology: &MacDisplayTopologySnapshot,
    ) -> Option<MacWindowNativeObservation> {
        let first = self.sample_native_observation(topology);
        let second = self.sample_native_observation(topology);
        let mut observation = None;
        commit_stable_native_observation(&mut observation, first, second).then_some(())?;
        observation
    }

    fn retained_native_observation(&self) -> Option<MacWindowNativeObservation> {
        let topology = self.display_topology.retained_snapshot()?;
        // Event-bound facts cannot be reconstructed after a later native edge. Retry only within
        // this delegate turn and keep the attempt count bounded.
        (0..3).find_map(|_| self.stable_native_observation(&topology))
    }

    fn native_observation_for_generation(
        &self,
        target_generation: u64,
    ) -> Result<MacWindowNativeObservation, MacWindowNativeObservationRefreshFailure> {
        if self.fullscreen_transition.is_some() {
            return Err(MacWindowNativeObservationRefreshFailure::AwaitFullscreenTerminal);
        }
        let Ok(topology) = self.display_topology.exact_snapshot() else {
            return Err(MacWindowNativeObservationRefreshFailure::AwaitTopologyPublication);
        };
        if topology.generation() < target_generation {
            return Err(MacWindowNativeObservationRefreshFailure::AwaitTopologyPublication);
        }
        self.stable_native_observation(&topology)
            .ok_or(MacWindowNativeObservationRefreshFailure::UnstableNativeSample)
    }

    fn refresh_native_observation(&mut self) -> bool {
        let Ok(observation) = self.native_observation_for_generation(0) else {
            return false;
        };
        self.store_native_observation(observation);
        true
    }

    fn store_native_observation(&mut self, observation: MacWindowNativeObservation) {
        self.native_observation = Some(observation);
        if !observation.is_fullscreen && !observation.is_maximized && !observation.is_minimized {
            self.windowed_restore_bounds = observation.bounds;
        }
    }

    fn apply_native_observation(
        &mut self,
        observation: MacWindowNativeObservation,
    ) -> MacWindowNativeObservationChanges {
        let previous = self.committed_native_observation();
        self.store_native_observation(observation);
        let changes = MacWindowNativeObservationChanges::between(previous, observation);
        self.update_renderer_geometry(observation.renderer_geometry());
        changes
    }

    fn update_renderer_geometry(&mut self, geometry: MacRendererGeometry) -> bool {
        if self.renderer_geometry_initialized && self.renderer_geometry == geometry {
            return false;
        }

        if let Some(layer) = self.renderer.layer() {
            layer.set_contents_scale(geometry.scale_factor as f64);
        }
        self.renderer.update_drawable_size(
            geometry
                .content_size
                .to_device_pixels(geometry.scale_factor),
        );
        self.renderer_geometry = geometry;
        self.renderer_geometry_initialized = true;
        true
    }

    fn is_maximized(&self) -> bool {
        self.committed_native_observation().is_maximized
    }

    fn is_fullscreen(&self) -> bool {
        self.committed_native_observation().is_fullscreen
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.committed_native_observation().bounds
    }

    fn content_size(&self) -> Size<Pixels> {
        self.committed_native_observation().bounds.size
    }

    fn scale_factor(&self) -> f32 {
        self.committed_native_observation().display.scale_factor()
    }

    fn titlebar_height(&self) -> Pixels {
        unsafe {
            let frame = NSWindow::frame(self.native_window);
            let content_layout_rect: CGRect = msg_send![self.native_window, contentLayoutRect];
            px((frame.size.height - content_layout_rect.size.height) as f32)
        }
    }

    fn window_bounds(&self) -> WindowBounds {
        let observation = self.committed_native_observation();
        self.window_bounds_from_client_bounds(
            observation.bounds,
            observation.is_fullscreen,
            observation.is_maximized,
        )
    }

    fn window_bounds_from_client_bounds(
        &self,
        bounds: Bounds<Pixels>,
        is_fullscreen: bool,
        is_maximized: bool,
    ) -> WindowBounds {
        if is_fullscreen {
            WindowBounds::Fullscreen(self.fullscreen_restore_bounds)
        } else if is_maximized {
            WindowBounds::Maximized(self.windowed_restore_bounds)
        } else {
            WindowBounds::Windowed(bounds)
        }
    }
}

unsafe impl Send for MacWindowState {}

pub(crate) struct MacWindow(
    Arc<Mutex<MacWindowState>>,
    Arc<Mutex<MacPresentationShutdownAuthority>>,
    // Native disposal has either completed synchronously or been queued on the main executor.
    bool,
    Option<MacWindowRegistration>,
);

struct MacNativeObjectConstructionGuard {
    object: id,
}

impl MacNativeObjectConstructionGuard {
    unsafe fn new(object: id) -> Self {
        debug_assert!(!object.is_null());
        Self { object }
    }

    fn disarm(&mut self) {
        self.object = nil;
    }

    unsafe fn into_autoreleased(mut self) -> id {
        let object = self.object;
        self.disarm();
        unsafe { object.autorelease() }
    }
}

impl Drop for MacNativeObjectConstructionGuard {
    fn drop(&mut self) {
        if !self.object.is_null() {
            unsafe {
                let _: () = msg_send![self.object, release];
            }
        }
    }
}

struct MacWindowConstructionGuard<'a> {
    window: &'a mut MacWindow,
    armed: bool,
}

impl<'a> MacWindowConstructionGuard<'a> {
    fn new(window: &'a mut MacWindow) -> Self {
        Self {
            window,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for MacWindowConstructionGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.window.dispose_native_window(true);
        }
    }
}

struct MacTransientAttachmentGuard {
    owner: MacTransientOwnerLease,
    child_window: StrongPtr,
    expected_child: AnyWindowHandle,
    expected_level: NSInteger,
    prior_parent: Option<StrongPtr>,
    prior_visible: bool,
    armed: bool,
}

struct MacTransientOrderingScope {
    window_state: Weak<Mutex<MacWindowState>>,
    native_window: id,
}

impl MacTransientOrderingScope {
    fn begin(window_state: &Arc<Mutex<MacWindowState>>) -> Option<Self> {
        let native_window = {
            let mut state = window_state.lock();
            if state.is_closed()
                || state.interaction_is_quiesced()
                || state.transient_ordering_in_progress
            {
                return None;
            }
            state.transient_ordering_in_progress = true;
            state.native_window
        };
        Some(Self {
            window_state: Arc::downgrade(window_state),
            native_window,
        })
    }
}

impl Drop for MacTransientOrderingScope {
    fn drop(&mut self) {
        let Some(window_state) = self.window_state.upgrade() else {
            return;
        };
        let mut state = window_state.lock();
        if state.native_window == self.native_window {
            state.transient_ordering_in_progress = false;
        }
    }
}

impl MacTransientAttachmentGuard {
    fn prepare(
        owner: MacTransientOwnerLease,
        child_window: id,
        expected_child: AnyWindowHandle,
    ) -> Result<Self> {
        anyhow::ensure!(
            owner.is_current(),
            "transient owner is no longer current before AppKit attachment"
        );
        anyhow::ensure!(
            unsafe { native_window_matches_handle(child_window, expected_child) },
            "macOS transient child changed before AppKit attachment"
        );
        anyhow::ensure!(
            owner.native_window() != child_window,
            "a macOS transient window cannot own itself"
        );
        let parent_before: id = unsafe { msg_send![child_window, parentWindow] };
        anyhow::ensure!(
            parent_before.is_null() || parent_before == owner.native_window(),
            "macOS transient window already has an unexpected native parent"
        );
        let child_level: NSInteger = unsafe { msg_send![child_window, level] };
        let child_visible: BOOL = unsafe { msg_send![child_window, isVisible] };
        let guard = Self {
            owner,
            child_window: unsafe { StrongPtr::retain(child_window) },
            expected_child,
            expected_level: child_level,
            prior_parent: (!parent_before.is_null())
                .then(|| unsafe { StrongPtr::retain(parent_before) }),
            prior_visible: child_visible == YES,
            armed: true,
        };

        unsafe {
            if !parent_before.is_null() {
                let _: () = msg_send![parent_before, removeChildWindow: child_window];
            }
            let parent_after_detach: id = msg_send![child_window, parentWindow];
            anyhow::ensure!(
                parent_after_detach.is_null(),
                "macOS transient child could not leave its previous sibling order"
            );
            anyhow::ensure!(
                guard.owner.is_current(),
                "transient owner retired while preparing AppKit attachment"
            );
            let _: () = msg_send![
                guard.owner.native_window(),
                addChildWindow: child_window
                ordered: NSWindowOrderingMode::NSWindowAbove
            ];
            child_window.setLevel_(child_level);
        }
        let parent_after: id = unsafe { msg_send![child_window, parentWindow] };
        let level_after: NSInteger = unsafe { msg_send![child_window, level] };
        anyhow::ensure!(
            guard.owner.is_current()
                && parent_after == guard.owner.native_window()
                && level_after == child_level,
            "macOS transient owner or window level changed while the native relationship was installed"
        );
        Ok(guard)
    }

    fn disarm(&mut self) {
        self.armed = false;
    }

    fn is_committed(&self, expected_child: AnyWindowHandle) -> bool {
        let child_window = *self.child_window;
        let parent_window: id = unsafe { msg_send![child_window, parentWindow] };
        let child_level: NSInteger = unsafe { msg_send![child_window, level] };
        self.owner.is_presentable()
            && parent_window == self.owner.native_window()
            && child_level == self.expected_level
            && unsafe { native_window_matches_handle(child_window, expected_child) }
    }

    fn commit_after_owner_retirement(&self) -> bool {
        if self.owner.state() != MacTransientOwnerLeaseState::Retired {
            return false;
        }
        let child_window = *self.child_window;
        if !unsafe { native_window_matches_handle(child_window, self.expected_child) } {
            return false;
        }
        if detach_retired_mac_transient_owner(
            child_window,
            self.owner.handle(),
            Some(self.owner.native_window()),
        )
        .is_err()
        {
            return false;
        }
        unsafe {
            child_window.setLevel_(self.expected_level);
        }
        let parent_after: id = unsafe { msg_send![child_window, parentWindow] };
        let level_after: NSInteger = unsafe { msg_send![child_window, level] };
        let visible_after: BOOL = unsafe { msg_send![child_window, isVisible] };
        let committed = parent_after.is_null()
            && level_after == self.expected_level
            && visible_after == YES
            && unsafe { native_window_matches_handle(child_window, self.expected_child) };
        committed
    }

    fn rollback(&self) {
        let child_window = *self.child_window;
        for _ in 0..2 {
            // Quiescence revokes forward interaction authority, but an exact live child must
            // still receive the native compensation for side effects already performed.
            if !unsafe { native_window_is_exact_live_handle(child_window, self.expected_child) } {
                return;
            }
            let expected_parent = self
                .owner
                .is_current()
                .then(|| self.prior_parent.as_ref().map(|parent| **parent))
                .flatten();
            let current_parent: id = unsafe { msg_send![child_window, parentWindow] };
            if !current_parent.is_null() && Some(current_parent) != expected_parent {
                unsafe {
                    let _: () = msg_send![current_parent, removeChildWindow: child_window];
                }
            }
            let parent_after_remove: id = unsafe { msg_send![child_window, parentWindow] };
            if let Some(expected_parent) = expected_parent {
                if parent_after_remove != expected_parent {
                    if !parent_after_remove.is_null() {
                        continue;
                    }
                    unsafe {
                        let _: () = msg_send![
                            expected_parent,
                            addChildWindow: child_window
                            ordered: NSWindowOrderingMode::NSWindowAbove
                        ];
                    }
                }
            } else if !parent_after_remove.is_null() {
                continue;
            }

            unsafe {
                child_window.setLevel_(self.expected_level);
                if self.prior_visible {
                    let _: () = msg_send![child_window, orderFront: nil];
                } else {
                    let _: () = msg_send![child_window, orderOut: nil];
                }
            }
            let committed_parent: id = unsafe { msg_send![child_window, parentWindow] };
            let committed_level: NSInteger = unsafe { msg_send![child_window, level] };
            let committed_visible: BOOL = unsafe { msg_send![child_window, isVisible] };
            let parent_restored = expected_parent.map_or_else(
                || committed_parent.is_null(),
                |expected_parent| committed_parent == expected_parent,
            );
            if parent_restored
                && committed_level == self.expected_level
                && (committed_visible == YES) == self.prior_visible
            {
                return;
            }
        }
        log::error!(
            "failed to restore a macOS transient window's parent, visibility, and level after a rejected presentation transaction"
        );
    }
}

impl Drop for MacTransientAttachmentGuard {
    fn drop(&mut self) {
        if self.armed {
            self.rollback();
        }
    }
}

unsafe fn detach_native_transient_relationships(native_window: id) {
    unsafe {
        let parent_window: id = msg_send![native_window, parentWindow];
        if !parent_window.is_null() {
            let _: () = msg_send![parent_window, removeChildWindow: native_window];
        }

        let child_windows: id = msg_send![native_window, childWindows];
        let child_count: NSUInteger = msg_send![child_windows, count];
        let mut retained_children = Vec::with_capacity(child_count as usize);
        for index in 0..child_count {
            let child_window: id = msg_send![child_windows, objectAtIndex: index];
            if is_gpui_window(child_window) {
                retained_children.push(StrongPtr::retain(child_window));
            }
        }
        for child_window in retained_children {
            let child_window = *child_window;
            let current_parent: id = msg_send![child_window, parentWindow];
            if current_parent == native_window {
                let _: () = msg_send![native_window, removeChildWindow: child_window];
            }
        }
    }
}

unsafe fn native_window_matches_handle(
    native_window: id,
    expected_handle: AnyWindowHandle,
) -> bool {
    unsafe {
        native_window_satisfies_handle(
            native_window,
            expected_handle,
            MacNativeWindowHandleRequirement::InteractionAdmitted,
        )
    }
}

unsafe fn native_window_is_exact_live_handle(
    native_window: id,
    expected_handle: AnyWindowHandle,
) -> bool {
    unsafe {
        native_window_satisfies_handle(
            native_window,
            expected_handle,
            MacNativeWindowHandleRequirement::ExactLive,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MacNativeWindowHandleRequirement {
    ExactLive,
    InteractionAdmitted,
}

unsafe fn native_window_satisfies_handle(
    native_window: id,
    expected_handle: AnyWindowHandle,
    requirement: MacNativeWindowHandleRequirement,
) -> bool {
    unsafe {
        if native_window.is_null() || !is_gpui_window(native_window) {
            return false;
        }
        let state = get_window_state(&*native_window);
        let Some(state) = state.try_lock() else {
            return false;
        };
        !state.is_closed()
            && state.native_window == native_window
            && state.handle == expected_handle
            && (requirement == MacNativeWindowHandleRequirement::ExactLive
                || !state.interaction_is_quiesced())
    }
}

unsafe fn native_window_has_handle(native_window: id, expected_handle: AnyWindowHandle) -> bool {
    unsafe {
        if native_window.is_null() || !is_gpui_window(native_window) {
            return false;
        }
        let state = get_window_state(&*native_window);
        let Some(state) = state.try_lock() else {
            return false;
        };
        state.native_window == native_window && state.handle == expected_handle
    }
}

fn detach_retired_mac_transient_owner(
    native_window: id,
    expected_owner: AnyWindowHandle,
    expected_parent: Option<id>,
) -> Result<()> {
    let current_parent: id = unsafe { msg_send![native_window, parentWindow] };
    let was_visible: BOOL = unsafe { msg_send![native_window, isVisible] };
    if !current_parent.is_null() {
        let parent_matches = expected_parent
            .is_some_and(|expected_parent| current_parent == expected_parent)
            || expected_parent.is_none()
                && unsafe { native_window_has_handle(current_parent, expected_owner) };
        anyhow::ensure!(
            parent_matches,
            "macOS transient child has an unexpected native parent after owner retirement"
        );
        unsafe {
            let _: () = msg_send![current_parent, removeChildWindow: native_window];
        }
    }
    let parent_after: id = unsafe { msg_send![native_window, parentWindow] };
    anyhow::ensure!(
        parent_after.is_null(),
        "macOS transient child remained attached after owner retirement"
    );
    unsafe {
        if was_visible == YES {
            let _: () = msg_send![native_window, orderFront: nil];
        } else {
            let _: () = msg_send![native_window, orderOut: nil];
        }
    }
    let visible_after: BOOL = unsafe { msg_send![native_window, isVisible] };
    anyhow::ensure!(
        visible_after == was_visible,
        "macOS transient child visibility changed while retiring its owner"
    );
    Ok(())
}

fn prepare_mac_transient_attachment(
    native_window: id,
    expected_window: AnyWindowHandle,
    transient_for: Option<AnyWindowHandle>,
    registry: &MacWindowRegistry,
) -> Result<Option<MacTransientAttachmentGuard>> {
    let Some(expected_owner) = transient_for else {
        return Ok(None);
    };
    anyhow::ensure!(
        unsafe { native_window_matches_handle(native_window, expected_window) },
        "macOS transient child changed before native presentation"
    );

    let owner = match registry.observe_transient_owner(expected_owner) {
        MacTransientOwnerResolution::Current(owner) => owner,
        MacTransientOwnerResolution::Retired(owner) => {
            detach_retired_mac_transient_owner(
                native_window,
                expected_owner,
                Some(owner.native_window()),
            )?;
            return Ok(None);
        }
        MacTransientOwnerResolution::Gone => {
            detach_retired_mac_transient_owner(native_window, expected_owner, None)?;
            return Ok(None);
        }
        MacTransientOwnerResolution::Busy => {
            return Err(anyhow!(
                "macOS transient owner authority is busy during native presentation"
            ));
        }
    };
    if !owner.is_presentable() {
        if detach_mac_transient_after_concurrent_owner_retirement(
            native_window,
            expected_window,
            expected_owner,
            registry,
        )? {
            return Ok(None);
        }
        return Err(anyhow!(
            "macOS transient owner is not visible on the active Space"
        ));
    }

    match MacTransientAttachmentGuard::prepare(owner, native_window, expected_window) {
        Ok(attachment) => Ok(Some(attachment)),
        Err(error) => {
            if detach_mac_transient_after_concurrent_owner_retirement(
                native_window,
                expected_window,
                expected_owner,
                registry,
            )? {
                Ok(None)
            } else {
                Err(error)
            }
        }
    }
}

fn detach_mac_transient_after_concurrent_owner_retirement(
    native_window: id,
    expected_window: AnyWindowHandle,
    expected_owner: AnyWindowHandle,
    registry: &MacWindowRegistry,
) -> Result<bool> {
    if !unsafe { native_window_matches_handle(native_window, expected_window) } {
        return Ok(false);
    }
    match registry.observe_transient_owner(expected_owner) {
        MacTransientOwnerResolution::Retired(owner) => {
            detach_retired_mac_transient_owner(
                native_window,
                expected_owner,
                Some(owner.native_window()),
            )?;
            Ok(true)
        }
        MacTransientOwnerResolution::Gone => {
            detach_retired_mac_transient_owner(native_window, expected_owner, None)?;
            Ok(true)
        }
        MacTransientOwnerResolution::Current(_) | MacTransientOwnerResolution::Busy => Ok(false),
    }
}

struct MacTransientPresentationTransaction {
    attachment: Option<MacTransientAttachmentGuard>,
    _ordering_scope: Option<MacTransientOrderingScope>,
    expected_child: AnyWindowHandle,
    native_window: id,
    proof: MacTransientPresentationProof,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MacTransientPresentationProof {
    Visible,
    VisibleAndKey,
}

impl MacTransientPresentationProof {
    fn is_satisfied(self, native_window: id, expected_child: AnyWindowHandle) -> bool {
        let visible: BOOL = unsafe { msg_send![native_window, isVisible] };
        if visible != YES || !unsafe { native_window_matches_handle(native_window, expected_child) }
        {
            return false;
        }
        if self == Self::VisibleAndKey {
            unsafe {
                let app = NSApplication::sharedApplication(nil);
                let key_window: id = msg_send![app, keyWindow];
                if key_window != native_window {
                    return false;
                }
            }
        }
        true
    }
}

impl MacTransientPresentationTransaction {
    fn begin(
        window_state: &Arc<Mutex<MacWindowState>>,
        native_window: id,
        expected_child: AnyWindowHandle,
        transient_for: Option<AnyWindowHandle>,
        registry: &MacWindowRegistry,
        proof: MacTransientPresentationProof,
    ) -> Result<Self> {
        if proof == MacTransientPresentationProof::VisibleAndKey {
            anyhow::ensure!(
                proof.is_satisfied(native_window, expected_child),
                "macOS transient child lost exact key-window authority before sibling ordering"
            );
        }
        let ordering_scope = if transient_for.is_some() {
            Some(
                MacTransientOrderingScope::begin(window_state).ok_or_else(|| {
                    anyhow!("macOS transient sibling ordering is already in progress")
                })?,
            )
        } else {
            None
        };
        let attachment = prepare_mac_transient_attachment(
            native_window,
            expected_child,
            transient_for,
            registry,
        )?;
        Ok(Self {
            attachment,
            _ordering_scope: ordering_scope,
            expected_child,
            native_window,
            proof,
        })
    }

    fn commit(mut self) -> bool {
        if !self
            .proof
            .is_satisfied(self.native_window, self.expected_child)
        {
            return false;
        }
        if let Some(attachment) = self.attachment.as_ref() {
            if !attachment.is_committed(self.expected_child)
                && !attachment.commit_after_owner_retirement()
            {
                return false;
            }
        }
        if !self
            .proof
            .is_satisfied(self.native_window, self.expected_child)
        {
            return false;
        }
        if let Some(attachment) = self.attachment.as_mut() {
            attachment.disarm();
        }
        true
    }
}

struct MacWindowInteractionQuiescenceTarget {
    native_window: StrongPtr,
    window_state: Weak<Mutex<MacWindowState>>,
    quiesced: Arc<AtomicBool>,
}

impl MacWindowInteractionQuiescenceTarget {
    unsafe fn new(
        native_window: id,
        window_state: Weak<Mutex<MacWindowState>>,
        quiesced: Arc<AtomicBool>,
    ) -> Self {
        Self {
            native_window: unsafe { StrongPtr::retain(native_window) },
            window_state,
            quiesced,
        }
    }

    fn revoke(&self) {
        // Closing the logical gate and completing AppKit cleanup are separate
        // authorities. Retry every idempotent native effect until this call
        // returns; the platform quiescence receipt is the completion proof.
        self.quiesced.store(true, Ordering::Release);

        unsafe {
            let native_window = *self.native_window;
            let _: () = msg_send![native_window, setIgnoresMouseEvents: YES];
            let _: () = msg_send![native_window, setMovable: NO];
            let _: () = msg_send![native_window, resignKeyWindow];
        }

        let a11y_events = self.window_state.upgrade().and_then(|window_state| {
            let mut state = window_state.lock();
            if state.is_closed() {
                None
            } else {
                state
                    .accesskit_adapter
                    .as_mut()
                    .and_then(|adapter| adapter.update_view_focus_state(false))
            }
        });
        if let Some(events) = a11y_events {
            events.raise();
        }
    }
}

impl MacWindow {
    fn dispose_native_window(&mut self, synchronously: bool) {
        if self.2 {
            return;
        }
        self.2 = true;
        self.3.take();

        let (event_callback, input_handler, window, foreground_executor) = {
            let mut state = self.0.lock();
            let (event_callback, input_handler) = state.mark_closed();
            state.renderer.destroy();
            let window = state.native_window;
            unsafe {
                state.native_window.setDelegate_(nil);
            }
            (
                event_callback,
                input_handler,
                window,
                state.foreground_executor.clone(),
            )
        };
        event_callback.terminate();
        input_handler.terminate();
        unsafe {
            detach_native_transient_relationships(window);
        }

        if synchronously {
            unsafe {
                let window = &*window;
                let superclass = window_callback_superclass(window);
                let _: () = msg_send![super(window, superclass), close];
                let _: () = msg_send![window, release];
            }
        } else {
            foreground_executor
                .spawn(async move {
                    unsafe {
                        window.close();
                        window.autorelease();
                    }
                })
                .detach();
        }
    }

    pub fn open(
        handle: AnyWindowHandle,
        WindowParams {
            window_bounds,
            titlebar,
            kind,
            is_movable,
            is_resizable,
            is_minimizable,
            accepts_pointer_input,
            focus_on_appearing,
            activation_policy,
            transient_for,
            show,
            display_id: _,
            window_min_size,
            tabbing_identifier,
            ..
        }: WindowParams,
        transient_owner: Option<MacTransientOwnerLease>,
        window_registry: MacWindowRegistry,
        display_topology: MacDisplayTopologyHandle,
        display_snapshot: MacDisplayTopologySnapshot,
        target_display: ValidatedMacDisplayTarget,
        cursor_visible: Arc<AtomicBool>,
        foreground_executor: ForegroundExecutor,
        background_executor: BackgroundExecutor,
        renderer_context: renderer::Context,
    ) -> Result<Self> {
        match (transient_for, transient_owner.as_ref()) {
            (Some(requested), Some(owner)) if requested == owner.handle() => {}
            (None, None) => {}
            _ => {
                return Err(anyhow!(
                    "macOS transient owner resolution did not match the requested owner"
                ));
            }
        }
        anyhow::ensure!(
            transient_for.is_none() || !matches!(kind, WindowKind::PopUp),
            "macOS popup windows do not support a transient owner relationship"
        );
        anyhow::ensure!(
            transient_for.is_none() || tabbing_identifier.is_none(),
            "macOS transient windows cannot also join a native tab group"
        );
        let display = target_display.display();
        if target_display.generation() != display_snapshot.generation()
            || display_snapshot.display(display.id()) != Some(display)
        {
            return Err(anyhow!(
                "the resolved macOS display target does not belong to the creation snapshot"
            ));
        }

        unsafe {
            let creation = MacWindowCreationProjection::new(
                window_bounds,
                &kind,
                accepts_pointer_input,
                focus_on_appearing,
                activation_policy,
            );
            let bounds = creation.bounds;
            let pool = NSAutoreleasePool::new(nil);

            let allows_automatic_window_tabbing = tabbing_identifier.is_some();
            let initial_presentation = MacInitialPresentation {
                show,
                allows_automatic_window_tabbing,
                state: creation.state,
                mapped: false,
                completed: false,
            };
            let creation_facts = WindowCreationFacts {
                show,
                focus_on_appearing: creation.focus_on_appearing,
                transient_for,
            };
            if transient_for.is_none() {
                if allows_automatic_window_tabbing {
                    let () = msg_send![class!(NSWindow), setAllowsAutomaticWindowTabbing: YES];
                } else {
                    let () = msg_send![class!(NSWindow), setAllowsAutomaticWindowTabbing: NO];
                }
            }

            let mut style_mask;
            if let Some(titlebar) = titlebar.as_ref() {
                style_mask =
                    NSWindowStyleMask::NSClosableWindowMask | NSWindowStyleMask::NSTitledWindowMask;

                if is_resizable {
                    style_mask |= NSWindowStyleMask::NSResizableWindowMask;
                }

                if is_minimizable {
                    style_mask |= NSWindowStyleMask::NSMiniaturizableWindowMask;
                }

                if titlebar.appears_transparent {
                    style_mask |= NSWindowStyleMask::NSFullSizeContentViewWindowMask;
                }
            } else {
                style_mask = NSWindowStyleMask::NSTitledWindowMask
                    | NSWindowStyleMask::NSFullSizeContentViewWindowMask;
            }

            let native_window: id = match kind {
                _ if creation.requires_nonactivating_panel() => {
                    style_mask |= NSWindowStyleMaskNonactivatingPanel;
                    msg_send![PANEL_CLASS, alloc]
                }
                WindowKind::Normal => {
                    msg_send![WINDOW_CLASS, alloc]
                }
                WindowKind::PopUp => {
                    style_mask |= NSWindowStyleMaskNonactivatingPanel;
                    msg_send![PANEL_CLASS, alloc]
                }
                WindowKind::Floating | WindowKind::Dialog => {
                    msg_send![PANEL_CLASS, alloc]
                }
            };

            let target_screen = target_display.screen();
            let screen_frame = NSScreen::frame(target_screen);
            let display_bounds = display.bounds();
            let content_rect =
                global_client_bounds_to_appkit_content_rect(bounds, screen_frame, display_bounds);

            let native_window = native_window.initWithContentRect_styleMask_backing_defer_screen_(
                content_rect,
                style_mask,
                NSBackingStoreBuffered,
                NO,
                target_screen,
            );
            assert!(!native_window.is_null());
            let mut native_window_ownership = MacNativeObjectConstructionGuard::new(native_window);
            if creation.requires_nonactivating_panel() {
                let hides_on_deactivate = creation.panel_hides_on_deactivate().to_objc();
                let _: () = msg_send![native_window, setHidesOnDeactivate: hides_on_deactivate];
                let becomes_key_only_if_needed = creation.becomes_key_only_if_needed().to_objc();
                let _: () = msg_send![
                    native_window,
                    setBecomesKeyOnlyIfNeeded: becomes_key_only_if_needed
                ];
            }
            let () = msg_send![
                native_window,
                registerForDraggedTypes:
                    NSArray::arrayWithObject(nil, NSFilenamesPboardType)
            ];
            let () = msg_send![
                native_window,
                setReleasedWhenClosed: NO
            ];
            if transient_for.is_some() {
                native_window.setTabbingMode_(NSWindowTabbingMode::NSWindowTabbingModeDisallowed);
                let _: () = msg_send![native_window, setTabbingIdentifier:nil];
            }

            let content_view = native_window.contentView();
            let native_view: id = msg_send![VIEW_CLASS, alloc];
            let native_view = NSView::initWithFrame_(native_view, NSView::bounds(content_view));
            assert!(!native_view.is_null());
            let native_view_ownership = MacNativeObjectConstructionGuard::new(native_view);

            let presentation_shutdown_authority = Arc::new(Mutex::new(
                MacPresentationShutdownAuthority::new(handle.window_id()),
            ));
            let mut window = Self(
                Arc::new(Mutex::new(MacWindowState {
                    handle,
                    window_registry: window_registry.clone(),
                    foreground_executor,
                    background_executor,
                    native_window,
                    native_view: NonNull::new_unchecked(native_view),
                    blurred_view: None,
                    background_appearance: WindowBackgroundAppearance::Opaque,
                    cursor_style: CursorStyle::Arrow,
                    cursor_visible,
                    display_link: None,
                    renderer: renderer::new_renderer(
                        renderer_context,
                        native_window as *mut _,
                        native_view as *mut _,
                        bounds.size.map(|pixels| pixels.as_f32()),
                        false,
                    ),
                    presentation_shutdown_authority: presentation_shutdown_authority.clone(),
                    request_frame_callback: None,
                    event_callback: PlatformInputCallbackSlot::default(),
                    activate_callback: None,
                    resize_callback: None,
                    moved_callback: None,
                    window_state_change_callback: None,
                    should_close_callback: None,
                    close_callback: None,
                    appearance_changed_callback: None,
                    input_handler: PlatformInputHandlerSlot::default(),
                    last_key_equivalent: None,
                    synthetic_drag_counter: 0,
                    traffic_light_position: titlebar
                        .as_ref()
                        .and_then(|titlebar| titlebar.traffic_light_position),
                    transparent_titlebar: titlebar
                        .as_ref()
                        .is_none_or(|titlebar| titlebar.appears_transparent),
                    previous_modifiers_changed_event: None,
                    keystroke_for_do_command: None,
                    do_command_handled: None,
                    external_files_dragged: false,
                    first_mouse: false,
                    display_topology,
                    display_topology_subscription: None,
                    observation_commit: MacWindowObservationCommitCoordinator::default(),
                    observation_effects: MacWindowSerialEffectDrain::default(),
                    renderer_geometry: MacRendererGeometry {
                        content_size: bounds.size,
                        scale_factor: display.scale_factor(),
                    },
                    renderer_geometry_initialized: false,
                    native_observation: None,
                    windowed_restore_bounds: creation.restore_bounds,
                    fullscreen_restore_bounds: creation.restore_bounds,
                    pending_fullscreen_restore_bounds: matches!(
                        creation.state,
                        MacWindowCreationState::Fullscreen
                    )
                    .then_some(creation.restore_bounds),
                    fullscreen_transition: None,
                    move_tab_to_new_window_callback: None,
                    merge_all_windows_callback: None,
                    select_next_tab_callback: None,
                    select_previous_tab_callback: None,
                    toggle_tab_bar_callback: None,
                    activated_least_once: false,
                    closed: Arc::new(AtomicBool::new(false)),
                    accesskit_adapter: None,
                    creation_facts,
                    accepts_pointer_input: creation.accepts_pointer_input,
                    activation_policy: creation.activation_policy,
                    topmost: creation.topmost,
                    taskbar_visible: creation.taskbar_visible,
                    initial_presentation,
                    transient_ordering_in_progress: false,
                    attempted_window_draw: false,
                    interaction_quiesced: Arc::new(AtomicBool::new(false)),
                })),
                presentation_shutdown_authority,
                false,
                None,
            );
            let mut construction_guard = MacWindowConstructionGuard::new(&mut window);
            native_window_ownership.disarm();

            (*native_window).set_ivar(
                WINDOW_STATE_IVAR,
                Arc::into_raw(construction_guard.window.0.clone()) as *const c_void,
            );
            native_window.setDelegate_(native_window);
            (*native_view).set_ivar(
                WINDOW_STATE_IVAR,
                Arc::into_raw(construction_guard.window.0.clone()) as *const c_void,
            );

            if let Some(title) = titlebar
                .as_ref()
                .and_then(|t| t.title.as_ref().map(AsRef::as_ref))
            {
                construction_guard.window.set_title(title);
            }

            native_window.setMovable_(is_movable as BOOL);
            let _: () = msg_send![
                native_window,
                setIgnoresMouseEvents: !creation.accepts_pointer_input
            ];

            if let Some(window_min_size) = window_min_size {
                native_window.setContentMinSize_(NSSize {
                    width: window_min_size.width.to_f64(),
                    height: window_min_size.height.to_f64(),
                });
            }

            if titlebar.is_none_or(|titlebar| titlebar.appears_transparent) {
                native_window.setTitlebarAppearsTransparent_(YES);
                native_window.setTitleVisibility_(NSWindowTitleVisibility::NSWindowTitleHidden);
            }

            native_view.setAutoresizingMask_(NSViewWidthSizable | NSViewHeightSizable);
            native_view.setWantsBestResolutionOpenGLSurface_(YES);

            // From winit crate: On Mojave, views automatically become layer-backed shortly after
            // being added to a native_window. Changing the layer-backedness of a view breaks the
            // association between the view and its associated OpenGL context. To work around this,
            // on we explicitly make the view layer-backed up front so that AppKit doesn't do it
            // itself and break the association with its context.
            native_view.setWantsLayer(YES);
            let _: () = msg_send![
            native_view,
            setLayerContentsRedrawPolicy: NSViewLayerContentsRedrawDuringViewResize
            ];

            let native_view = native_view_ownership.into_autoreleased();
            content_view.addSubview_(native_view);
            native_window.makeFirstResponder_(native_view);

            // Reapply the requested content rect after titlebar configuration. AppKit accepts an
            // outer frame here, so projecting through the live window decoration policy keeps
            // WindowBounds in client coordinates.
            let frame_rect = NSWindow::frameRectForContentRect_(native_window, content_rect);
            NSWindow::setFrame_display_(native_window, frame_rect, NO);
            if creation.state == MacWindowCreationState::Maximized {
                let visible_frame = NSScreen::visibleFrame(target_screen);
                let _: () = msg_send![native_window, setFrame: visible_frame display: NO];
            }

            match kind {
                WindowKind::Normal | WindowKind::Floating => {
                    if kind == WindowKind::Floating {
                        // Let the window float keep above normal windows.
                        native_window.setLevel_(NSFloatingWindowLevel);
                    } else {
                        native_window.setLevel_(NSNormalWindowLevel);
                    }
                    native_window.setAcceptsMouseMovedEvents_(YES);

                    if let Some(tabbing_identifier) = tabbing_identifier {
                        let tabbing_id = ns_string(tabbing_identifier.as_str());
                        let _: () = msg_send![native_window, setTabbingIdentifier: tabbing_id];
                    } else {
                        let _: () = msg_send![native_window, setTabbingIdentifier:nil];
                    }
                }
                WindowKind::PopUp => {
                    // Use a tracking area to allow receiving MouseMoved events even when
                    // the window or application aren't active, which is often the case
                    // e.g. for notification windows.
                    let tracking_area: id = msg_send![class!(NSTrackingArea), alloc];
                    let _: () = msg_send![
                        tracking_area,
                        initWithRect: NSRect::new(NSPoint::new(0., 0.), NSSize::new(0., 0.))
                        options: NSTrackingMouseEnteredAndExited | NSTrackingMouseMoved | NSTrackingActiveAlways | NSTrackingInVisibleRect
                        owner: native_view
                        userInfo: nil
                    ];
                    let _: () =
                        msg_send![native_view, addTrackingArea: tracking_area.autorelease()];

                    native_window.setLevel_(NSPopUpWindowLevel);
                    let _: () = msg_send![
                        native_window,
                        setAnimationBehavior: NSWindowAnimationBehaviorUtilityWindow
                    ];
                    native_window.setCollectionBehavior_(
                        NSWindowCollectionBehavior::NSWindowCollectionBehaviorCanJoinAllSpaces |
                        NSWindowCollectionBehavior::NSWindowCollectionBehaviorFullScreenAuxiliary
                    );
                }
                WindowKind::Dialog => {}
            }

            let observation_committed = {
                let mut window_state = construction_guard.window.0.lock();
                let committed = window_state.refresh_native_observation();
                if committed {
                    let observation = window_state.committed_native_observation();
                    window_state.update_renderer_geometry(observation.renderer_geometry());
                    window_state.move_traffic_light();
                }
                committed
            };
            if !observation_committed {
                drop(construction_guard);
                pool.drain();
                return Err(anyhow!(
                    "AppKit did not provide a coherent initial client-geometry observation"
                ));
            }
            if let Err(error) = subscribe_window_to_display_topology(&construction_guard.window.0) {
                drop(construction_guard);
                pool.drain();
                return Err(anyhow!(
                    "cannot subscribe the macOS window to display publications: {error}"
                ));
            }

            let registration = match window_registry
                .register(construction_guard.window, transient_owner.as_ref())
            {
                Ok(registration) => registration,
                Err(error) => {
                    drop(construction_guard);
                    pool.drain();
                    return Err(error);
                }
            };
            construction_guard.window.3 = Some(registration);

            construction_guard.disarm();
            drop(construction_guard);
            pool.drain();

            Ok(window)
        }
    }

    pub fn active_window() -> Option<AnyWindowHandle> {
        unsafe {
            let app = NSApplication::sharedApplication(nil);
            let main_window: id = msg_send![app, mainWindow];
            if main_window.is_null() {
                return None;
            }

            if is_gpui_window(main_window) {
                let handle = get_window_state(&*main_window).lock().handle;
                Some(handle)
            } else {
                None
            }
        }
    }

    pub fn focused_window() -> Option<AnyWindowHandle> {
        unsafe {
            let app = NSApplication::sharedApplication(nil);
            let key_window: id = msg_send![app, keyWindow];
            if key_window.is_null() {
                return None;
            }

            if is_gpui_window(key_window) {
                let handle = get_window_state(&*key_window).lock().handle;
                Some(handle)
            } else {
                None
            }
        }
    }

    pub fn hovered_window() -> Option<AnyWindowHandle> {
        unsafe {
            hovered_gpui_native_window().map(|window| get_window_state(&*window).lock().handle)
        }
    }

    pub fn ordered_windows() -> Vec<AnyWindowHandle> {
        unsafe {
            let mut window_handles = Vec::new();
            for window in visible_app_windows_front_to_back() {
                if is_gpui_window(window) {
                    let state = get_window_state(&*window);
                    let state = state.lock();
                    if !state.is_closed() {
                        window_handles.push(state.handle);
                    }
                }
            }

            window_handles
        }
    }

    pub fn get_user_tabbing_preference() -> Option<UserTabbingPreference> {
        unsafe {
            let defaults: id = NSUserDefaults::standardUserDefaults();
            let domain = ns_string("NSGlobalDomain");
            let key = ns_string("AppleWindowTabbingMode");

            let dict: id = msg_send![defaults, persistentDomainForName: domain];
            let value: id = if !dict.is_null() {
                msg_send![dict, objectForKey: key]
            } else {
                nil
            };

            let value_str = if !value.is_null() {
                CStr::from_ptr(NSString::UTF8String(value)).to_string_lossy()
            } else {
                "".into()
            };

            match value_str.as_ref() {
                "manual" => Some(UserTabbingPreference::Never),
                "always" => Some(UserTabbingPreference::Always),
                _ => Some(UserTabbingPreference::InFullScreen),
            }
        }
    }
}

fn ns_rect_contains_point(rect: NSRect, point: NSPoint) -> bool {
    point.x >= rect.origin.x
        && point.x < rect.origin.x + rect.size.width
        && point.y >= rect.origin.y
        && point.y < rect.origin.y + rect.size.height
}

fn subscribe_window_to_display_topology(window_state: &Arc<Mutex<MacWindowState>>) -> Result<()> {
    let (display_topology, observed_generation) = {
        let state = window_state.lock();
        (
            state.display_topology.clone(),
            state.committed_native_observation().topology_generation,
        )
    };
    let weak_window_state = Arc::downgrade(window_state);
    let listener = Arc::new(move |generation| {
        let Some(window_state) = weak_window_state.upgrade() else {
            return;
        };
        request_window_observation_commit(&window_state, generation, None, false);
    }) as Arc<dyn Fn(u64) + Send + Sync>;
    let subscription = display_topology.subscribe_publications(observed_generation, listener)?;
    let mut state = window_state.lock();
    if !state.is_closed() {
        state.display_topology_subscription = Some(subscription);
    }
    Ok(())
}

#[derive(Clone)]
struct MacWindowObservationCommitJob {
    window_state: Weak<Mutex<MacWindowState>>,
    job_epoch: u64,
}

fn enqueue_window_observation_commit_job(job: MacWindowObservationCommitJob, delay: Duration) {
    let queue = DispatchQueue::main();
    if delay.is_zero() {
        queue.exec_async(move || complete_window_observation_commit_job(job));
        return;
    }

    let delay_nanos = i64::try_from(delay.as_nanos())
        .expect("macOS window observation retry delay must fit in dispatch time");
    let when = DispatchTime::NOW.time(delay_nanos);
    let fallback_job = job.clone();
    if let Err(error) = queue.after(when, move || complete_window_observation_commit_job(job)) {
        log::error!("cannot schedule macOS window observation retry: {error:?}");
        queue.exec_async(move || complete_window_observation_commit_job(fallback_job));
    }
}

fn complete_window_observation_commit_job(job: MacWindowObservationCommitJob) {
    let Some(window_state) = job.window_state.upgrade() else {
        return;
    };
    let job_is_current = {
        let state = window_state.lock();
        !state.is_closed()
            && state
                .observation_commit
                .target_for_job(job.job_epoch)
                .is_some()
    };
    if !job_is_current {
        return;
    }

    complete_window_observation_commit(&window_state, job.job_epoch);
}

fn request_window_observation_commit(
    window_state: &Arc<Mutex<MacWindowState>>,
    target_generation: u64,
    event: Option<MacWindowPendingObservationEvent>,
    attempt_immediately: bool,
) {
    let job_epoch = {
        let mut state = window_state.lock();
        if state.is_closed() {
            return;
        }
        state.observation_commit.request(target_generation, event)
    };
    let Some(job_epoch) = job_epoch else {
        return;
    };

    if attempt_immediately {
        complete_window_observation_commit(window_state, job_epoch);
    } else {
        enqueue_window_observation_commit_job(
            MacWindowObservationCommitJob {
                window_state: Arc::downgrade(window_state),
                job_epoch,
            },
            Duration::ZERO,
        );
    }
}

fn request_window_observation_commit_for_retained_topology(
    window_state: &Arc<Mutex<MacWindowState>>,
    event: Option<MacWindowPendingObservationEvent>,
    attempt_immediately: bool,
) {
    let target_generation = {
        let state = window_state.lock();
        if state.is_closed() {
            return;
        }
        state.observation_target_generation()
    };
    request_window_observation_commit(window_state, target_generation, event, attempt_immediately);
}

enum MacWindowObservationCommitOutcome {
    Committed { should_drain_effects: bool },
    Retry { delay: Duration },
    Paused,
}

fn complete_window_observation_commit(window_state: &Arc<Mutex<MacWindowState>>, job_epoch: u64) {
    let outcome = {
        let mut state = window_state.lock();
        if state.is_closed() {
            state.observation_commit.cancel();
            return;
        }
        let Some(target_generation) = state.observation_commit.target_for_job(job_epoch) else {
            return;
        };

        match state.native_observation_for_generation(target_generation) {
            Ok(observation) => {
                let Some(events) = state
                    .observation_commit
                    .commit(job_epoch, observation.topology_generation)
                else {
                    return;
                };
                // Commit ownership and effect publication are atomic under the window lock, so a
                // newer native event cannot publish effects ahead of this committed batch.
                MacWindowObservationCommitOutcome::Committed {
                    should_drain_effects: enqueue_window_observation_effect_batches(
                        &mut state,
                        events.into_effect_batches(observation),
                    ),
                }
            }
            Err(MacWindowNativeObservationRefreshFailure::UnstableNativeSample) => {
                // A delay only spaces repeated native reads; it never decides success or failure.
                let Some(delay) = state.observation_commit.retry_delay(job_epoch) else {
                    return;
                };
                MacWindowObservationCommitOutcome::Retry { delay }
            }
            Err(
                MacWindowNativeObservationRefreshFailure::AwaitTopologyPublication
                | MacWindowNativeObservationRefreshFailure::AwaitFullscreenTerminal,
            ) => {
                state.observation_commit.pause(job_epoch);
                MacWindowObservationCommitOutcome::Paused
            }
        }
    };

    match outcome {
        MacWindowObservationCommitOutcome::Committed {
            should_drain_effects,
        } => {
            if should_drain_effects {
                drain_window_observation_effects(window_state);
            }
        }
        MacWindowObservationCommitOutcome::Retry { delay } => {
            enqueue_window_observation_commit_job(
                MacWindowObservationCommitJob {
                    window_state: Arc::downgrade(window_state),
                    job_epoch,
                },
                delay,
            );
        }
        MacWindowObservationCommitOutcome::Paused => {}
    }
}

fn enqueue_window_observation_effect_batches(
    state: &mut MacWindowState,
    batches: impl IntoIterator<Item = MacWindowObservationEffectBatch>,
) -> bool {
    let mut should_drain = false;
    for batch in batches {
        should_drain |= state.observation_effects.enqueue(batch);
    }
    should_drain
}

fn drain_window_observation_effects(window_state: &Arc<Mutex<MacWindowState>>) {
    let mut panic_boundary = MacWindowCallbackPanicBoundary::default();
    loop {
        let batch = {
            let mut state = window_state.lock();
            if state.is_closed() {
                state.observation_effects.cancel();
                None
            } else {
                state.observation_effects.pop_next()
            }
        };
        let Some(batch) = batch else {
            break;
        };
        let delivery = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            deliver_window_observation_effect_batch(window_state, batch, &mut panic_boundary);
        }));
        if let Err(panic) = delivery {
            // User callbacks are isolated individually below. Retain an unexpected internal panic
            // as well, then continue every older and reentrant batch owned by this serial drain.
            panic_boundary.retain(panic);
        }
    }

    panic_boundary.isolate_at_native_boundary("a macOS window observation effect");
}

fn deliver_window_observation_effect_batch(
    window_state: &Arc<Mutex<MacWindowState>>,
    batch: MacWindowObservationEffectBatch,
    panic_boundary: &mut MacWindowCallbackPanicBoundary,
) {
    let changes = {
        let mut state = window_state.lock();
        if state.is_closed() {
            return;
        }
        let changes = state.apply_native_observation(batch.observation);
        if matches!(
            batch.event.map(|event| event.kind),
            Some(MacWindowObservationEvent::Fullscreen(_) | MacWindowObservationEvent::State(_))
        ) {
            state.move_traffic_light();
        }
        changes
    };

    if changes.renderer_geometry_changed {
        panic_boundary.deliver(|| notify_window_resize(window_state));
    }
    if changes.moved
        || matches!(
            batch.event.map(|event| event.kind),
            Some(MacWindowObservationEvent::Moved)
        )
    {
        panic_boundary.deliver(|| notify_window_moved(window_state));
    }

    let Some(event) = batch.event else {
        if changes.state_changed {
            panic_boundary.deliver(|| notify_window_state_changed(window_state));
        }
        return;
    };
    match event.kind {
        MacWindowObservationEvent::Active(active_event) => {
            deliver_window_active_event(
                window_state,
                active_event,
                batch.observation.is_active,
                event.is_latest_active,
                panic_boundary,
            );
            if changes.state_changed {
                panic_boundary.deliver(|| notify_window_state_changed(window_state));
            }
        }
        MacWindowObservationEvent::Fullscreen(terminal) => {
            if batch.observation.is_fullscreen == terminal.is_fullscreen() {
                panic_boundary.deliver(|| notify_window_state_changed(window_state));
            } else {
                log::warn!(
                    "discarding macOS fullscreen terminal {terminal:?} without a matching complete observation"
                );
                if changes.state_changed {
                    panic_boundary.deliver(|| notify_window_state_changed(window_state));
                }
            }
        }
        MacWindowObservationEvent::State(state_event) => {
            let observation_matches = batch.observation.state_expectation() == state_event.expected;
            let should_notify = match state_event.source {
                MacWindowStateEventSource::Resized => changes.minimized_or_maximized_changed,
                MacWindowStateEventSource::Miniaturized
                | MacWindowStateEventSource::Deminiaturized => observation_matches,
            };
            if should_notify && observation_matches {
                panic_boundary.deliver(|| notify_window_state_changed(window_state));
            } else if !observation_matches {
                log::warn!(
                    "discarding macOS window-state edge {:?} without a matching complete observation",
                    state_event.source
                );
                if changes.state_changed {
                    panic_boundary.deliver(|| notify_window_state_changed(window_state));
                }
            }
        }
        MacWindowObservationEvent::Moved => {
            if changes.state_changed {
                panic_boundary.deliver(|| notify_window_state_changed(window_state));
            }
        }
    }
}

struct MacWindowCallbackCheckout<T, Restore>
where
    Restore: FnOnce(T),
{
    callback: Option<T>,
    restore: Option<Restore>,
}

impl<T, Restore> MacWindowCallbackCheckout<T, Restore>
where
    Restore: FnOnce(T),
{
    fn new(callback: T, restore: Restore) -> Self {
        Self {
            callback: Some(callback),
            restore: Some(restore),
        }
    }

    fn callback(&mut self) -> &mut T {
        self.callback
            .as_mut()
            .expect("checked-out macOS window callback must remain available")
    }
}

impl<T, Restore> Drop for MacWindowCallbackCheckout<T, Restore>
where
    Restore: FnOnce(T),
{
    fn drop(&mut self) {
        if let (Some(callback), Some(restore)) = (self.callback.take(), self.restore.take()) {
            restore(callback);
        }
    }
}

fn restore_callback_if_vacant<T>(slot: &mut Option<T>, callback: T) {
    if slot.is_none() {
        *slot = Some(callback);
    }
}

fn checkout_mac_window_callback<T>(
    window_state: &Arc<Mutex<MacWindowState>>,
    callback: T,
    slot: for<'a> fn(&'a mut MacWindowState) -> &'a mut Option<T>,
) -> MacWindowCallbackCheckout<T, impl FnOnce(T)> {
    let window_state = window_state.clone();
    MacWindowCallbackCheckout::new(callback, move |callback| {
        let mut state = window_state.lock();
        if state.is_closed() {
            return;
        }
        restore_callback_if_vacant(slot(&mut state), callback);
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MacRequestFrameDeliveryMode {
    DisplayTransaction,
    DisplayLinkTick,
}

impl MacRequestFrameDeliveryMode {
    fn uses_display_transaction(self) -> bool {
        matches!(self, Self::DisplayTransaction)
    }
}

fn request_frame_callback_slot(
    state: &mut MacWindowState,
) -> &mut Option<Box<dyn FnMut(RequestFrameOptions)>> {
    &mut state.request_frame_callback
}

fn activate_callback_slot(
    state: &mut MacWindowState,
) -> &mut Option<Box<dyn FnMut(PlatformWindowActiveStatusObservation)>> {
    &mut state.activate_callback
}

fn checkout_mac_request_frame_callback(
    window_state: &Arc<Mutex<MacWindowState>>,
    callback: Box<dyn FnMut(RequestFrameOptions)>,
    mode: MacRequestFrameDeliveryMode,
) -> MacWindowCallbackCheckout<
    Box<dyn FnMut(RequestFrameOptions)>,
    impl FnOnce(Box<dyn FnMut(RequestFrameOptions)>),
> {
    let window_state = window_state.clone();
    MacWindowCallbackCheckout::new(callback, move |callback| {
        let mut state = window_state.lock();
        if state.is_closed() {
            return;
        }
        if mode.uses_display_transaction() {
            state.renderer.set_presents_with_transaction(false);
            state.start_display_link();
        }
        restore_callback_if_vacant(request_frame_callback_slot(&mut state), callback);
    })
}

fn deliver_window_request_frame(
    window_state: &Arc<Mutex<MacWindowState>>,
    mode: MacRequestFrameDeliveryMode,
    panic_boundary: &mut MacWindowCallbackPanicBoundary,
) {
    let callback = {
        let mut state = window_state.lock();
        if state.is_closed() {
            return;
        }
        let Some(callback) = state.request_frame_callback.take() else {
            return;
        };
        if mode.uses_display_transaction() {
            state.renderer.set_presents_with_transaction(true);
            state.stop_display_link();
        }
        callback
    };

    let mut callback = checkout_mac_request_frame_callback(window_state, callback, mode);
    panic_boundary.deliver(|| (callback.callback())(Default::default()));
    panic_boundary.deliver(|| drop(callback));
}

fn resize_callback_slot(
    state: &mut MacWindowState,
) -> &mut Option<Box<dyn FnMut(Size<Pixels>, f32)>> {
    &mut state.resize_callback
}

fn moved_callback_slot(state: &mut MacWindowState) -> &mut Option<Box<dyn FnMut()>> {
    &mut state.moved_callback
}

fn window_state_change_callback_slot(state: &mut MacWindowState) -> &mut Option<Box<dyn FnMut()>> {
    &mut state.window_state_change_callback
}

fn notify_window_resize(window_state: &Arc<Mutex<MacWindowState>>) {
    let (callback, content_size, scale_factor) = {
        let mut state = window_state.lock();
        if state.is_closed() {
            return;
        }
        let Some(callback) = state.resize_callback.take() else {
            return;
        };
        (callback, state.content_size(), state.scale_factor())
    };
    let mut callback = checkout_mac_window_callback(window_state, callback, resize_callback_slot);
    (callback.callback())(content_size, scale_factor);
}

fn notify_window_moved(window_state: &Arc<Mutex<MacWindowState>>) {
    let callback = {
        let mut state = window_state.lock();
        if state.is_closed() {
            return;
        }
        let Some(callback) = state.moved_callback.take() else {
            return;
        };
        callback
    };
    let mut callback = checkout_mac_window_callback(window_state, callback, moved_callback_slot);
    (callback.callback())();
}

fn notify_window_state_changed(window_state: &Arc<Mutex<MacWindowState>>) {
    let callback = {
        let mut state = window_state.lock();
        if state.is_closed() {
            return;
        }
        let Some(callback) = state.window_state_change_callback.take() else {
            return;
        };
        callback
    };
    let mut callback =
        checkout_mac_window_callback(window_state, callback, window_state_change_callback_slot);
    (callback.callback())();
}

impl Drop for MacWindow {
    fn drop(&mut self) {
        self.dispose_native_window(false);
    }
}

/// Calls `f` if the window is not closed.
///
/// This should be used when spawning foreground tasks interacting with the
/// window, as some messages will end hard faulting if dispatched to no longer
/// valid window handles.
fn if_window_not_closed(closed: Arc<AtomicBool>, f: impl FnOnce()) {
    if !closed.load(Ordering::Acquire) {
        f();
    }
}

fn complete_mac_initial_presentation(
    window_state: &Arc<Mutex<MacWindowState>>,
    activate: bool,
) -> bool {
    let (handle, native_window, presentation, activate, transient_for, window_registry) = {
        let mut state = window_state.lock();
        if state.is_closed() {
            return false;
        }
        if state.initial_presentation.completed {
            return true;
        }
        state.initial_presentation.completed = true;
        (
            state.handle,
            state.native_window,
            state.initial_presentation,
            activate
                && !state.interaction_is_quiesced()
                && state.activation_policy.accepts_activation,
            state.creation_facts.transient_for,
            state.window_registry.clone(),
        )
    };

    if !presentation.show {
        return true;
    }

    let presentation_transaction = match MacTransientPresentationTransaction::begin(
        window_state,
        native_window,
        handle,
        transient_for,
        &window_registry,
        MacTransientPresentationProof::Visible,
    ) {
        Ok(attachment) => attachment,
        Err(_) => {
            let mut state = window_state.lock();
            if !state.is_closed() && state.native_window == native_window {
                state.initial_presentation.completed = false;
            }
            return false;
        }
    };

    unsafe {
        let app: id = NSApplication::sharedApplication(nil);
        let main_window: id = msg_send![app, mainWindow];
        let mut added_to_fullscreen_tab = false;

        if presentation.should_apply_automatic_tabbing()
            && !main_window.is_null()
            && main_window != native_window
        {
            let main_window_is_fullscreen = main_window
                .styleMask()
                .contains(NSWindowStyleMask::NSFullScreenWindowMask);
            let user_tabbing_preference = MacWindow::get_user_tabbing_preference()
                .unwrap_or(UserTabbingPreference::InFullScreen);
            let should_add_as_tab = user_tabbing_preference == UserTabbingPreference::Always
                || user_tabbing_preference == UserTabbingPreference::InFullScreen
                    && main_window_is_fullscreen;

            if should_add_as_tab {
                let main_window_can_tab: BOOL =
                    msg_send![main_window, respondsToSelector: sel!(addTabbedWindow:ordered:)];
                let main_window_visible: BOOL = msg_send![main_window, isVisible];

                let main_window_admits_tabbing = !is_gpui_window(main_window)
                    || get_window_state(&*main_window)
                        .try_lock()
                        .is_some_and(|state| state.admits_native_tabbing());

                if main_window_can_tab == YES
                    && main_window_visible == YES
                    && main_window_admits_tabbing
                {
                    let _: () = msg_send![
                        main_window,
                        addTabbedWindow: native_window
                        ordered: NSWindowOrderingMode::NSWindowAbove
                    ];
                    added_to_fullscreen_tab = main_window_is_fullscreen;
                }
            }
        }

        if activate {
            let _: () = msg_send![native_window, makeKeyAndOrderFront: nil];
        } else if !added_to_fullscreen_tab {
            let _: () = msg_send![native_window, orderFront: nil];
        }

        if !presentation_transaction.commit() {
            let mut state = window_state.lock();
            if !state.is_closed() && state.native_window == native_window {
                state.initial_presentation.completed = false;
            }
            return false;
        }

        if presentation.state == MacWindowCreationState::Fullscreen {
            let _: () = msg_send![native_window, toggleFullScreen: nil];
        }
    }
    true
}

fn activate_mac_window(window_state: &Arc<Mutex<MacWindowState>>) -> bool {
    let (handle, window, transient_for, window_registry) = {
        let state = window_state.lock();
        if state.is_closed()
            || state.interaction_is_quiesced()
            || !state.activation_policy.accepts_activation
        {
            return false;
        }
        (
            state.handle,
            state.native_window,
            state.creation_facts.transient_for,
            state.window_registry.clone(),
        )
    };

    let activation_transaction = match MacTransientPresentationTransaction::begin(
        window_state,
        window,
        handle,
        transient_for,
        &window_registry,
        MacTransientPresentationProof::Visible,
    ) {
        Ok(transaction) => transaction,
        Err(_) => return false,
    };
    unsafe {
        let _: () = msg_send![window, makeKeyAndOrderFront: nil];
    }
    activation_transaction.commit()
}

fn reorder_mac_transient_after_native_activation(
    window_state: &Arc<Mutex<MacWindowState>>,
) -> bool {
    let (handle, native_window, transient_for, window_registry, ordering_in_progress) = {
        let state = window_state.lock();
        (
            state.handle,
            state.native_window,
            state.creation_facts.transient_for,
            state.window_registry.clone(),
            state.transient_ordering_in_progress,
        )
    };
    if transient_for.is_none() || ordering_in_progress {
        return true;
    }
    let transaction = match MacTransientPresentationTransaction::begin(
        window_state,
        native_window,
        handle,
        transient_for,
        &window_registry,
        MacTransientPresentationProof::VisibleAndKey,
    ) {
        Ok(attachment) => attachment,
        Err(_) => return false,
    };
    transaction.commit()
}

fn start_mac_window_move(window_state: &Arc<Mutex<MacWindowState>>) -> bool {
    let window = {
        let state = window_state.lock();
        if state.is_closed() || state.interaction_is_quiesced() {
            return false;
        }
        state.native_window
    };

    unsafe {
        let app = NSApplication::sharedApplication(nil);
        let event: id = msg_send![app, currentEvent];
        let _: () = msg_send![window, performWindowDragWithEvent: event];
    }
    true
}

fn dispatch_mac_window_command(
    window_state: &Weak<Mutex<MacWindowState>>,
    command: PlatformWindowCommand,
) -> PlatformWindowCommandOutcome {
    let Some(window_state) = window_state.upgrade() else {
        return PlatformWindowCommandOutcome::WindowClosed;
    };
    if window_state.lock().is_closed() {
        return PlatformWindowCommandOutcome::WindowClosed;
    }

    match command {
        PlatformWindowCommand::CompleteInitialPresentation { activate } => {
            if complete_mac_initial_presentation(&window_state, activate) {
                PlatformWindowCommandOutcome::Accepted
            } else {
                rejected_or_closed_mac_window_command(&window_state)
            }
        }
        PlatformWindowCommand::RevealDeferredInitialPresentation { .. } => {
            PlatformWindowCommandOutcome::Rejected
        }
        PlatformWindowCommand::Activate { .. } => {
            if activate_mac_window(&window_state) {
                PlatformWindowCommandOutcome::Accepted
            } else {
                rejected_or_closed_mac_window_command(&window_state)
            }
        }
        PlatformWindowCommand::StartWindowMove => {
            if start_mac_window_move(&window_state) {
                PlatformWindowCommandOutcome::Accepted
            } else {
                rejected_or_closed_mac_window_command(&window_state)
            }
        }
        PlatformWindowCommand::ShowWindowMenu(_) | PlatformWindowCommand::StartWindowResize(_) => {
            PlatformWindowCommandOutcome::Rejected
        }
    }
}

fn rejected_or_closed_mac_window_command(
    window_state: &Arc<Mutex<MacWindowState>>,
) -> PlatformWindowCommandOutcome {
    if window_state.lock().is_closed() {
        PlatformWindowCommandOutcome::WindowClosed
    } else {
        PlatformWindowCommandOutcome::Rejected
    }
}

impl PlatformWindow for MacWindow {
    fn command_dispatcher(&self) -> PlatformWindowCommandDispatcher {
        let window_state = Arc::downgrade(&self.0);
        PlatformWindowCommandDispatcher::new(move |command| {
            dispatch_mac_window_command(&window_state, command)
        })
    }

    fn interaction_quiescence(&self) -> PlatformWindowInteractionQuiescence {
        let target = {
            let state = self.0.lock();
            unsafe {
                Rc::new(MacWindowInteractionQuiescenceTarget::new(
                    state.native_window,
                    Arc::downgrade(&self.0),
                    state.interaction_quiesced.clone(),
                ))
            }
        };
        PlatformWindowInteractionQuiescence::new(move || target.revoke())
    }

    fn prepare_presentation_shutdown(
        &self,
        shutdown: WindowPresentationShutdownTicket,
    ) -> PreparedPlatformPresentationShutdown {
        // The prepared shutdown owns the native state until it has detached the
        // AppKit layer and drained the last Metal command buffer. A weak
        // reference would allow a shutdown fallback to acknowledge quiescence
        // without proving either condition.
        let window_state = self.0.clone();
        let shutdown = self
            .1
            .lock()
            .claim(shutdown)
            .expect("a platform-window shutdown ticket must match its native window");
        PreparedPlatformPresentationShutdown::new(shutdown, move |shutdown| {
            let native_view = {
                let Some(mut state) = window_state.try_lock() else {
                    return PlatformPresentationShutdownOutcome::Rejected;
                };

                if shutdown.snapshot().window_id() != state.handle.window_id() {
                    return PlatformPresentationShutdownOutcome::Rejected;
                }

                if state.renderer.is_quiesced_for(shutdown) {
                    return PlatformPresentationShutdownOutcome::Quiesced;
                }
                if !state.renderer.reserve_surface_quiescence(shutdown) {
                    return PlatformPresentationShutdownOutcome::Rejected;
                }

                state.stop_display_link();
                state.request_frame_callback = None;
                state.native_view
            };

            // AppKit may synchronously ask the view to make its backing layer. That callback
            // needs the same window-state mutex, so no Objective-C message is sent while it is
            // held.
            unsafe {
                let _: () = msg_send![native_view.as_ptr(), setLayer: nil];
            }

            let Some(mut state) = window_state.try_lock() else {
                return PlatformPresentationShutdownOutcome::Rejected;
            };
            if shutdown.snapshot().window_id() != state.handle.window_id() {
                return PlatformPresentationShutdownOutcome::Rejected;
            }
            if state.renderer.finish_surface_quiescence(shutdown) {
                PlatformPresentationShutdownOutcome::Quiesced
            } else {
                PlatformPresentationShutdownOutcome::Rejected
            }
        })
    }

    fn retire_native_window(
        &self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> PlatformNativeWindowRetirementOutcome {
        let Some(state) = self.0.try_lock() else {
            return PlatformNativeWindowRetirementOutcome::Rejected;
        };
        if shutdown.snapshot().window_id() != state.handle.window_id() {
            PlatformNativeWindowRetirementOutcome::Rejected
        } else if !state.renderer.is_quiesced_for(shutdown) {
            PlatformNativeWindowRetirementOutcome::Rejected
        } else if state.is_closed() {
            PlatformNativeWindowRetirementOutcome::NativeWindowTerminal
        } else {
            PlatformNativeWindowRetirementOutcome::Accepted
        }
    }

    fn bounds(&self) -> Bounds<Pixels> {
        self.0.as_ref().lock().bounds()
    }

    fn map_window(&mut self) -> anyhow::Result<()> {
        let mut state = self.0.lock();
        if !state.initial_presentation.mapped {
            state.initial_presentation.mapped = true;
        }
        Ok(())
    }

    fn window_bounds(&self) -> WindowBounds {
        self.0.as_ref().lock().window_bounds()
    }

    fn is_maximized(&self) -> bool {
        self.0.as_ref().lock().is_maximized()
    }

    fn is_minimized(&self) -> bool {
        self.0
            .as_ref()
            .lock()
            .committed_native_observation()
            .is_minimized
    }

    fn accepts_pointer_input(&self) -> bool {
        self.0.as_ref().lock().accepts_pointer_input
    }

    fn creation_facts(&self) -> WindowCreationFacts {
        self.0.as_ref().lock().creation_facts.clone()
    }

    fn is_visible(&self) -> bool {
        unsafe { self.0.as_ref().lock().native_window.isVisible() == YES }
    }

    fn platform_facts(&self) -> open_gpui::WindowPlatformFacts {
        let state = self.0.as_ref().lock();
        let observation = state.committed_native_observation();
        let window_bounds = state.window_bounds_from_client_bounds(
            observation.bounds,
            observation.is_fullscreen,
            observation.is_maximized,
        );
        let activation_policy = if state.interaction_is_quiesced() {
            WindowActivationPolicy {
                accepts_activation: false,
                focus_on_click: false,
            }
        } else {
            state.activation_policy
        };
        open_gpui::WindowPlatformFacts {
            bounds: observation.bounds,
            coordinate_space: open_gpui::WindowCoordinateSpace::GlobalScreen,
            physical_geometry: None,
            window_bounds,
            inner_window_bounds: window_bounds,
            content_size: observation.bounds.size,
            scale_factor: observation.display.scale_factor(),
            display_id: Some(observation.display.id()),
            is_minimized: observation.is_minimized,
            is_maximized: observation.is_maximized,
            is_fullscreen: observation.is_fullscreen,
            accepts_pointer_input: state.accepts_pointer_input,
            accepts_activation: activation_policy.accepts_activation,
            focus_on_click: activation_policy.focus_on_click,
            background_appearance: state.background_appearance,
            topmost: state.topmost,
            taskbar_visible: state.taskbar_visible,
            is_active: !state.interaction_is_quiesced() && observation.is_active,
        }
    }

    fn content_size(&self) -> Size<Pixels> {
        self.0.as_ref().lock().content_size()
    }

    fn merge_all_windows(&self) {
        let (window_state, executor) = {
            let state = self.0.lock();
            if !state.admits_native_tabbing() {
                return;
            }
            (Arc::downgrade(&self.0), state.foreground_executor.clone())
        };
        executor
            .spawn(async move {
                let Some(window_state) = window_state.upgrade() else {
                    return;
                };
                let native_window = {
                    let state = window_state.lock();
                    if !state.admits_native_tabbing() {
                        return;
                    }
                    state.native_window
                };
                unsafe {
                    let _: () = msg_send![native_window, mergeAllWindows:nil];
                }
            })
            .detach();
    }

    fn move_tab_to_new_window(&self) {
        let (window_state, executor) = {
            let state = self.0.lock();
            if !state.admits_native_tabbing() {
                return;
            }
            (Arc::downgrade(&self.0), state.foreground_executor.clone())
        };
        executor
            .spawn(async move {
                let Some(window_state) = window_state.upgrade() else {
                    return;
                };
                let native_window = {
                    let state = window_state.lock();
                    if !state.admits_native_tabbing() {
                        return;
                    }
                    state.native_window
                };
                unsafe {
                    let _: () = msg_send![native_window, moveTabToNewWindow:nil];
                }

                let state = window_state.lock();
                if !state.admits_native_tabbing() {
                    return;
                }
                let native_window = state.native_window;
                drop(state);
                unsafe {
                    let _: () = msg_send![native_window, makeKeyAndOrderFront: nil];
                }
            })
            .detach();
    }

    fn toggle_window_tab_overview(&self) {
        let native_window = {
            let state = self.0.lock();
            if !state.admits_native_tabbing() {
                return;
            }
            state.native_window
        };
        unsafe {
            let _: () = msg_send![native_window, toggleTabOverview:nil];
        }
    }

    fn set_tabbing_identifier(&self, tabbing_identifier: Option<String>) {
        let native_window = {
            let state = self.0.lock();
            if !state.admits_native_tabbing() {
                return;
            }
            state.native_window
        };
        unsafe {
            if tabbing_identifier.is_some() {
                let () = msg_send![class!(NSWindow), setAllowsAutomaticWindowTabbing: YES];
            } else {
                let () = msg_send![class!(NSWindow), setAllowsAutomaticWindowTabbing: NO];
            }

            if let Some(tabbing_identifier) = tabbing_identifier {
                let tabbing_id = ns_string(tabbing_identifier.as_str());
                let _: () = msg_send![native_window, setTabbingIdentifier: tabbing_id];
            } else {
                let _: () = msg_send![native_window, setTabbingIdentifier:nil];
            }
        }
    }

    fn set_traffic_light_position(&self, position: Point<Pixels>) {
        let mut state = self.0.lock();
        state.traffic_light_position = Some(position);
        state.move_traffic_light();
    }

    fn scale_factor(&self) -> f32 {
        self.0.as_ref().lock().scale_factor()
    }

    fn appearance(&self) -> WindowAppearance {
        unsafe {
            let appearance: id = msg_send![self.0.lock().native_window, effectiveAppearance];
            crate::window_appearance::window_appearance_from_native(appearance)
        }
    }

    fn display(&self) -> Option<Rc<dyn PlatformDisplay>> {
        let state = self.0.lock();
        let display = state.committed_native_observation().display;
        Some(Rc::new(display) as Rc<dyn PlatformDisplay>)
    }

    fn mouse_position(&self) -> Point<Pixels> {
        let position = unsafe {
            self.0
                .lock()
                .native_window
                .mouseLocationOutsideOfEventStream()
        };
        convert_mouse_position(position, self.content_size().height)
    }

    fn set_cursor_style(&self, style: CursorStyle) {
        let native_window = self.0.lock().native_window;
        unsafe {
            set_native_window_cursor_style(native_window, style);
        }
    }

    fn modifiers(&self) -> Modifiers {
        unsafe {
            let modifiers: NSEventModifierFlags = msg_send![class!(NSEvent), modifierFlags];

            let control = modifiers.contains(NSEventModifierFlags::NSControlKeyMask);
            let alt = modifiers.contains(NSEventModifierFlags::NSAlternateKeyMask);
            let shift = modifiers.contains(NSEventModifierFlags::NSShiftKeyMask);
            let command = modifiers.contains(NSEventModifierFlags::NSCommandKeyMask);
            let function = modifiers.contains(NSEventModifierFlags::NSFunctionKeyMask);

            Modifiers {
                control,
                alt,
                shift,
                platform: command,
                function,
            }
        }
    }

    fn capslock(&self) -> Capslock {
        unsafe {
            let modifiers: NSEventModifierFlags = msg_send![class!(NSEvent), modifierFlags];

            Capslock {
                on: modifiers.contains(NSEventModifierFlags::NSAlphaShiftKeyMask),
            }
        }
    }

    fn set_input_handler(&mut self, input_handler: PlatformInputHandler) {
        let input_handler_slot = {
            let lock = self.0.as_ref().lock();
            lock.input_handler.clone()
        };
        input_handler_slot.set(input_handler);
    }

    fn take_input_handler(&mut self) -> Option<PlatformInputHandler> {
        let input_handler_slot = {
            let lock = self.0.as_ref().lock();
            lock.input_handler.clone()
        };
        input_handler_slot.take()
    }

    fn prompt(
        &self,
        level: PromptLevel,
        msg: &str,
        detail: Option<&str>,
        answers: &[PromptButton],
    ) -> Option<oneshot::Receiver<usize>> {
        // NSAlert's first button keeps Return and Cancel keeps Escape, but the keyboard
        // focus (and therefore Space) defaults to Cancel, leaving the middle button of
        // prompts like "Save / Don't Save / Cancel" unreachable from the keyboard. Move
        // the initial focus onto the last non-cancel, non-default button instead.
        let initial_focus_ix = answers
            .iter()
            .enumerate()
            .rev()
            .find(|(_, label)| !label.is_cancel())
            .map(|(ix, _)| ix)
            .filter(|&ix| ix > 0);

        unsafe {
            let alert: id = msg_send![class!(NSAlert), alloc];
            let alert: id = msg_send![alert, init];
            let alert_style = match level {
                PromptLevel::Info => 1,
                PromptLevel::Warning => 0,
                PromptLevel::Critical => 2,
            };
            let _: () = msg_send![alert, setAlertStyle: alert_style];
            let _: () = msg_send![alert, setMessageText: ns_string(msg)];
            if let Some(detail) = detail {
                let _: () = msg_send![alert, setInformativeText: ns_string(detail)];
            }

            let mut initial_focus_button: Option<id> = None;
            for (ix, answer) in answers.iter().enumerate() {
                let button: id = msg_send![alert, addButtonWithTitle: ns_string(answer.label())];
                let _: () = msg_send![button, setTag: ix as NSInteger];

                if answer.is_cancel() {
                    if let Some(key) = std::char::from_u32(crate::events::ESCAPE_KEY as u32) {
                        let _: () =
                            msg_send![button, setKeyEquivalent: ns_string(&key.to_string())];
                    }
                } else if Some(ix) == initial_focus_ix {
                    initial_focus_button = Some(button);
                }
            }

            if let Some(button) = initial_focus_button {
                let alert_window: id = msg_send![alert, window];
                let _: () = msg_send![alert_window, setInitialFirstResponder: button];
            }

            let (done_tx, done_rx) = oneshot::channel();
            let done_tx = Cell::new(Some(done_tx));
            let block: RcBlock<dyn Fn(NSInteger)> = RcBlock::new(move |answer: NSInteger| {
                let _: () = msg_send![alert, release];
                if let Some(done_tx) = done_tx.take() {
                    let _ = done_tx.send(answer.try_into().unwrap());
                }
            });
            let lock = self.0.lock();
            let native_window = lock.native_window;
            let closed = lock.closed.clone();
            let executor = lock.foreground_executor.clone();
            executor
                .spawn(async move {
                    if !closed.load(Ordering::Acquire) {
                        let _: () = msg_send![
                            alert,
                            beginSheetModalForWindow: native_window
                            completionHandler: &*block
                        ];
                    } else {
                        let _: () = msg_send![alert, release];
                    }
                })
                .detach();

            Some(done_rx)
        }
    }

    fn is_active(&self) -> bool {
        let state = self.0.lock();
        !state.interaction_is_quiesced() && state.committed_native_observation().is_active
    }

    // is_hovered is unused on macOS. See Window::is_window_hovered.
    fn is_hovered(&self) -> bool {
        false
    }

    fn set_title(&mut self, title: &str) {
        unsafe {
            let app = NSApplication::sharedApplication(nil);
            let window = self.0.lock().native_window;
            let title = ns_string(title);
            let _: () = msg_send![app, changeWindowsItem:window title:title filename:false];
            let _: () = msg_send![window, setTitle: title];
            self.0.lock().move_traffic_light();
        }
    }

    fn get_title(&self) -> String {
        unsafe {
            let title: id = msg_send![self.0.lock().native_window, title];
            if title.is_null() {
                "".to_string()
            } else {
                title.to_str().to_string()
            }
        }
    }

    fn set_app_id(&mut self, _app_id: &str) {}

    fn set_background_appearance(&self, background_appearance: WindowBackgroundAppearance) {
        let projection = MacWindowBackgroundProjection::new(background_appearance);
        let mut this = self.0.as_ref().lock();
        this.background_appearance = projection.appearance;

        this.renderer
            .update_transparency(projection.renderer_transparent);

        unsafe {
            this.native_window
                .setOpaque_(projection.native_opaque as BOOL);
            // Not using `+[NSColor clearColor]` to avoid broken shadow.
            let background_color = NSColor::colorWithSRGBRed_green_blue_alpha_(
                nil,
                0f64,
                0f64,
                0f64,
                projection.background_alpha,
            );
            this.native_window.setBackgroundColor_(background_color);

            if NSAppKitVersionNumber < NSAppKitVersionNumber12_0 {
                // Whether `-[NSVisualEffectView respondsToSelector:@selector(_updateProxyLayer)]`.
                // On macOS Catalina/Big Sur `NSVisualEffectView` doesn’t own concrete sublayers
                // but uses a `CAProxyLayer`. Use the legacy WindowServer API.
                let blur_radius = if projection.blur_enabled { 80 } else { 0 };

                let window_number = this.native_window.windowNumber();
                CGSSetWindowBackgroundBlurRadius(CGSMainConnectionID(), window_number, blur_radius);
            } else {
                // On newer macOS `NSVisualEffectView` manages the effect layer directly. Using it
                // could have a better performance (it downsamples the backdrop) and more control
                // over the effect layer.
                if !projection.blur_enabled {
                    if let Some(blur_view) = this.blurred_view {
                        NSView::removeFromSuperview(blur_view);
                        this.blurred_view = None;
                    }
                } else if this.blurred_view.is_none() {
                    let content_view = this.native_window.contentView();
                    let frame = NSView::bounds(content_view);
                    let mut blur_view: id = msg_send![BLURRED_VIEW_CLASS, alloc];
                    blur_view = NSView::initWithFrame_(blur_view, frame);
                    blur_view.setAutoresizingMask_(NSViewWidthSizable | NSViewHeightSizable);

                    let _: () = msg_send![
                        content_view,
                        addSubview: blur_view
                        positioned: NSWindowOrderingMode::NSWindowBelow
                        relativeTo: nil
                    ];
                    this.blurred_view = Some(blur_view.autorelease());
                }
            }
        }
    }

    fn background_appearance(&self) -> WindowBackgroundAppearance {
        self.0.as_ref().lock().background_appearance
    }

    fn is_subpixel_rendering_supported(&self) -> bool {
        false
    }

    fn set_edited(&mut self, edited: bool) {
        unsafe {
            let window = self.0.lock().native_window;
            msg_send![window, setDocumentEdited: edited as BOOL]
        }

        // Changing the document edited state resets the traffic light position,
        // so we have to move it again.
        self.0.lock().move_traffic_light();
    }

    fn set_document_path(&self, path: Option<&std::path::Path>) {
        unsafe {
            let window = self.0.lock().native_window;
            let filename = path.map_or(ns_string(""), |p| ns_string(&p.to_string_lossy()));
            let _: () = msg_send![window, setRepresentedFilename: filename];
        }

        // Changing the document path state resets the traffic light position,
        // so we have to move it again.
        self.0.lock().move_traffic_light();
    }

    fn show_character_palette(&self) {
        let this = self.0.lock();
        let window = this.native_window;
        this.foreground_executor
            .spawn(async move {
                unsafe {
                    let app = NSApplication::sharedApplication(nil);
                    let _: () = msg_send![app, orderFrontCharacterPalette: window];
                }
            })
            .detach();
    }

    fn is_fullscreen(&self) -> bool {
        self.0.lock().is_fullscreen()
    }

    fn on_request_frame(&self, callback: Box<dyn FnMut(RequestFrameOptions)>) {
        let mut lock = self.0.as_ref().lock();
        if !lock.is_closed() {
            lock.request_frame_callback = Some(callback);
        }
    }

    fn on_input(&self, callback: PlatformInputCallback) {
        let event_callback = {
            let lock = self.0.as_ref().lock();
            if lock.is_closed() {
                return;
            }
            lock.event_callback.clone()
        };
        event_callback.set(callback);
    }

    fn on_active_status_change(
        &self,
        callback: Box<dyn FnMut(PlatformWindowActiveStatusObservation)>,
    ) {
        let mut lock = self.0.as_ref().lock();
        if !lock.is_closed() {
            lock.activate_callback = Some(callback);
        }
    }

    fn on_hover_status_change(&self, _: Box<dyn FnMut(bool)>) {}

    fn on_resize(&self, callback: Box<dyn FnMut(Size<Pixels>, f32)>) {
        let mut lock = self.0.as_ref().lock();
        if !lock.is_closed() {
            lock.resize_callback = Some(callback);
        }
    }

    fn on_moved(&self, callback: Box<dyn FnMut()>) {
        let mut lock = self.0.as_ref().lock();
        if !lock.is_closed() {
            lock.moved_callback = Some(callback);
        }
    }

    fn on_window_state_change(&self, callback: Box<dyn FnMut()>) {
        let mut lock = self.0.as_ref().lock();
        if !lock.is_closed() {
            lock.window_state_change_callback = Some(callback);
        }
    }

    fn on_should_close(&self, callback: Box<dyn FnMut() -> bool>) {
        let mut lock = self.0.as_ref().lock();
        if !lock.is_closed() {
            lock.should_close_callback = Some(callback);
        }
    }

    fn on_close(&self, callback: Box<dyn FnOnce()>) {
        self.0.as_ref().lock().close_callback = Some(callback);
    }

    fn on_hit_test_window_control(&self, _callback: Box<dyn FnMut() -> Option<WindowControlArea>>) {
    }

    fn on_appearance_changed(&self, callback: Box<dyn FnMut()>) {
        let mut lock = self.0.lock();
        if !lock.is_closed() {
            lock.appearance_changed_callback = Some(callback);
        }
    }

    fn tabbed_windows(&self) -> Option<Vec<SystemWindowTab>> {
        unsafe {
            let windows: id = msg_send![self.0.lock().native_window, tabbedWindows];
            if windows.is_null() {
                return None;
            }

            let count: NSUInteger = msg_send![windows, count];
            let mut result = Vec::new();
            for i in 0..count {
                let window: id = msg_send![windows, objectAtIndex:i];
                if msg_send![window, isKindOfClass: WINDOW_CLASS] {
                    let handle = get_window_state(&*window).lock().handle;
                    let title: id = msg_send![window, title];
                    let title = SharedString::from(title.to_str().to_string());

                    result.push(SystemWindowTab::new(title, handle));
                }
            }

            Some(result)
        }
    }

    fn tab_bar_visible(&self) -> bool {
        unsafe {
            let tab_group: id = msg_send![self.0.lock().native_window, tabGroup];
            if tab_group.is_null() {
                false
            } else {
                let tab_bar_visible: BOOL = msg_send![tab_group, isTabBarVisible];
                tab_bar_visible == YES
            }
        }
    }

    fn on_move_tab_to_new_window(&self, callback: Box<dyn FnMut()>) {
        let mut lock = self.0.as_ref().lock();
        if !lock.is_closed() {
            lock.move_tab_to_new_window_callback = Some(callback);
        }
    }

    fn on_merge_all_windows(&self, callback: Box<dyn FnMut()>) {
        let mut lock = self.0.as_ref().lock();
        if !lock.is_closed() {
            lock.merge_all_windows_callback = Some(callback);
        }
    }

    fn on_select_next_tab(&self, callback: Box<dyn FnMut()>) {
        let mut lock = self.0.as_ref().lock();
        if !lock.is_closed() {
            lock.select_next_tab_callback = Some(callback);
        }
    }

    fn on_select_previous_tab(&self, callback: Box<dyn FnMut()>) {
        let mut lock = self.0.as_ref().lock();
        if !lock.is_closed() {
            lock.select_previous_tab_callback = Some(callback);
        }
    }

    fn on_toggle_tab_bar(&self, callback: Box<dyn FnMut()>) {
        let mut lock = self.0.as_ref().lock();
        if !lock.is_closed() {
            lock.toggle_tab_bar_callback = Some(callback);
        }
    }

    fn draw(&self, scene: &open_gpui::Scene) -> PlatformWindowPresentOutcome {
        let mut this = self.0.lock();
        let visible = unsafe {
            this.native_window
                .occlusionState()
                .contains(NSWindowOcclusionState::NSWindowOcclusionStateVisible)
        };
        // AppKit can block nextDrawable for roughly one second while a window is fully occluded.
        // Permit the first attempt to avoid initial flicker, then wait for the occlusion callback
        // to restart frame production once the native window becomes visible again.
        if should_defer_occluded_draw(this.attempted_window_draw, visible) {
            return PlatformWindowPresentOutcome::Deferred;
        }
        this.attempted_window_draw = true;
        this.renderer.draw(scene)
    }

    fn sprite_atlas(&self) -> Arc<dyn PlatformAtlas> {
        self.0.lock().renderer.sprite_atlas().clone()
    }

    fn gpu_specs(&self) -> Option<open_gpui::GpuSpecs> {
        None
    }

    fn update_ime_position(&self, _bounds: Bounds<Pixels>) {
        let executor = self.0.lock().foreground_executor.clone();
        executor
            .spawn(async move {
                unsafe {
                    let input_context: id =
                        msg_send![class!(NSTextInputContext), currentInputContext];
                    if input_context.is_null() {
                        return;
                    }
                    let _: () = msg_send![input_context, invalidateCharacterCoordinates];
                }
            })
            .detach()
    }

    fn titlebar_double_click(&self) {
        let this = self.0.lock();
        let window = this.native_window;
        let closed = this.closed.clone();
        this.foreground_executor
            .spawn(async move {
                if_window_not_closed(closed, || {
                    unsafe {
                        let defaults: id = NSUserDefaults::standardUserDefaults();
                        let domain = ns_string("NSGlobalDomain");
                        let key = ns_string("AppleActionOnDoubleClick");

                        let dict: id = msg_send![defaults, persistentDomainForName: domain];
                        let action: id = if !dict.is_null() {
                            msg_send![dict, objectForKey: key]
                        } else {
                            nil
                        };

                        let action_str = if !action.is_null() {
                            CStr::from_ptr(NSString::UTF8String(action)).to_string_lossy()
                        } else {
                            "".into()
                        };

                        match action_str.as_ref() {
                            "None" => {
                                // "Do Nothing" selected, so do no action
                            }
                            "Minimize" => {
                                window.miniaturize_(nil);
                            }
                            "Maximize" => {
                                window.zoom_(nil);
                            }
                            "Fill" => {
                                // There is no documented API for "Fill" action, so we'll just zoom the window
                                window.zoom_(nil);
                            }
                            _ => {
                                window.zoom_(nil);
                            }
                        }
                    }
                })
            })
            .detach();
    }

    fn play_system_bell(&self) {
        NSBeep()
    }

    #[cfg(any(test, feature = "test-support"))]
    fn render_to_image(&self, scene: &open_gpui::Scene) -> Result<RgbaImage> {
        let mut this = self.0.lock();
        this.renderer.render_to_image(scene)
    }

    fn a11y_init(&self, callbacks: open_gpui::A11yCallbacks) {
        let mut lock = self.0.lock();
        let interaction_quiesced = lock.interaction_quiesced.clone();

        let activation_handler = A11yActivationHandler {
            callback: callbacks.activation,
            interaction_quiesced: interaction_quiesced.clone(),
        };
        let action_handler = A11yActionHandler {
            callback: callbacks.action,
            interaction_quiesced,
        };

        let adapter = unsafe {
            accesskit_macos::SubclassingAdapter::for_window(
                lock.native_window as *mut c_void,
                activation_handler,
                action_handler,
            )
        };

        lock.accesskit_adapter = Some(adapter);
    }

    fn a11y_tree_update(&self, tree_update: accesskit::TreeUpdate) {
        let events = {
            let mut lock = self.0.lock();
            lock.accesskit_adapter
                .as_mut()
                .and_then(|adapter| adapter.update_if_active(|| tree_update))
        };
        if let Some(events) = events {
            events.raise();
        }
    }

    fn a11y_update_window_bounds(&self) {
        // macOS handles window bounds tracking automatically via NSAccessibility.
    }
}

struct A11yActivationHandler {
    callback: Box<dyn Fn() -> Option<accesskit::TreeUpdate> + Send + 'static>,
    interaction_quiesced: Arc<AtomicBool>,
}

impl accesskit::ActivationHandler for A11yActivationHandler {
    fn request_initial_tree(&mut self) -> Option<accesskit::TreeUpdate> {
        if self.interaction_quiesced.load(Ordering::Acquire) {
            None
        } else {
            (self.callback)()
        }
    }
}

struct A11yActionHandler {
    callback: Box<dyn Fn(accesskit::ActionRequest) + Send + 'static>,
    interaction_quiesced: Arc<AtomicBool>,
}

impl accesskit::ActionHandler for A11yActionHandler {
    fn do_action(&mut self, request: accesskit::ActionRequest) {
        if !self.interaction_quiesced.load(Ordering::Acquire) {
            (self.callback)(request);
        }
    }
}

impl rwh::HasWindowHandle for MacWindow {
    fn window_handle(&self) -> Result<rwh::WindowHandle<'_>, rwh::HandleError> {
        // SAFETY: The AppKitWindowHandle is a wrapper around a pointer to an NSView
        unsafe {
            Ok(rwh::WindowHandle::borrow_raw(rwh::RawWindowHandle::AppKit(
                rwh::AppKitWindowHandle::new(self.0.lock().native_view.cast()),
            )))
        }
    }
}

impl rwh::HasDisplayHandle for MacWindow {
    fn display_handle(&self) -> Result<rwh::DisplayHandle<'_>, rwh::HandleError> {
        Ok(rwh::DisplayHandle::appkit())
    }
}

/// Returns whether `window` is one of GPUI's managed windows.
unsafe fn is_gpui_window(window: id) -> bool {
    unsafe {
        msg_send![window, isKindOfClass: WINDOW_CLASS]
            || msg_send![window, isKindOfClass: PANEL_CLASS]
    }
}

unsafe fn get_window_state(object: &Object) -> Arc<Mutex<MacWindowState>> {
    unsafe {
        let raw: *mut c_void = *object.get_ivar(WINDOW_STATE_IVAR);
        let rc1 = Arc::from_raw(raw as *mut Mutex<MacWindowState>);
        let rc2 = rc1.clone();
        mem::forget(rc1);
        rc2
    }
}

unsafe fn drop_window_state(object: &Object) {
    unsafe {
        let raw: *mut c_void = *object.get_ivar(WINDOW_STATE_IVAR);
        if raw.is_null() {
            return;
        }
        drop(Arc::from_raw(raw as *mut Mutex<MacWindowState>));
    }
}

unsafe fn window_callback_superclass(this: &Object) -> &'static Class {
    unsafe {
        let is_panel: BOOL = msg_send![this, isKindOfClass: class!(NSPanel)];
        if is_panel == YES {
            class!(NSPanel)
        } else {
            class!(NSWindow)
        }
    }
}

extern "C" fn can_become_active_window(this: &Object, _: Sel) -> BOOL {
    unsafe {
        let raw: *mut c_void = *this.get_ivar(WINDOW_STATE_IVAR);
        if raw.is_null() {
            // AppKit may query this while initWithContentRect is still constructing the object.
            return YES;
        }
        let state = get_window_state(this);
        let state = state.lock();
        if !state.is_closed()
            && !state.interaction_is_quiesced()
            && (state.activation_policy.accepts_activation
                || state.activation_policy.focus_on_click)
        {
            YES
        } else {
            NO
        }
    }
}

extern "C" fn dealloc_window(this: &Object, _: Sel) {
    unsafe {
        drop_window_state(this);
        let superclass = window_callback_superclass(this);
        let _: () = msg_send![super(this, superclass), dealloc];
    }
}

extern "C" fn dealloc_view(this: &Object, _: Sel) {
    unsafe {
        drop_window_state(this);
        let _: () = msg_send![super(this, class!(NSView)), dealloc];
    }
}

extern "C" fn reset_cursor_rects(this: &Object, _: Sel) {
    // SAFETY: AppKit invokes cursor-rect updates on the main thread for GPUIView instances,
    // whose WINDOW_STATE_IVAR is initialized when the view is created. The cursor registered
    // below is a valid NSCursor.
    unsafe {
        let _: () = msg_send![super(this, class!(NSView)), resetCursorRects];

        let window_state = get_window_state(this);
        let cursor_style = window_state.lock().cursor_style;

        let cursor: id = match cursor_style {
            CursorStyle::Arrow => msg_send![class!(NSCursor), arrowCursor],
            CursorStyle::IBeam => msg_send![class!(NSCursor), IBeamCursor],
            CursorStyle::Crosshair => msg_send![class!(NSCursor), crosshairCursor],
            CursorStyle::ClosedHand => msg_send![class!(NSCursor), closedHandCursor],
            CursorStyle::OpenHand => msg_send![class!(NSCursor), openHandCursor],
            CursorStyle::PointingHand => msg_send![class!(NSCursor), pointingHandCursor],
            CursorStyle::ResizeLeftRight => msg_send![class!(NSCursor), resizeLeftRightCursor],
            CursorStyle::ResizeUpDown => msg_send![class!(NSCursor), resizeUpDownCursor],
            CursorStyle::ResizeLeft => msg_send![class!(NSCursor), resizeLeftCursor],
            CursorStyle::ResizeRight => msg_send![class!(NSCursor), resizeRightCursor],
            CursorStyle::ResizeColumn => msg_send![class!(NSCursor), resizeLeftRightCursor],
            CursorStyle::ResizeRow => msg_send![class!(NSCursor), resizeUpDownCursor],
            CursorStyle::ResizeUp => msg_send![class!(NSCursor), resizeUpCursor],
            CursorStyle::ResizeDown => msg_send![class!(NSCursor), resizeDownCursor],

            // Undocumented, private class methods:
            // https://stackoverflow.com/questions/27242353/cocoa-predefined-resize-mouse-cursor
            CursorStyle::ResizeUpLeftDownRight => {
                msg_send![class!(NSCursor), _windowResizeNorthWestSouthEastCursor]
            }
            CursorStyle::ResizeUpRightDownLeft => {
                msg_send![class!(NSCursor), _windowResizeNorthEastSouthWestCursor]
            }

            CursorStyle::IBeamCursorForVerticalLayout => {
                msg_send![class!(NSCursor), IBeamCursorForVerticalLayout]
            }
            CursorStyle::OperationNotAllowed => {
                msg_send![class!(NSCursor), operationNotAllowedCursor]
            }
            CursorStyle::DragLink => msg_send![class!(NSCursor), dragLinkCursor],
            CursorStyle::DragCopy => msg_send![class!(NSCursor), dragCopyCursor],
            CursorStyle::ContextualMenu => msg_send![class!(NSCursor), contextualMenuCursor],
        };

        let bounds = NSView::bounds(this as *const Object as id);
        let _: () = msg_send![this, addCursorRect: bounds cursor: cursor];
    }
}

extern "C" fn handle_key_equivalent(this: &Object, _: Sel, native_event: id) -> BOOL {
    handle_key_event(this, native_event, true)
}

extern "C" fn handle_key_down(this: &Object, _: Sel, native_event: id) {
    handle_key_event(this, native_event, false);
}

extern "C" fn handle_key_up(this: &Object, _: Sel, native_event: id) {
    handle_key_event(this, native_event, false);
}

// Things to test if you're modifying this method:
//  U.S. layout:
//   - The IME consumes characters like 'j' and 'k', which makes paging through `less` in
//     the terminal behave incorrectly by default. This behavior should be patched by our
//     IME integration
//   - `alt-t` should open the tasks menu
//   - In vim mode, this keybinding should work:
//     ```
//        {
//          "context": "Editor && vim_mode == insert",
//          "bindings": {"j j": "vim::NormalBefore"}
//        }
//     ```
//     and typing 'j k' in insert mode with this keybinding should insert the two characters
//  Brazilian layout:
//   - `" space` should create an unmarked quote
//   - `" backspace` should delete the marked quote
//   - `" "`should create an unmarked quote and a second marked quote
//   - `" up` should insert a quote, unmark it, and move up one line
//   - `" cmd-down` should insert a quote, unmark it, and move to the end of the file
//   - `cmd-ctrl-space` and clicking on an emoji should type it
//  Czech (QWERTY) layout:
//   - in vim mode `option-4`  should go to end of line (same as $)
//  Japanese (Romaji) layout:
//   - type `a i left down up enter enter` should create an unmarked text "愛"
//   - In vim mode with `jj` bound to `vim::NormalBefore` in insert mode, typing 'j i' with
//     Japanese IME should produce "じ" (ji), not "jい"

/// Returns true if the current keyboard input source is a composition-based IME
/// (e.g. Japanese Hiragana, Korean, Chinese Pinyin) that produces non-ASCII output.
///
/// This checks two properties:
/// 1. The source type is `kTISTypeKeyboardInputMode` (an IME input mode, not a plain
///    keyboard layout). This excludes non-ASCII layouts like Armenian and Ukrainian
///    that map keys directly without composition.
/// 2. The source is not ASCII-capable, which excludes modes like Japanese Romaji that
///    produce ASCII characters and should allow multi-stroke keybindings like `jj`.
unsafe fn is_ime_input_source_active() -> bool {
    unsafe {
        let source = TISCopyCurrentKeyboardInputSource();
        if source.is_null() {
            return false;
        }

        let source_type =
            TISGetInputSourceProperty(source, kTISPropertyInputSourceType as *const c_void);
        let is_input_mode = !source_type.is_null()
            && CFEqual(
                source_type as CFTypeRef,
                kTISTypeKeyboardInputMode as CFTypeRef,
            ) != 0;

        let is_ascii = TISGetInputSourceProperty(
            source,
            kTISPropertyInputSourceIsASCIICapable as *const c_void,
        );
        let is_ascii_capable = !is_ascii.is_null() && CFBooleanGetValue(is_ascii as CFBooleanRef);

        CFRelease(source as CFTypeRef);

        is_input_mode && !is_ascii_capable
    }
}

extern "C" fn handle_key_event(this: &Object, native_event: id, key_equivalent: bool) -> BOOL {
    let blocked = if key_equivalent { YES } else { NO };
    let window_state = unsafe { get_window_state(this) };
    let mut lock = window_state.as_ref().lock();
    if lock.is_closed() || lock.interaction_is_quiesced() {
        return blocked;
    }

    let window_height = lock.content_size().height;
    let event = unsafe { platform_input_from_native(native_event, Some(window_height)) };

    let Some(event) = event else {
        return NO;
    };

    let event_callback = lock.event_callback.clone();
    let interaction_quiesced = lock.interaction_quiesced.clone();
    let run_callback = |event: PlatformInput| -> BOOL {
        if interaction_quiesced.load(Ordering::Acquire) {
            blocked
        } else {
            (!event_callback.dispatch(event).propagate) as BOOL
        }
    };

    match event {
        PlatformInput::KeyDown(key_down_event) => {
            // For certain keystrokes, macOS will first dispatch a "key equivalent" event.
            // If that event isn't handled, it will then dispatch a "key down" event. GPUI
            // makes no distinction between these two types of events, so we need to ignore
            // the "key down" event if we've already just processed its "key equivalent" version.
            if key_equivalent {
                lock.last_key_equivalent = Some(key_down_event.clone());
            } else if lock.last_key_equivalent.take().as_ref() == Some(&key_down_event) {
                return NO;
            }

            drop(lock);

            let is_composing =
                with_input_handler(this, |input_handler| input_handler.marked_text_range())
                    .flatten()
                    .is_some();

            // If we're composing, send the key to the input handler first;
            // otherwise we only send to the input handler if we don't have a matching binding.
            // The input handler may call `do_command_by_selector` if it doesn't know how to handle
            // a key. If it does so, it will return YES so we won't send the key twice.
            // We also do this for non-printing keys (like arrow keys and escape) as the IME menu
            // may need them even if there is no marked text;
            // however we skip keys with control or the input handler adds control-characters to the buffer.
            // and keys with function, as the input handler swallows them.
            // and keys with platform (Cmd), so that Cmd+key events (e.g. Cmd+`) are not
            // consumed by the IME on non-QWERTY / dead-key layouts.
            // We also send printable keys to the IME first when an IME input source (e.g. Japanese,
            // Korean, Chinese) is active and the input handler accepts text input. This prevents
            // multi-stroke keybindings like `jj` from intercepting keys that the IME should compose
            // (e.g. typing 'ji' should produce 'じ', not 'jい'). If the IME doesn't handle the key,
            // it calls `doCommandBySelector:` which routes it back to keybinding matching.
            let is_ime_printable_key = !is_composing
                && key_down_event
                    .keystroke
                    .key_char
                    .as_ref()
                    .is_some_and(|key_char| key_char.chars().all(|c| !c.is_control()))
                && !key_down_event.keystroke.modifiers.control
                && !key_down_event.keystroke.modifiers.function
                && !key_down_event.keystroke.modifiers.platform
                && unsafe { is_ime_input_source_active() }
                && with_input_handler(this, |input_handler| {
                    input_handler.query_prefers_ime_for_printable_keys()
                })
                .unwrap_or(false);

            if is_composing
                || is_ime_printable_key
                || (key_down_event.keystroke.key_char.is_none()
                    && !key_down_event.keystroke.modifiers.control
                    && !key_down_event.keystroke.modifiers.function
                    && !key_down_event.keystroke.modifiers.platform)
            {
                {
                    let mut lock = window_state.as_ref().lock();
                    lock.keystroke_for_do_command = Some(key_down_event.keystroke.clone());
                    lock.do_command_handled.take();
                    drop(lock);
                }

                if interaction_quiesced.load(Ordering::Acquire) {
                    return blocked;
                }
                let handled: BOOL = unsafe {
                    let input_context: id = msg_send![this, inputContext];
                    msg_send![input_context, handleEvent: native_event]
                };
                window_state.as_ref().lock().keystroke_for_do_command.take();
                if let Some(handled) = window_state.as_ref().lock().do_command_handled.take() {
                    return handled as BOOL;
                } else if handled == YES {
                    return YES;
                }

                let handled = run_callback(PlatformInput::KeyDown(key_down_event));
                return handled;
            }

            let handled = run_callback(PlatformInput::KeyDown(key_down_event.clone()));
            if handled == YES {
                return YES;
            }

            if key_down_event.is_held
                && let Some(key_char) = key_down_event.keystroke.key_char.as_ref()
            {
                let handled = with_input_handler(this, |input_handler| {
                    if input_handler.apple_press_and_hold_enabled() {
                        NO
                    } else {
                        input_handler.replace_text_in_range(None, key_char);
                        YES
                    }
                });
                if handled == Some(YES) {
                    return YES;
                }
            }

            // Don't send key equivalents to the input handler if there are key modifiers other
            // than Function key, or macOS shortcuts like cmd-` will stop working.
            if key_equivalent && key_down_event.keystroke.modifiers != Modifiers::function() {
                return NO;
            }

            if interaction_quiesced.load(Ordering::Acquire) {
                return blocked;
            }
            unsafe {
                let input_context: id = msg_send![this, inputContext];
                msg_send![input_context, handleEvent: native_event]
            }
        }

        PlatformInput::KeyUp(_) => {
            drop(lock);
            run_callback(event)
        }

        _ => NO,
    }
}

extern "C" fn handle_view_event(this: &Object, _: Sel, native_event: id) {
    let window_state = unsafe { get_window_state(this) };
    let weak_window_state = Arc::downgrade(&window_state);
    let mut lock = window_state.as_ref().lock();
    if lock.is_closed() || lock.interaction_is_quiesced() {
        return;
    }
    let window_height = lock.content_size().height;
    let event = unsafe { platform_input_from_native(native_event, Some(window_height)) };

    if let Some(mut event) = event {
        // AppKit unhides the cursor on the next mouse movement; mirror that here.
        if matches!(
            event,
            PlatformInput::MouseMove(_)
                | PlatformInput::MouseDown(_)
                | PlatformInput::MouseUp(_)
                | PlatformInput::MousePressure(_)
                | PlatformInput::MouseExited(_)
                | PlatformInput::ScrollWheel(_)
                | PlatformInput::Pinch(_)
        ) {
            lock.cursor_visible.store(true, Ordering::Relaxed);
        }

        match &mut event {
            PlatformInput::MouseDown(
                event @ MouseDownEvent {
                    button: MouseButton::Left,
                    modifiers: Modifiers { control: true, .. },
                    ..
                },
            ) => {
                // On mac, a ctrl-left click should be handled as a right click.
                *event = MouseDownEvent {
                    button: MouseButton::Right,
                    modifiers: Modifiers {
                        control: false,
                        ..event.modifiers
                    },
                    click_count: 1,
                    ..*event
                };
            }

            // Handles focusing click.
            PlatformInput::MouseDown(
                event @ MouseDownEvent {
                    button: MouseButton::Left,
                    ..
                },
            ) if (lock.first_mouse) => {
                *event = MouseDownEvent {
                    first_mouse: true,
                    ..*event
                };
                lock.first_mouse = false;
            }

            // Because we map a ctrl-left_down to a right_down -> right_up let's ignore
            // the ctrl-left_up to avoid having a mismatch in button down/up events if the
            // user is still holding ctrl when releasing the left mouse button
            PlatformInput::MouseUp(
                event @ MouseUpEvent {
                    button: MouseButton::Left,
                    modifiers: Modifiers { control: true, .. },
                    ..
                },
            ) => {
                *event = MouseUpEvent {
                    button: MouseButton::Right,
                    modifiers: Modifiers {
                        control: false,
                        ..event.modifiers
                    },
                    click_count: 1,
                    ..*event
                };
            }

            _ => {}
        };

        match &event {
            PlatformInput::MouseDown(_) => {
                drop(lock);
                unsafe {
                    let input_context: id = msg_send![this, inputContext];
                    msg_send![input_context, handleEvent: native_event]
                }
                lock = window_state.as_ref().lock();
                if lock.interaction_is_quiesced() {
                    return;
                }
            }
            PlatformInput::MouseMove(
                event @ MouseMoveEvent {
                    pressed_button: Some(_),
                    ..
                },
            ) => {
                // Synthetic drag is used for selecting long buffer contents while buffer is being scrolled.
                // External file drag and drop is able to emit its own synthetic mouse events which will conflict
                // with these ones.
                if !lock.external_files_dragged {
                    lock.synthetic_drag_counter += 1;
                    let executor = lock.foreground_executor.clone();
                    executor
                        .spawn(synthetic_drag(
                            weak_window_state,
                            lock.synthetic_drag_counter,
                            event.clone(),
                            lock.background_executor.clone(),
                        ))
                        .detach();
                }
            }

            PlatformInput::MouseUp(MouseUpEvent { .. }) => {
                lock.synthetic_drag_counter += 1;
            }

            PlatformInput::ModifiersChanged(ModifiersChangedEvent {
                modifiers,
                capslock,
            }) => {
                // Only raise modifiers changed event when they have actually changed
                if let Some(PlatformInput::ModifiersChanged(ModifiersChangedEvent {
                    modifiers: prev_modifiers,
                    capslock: prev_capslock,
                })) = &lock.previous_modifiers_changed_event
                    && prev_modifiers == modifiers
                    && prev_capslock == capslock
                {
                    return;
                }

                lock.previous_modifiers_changed_event = Some(event.clone());
            }

            _ => {}
        }

        let event_callback = lock.event_callback.clone();
        let interaction_quiesced = lock.interaction_quiesced.clone();
        drop(lock);
        if !interaction_quiesced.load(Ordering::Acquire) {
            event_callback.dispatch(event);
        }
    }
}

extern "C" fn window_did_change_occlusion_state(this: &Object, _: Sel, _: id) {
    let window_state = unsafe { get_window_state(this) };
    let lock = &mut *window_state.lock();
    if lock.is_closed() {
        return;
    }
    unsafe {
        if lock
            .native_window
            .occlusionState()
            .contains(NSWindowOcclusionState::NSWindowOcclusionStateVisible)
        {
            lock.move_traffic_light();
            lock.start_display_link();
        } else {
            lock.stop_display_link();
        }
    }
}

extern "C" fn window_did_resize(this: &Object, selector: Sel, _: id) {
    window_state_observation_did_change(this, selector);
}

extern "C" fn window_will_enter_fullscreen(this: &Object, _: Sel, _: id) {
    let window_state = unsafe { get_window_state(this) };
    let mut lock = window_state.as_ref().lock();
    lock.fullscreen_transition = Some(MacFullscreenTransition::Entering);
    lock.fullscreen_restore_bounds = lock
        .pending_fullscreen_restore_bounds
        .take()
        .unwrap_or_else(|| lock.bounds());

    let min_version = NSOperatingSystemVersion::new(15, 3, 0);

    if is_macos_version_at_least(min_version) {
        unsafe {
            lock.native_window.setTitlebarAppearsTransparent_(NO);
        }
    }
}

extern "C" fn window_will_exit_fullscreen(this: &Object, _: Sel, _: id) {
    let window_state = unsafe { get_window_state(this) };
    let mut lock = window_state.as_ref().lock();
    lock.fullscreen_transition = Some(MacFullscreenTransition::Exiting);

    let min_version = NSOperatingSystemVersion::new(15, 3, 0);

    if is_macos_version_at_least(min_version) && lock.transparent_titlebar {
        unsafe {
            lock.native_window.setTitlebarAppearsTransparent_(YES);
        }
    }
}

fn fullscreen_terminal_for_delegate_selector(
    selector: Sel,
) -> Option<MacFullscreenTransitionTerminal> {
    if selector == sel!(windowDidEnterFullScreen:) {
        Some(MacFullscreenTransitionTerminal::Entered)
    } else if selector == sel!(windowDidExitFullScreen:) {
        Some(MacFullscreenTransitionTerminal::Exited)
    } else if selector == sel!(windowDidFailToEnterFullScreen:) {
        Some(MacFullscreenTransitionTerminal::FailedToEnter)
    } else if selector == sel!(windowDidFailToExitFullScreen:) {
        Some(MacFullscreenTransitionTerminal::FailedToExit)
    } else {
        None
    }
}

extern "C" fn window_fullscreen_transition_did_finish(this: &Object, selector: Sel, _: id) {
    let Some(terminal) = fullscreen_terminal_for_delegate_selector(selector) else {
        log::error!("received an unknown macOS fullscreen terminal delegate selector");
        return;
    };
    let window_state = unsafe { get_window_state(this) };
    let (target_generation, event) = {
        let mut lock = window_state.as_ref().lock();
        if lock.is_closed() {
            return;
        }
        if !terminal.finish(&mut lock.fullscreen_transition) {
            log::debug!(
                "received unexpected macOS fullscreen terminal {terminal:?} for window {:?}",
                lock.handle.window_id()
            );
            return;
        }

        if is_macos_version_at_least(NSOperatingSystemVersion::new(15, 3, 0)) {
            let appears_transparent =
                terminal.titlebar_appears_transparent(lock.transparent_titlebar);
            unsafe {
                lock.native_window
                    .setTitlebarAppearsTransparent_(appears_transparent.to_objc());
            }
        }

        let kind = MacWindowObservationEvent::Fullscreen(terminal);
        let mut event = lock.pending_observation_event(kind);
        if event
            .observation
            .is_some_and(|observation| observation.is_fullscreen != terminal.is_fullscreen())
        {
            event.observation = None;
        }
        (lock.observation_target_generation(), event)
    };

    request_window_observation_commit(&window_state, target_generation, Some(event), true);
}

fn state_event_for_delegate_selector(selector: Sel) -> Option<MacWindowStateEventSource> {
    if selector == sel!(windowDidResize:) {
        Some(MacWindowStateEventSource::Resized)
    } else if selector == sel!(windowDidMiniaturize:) {
        Some(MacWindowStateEventSource::Miniaturized)
    } else if selector == sel!(windowDidDeminiaturize:) {
        Some(MacWindowStateEventSource::Deminiaturized)
    } else {
        None
    }
}

fn request_window_state_observation(this: &Object, source: MacWindowStateEventSource) {
    let window_state = unsafe { get_window_state(this) };
    let (target_generation, event) = {
        let state = window_state.lock();
        if state.is_closed() {
            return;
        }
        let observation = state.retained_native_observation();
        let native_state = observation
            .map(MacWindowNativeObservation::state_expectation)
            .unwrap_or_else(|| state.native_state_expectation());
        let expected = source.expected_state(native_state);
        let kind = MacWindowObservationEvent::State(MacWindowStateEvent::new(source, expected));
        let observation =
            observation.filter(|observation| observation.state_expectation() == expected);
        let event = match observation {
            Some(observation) => MacWindowPendingObservationEvent::observed(kind, observation),
            None => MacWindowPendingObservationEvent::unobserved(kind),
        };
        (state.observation_target_generation(), event)
    };
    request_window_observation_commit(&window_state, target_generation, Some(event), true);
}

extern "C" fn window_state_did_change(this: &Object, selector: Sel, _: id) {
    window_state_observation_did_change(this, selector);
}

fn window_state_observation_did_change(this: &Object, selector: Sel) {
    let Some(event) = state_event_for_delegate_selector(selector) else {
        log::error!("received an unknown macOS window-state delegate selector");
        return;
    };
    request_window_state_observation(this, event);
}

pub(crate) fn is_macos_version_at_least(version: NSOperatingSystemVersion) -> bool {
    unsafe { NSProcessInfo::processInfo(nil).isOperatingSystemAtLeastVersion(version) }
}

extern "C" fn window_did_move(this: &Object, _: Sel, _: id) {
    let window_state = unsafe { get_window_state(this) };
    let (target_generation, event) = {
        let state = window_state.lock();
        if state.is_closed() {
            return;
        }
        (
            state.observation_target_generation(),
            state.pending_observation_event(MacWindowObservationEvent::Moved),
        )
    };
    request_window_observation_commit(&window_state, target_generation, Some(event), true);
}

// Update the window scale factor and drawable size, and call the resize callback if any.
fn update_window_scale_factor(window_state: &Arc<Mutex<MacWindowState>>) {
    request_window_observation_commit_for_retained_topology(window_state, None, true);
}

extern "C" fn window_did_change_screen(this: &Object, _: Sel, _: id) {
    let window_state = unsafe { get_window_state(this) };
    let mut lock = window_state.as_ref().lock();
    if lock.is_closed() {
        return;
    }
    lock.start_display_link();
    drop(lock);
    update_window_scale_factor(&window_state);
}

fn active_event_for_delegate_selector(selector: Sel) -> Option<MacWindowActiveEvent> {
    if selector == sel!(windowDidBecomeKey:) {
        Some(MacWindowActiveEvent::BecameKey)
    } else if selector == sel!(windowDidResignKey:) {
        Some(MacWindowActiveEvent::ResignedKey)
    } else {
        None
    }
}

extern "C" fn window_did_change_key_status(this: &Object, selector: Sel, _: id) {
    let Some(event) = active_event_for_delegate_selector(selector) else {
        log::error!("received an unknown macOS key-status delegate selector");
        return;
    };
    let window_state = unsafe { get_window_state(this) };
    let (target_generation, event) = {
        let state = window_state.lock();
        if state.is_closed() || state.interaction_is_quiesced() {
            return;
        }
        (
            state.observation_target_generation(),
            state.pending_observation_event(MacWindowObservationEvent::Active(event)),
        )
    };
    request_window_observation_commit(&window_state, target_generation, Some(event), true);
}

fn deliver_window_active_event(
    window_state: &Arc<Mutex<MacWindowState>>,
    event: MacWindowActiveEvent,
    observed_is_active: bool,
    event_is_latest: bool,
    panic_boundary: &mut MacWindowCallbackPanicBoundary,
) {
    let is_active = event.is_active();
    let mut lock = window_state.lock();
    if lock.is_closed() || lock.interaction_is_quiesced() {
        return;
    }

    // AppKit also unhides the cursor on activation changes, so mirror that here.
    lock.cursor_visible.store(true, Ordering::Relaxed);

    // When opening a pop-up while the application isn't active, Cocoa sends a spurious
    // `windowDidBecomeKey` message to the previous key window even though that window
    // isn't actually key. This causes a bug if the application is later activated while
    // the pop-up is still open, making it impossible to activate the previous key window
    // even if the pop-up gets closed. The only way to activate it again is to de-activate
    // the app and re-activate it, which is a pretty bad UX.
    // The following code detects the spurious event and invokes `resignKeyWindow`:
    // in theory, we're not supposed to invoke this method manually but it balances out
    // the spurious `becomeKeyWindow` event and helps us work around that bug.
    if event == MacWindowActiveEvent::BecameKey && event_is_latest && !observed_is_active {
        let native_window = lock.native_window;
        drop(lock);
        unsafe {
            let _: () = msg_send![native_window, resignKeyWindow];
        }
        return;
    }

    drop(lock);

    let exact_reorder_positive =
        event == MacWindowActiveEvent::BecameKey && event_is_latest && observed_is_active && {
            let native_window = window_state.lock().native_window;
            unsafe {
                let app = NSApplication::sharedApplication(nil);
                let key_window: id = msg_send![app, keyWindow];
                key_window == native_window
            }
        };
    if exact_reorder_positive && !reorder_mac_transient_after_native_activation(window_state) {
        log::error!("failed to reorder an exact active macOS transient window");
    }

    let a11y_events = {
        let mut lock = window_state.lock();
        if lock.interaction_is_quiesced() {
            None
        } else {
            lock.accesskit_adapter
                .as_mut()
                .and_then(|adapter| adapter.update_view_focus_state(is_active))
        }
    };
    if let Some(events) = a11y_events {
        panic_boundary.deliver(|| events.raise());
    }

    // When a window becomes active, trigger an immediate synchronous frame request to prevent
    // tab flicker when switching between windows in native tabs mode.
    //
    // This is only done on subsequent activations (not the first) to ensure the initial focus
    // path is properly established. Without this guard, the focus state would remain unset until
    // the first mouse click, causing keybindings to be non-functional.
    if event == MacWindowActiveEvent::BecameKey && is_active {
        let should_request_frame = {
            let mut lock = window_state.lock();
            if lock.is_closed() {
                return;
            }
            if lock.activated_least_once {
                true
            } else {
                lock.activated_least_once = true;
                false
            }
        };
        if should_request_frame {
            deliver_window_request_frame(
                window_state,
                MacRequestFrameDeliveryMode::DisplayTransaction,
                panic_boundary,
            );
        }
    }

    let (callback, native_window) = {
        let mut lock = window_state.lock();
        if lock.is_closed() || lock.interaction_is_quiesced() {
            return;
        }
        if is_active {
            lock.move_traffic_light();
        }
        let Some(callback) = lock.activate_callback.take() else {
            return;
        };
        (callback, lock.native_window)
    };
    let exact_native_positive = is_active
        && event_is_latest
        && observed_is_active
        && unsafe {
            let app = NSApplication::sharedApplication(nil);
            let key_window: id = msg_send![app, keyWindow];
            key_window == native_window
        };
    let mut callback = checkout_mac_window_callback(window_state, callback, activate_callback_slot);
    panic_boundary.deliver(|| {
        (callback.callback())(PlatformWindowActiveStatusObservation::new(
            is_active,
            exact_native_positive,
        ));
    });
    panic_boundary.deliver(|| drop(callback));
}

extern "C" fn window_should_close(this: &Object, _: Sel, _: id) -> BOOL {
    let window_state = unsafe { get_window_state(this) };
    let mut lock = window_state.as_ref().lock();
    if lock.is_closed() {
        return YES;
    }
    if let Some(mut callback) = lock.should_close_callback.take() {
        drop(lock);
        let should_close = callback();
        let mut lock = window_state.lock();
        if !lock.is_closed() {
            lock.should_close_callback = Some(callback);
        }
        should_close as BOOL
    } else {
        YES
    }
}

extern "C" fn close_window(this: &Object, _: Sel) {
    unsafe {
        let (event_callback, input_handler, close_callback) = {
            let window_state = get_window_state(this);
            let mut lock = window_state.as_ref().lock();
            let shutdown = lock.presentation_shutdown_authority.lock().ticket();
            if !shutdown.acknowledge_native_terminal() {
                let snapshot = shutdown.snapshot();
                log::error!(
                    "native macOS window reached close before presentation quiescence was acknowledged for window {:?}, generation {}",
                    snapshot.window_id(),
                    snapshot.generation(),
                );
            }
            let (event_callback, input_handler) = lock.mark_closed();
            (event_callback, input_handler, lock.close_callback.take())
        };

        event_callback.terminate();
        input_handler.terminate();
        detach_native_transient_relationships(this as *const Object as id);
        if let Some(callback) = close_callback {
            callback();
        }

        let superclass = window_callback_superclass(this);
        let _: () = msg_send![super(this, superclass), close];
    }
}

extern "C" fn make_backing_layer(this: &Object, _: Sel) -> id {
    let window_state = unsafe { get_window_state(this) };
    let window_state = window_state.as_ref().lock();
    window_state.renderer.layer_ptr() as id
}

extern "C" fn view_did_change_backing_properties(this: &Object, _: Sel) {
    let window_state = unsafe { get_window_state(this) };
    update_window_scale_factor(&window_state);
}

extern "C" fn set_frame_size(this: &Object, _: Sel, size: NSSize) {
    fn convert(value: NSSize) -> Size<Pixels> {
        Size {
            width: px(value.width as f32),
            height: px(value.height as f32),
        }
    }

    let window_state = unsafe { get_window_state(this) };
    let mut lock = window_state.as_ref().lock();

    let new_size = convert(size);
    let old_size = unsafe {
        let old_frame: NSRect = msg_send![this, frame];
        convert(old_frame.size)
    };

    if old_size == new_size {
        return;
    }

    unsafe {
        let _: () = msg_send![super(this, class!(NSView)), setFrameSize: size];
    }

    let renderer_geometry = renderer_geometry_for_frame_size(lock.renderer_geometry, new_size);
    lock.update_renderer_geometry(renderer_geometry);
}

extern "C" fn display_layer(this: &Object, _: Sel, _: id) {
    let window_state = unsafe { get_window_state(this) };
    let mut panic_boundary = MacWindowCallbackPanicBoundary::default();
    let delivery = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        deliver_window_request_frame(
            &window_state,
            MacRequestFrameDeliveryMode::DisplayTransaction,
            &mut panic_boundary,
        );
    }));
    if let Err(panic) = delivery {
        panic_boundary.retain(panic);
    }
    panic_boundary.isolate_at_native_boundary("a macOS display-layer frame callback");
}

extern "C" fn step(view: *mut c_void) {
    let view = view as id;
    let window_state = unsafe { get_window_state(&*view) };
    let mut panic_boundary = MacWindowCallbackPanicBoundary::default();
    let delivery = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        deliver_window_request_frame(
            &window_state,
            MacRequestFrameDeliveryMode::DisplayLinkTick,
            &mut panic_boundary,
        );
    }));
    if let Err(panic) = delivery {
        panic_boundary.retain(panic);
    }
    panic_boundary.isolate_at_native_boundary("a macOS display-link frame callback");
}

extern "C" fn valid_attributes_for_marked_text(_: &Object, _: Sel) -> id {
    unsafe { msg_send![class!(NSArray), array] }
}

extern "C" fn has_marked_text(this: &Object, _: Sel) -> BOOL {
    let has_marked_text_result =
        with_input_handler(this, |input_handler| input_handler.marked_text_range()).flatten();

    has_marked_text_result.is_some() as BOOL
}

extern "C" fn marked_range(this: &Object, _: Sel) -> NSRange {
    let marked_range_result =
        with_input_handler(this, |input_handler| input_handler.marked_text_range()).flatten();

    marked_range_result.map_or(NSRange::invalid(), |range| range.into())
}

extern "C" fn selected_range(this: &Object, _: Sel) -> NSRange {
    let selected_range_result = with_input_handler(this, |input_handler| {
        input_handler.selected_text_range(false)
    })
    .flatten();

    selected_range_result.map_or(NSRange::invalid(), |selection| selection.range.into())
}

extern "C" fn first_rect_for_character_range(
    this: &Object,
    _: Sel,
    range: NSRange,
    _: id,
) -> NSRect {
    let frame = get_frame(this);
    let Some(range) = range.to_range() else {
        return NSRect::new(NSPoint::new(0., 0.), NSSize::new(0., 0.));
    };
    with_input_handler(this, |input_handler| input_handler.bounds_for_range(range))
        .flatten()
        .map_or(
            NSRect::new(NSPoint::new(0., 0.), NSSize::new(0., 0.)),
            |bounds| {
                NSRect::new(
                    NSPoint::new(
                        frame.origin.x + bounds.origin.x.as_f32() as f64,
                        frame.origin.y + frame.size.height
                            - bounds.origin.y.as_f32() as f64
                            - bounds.size.height.as_f32() as f64,
                    ),
                    NSSize::new(
                        bounds.size.width.as_f32() as f64,
                        bounds.size.height.as_f32() as f64,
                    ),
                )
            },
        )
}

fn get_frame(this: &Object) -> NSRect {
    unsafe {
        let state = get_window_state(this);
        let lock = state.lock();
        let mut frame = NSWindow::frame(lock.native_window);
        let content_layout_rect: CGRect = msg_send![lock.native_window, contentLayoutRect];
        let style_mask: NSWindowStyleMask = msg_send![lock.native_window, styleMask];
        if !style_mask.contains(NSWindowStyleMask::NSFullSizeContentViewWindowMask) {
            frame.origin.y -= frame.size.height - content_layout_rect.size.height;
        }
        frame
    }
}

extern "C" fn insert_text(this: &Object, _: Sel, text: id, replacement_range: NSRange) {
    unsafe {
        let is_attributed_string: BOOL =
            msg_send![text, isKindOfClass: [class!(NSAttributedString)]];
        let text: id = if is_attributed_string == YES {
            msg_send![text, string]
        } else {
            text
        };

        let text = text.to_str();
        let replacement_range = replacement_range.to_range();
        let _ = with_input_handler(this, |input_handler| {
            input_handler.replace_text_in_range(replacement_range, text)
        });
    }
}

extern "C" fn set_marked_text(
    this: &Object,
    _: Sel,
    text: id,
    selected_range: NSRange,
    replacement_range: NSRange,
) {
    unsafe {
        let is_attributed_string: BOOL =
            msg_send![text, isKindOfClass: [class!(NSAttributedString)]];
        let text: id = if is_attributed_string == YES {
            msg_send![text, string]
        } else {
            text
        };
        let selected_range = selected_range.to_range();
        let replacement_range = replacement_range.to_range();
        let text = text.to_str();
        let _ = with_input_handler(this, |input_handler| {
            input_handler.replace_and_mark_text_in_range(replacement_range, text, selected_range)
        });
    }
}
extern "C" fn unmark_text(this: &Object, _: Sel) {
    let _ = with_input_handler(this, |input_handler| input_handler.unmark_text());
}

extern "C" fn attributed_substring_for_proposed_range(
    this: &Object,
    _: Sel,
    range: NSRange,
    actual_range: *mut c_void,
) -> id {
    with_input_handler(this, |input_handler| {
        let range = range.to_range()?;
        if range.is_empty() {
            return None;
        }
        let mut adjusted: Option<Range<usize>> = None;

        let selected_text = input_handler.text_for_range(range.clone(), &mut adjusted)?;
        if let Some(adjusted) = adjusted
            && adjusted != range
        {
            unsafe { (actual_range as *mut NSRange).write(NSRange::from(adjusted)) };
        }
        unsafe {
            let string: id = msg_send![class!(NSAttributedString), alloc];
            let string: id = msg_send![string, initWithString: ns_string(&selected_text)];
            Some(string)
        }
    })
    .flatten()
    .unwrap_or(nil)
}

// We ignore which selector it asks us to do because the user may have
// bound the shortcut to something else.
extern "C" fn do_command_by_selector(this: &Object, _: Sel, _: Sel) {
    let state = unsafe { get_window_state(this) };
    let mut lock = state.as_ref().lock();
    if lock.is_closed() || lock.interaction_is_quiesced() {
        return;
    }
    let keystroke = lock.keystroke_for_do_command.take();
    let event_callback = lock.event_callback.clone();
    drop(lock);

    if let Some(handled) = keystroke.and_then(|keystroke| {
        event_callback.dispatch(PlatformInput::KeyDown(KeyDownEvent {
            keystroke,
            is_held: false,
            prefer_character_input: false,
        }))
    }) {
        state.as_ref().lock().do_command_handled = Some(!handled.propagate);
    }
}

extern "C" fn view_did_change_effective_appearance(this: &Object, _: Sel) {
    unsafe {
        let state = get_window_state(this);
        let mut lock = state.as_ref().lock();
        if lock.is_closed() {
            return;
        }
        if let Some(mut callback) = lock.appearance_changed_callback.take() {
            drop(lock);
            callback();
            let mut lock = state.lock();
            if !lock.is_closed() {
                lock.appearance_changed_callback = Some(callback);
            }
        }
    }
}

extern "C" fn accepts_first_mouse(this: &Object, _: Sel, _: id) -> BOOL {
    let window_state = unsafe { get_window_state(this) };
    let mut lock = window_state.as_ref().lock();
    if lock.interaction_is_quiesced() {
        lock.first_mouse = false;
        return NO;
    }
    lock.first_mouse = macos_click_can_activate(lock.activation_policy);
    YES
}

extern "C" fn character_index_for_point(this: &Object, _: Sel, position: NSPoint) -> u64 {
    let position = screen_point_to_gpui_point(this, position);
    with_input_handler(this, |input_handler| {
        input_handler.character_index_for_point(position)
    })
    .flatten()
    .map(|index| index as u64)
    .unwrap_or(NSNotFound as u64)
}

fn screen_point_to_gpui_point(this: &Object, position: NSPoint) -> Point<Pixels> {
    let frame = get_frame(this);
    let window_x = position.x - frame.origin.x;
    let window_y = frame.size.height - (position.y - frame.origin.y);

    point(px(window_x as f32), px(window_y as f32))
}

extern "C" fn dragging_entered(this: &Object, _: Sel, dragging_info: id) -> NSDragOperation {
    let window_state = unsafe { get_window_state(this) };
    let position = drag_event_position(&window_state, dragging_info);
    let paths = external_paths_from_event(dragging_info);
    if let Some(event) = paths.map(|paths| FileDropEvent::Entered { position, paths })
        && send_file_drop_event(window_state, event)
    {
        return NSDragOperationCopy;
    }
    NSDragOperationNone
}

extern "C" fn dragging_updated(this: &Object, _: Sel, dragging_info: id) -> NSDragOperation {
    let window_state = unsafe { get_window_state(this) };
    let position = drag_event_position(&window_state, dragging_info);
    if send_file_drop_event(window_state, FileDropEvent::Pending { position }) {
        NSDragOperationCopy
    } else {
        NSDragOperationNone
    }
}

extern "C" fn dragging_exited(this: &Object, _: Sel, _: id) {
    let window_state = unsafe { get_window_state(this) };
    send_file_drop_event(window_state, FileDropEvent::Exited);
}

extern "C" fn perform_drag_operation(this: &Object, _: Sel, dragging_info: id) -> BOOL {
    let window_state = unsafe { get_window_state(this) };
    let position = drag_event_position(&window_state, dragging_info);
    send_file_drop_event(window_state, FileDropEvent::Submit { position }).to_objc()
}

fn external_paths_from_event(dragging_info: *mut Object) -> Option<ExternalPaths> {
    let mut paths = SmallVec::new();
    let pasteboard: id = unsafe { msg_send![dragging_info, draggingPasteboard] };
    let filenames = unsafe { NSPasteboard::propertyListForType(pasteboard, NSFilenamesPboardType) };
    if filenames == nil {
        return None;
    }
    for file in unsafe { filenames.iter() } {
        let path = unsafe {
            let f = NSString::UTF8String(file);
            CStr::from_ptr(f).to_string_lossy().into_owned()
        };
        paths.push(PathBuf::from(path))
    }
    Some(ExternalPaths(paths))
}

extern "C" fn conclude_drag_operation(this: &Object, _: Sel, _: id) {
    let window_state = unsafe { get_window_state(this) };
    send_file_drop_event(window_state, FileDropEvent::Exited);
}

async fn synthetic_drag(
    window_state: Weak<Mutex<MacWindowState>>,
    drag_id: usize,
    event: MouseMoveEvent,
    executor: BackgroundExecutor,
) {
    loop {
        executor.timer(Duration::from_millis(16)).await;
        if let Some(window_state) = window_state.upgrade() {
            let event_callback = {
                let lock = window_state.lock();
                if lock.is_closed()
                    || lock.interaction_is_quiesced()
                    || lock.synthetic_drag_counter != drag_id
                {
                    break;
                }
                lock.event_callback.clone()
            };
            event_callback.dispatch(PlatformInput::MouseMove(event.clone()));
        }
    }
}

/// Sends the specified FileDropEvent using `PlatformInput::FileDrop` to the window
/// state and updates the window state according to the event passed.
fn send_file_drop_event(
    window_state: Arc<Mutex<MacWindowState>>,
    file_drop_event: FileDropEvent,
) -> bool {
    let external_files_dragged = match file_drop_event {
        FileDropEvent::Entered { .. } => Some(true),
        FileDropEvent::Exited => Some(false),
        _ => None,
    };

    let event_callback = {
        let lock = window_state.lock();
        if lock.is_closed() || lock.interaction_is_quiesced() {
            return false;
        }
        lock.event_callback.clone()
    };
    if event_callback
        .dispatch(PlatformInput::FileDrop(file_drop_event))
        .is_some()
    {
        let mut lock = window_state.lock();
        if !lock.is_closed() {
            if let Some(external_files_dragged) = external_files_dragged {
                lock.external_files_dragged = external_files_dragged;
            }
        }
        true
    } else {
        false
    }
}

fn drag_event_position(window_state: &Mutex<MacWindowState>, dragging_info: id) -> Point<Pixels> {
    let drag_location: NSPoint = unsafe { msg_send![dragging_info, draggingLocation] };
    convert_mouse_position(drag_location, window_state.lock().content_size().height)
}

fn with_input_handler<F, R>(window: &Object, f: F) -> Option<R>
where
    F: FnOnce(&mut PlatformInputHandler) -> R,
{
    let window_state = unsafe { get_window_state(window) };
    let (input_handler_slot, interaction_quiesced) = {
        let lock = window_state.as_ref().lock();
        if lock.is_closed() || lock.interaction_is_quiesced() {
            return None;
        }
        (
            lock.input_handler.clone(),
            lock.interaction_quiesced.clone(),
        )
    };
    input_handler_slot
        .with_handler(|handler| (!interaction_quiesced.load(Ordering::Acquire)).then(|| f(handler)))
        .flatten()
}

unsafe fn display_id_for_screen(screen: id) -> CGDirectDisplayID {
    unsafe {
        let device_description = NSScreen::deviceDescription(screen);
        let screen_number_key: id = ns_string("NSScreenNumber");
        let screen_number = device_description.objectForKey_(screen_number_key);
        let screen_number: NSUInteger = msg_send![screen_number, unsignedIntegerValue];
        screen_number as CGDirectDisplayID
    }
}

extern "C" fn blurred_view_init_with_frame(this: &Object, _: Sel, frame: NSRect) -> id {
    unsafe {
        let view = msg_send![super(this, class!(NSVisualEffectView)), initWithFrame: frame];
        // Use a colorless semantic material. The default value `AppearanceBased`, though not
        // manually set, is deprecated.
        NSVisualEffectView::setMaterial_(view, NSVisualEffectMaterial::Selection);
        NSVisualEffectView::setState_(view, NSVisualEffectState::Active);
        view
    }
}

extern "C" fn blurred_view_update_layer(this: &Object, _: Sel) {
    unsafe {
        let _: () = msg_send![super(this, class!(NSVisualEffectView)), updateLayer];
        let layer: id = msg_send![this, layer];
        if !layer.is_null() {
            remove_layer_background(layer);
        }
    }
}

unsafe fn remove_layer_background(layer: id) {
    unsafe {
        let _: () = msg_send![layer, setBackgroundColor:nil];

        let class_name: id = msg_send![layer, className];
        if class_name.isEqualToString("CAChameleonLayer") {
            // Remove the desktop tinting effect.
            let _: () = msg_send![layer, setHidden: YES];
            return;
        }

        let filters: id = msg_send![layer, filters];
        if !filters.is_null() {
            // Remove the increased saturation.
            // The effect of a `CAFilter` or `CIFilter` is determined by its name, and the
            // `description` reflects its name and some parameters. Currently `NSVisualEffectView`
            // uses a `CAFilter` named "colorSaturate". If one day they switch to `CIFilter`, the
            // `description` will still contain "Saturat" ("... inputSaturation = ...").
            let test_string: id = ns_string("Saturat");
            let count = NSArray::count(filters);
            for i in 0..count {
                let description: id = msg_send![filters.objectAtIndex(i), description];
                let hit: BOOL = msg_send![description, containsString: test_string];
                if hit == NO {
                    continue;
                }

                let all_indices = NSRange {
                    location: 0,
                    length: count,
                };
                let indices: id = msg_send![class!(NSMutableIndexSet), indexSet];
                let _: () = msg_send![indices, addIndexesInRange: all_indices];
                let _: () = msg_send![indices, removeIndex:i];
                let filtered: id = msg_send![filters, objectsAtIndexes: indices];
                let _: () = msg_send![layer, setFilters: filtered];
                break;
            }
        }

        let sublayers: id = msg_send![layer, sublayers];
        if !sublayers.is_null() {
            let count = NSArray::count(sublayers);
            for i in 0..count {
                let sublayer = sublayers.objectAtIndex(i);
                remove_layer_background(sublayer);
            }
        }
    }
}

extern "C" fn add_titlebar_accessory_view_controller(this: &Object, _: Sel, view_controller: id) {
    unsafe {
        let superclass = window_callback_superclass(this);
        let _: () =
            msg_send![super(this, superclass), addTitlebarAccessoryViewController: view_controller];

        // Hide the native tab bar and set its height to 0, since we render our own.
        let accessory_view: id = msg_send![view_controller, view];
        let _: () = msg_send![accessory_view, setHidden: YES];
        let mut frame: NSRect = msg_send![accessory_view, frame];
        frame.size.height = 0.0;
        let _: () = msg_send![accessory_view, setFrame: frame];
    }
}

extern "C" fn move_tab_to_new_window(this: &Object, _: Sel, _: id) {
    unsafe {
        let window_state = get_window_state(this);
        if !window_state.lock().admits_native_tabbing() {
            return;
        }
        let superclass = window_callback_superclass(this);
        let _: () = msg_send![super(this, superclass), moveTabToNewWindow:nil];

        let mut lock = window_state.as_ref().lock();
        if !lock.admits_native_tabbing() {
            return;
        }
        if let Some(mut callback) = lock.move_tab_to_new_window_callback.take() {
            drop(lock);
            callback();
            let mut lock = window_state.lock();
            if lock.admits_native_tabbing() {
                lock.move_tab_to_new_window_callback = Some(callback);
            }
        }
    }
}

extern "C" fn merge_all_windows(this: &Object, _: Sel, _: id) {
    unsafe {
        let window_state = get_window_state(this);
        if !window_state.lock().admits_native_tabbing() {
            return;
        }
        let superclass = window_callback_superclass(this);
        let _: () = msg_send![super(this, superclass), mergeAllWindows:nil];

        let mut lock = window_state.as_ref().lock();
        if !lock.admits_native_tabbing() {
            return;
        }
        if let Some(mut callback) = lock.merge_all_windows_callback.take() {
            drop(lock);
            callback();
            let mut lock = window_state.lock();
            if lock.admits_native_tabbing() {
                lock.merge_all_windows_callback = Some(callback);
            }
        }
    }
}

extern "C" fn select_next_tab(this: &Object, _sel: Sel, _id: id) {
    let window_state = unsafe { get_window_state(this) };
    let mut lock = window_state.as_ref().lock();
    if !lock.admits_native_tabbing() {
        return;
    }
    if let Some(mut callback) = lock.select_next_tab_callback.take() {
        drop(lock);
        callback();
        let mut lock = window_state.lock();
        if lock.admits_native_tabbing() {
            lock.select_next_tab_callback = Some(callback);
        }
    }
}

extern "C" fn select_previous_tab(this: &Object, _sel: Sel, _id: id) {
    let window_state = unsafe { get_window_state(this) };
    let mut lock = window_state.as_ref().lock();
    if !lock.admits_native_tabbing() {
        return;
    }
    if let Some(mut callback) = lock.select_previous_tab_callback.take() {
        drop(lock);
        callback();
        let mut lock = window_state.lock();
        if lock.admits_native_tabbing() {
            lock.select_previous_tab_callback = Some(callback);
        }
    }
}

extern "C" fn toggle_tab_bar(this: &Object, _sel: Sel, _id: id) {
    unsafe {
        let window_state = get_window_state(this);
        if !window_state.lock().admits_native_tabbing() {
            return;
        }
        let superclass = window_callback_superclass(this);
        let _: () = msg_send![super(this, superclass), toggleTabBar:nil];

        let mut lock = window_state.as_ref().lock();
        if !lock.admits_native_tabbing() {
            return;
        }
        lock.move_traffic_light();

        if let Some(mut callback) = lock.toggle_tab_bar_callback.take() {
            drop(lock);
            callback();
            let mut lock = window_state.lock();
            if lock.admits_native_tabbing() {
                lock.toggle_tab_bar_callback = Some(callback);
            }
        }
    }
}

#[cfg(test)]
mod creation_projection_tests {
    use super::*;

    fn restore_bounds() -> Bounds<Pixels> {
        Bounds::new(point(px(120.0), px(80.0)), size(px(1024.0), px(768.0)))
    }

    fn assert_client_geometry_round_trip(
        screen_frame: NSRect,
        display_bounds: Bounds<Pixels>,
        client_bounds: Bounds<Pixels>,
    ) {
        let content_rect = global_client_bounds_to_appkit_content_rect(
            client_bounds,
            screen_frame,
            display_bounds,
        );
        assert_eq!(
            appkit_content_rect_to_global_client_bounds(content_rect, screen_frame, display_bounds,),
            client_bounds
        );
    }

    #[test]
    fn client_geometry_round_trips_across_signed_desktop_coordinates() {
        assert_client_geometry_round_trip(
            NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(1920.0, 1080.0)),
            Bounds::new(point(px(0.0), px(0.0)), size(px(1920.0), px(1080.0))),
            restore_bounds(),
        );
        assert_client_geometry_round_trip(
            NSRect::new(NSPoint::new(-1440.0, 0.0), NSSize::new(1440.0, 900.0)),
            Bounds::new(point(px(-1440.0), px(0.0)), size(px(1440.0), px(900.0))),
            Bounds::new(point(px(-1300.0), px(140.0)), size(px(900.0), px(600.0))),
        );
        assert_client_geometry_round_trip(
            NSRect::new(NSPoint::new(0.0, 1080.0), NSSize::new(1440.0, 900.0)),
            Bounds::new(point(px(0.0), px(-900.0)), size(px(1440.0), px(900.0))),
            Bounds::new(point(px(160.0), px(-780.0)), size(px(800.0), px(500.0))),
        );
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    struct TestNativeObservation {
        topology_generation: u64,
        bounds: Bounds<Pixels>,
        scale_factor: f32,
        display_id: u64,
        is_fullscreen: bool,
        is_active: bool,
    }

    fn native_observation(
        bounds: Bounds<Pixels>,
        topology_generation: u64,
    ) -> TestNativeObservation {
        TestNativeObservation {
            topology_generation,
            bounds,
            scale_factor: 2.0,
            display_id: 7,
            is_fullscreen: false,
            is_active: true,
        }
    }

    #[test]
    fn unstable_native_sample_retains_the_previous_whole_fact() {
        let previous = native_observation(restore_bounds(), 1);
        let mut committed = Some(previous);
        let next_bounds = Bounds::new(point(px(-800.0), px(120.0)), size(px(900.0), px(640.0)));
        let first = native_observation(next_bounds, 2);
        let second = TestNativeObservation {
            is_fullscreen: true,
            ..first
        };

        assert!(!commit_stable_native_observation(
            &mut committed,
            Some(first),
            Some(second),
        ));
        assert_eq!(committed, Some(previous));
    }

    #[test]
    fn stable_native_sample_commits_geometry_display_scale_and_state_together() {
        let previous = native_observation(restore_bounds(), 1);
        let next = TestNativeObservation {
            topology_generation: 2,
            bounds: Bounds::new(point(px(-800.0), px(120.0)), size(px(900.0), px(640.0))),
            scale_factor: 1.0,
            display_id: 9,
            is_active: false,
            ..previous
        };
        let mut committed = Some(previous);

        assert!(commit_stable_native_observation(
            &mut committed,
            Some(next),
            Some(next),
        ));
        assert_eq!(committed, Some(next));
    }

    #[test]
    fn display_fact_only_changes_wake_window_consumers() {
        let origin = point(px(40.0), px(60.0));
        let moved_origin = point(px(41.0), px(60.0));

        assert!(!moved_or_display_facts_changed(
            origin, &7_u64, origin, &7_u64
        ));
        assert!(moved_or_display_facts_changed(
            origin,
            &7_u64,
            moved_origin,
            &7_u64
        ));
        assert!(moved_or_display_facts_changed(
            origin, &7_u64, origin, &8_u64
        ));
    }

    #[test]
    fn observation_effect_drain_keeps_reentrant_batches_fifo() {
        let mut drain = MacWindowSerialEffectDrain::default();

        assert!(drain.enqueue("became-key"));
        assert!(!drain.enqueue("state-changed"));
        assert!(!drain.enqueue("fullscreen-entered"));
        assert_eq!(drain.pop_next(), Some("became-key"));
        assert_eq!(drain.pop_next(), Some("state-changed"));

        // This models a state callback committing a new batch while the outer drain still owns an
        // older fullscreen effect. The reentrant edge must append after every older batch.
        assert!(!drain.enqueue("resigned-key"));
        assert_eq!(drain.pop_next(), Some("fullscreen-entered"));
        assert_eq!(drain.pop_next(), Some("resigned-key"));

        // Ownership also remains with the active drain while the last popped callback runs.
        assert!(!drain.enqueue("reentrant-after-last-pop"));
        assert_eq!(drain.pop_next(), Some("reentrant-after-last-pop"));
        assert_eq!(drain.pop_next(), None);

        assert!(drain.enqueue("next-independent-drain"));
        assert_eq!(drain.pop_next(), Some("next-independent-drain"));
        assert_eq!(drain.pop_next(), None);

        assert!(drain.enqueue("cancelled"));
        drain.cancel();
        assert_eq!(drain.pop_next(), None);
        assert!(drain.enqueue("after-cancel"));
        assert_eq!(drain.pop_next(), Some("after-cancel"));
        assert_eq!(drain.pop_next(), None);
    }

    #[test]
    fn active_effect_completion_precedes_reentrant_moved_and_fullscreen_batches() {
        let mut drain = MacWindowSerialEffectDrain::default();
        let mut delivered = Vec::new();
        assert!(drain.enqueue("active"));

        while let Some(effect) = drain.pop_next() {
            if effect == "active" {
                delivered.push("active-start");
                assert!(!drain.enqueue("moved"));
                assert!(!drain.enqueue("fullscreen"));
                delivered.push("active-end");
            } else {
                delivered.push(effect);
            }
        }

        assert_eq!(
            delivered,
            vec!["active-start", "active-end", "moved", "fullscreen"]
        );
    }

    #[test]
    fn observation_effect_drain_continues_after_callback_panic_without_replay() {
        let mut drain = MacWindowSerialEffectDrain::default();
        assert!(drain.enqueue("already-applied"));
        assert!(!drain.enqueue("older-pending"));

        let mut panic_boundary = MacWindowCallbackPanicBoundary::default();
        let mut delivered = Vec::new();
        while let Some(effect) = drain.pop_next() {
            delivered.push(effect);
            panic_boundary.deliver(|| {
                if effect == "already-applied" {
                    assert!(!drain.enqueue("callback-reentrant"));
                    panic!("effect callback panic");
                }
            });
            if effect == "already-applied" {
                assert!(!drain.enqueue("after-panic"));
            }
        }

        assert_eq!(
            delivered,
            vec![
                "already-applied",
                "older-pending",
                "callback-reentrant",
                "after-panic",
            ]
        );
        let panic = panic_boundary.into_first_panic().unwrap();
        assert_eq!(
            panic.downcast_ref::<&'static str>(),
            Some(&"effect callback panic")
        );
        mem::forget(panic);

        assert!(drain.enqueue("next-independent-drain"));
        assert_eq!(drain.pop_next(), Some("next-independent-drain"));
        assert_eq!(drain.pop_next(), None);
    }

    #[test]
    fn callback_panic_boundary_continues_remaining_same_batch_effects_exactly_once() {
        let mut panic_boundary = MacWindowCallbackPanicBoundary::default();
        let mut delivered = Vec::new();

        panic_boundary.deliver(|| {
            delivered.push("resize");
            panic!("resize callback panic");
        });
        panic_boundary.deliver(|| delivered.push("moved"));
        panic_boundary.deliver(|| delivered.push("state"));

        assert_eq!(delivered, vec!["resize", "moved", "state"]);
        let panic = panic_boundary.into_first_panic().unwrap();
        assert_eq!(
            panic.downcast_ref::<&'static str>(),
            Some(&"resize callback panic")
        );
        mem::forget(panic);
    }

    #[test]
    fn callback_panic_boundary_never_drops_arbitrary_payloads() {
        struct PanicPayload(Arc<AtomicU64>);

        impl Drop for PanicPayload {
            fn drop(&mut self) {
                self.0.fetch_add(1, Ordering::Relaxed);
                panic!("panic payload destructor must remain isolated");
            }
        }

        let drops = Arc::new(AtomicU64::new(0));
        let mut panic_boundary = MacWindowCallbackPanicBoundary::default();
        for _ in 0..2 {
            let drops = Arc::clone(&drops);
            panic_boundary.deliver(|| std::panic::panic_any(PanicPayload(drops)));
        }

        panic_boundary.isolate_at_native_boundary("a test callback");
        assert_eq!(drops.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn request_frame_checkout_restores_callback_rendering_and_display_link_after_panic() {
        let calls = Rc::new(Cell::new(0));
        let callback_calls = Rc::clone(&calls);
        let presents_with_transaction = Rc::new(Cell::new(true));
        let display_link_running = Rc::new(Cell::new(false));
        let slot: Rc<std::cell::RefCell<Option<Box<dyn FnMut()>>>> =
            Rc::new(std::cell::RefCell::new(Some(Box::new(move || {
                let next = callback_calls.get() + 1;
                callback_calls.set(next);
                if next == 1 {
                    panic!("injected macOS window callback panic");
                }
            }))));

        let callback = slot
            .borrow_mut()
            .take()
            .expect("the callback must be installed before checkout");
        let restore_slot = Rc::clone(&slot);
        let restore_transaction = Rc::clone(&presents_with_transaction);
        let restore_display_link = Rc::clone(&display_link_running);
        let mut checkout = MacWindowCallbackCheckout::new(callback, move |callback| {
            restore_transaction.set(false);
            restore_display_link.set(true);
            let mut slot = restore_slot.borrow_mut();
            restore_callback_if_vacant(&mut slot, callback);
        });
        let mut panic_boundary = MacWindowCallbackPanicBoundary::default();
        panic_boundary.deliver(|| (checkout.callback())());
        panic_boundary.deliver(|| drop(checkout));

        let panic = panic_boundary.into_first_panic().unwrap();
        assert_eq!(
            panic.downcast_ref::<&'static str>(),
            Some(&"injected macOS window callback panic")
        );
        mem::forget(panic);
        let mut callback = slot
            .borrow_mut()
            .take()
            .expect("the panicking callback must be restored");
        callback();
        assert_eq!(calls.get(), 2);
        assert!(!presents_with_transaction.get());
        assert!(display_link_running.get());
    }

    #[test]
    fn request_frame_checkout_preserves_reentrant_replacement_and_restores_rendering() {
        struct CallbackDropPanic;

        impl Drop for CallbackDropPanic {
            fn drop(&mut self) {
                panic!("superseded request-frame callback destructor panic");
            }
        }

        let replacement_calls = Rc::new(Cell::new(0));
        let presents_with_transaction = Rc::new(Cell::new(true));
        let display_link_running = Rc::new(Cell::new(false));
        let slot: Rc<std::cell::RefCell<Option<Box<dyn FnMut()>>>> =
            Rc::new(std::cell::RefCell::new(None));
        let slot_from_callback = Rc::clone(&slot);
        let replacement_calls_from_callback = Rc::clone(&replacement_calls);
        let callback_drop_panic = CallbackDropPanic;
        *slot.borrow_mut() = Some(Box::new(move || {
            let _keep_drop_guard_captured = &callback_drop_panic;
            let replacement_calls = Rc::clone(&replacement_calls_from_callback);
            *slot_from_callback.borrow_mut() = Some(Box::new(move || {
                replacement_calls.set(replacement_calls.get() + 1);
            }));
            panic!("injected request-frame panic after reentrant replacement");
        }));

        let callback = slot
            .borrow_mut()
            .take()
            .expect("the callback must be installed before checkout");
        let restore_slot = Rc::clone(&slot);
        let restore_transaction = Rc::clone(&presents_with_transaction);
        let restore_display_link = Rc::clone(&display_link_running);
        let mut checkout = MacWindowCallbackCheckout::new(callback, move |callback| {
            restore_transaction.set(false);
            restore_display_link.set(true);
            let mut slot = restore_slot.borrow_mut();
            restore_callback_if_vacant(&mut slot, callback);
        });
        let mut panic_boundary = MacWindowCallbackPanicBoundary::default();
        panic_boundary.deliver(|| (checkout.callback())());
        panic_boundary.deliver(|| drop(checkout));

        let panic = panic_boundary.into_first_panic().unwrap();
        assert_eq!(
            panic.downcast_ref::<&'static str>(),
            Some(&"injected request-frame panic after reentrant replacement")
        );
        mem::forget(panic);
        let mut replacement = slot
            .borrow_mut()
            .take()
            .expect("the reentrant replacement must remain authoritative");
        replacement();
        assert_eq!(replacement_calls.get(), 1);
        assert!(!presents_with_transaction.get());
        assert!(display_link_running.get());
    }

    #[test]
    fn activate_callback_checkout_restores_the_callback_after_panic() {
        let calls = Rc::new(Cell::new(0));
        let callback_calls = Rc::clone(&calls);
        let slot: Rc<std::cell::RefCell<Option<Box<dyn FnMut()>>>> =
            Rc::new(std::cell::RefCell::new(Some(Box::new(move || {
                let next = callback_calls.get() + 1;
                callback_calls.set(next);
                if next == 1 {
                    panic!("injected activate callback panic");
                }
            }))));

        let callback = slot
            .borrow_mut()
            .take()
            .expect("the callback must be installed before checkout");
        let restore_slot = Rc::clone(&slot);
        let mut checkout = MacWindowCallbackCheckout::new(callback, move |callback| {
            let mut slot = restore_slot.borrow_mut();
            restore_callback_if_vacant(&mut slot, callback);
        });
        let mut panic_boundary = MacWindowCallbackPanicBoundary::default();
        panic_boundary.deliver(|| (checkout.callback())());
        panic_boundary.deliver(|| drop(checkout));

        let panic = panic_boundary.into_first_panic().unwrap();
        assert_eq!(
            panic.downcast_ref::<&'static str>(),
            Some(&"injected activate callback panic")
        );
        mem::forget(panic);
        let mut callback = slot
            .borrow_mut()
            .take()
            .expect("the panicking activate callback must be restored");
        callback();
        assert_eq!(calls.get(), 2);
    }

    #[test]
    fn activate_callback_checkout_preserves_a_reentrant_replacement_during_unwind() {
        let replacement_calls = Rc::new(Cell::new(0));
        let slot: Rc<std::cell::RefCell<Option<Box<dyn FnMut()>>>> =
            Rc::new(std::cell::RefCell::new(None));
        let slot_from_callback = Rc::clone(&slot);
        let replacement_calls_from_callback = Rc::clone(&replacement_calls);
        *slot.borrow_mut() = Some(Box::new(move || {
            let replacement_calls = Rc::clone(&replacement_calls_from_callback);
            *slot_from_callback.borrow_mut() = Some(Box::new(move || {
                replacement_calls.set(replacement_calls.get() + 1);
            }));
            panic!("injected callback panic after reentrant replacement");
        }));

        let callback = slot
            .borrow_mut()
            .take()
            .expect("the callback must be installed before checkout");
        let restore_slot = Rc::clone(&slot);
        let mut checkout = MacWindowCallbackCheckout::new(callback, move |callback| {
            let mut slot = restore_slot.borrow_mut();
            restore_callback_if_vacant(&mut slot, callback);
        });
        let mut panic_boundary = MacWindowCallbackPanicBoundary::default();
        panic_boundary.deliver(|| (checkout.callback())());
        panic_boundary.deliver(|| drop(checkout));

        let panic = panic_boundary.into_first_panic().unwrap();
        assert_eq!(
            panic.downcast_ref::<&'static str>(),
            Some(&"injected callback panic after reentrant replacement")
        );
        mem::forget(panic);
        let mut replacement = slot
            .borrow_mut()
            .take()
            .expect("the reentrant replacement must remain authoritative");
        replacement();
        assert_eq!(replacement_calls.get(), 1);
    }

    #[test]
    fn observation_effect_batches_preserve_cross_domain_native_order() {
        let mut events: MacWindowPendingObservationEvents<u8> =
            MacWindowPendingObservationEvents::default();
        events.record(MacWindowPendingObservationEvent::observed(
            MacWindowObservationEvent::Active(MacWindowActiveEvent::BecameKey),
            1,
        ));
        events.record(MacWindowPendingObservationEvent::observed(
            MacWindowObservationEvent::Moved,
            2,
        ));
        events.record(MacWindowPendingObservationEvent::observed(
            MacWindowObservationEvent::Fullscreen(MacFullscreenTransitionTerminal::Entered),
            3,
        ));

        let batches = events.into_effect_batches(4);
        assert_eq!(
            batches
                .iter()
                .map(|batch| batch.observation)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        assert_eq!(
            batches
                .iter()
                .map(|batch| batch.event.map(|event| event.kind))
                .collect::<Vec<_>>(),
            vec![
                Some(MacWindowObservationEvent::Active(
                    MacWindowActiveEvent::BecameKey,
                )),
                Some(MacWindowObservationEvent::Moved),
                Some(MacWindowObservationEvent::Fullscreen(
                    MacFullscreenTransitionTerminal::Entered,
                )),
                None,
            ]
        );
    }

    #[test]
    fn paused_state_edges_preserve_maximize_restore_and_minimize_restore_order() {
        let zoomed = MacWindowStateExpectation::new(false, true);
        assert_eq!(
            MacWindowStateEventSource::Miniaturized.expected_state(zoomed),
            MacWindowStateExpectation::new(true, true)
        );
        assert_eq!(
            MacWindowStateEventSource::Deminiaturized
                .expected_state(MacWindowStateExpectation::new(true, true)),
            zoomed
        );

        let mut events: MacWindowPendingObservationEvents<u8> =
            MacWindowPendingObservationEvents::default();
        let cases = [
            MacWindowStateEvent::new(
                MacWindowStateEventSource::Resized,
                MacWindowStateExpectation::new(false, true),
            ),
            MacWindowStateEvent::new(
                MacWindowStateEventSource::Resized,
                MacWindowStateExpectation::new(false, false),
            ),
            MacWindowStateEvent::new(
                MacWindowStateEventSource::Miniaturized,
                MacWindowStateExpectation::new(true, false),
            ),
            MacWindowStateEvent::new(
                MacWindowStateEventSource::Deminiaturized,
                MacWindowStateExpectation::new(false, false),
            ),
        ];

        for (observation, event) in (1_u8..).zip(cases) {
            events.record(MacWindowPendingObservationEvent::observed(
                MacWindowObservationEvent::State(event),
                observation,
            ));
        }

        assert_eq!(
            events
                .events
                .iter()
                .map(|event| event.kind)
                .collect::<Vec<_>>(),
            cases.map(MacWindowObservationEvent::State)
        );
        assert_eq!(
            events
                .events
                .iter()
                .map(|event| event.observation)
                .collect::<Vec<_>>(),
            vec![Some(1), Some(2), Some(3), Some(4)]
        );
        assert_eq!(
            events
                .into_effect_batches(5)
                .into_iter()
                .map(|batch| batch.observation)
                .collect::<Vec<_>>(),
            vec![1, 2, 3, 4, 5]
        );
    }

    #[test]
    fn coalesced_resize_never_replaces_a_complete_fact_with_an_unobserved_candidate() {
        let mut events: MacWindowPendingObservationEvents<u8> =
            MacWindowPendingObservationEvents::default();
        let state_event = MacWindowObservationEvent::State(MacWindowStateEvent::new(
            MacWindowStateEventSource::Resized,
            MacWindowStateExpectation::new(false, true),
        ));
        events.record(MacWindowPendingObservationEvent::observed(state_event, 7));
        events.record(MacWindowPendingObservationEvent::unobserved(state_event));

        assert_eq!(events.events.len(), 1);
        assert_eq!(events.events[0].observation, Some(7));

        events.record(MacWindowPendingObservationEvent::observed(state_event, 8));
        assert_eq!(events.events.len(), 1);
        assert_eq!(events.events[0].observation, Some(8));
    }

    #[test]
    fn unresolved_fullscreen_terminals_use_latest_wins_without_duplicate_final_callbacks() {
        let mut events: MacWindowPendingObservationEvents<()> =
            MacWindowPendingObservationEvents::default();
        events.record(MacWindowPendingObservationEvent::unobserved(
            MacWindowObservationEvent::Fullscreen(MacFullscreenTransitionTerminal::Entered),
        ));
        events.record(MacWindowPendingObservationEvent::unobserved(
            MacWindowObservationEvent::Active(MacWindowActiveEvent::ResignedKey),
        ));
        events.record(MacWindowPendingObservationEvent::unobserved(
            MacWindowObservationEvent::Fullscreen(MacFullscreenTransitionTerminal::Exited),
        ));

        assert_eq!(events.fullscreen_terminal_count(), 1);
        assert_eq!(
            events
                .events
                .iter()
                .map(|event| event.kind)
                .collect::<Vec<_>>(),
            vec![
                MacWindowObservationEvent::Active(MacWindowActiveEvent::ResignedKey),
                MacWindowObservationEvent::Fullscreen(MacFullscreenTransitionTerminal::Exited),
            ]
        );
        assert_eq!(
            events
                .into_effect_batches(())
                .into_iter()
                .filter(|batch| {
                    matches!(
                        batch.event.map(|event| event.kind),
                        Some(MacWindowObservationEvent::Fullscreen(_))
                    )
                })
                .count(),
            1
        );
    }

    #[test]
    fn observed_fullscreen_terminals_keep_their_own_complete_facts() {
        let mut events: MacWindowPendingObservationEvents<u8> =
            MacWindowPendingObservationEvents::default();
        events.record(MacWindowPendingObservationEvent::observed(
            MacWindowObservationEvent::Fullscreen(MacFullscreenTransitionTerminal::Entered),
            1,
        ));
        events.record(MacWindowPendingObservationEvent::observed(
            MacWindowObservationEvent::Fullscreen(MacFullscreenTransitionTerminal::Exited),
            2,
        ));

        assert_eq!(events.fullscreen_terminal_count(), 2);
        assert_eq!(
            events
                .events
                .iter()
                .map(|event| event.observation)
                .collect::<Vec<_>>(),
            vec![Some(1), Some(2)]
        );
        assert_eq!(
            events
                .into_effect_batches(2)
                .into_iter()
                .map(|batch| batch.observation)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
    }

    #[test]
    fn observation_commit_waits_for_the_target_topology_generation() {
        let mut coordinator = MacWindowObservationCommitCoordinator::default();
        let first_epoch = coordinator
            .request(
                4,
                Some(MacWindowPendingObservationEvent::unobserved(
                    MacWindowObservationEvent::Active(MacWindowActiveEvent::BecameKey),
                )),
            )
            .unwrap();

        assert_eq!(coordinator.target_for_job(first_epoch), Some(4));
        assert!(coordinator.commit(first_epoch, 3).is_none());
        assert_eq!(coordinator.target_for_job(first_epoch), Some(4));

        let replacement_epoch = coordinator.request(5, None).unwrap();
        assert_ne!(replacement_epoch, first_epoch);
        assert_eq!(coordinator.target_for_job(first_epoch), None);
        assert_eq!(coordinator.target_for_job(replacement_epoch), Some(5));

        let events = coordinator.commit(replacement_epoch, 5).unwrap();
        assert_eq!(
            events
                .events
                .iter()
                .map(|event| event.kind)
                .collect::<Vec<_>>(),
            vec![MacWindowObservationEvent::Active(
                MacWindowActiveEvent::BecameKey
            )]
        );
        assert!(coordinator.commit(replacement_epoch, 5).is_none());
        assert!(coordinator.pending_events.events.is_empty());
    }

    #[test]
    fn paused_observation_commit_rearms_without_dropping_typed_events() {
        let mut coordinator = MacWindowObservationCommitCoordinator::default();
        let first_epoch = coordinator
            .request(
                8,
                Some(MacWindowPendingObservationEvent::unobserved(
                    MacWindowObservationEvent::Fullscreen(
                        MacFullscreenTransitionTerminal::FailedToEnter,
                    ),
                )),
            )
            .unwrap();
        assert_eq!(
            coordinator.request(
                8,
                Some(MacWindowPendingObservationEvent::unobserved(
                    MacWindowObservationEvent::Active(MacWindowActiveEvent::BecameKey),
                )),
            ),
            None
        );

        coordinator.pause(first_epoch);
        assert_eq!(coordinator.target_for_job(first_epoch), None);

        let rearmed_epoch = coordinator.request(8, None).unwrap();
        assert_ne!(rearmed_epoch, first_epoch);
        let events = coordinator.commit(rearmed_epoch, 8).unwrap();
        assert_eq!(events.fullscreen_terminal_count(), 1);
        assert_eq!(
            events
                .events
                .iter()
                .map(|event| event.kind)
                .collect::<Vec<_>>(),
            vec![
                MacWindowObservationEvent::Fullscreen(
                    MacFullscreenTransitionTerminal::FailedToEnter,
                ),
                MacWindowObservationEvent::Active(MacWindowActiveEvent::BecameKey),
            ]
        );
        assert!(coordinator.commit(rearmed_epoch, 8).is_none());
    }

    #[test]
    fn observation_commit_retries_with_bounded_delayed_backoff() {
        let mut coordinator = MacWindowObservationCommitCoordinator::default();
        let job_epoch = coordinator.request(2, None).unwrap();
        let delays = (0..7)
            .map(|_| coordinator.retry_delay(job_epoch).unwrap())
            .collect::<Vec<_>>();

        assert_eq!(
            delays,
            vec![
                Duration::from_millis(16),
                Duration::from_millis(64),
                Duration::from_millis(250),
                Duration::from_secs(1),
                Duration::from_secs(2),
                Duration::from_secs(2),
                Duration::from_secs(2),
            ]
        );

        let replacement_epoch = coordinator.request(3, None).unwrap();
        assert_eq!(
            coordinator.retry_delay(replacement_epoch),
            Some(Duration::from_millis(16))
        );
    }

    #[test]
    fn observation_commit_preserves_active_and_state_edges_and_coalesces_moves() {
        let mut coordinator = MacWindowObservationCommitCoordinator::default();
        let job_epoch = coordinator
            .request(
                3,
                Some(MacWindowPendingObservationEvent::unobserved(
                    MacWindowObservationEvent::Active(MacWindowActiveEvent::BecameKey),
                )),
            )
            .unwrap();
        assert_eq!(
            coordinator.request(
                3,
                Some(MacWindowPendingObservationEvent::unobserved(
                    MacWindowObservationEvent::State(MacWindowStateEvent::new(
                        MacWindowStateEventSource::Resized,
                        MacWindowStateExpectation::new(false, true),
                    )),
                )),
            ),
            None
        );
        assert_eq!(
            coordinator.request(
                3,
                Some(MacWindowPendingObservationEvent::unobserved(
                    MacWindowObservationEvent::Active(MacWindowActiveEvent::ResignedKey),
                )),
            ),
            None
        );
        assert_eq!(
            coordinator.request(
                3,
                Some(MacWindowPendingObservationEvent::unobserved(
                    MacWindowObservationEvent::State(MacWindowStateEvent::new(
                        MacWindowStateEventSource::Miniaturized,
                        MacWindowStateExpectation::new(true, false),
                    )),
                )),
            ),
            None
        );
        assert_eq!(
            coordinator.request(
                3,
                Some(MacWindowPendingObservationEvent::unobserved(
                    MacWindowObservationEvent::Moved,
                )),
            ),
            None
        );
        assert_eq!(
            coordinator.request(
                3,
                Some(MacWindowPendingObservationEvent::unobserved(
                    MacWindowObservationEvent::Moved,
                )),
            ),
            None
        );

        let events = coordinator.commit(job_epoch, 3).unwrap();
        assert_eq!(events.last_active_event_index(), Some(2));
        assert!(events.has_window_state_event());
        assert!(events.has_moved_event());
        assert_eq!(
            events
                .events
                .iter()
                .map(|event| event.kind)
                .collect::<Vec<_>>(),
            vec![
                MacWindowObservationEvent::Active(MacWindowActiveEvent::BecameKey),
                MacWindowObservationEvent::State(MacWindowStateEvent::new(
                    MacWindowStateEventSource::Resized,
                    MacWindowStateExpectation::new(false, true),
                )),
                MacWindowObservationEvent::Active(MacWindowActiveEvent::ResignedKey),
                MacWindowObservationEvent::State(MacWindowStateEvent::new(
                    MacWindowStateEventSource::Miniaturized,
                    MacWindowStateExpectation::new(true, false),
                )),
                MacWindowObservationEvent::Moved,
            ]
        );
    }

    #[test]
    fn closing_cancels_pending_observation_work_and_events() {
        let mut coordinator = MacWindowObservationCommitCoordinator::default();
        let job_epoch = coordinator
            .request(
                6,
                Some(MacWindowPendingObservationEvent::unobserved(
                    MacWindowObservationEvent::Active(MacWindowActiveEvent::ResignedKey),
                )),
            )
            .unwrap();

        coordinator.cancel();

        assert_eq!(coordinator.target_for_job(job_epoch), None);
        assert!(coordinator.pending_events.events.is_empty());
        assert!(coordinator.commit(job_epoch, 6).is_none());
    }

    #[test]
    fn renderer_geometry_uses_frame_callback_size_until_a_complete_fact_commits() {
        let initial = MacRendererGeometry {
            content_size: size(px(800.0), px(600.0)),
            scale_factor: 2.0,
        };
        let new_size = size(px(1440.0), px(900.0));
        let resized = renderer_geometry_for_frame_size(initial, new_size);

        assert_eq!(resized.content_size, new_size);
        assert_eq!(resized.scale_factor, initial.scale_factor);
        assert!(resized.differs_from(initial));
        assert!(!initial.differs_from(initial));
    }

    #[test]
    fn fullscreen_delegate_selectors_cover_success_and_failure_terminals() {
        let cases = [
            (
                sel!(windowDidEnterFullScreen:),
                MacFullscreenTransitionTerminal::Entered,
            ),
            (
                sel!(windowDidExitFullScreen:),
                MacFullscreenTransitionTerminal::Exited,
            ),
            (
                sel!(windowDidFailToEnterFullScreen:),
                MacFullscreenTransitionTerminal::FailedToEnter,
            ),
            (
                sel!(windowDidFailToExitFullScreen:),
                MacFullscreenTransitionTerminal::FailedToExit,
            ),
        ];

        for (selector, terminal) in cases {
            assert_eq!(
                fullscreen_terminal_for_delegate_selector(selector),
                Some(terminal)
            );
        }
        assert_eq!(
            fullscreen_terminal_for_delegate_selector(sel!(description)),
            None
        );
    }

    #[test]
    fn observation_delegate_selectors_map_to_typed_events() {
        assert_eq!(
            active_event_for_delegate_selector(sel!(windowDidBecomeKey:)),
            Some(MacWindowActiveEvent::BecameKey)
        );
        assert_eq!(
            active_event_for_delegate_selector(sel!(windowDidResignKey:)),
            Some(MacWindowActiveEvent::ResignedKey)
        );
        assert!(MacWindowActiveEvent::BecameKey.is_active());
        assert!(!MacWindowActiveEvent::ResignedKey.is_active());

        assert_eq!(
            state_event_for_delegate_selector(sel!(windowDidResize:)),
            Some(MacWindowStateEventSource::Resized)
        );
        assert_eq!(
            state_event_for_delegate_selector(sel!(windowDidMiniaturize:)),
            Some(MacWindowStateEventSource::Miniaturized)
        );
        assert_eq!(
            state_event_for_delegate_selector(sel!(windowDidDeminiaturize:)),
            Some(MacWindowStateEventSource::Deminiaturized)
        );
        assert_eq!(active_event_for_delegate_selector(sel!(description)), None);
        assert_eq!(state_event_for_delegate_selector(sel!(description)), None);
    }

    #[test]
    fn fullscreen_terminals_clear_transition_and_restore_titlebar_policy() {
        let cases = [
            (
                MacFullscreenTransition::Entering,
                MacFullscreenTransitionTerminal::Entered,
                true,
            ),
            (
                MacFullscreenTransition::Exiting,
                MacFullscreenTransitionTerminal::Exited,
                false,
            ),
            (
                MacFullscreenTransition::Entering,
                MacFullscreenTransitionTerminal::FailedToEnter,
                false,
            ),
            (
                MacFullscreenTransition::Exiting,
                MacFullscreenTransitionTerminal::FailedToExit,
                true,
            ),
        ];

        for (pending, terminal, is_fullscreen) in cases {
            let mut transition = Some(pending);
            assert!(terminal.finish(&mut transition));
            assert_eq!(transition, None);
            assert_eq!(terminal.is_fullscreen(), is_fullscreen);
            assert_eq!(terminal.titlebar_appears_transparent(true), !is_fullscreen);
            assert!(!terminal.titlebar_appears_transparent(false));
        }

        let mut mismatched_transition = Some(MacFullscreenTransition::Entering);
        let mut published_side_effects = 0;
        if MacFullscreenTransitionTerminal::FailedToExit.finish(&mut mismatched_transition) {
            published_side_effects += 1;
        }
        assert_eq!(
            mismatched_transition,
            Some(MacFullscreenTransition::Entering)
        );
        assert_eq!(published_side_effects, 0);
    }

    #[test]
    fn creation_projection_separates_first_appearance_from_lifetime_activation() {
        let restore_bounds = restore_bounds();

        let windowed = MacWindowCreationProjection::new(
            WindowBounds::Windowed(restore_bounds),
            &WindowKind::Normal,
            true,
            true,
            WindowActivationPolicy::default(),
        );
        assert_eq!(windowed.bounds, restore_bounds);
        assert_eq!(windowed.state, MacWindowCreationState::Windowed);
        assert_eq!(windowed.restore_bounds, restore_bounds);
        assert!(windowed.accepts_pointer_input);
        assert!(windowed.focus_on_appearing);
        assert_eq!(
            windowed.activation_policy,
            WindowActivationPolicy::default()
        );
        assert!(!windowed.topmost);
        assert!(windowed.taskbar_visible);

        let maximized = MacWindowCreationProjection::new(
            WindowBounds::Maximized(restore_bounds),
            &WindowKind::Floating,
            false,
            false,
            WindowActivationPolicy {
                accepts_activation: true,
                focus_on_click: false,
            },
        );
        assert_eq!(maximized.bounds, restore_bounds);
        assert_eq!(maximized.state, MacWindowCreationState::Maximized);
        assert_eq!(maximized.restore_bounds, restore_bounds);
        assert!(!maximized.accepts_pointer_input);
        assert!(!maximized.focus_on_appearing);
        assert_eq!(
            maximized.activation_policy,
            WindowActivationPolicy {
                accepts_activation: true,
                focus_on_click: false,
            }
        );
        assert!(maximized.topmost);
        assert!(!maximized.taskbar_visible);

        let fullscreen = MacWindowCreationProjection::new(
            WindowBounds::Fullscreen(restore_bounds),
            &WindowKind::Normal,
            true,
            false,
            WindowActivationPolicy {
                accepts_activation: false,
                focus_on_click: true,
            },
        );
        assert_eq!(fullscreen.state, MacWindowCreationState::Fullscreen);
        assert_eq!(fullscreen.restore_bounds, restore_bounds);
        assert!(!fullscreen.activation_policy.accepts_activation);
        assert!(fullscreen.activation_policy.focus_on_click);
    }

    #[test]
    fn toplevel_creation_preserves_all_activation_policy_pairs_atomically() {
        for accepts_activation in [false, true] {
            for focus_on_click in [false, true] {
                let requested = WindowActivationPolicy {
                    accepts_activation,
                    focus_on_click,
                };
                let projection = MacWindowCreationProjection::new(
                    WindowBounds::Windowed(restore_bounds()),
                    &WindowKind::Normal,
                    true,
                    false,
                    requested,
                );

                assert_eq!(projection.activation_policy, requested);
                assert_eq!(projection.requires_nonactivating_panel(), !focus_on_click);
                assert_eq!(projection.becomes_key_only_if_needed(), !focus_on_click);
                assert!(!projection.panel_hides_on_deactivate());
                assert_eq!(
                    macos_click_can_activate(projection.activation_policy),
                    focus_on_click
                );
            }
        }
    }

    #[test]
    fn initial_presentation_tracks_automatic_tabbing() {
        let regular = MacInitialPresentation {
            show: true,
            allows_automatic_window_tabbing: false,
            state: MacWindowCreationState::Windowed,
            mapped: false,
            completed: false,
        };
        assert!(!regular.should_apply_automatic_tabbing());

        let tabbed = MacInitialPresentation {
            allows_automatic_window_tabbing: true,
            ..regular
        };
        assert!(tabbed.should_apply_automatic_tabbing());
    }

    #[test]
    fn occluded_draws_defer_after_the_first_attempt() {
        assert!(!should_defer_occluded_draw(false, false));
        assert!(!should_defer_occluded_draw(false, true));
        assert!(should_defer_occluded_draw(true, false));
        assert!(!should_defer_occluded_draw(true, true));
    }

    #[test]
    fn non_toplevel_kinds_do_not_project_unsupported_native_states() {
        let projection = MacWindowCreationProjection::new(
            WindowBounds::Fullscreen(restore_bounds()),
            &WindowKind::PopUp,
            true,
            true,
            WindowActivationPolicy::default(),
        );

        assert_eq!(projection.state, MacWindowCreationState::Windowed);
        assert_eq!(projection.bounds, restore_bounds());
        assert!(!projection.focus_on_appearing);
        assert_eq!(
            projection.activation_policy,
            WindowActivationPolicy {
                accepts_activation: false,
                focus_on_click: false,
            }
        );
        assert!(projection.topmost);
        assert!(!projection.taskbar_visible);
        assert!(projection.panel_hides_on_deactivate());

        let dialog = MacWindowCreationProjection::new(
            WindowBounds::Maximized(restore_bounds()),
            &WindowKind::Dialog,
            true,
            false,
            WindowActivationPolicy {
                accepts_activation: true,
                focus_on_click: false,
            },
        );
        assert_eq!(dialog.state, MacWindowCreationState::Windowed);
        assert_eq!(dialog.activation_policy, WindowActivationPolicy::default());
        assert!(!dialog.topmost);
        assert!(!dialog.taskbar_visible);
        assert!(dialog.panel_hides_on_deactivate());
        assert!(dialog.focus_on_appearing);
    }

    #[test]
    fn background_projection_covers_opaque_alpha_and_blur_creation() {
        let opaque = MacWindowBackgroundProjection::new(WindowBackgroundAppearance::Opaque);
        assert!(opaque.native_opaque);
        assert!(!opaque.renderer_transparent);
        assert_eq!(opaque.background_alpha, 1.0);
        assert!(!opaque.blur_enabled);

        let transparent =
            MacWindowBackgroundProjection::new(WindowBackgroundAppearance::Transparent);
        assert!(!transparent.native_opaque);
        assert!(transparent.renderer_transparent);
        assert_eq!(transparent.background_alpha, 0.0001);
        assert!(!transparent.blur_enabled);

        let blurred = MacWindowBackgroundProjection::new(WindowBackgroundAppearance::Blurred);
        assert!(!blurred.native_opaque);
        assert!(blurred.renderer_transparent);
        assert!(blurred.blur_enabled);

        for appearance in [
            WindowBackgroundAppearance::MicaBackdrop,
            WindowBackgroundAppearance::MicaAltBackdrop,
        ] {
            let projection = MacWindowBackgroundProjection::new(appearance);
            assert_eq!(
                projection.appearance,
                WindowBackgroundAppearance::Transparent
            );
            assert!(!projection.native_opaque);
            assert!(projection.renderer_transparent);
            assert!(!projection.blur_enabled);
        }
    }
}
