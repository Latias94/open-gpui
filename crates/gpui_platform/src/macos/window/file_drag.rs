use super::{MacWindowState, NSDragOperation, get_window_state, native_window_matches_handle};
use block2::RcBlock;
use objc2::{
    AnyThread, MainThreadMarker, Message,
    rc::Retained,
    runtime::{AnyObject, ProtocolObject, Sel},
};
use objc2_app_kit::{
    NSDraggingContext, NSDraggingImageComponent, NSDraggingImageComponentIconKey, NSDraggingItem,
    NSPasteboardWriting, NSWorkspace,
};
use objc2_foundation::{
    NSArray, NSInteger, NSMutableArray, NSPoint, NSRect, NSSize, NSString, NSUInteger, NSURL,
};
use objc2_uniform_type_identifiers::{UTType, UTTypeData, UTTypeFolder};
use open_gpui::{
    ExternalDragPayload, FileDropEvent, MouseButton, PlatformExternalDragSession, PlatformInput,
};
use parking_lot::Mutex;
use std::{cell::OnceCell, ffi::CString, os::unix::ffi::OsStrExt, ptr::NonNull, rc::Rc, sync::Arc};

pub(super) struct MacExternalDrag {
    session: PlatformExternalDragSession,
    mouse_down_event: Retained<AnyObject>,
    native_session: OnceCell<MacNativeDragSession>,
    synthetic_drag_counter: usize,
}

struct MacNativeDragSession {
    object: Retained<AnyObject>,
    sequence: NSInteger,
}

impl MacExternalDrag {
    unsafe fn bind_native_session(&self, native_session: *mut AnyObject) -> bool {
        // SAFETY: AppKit supplies a live NSDraggingSession on its main thread. Retaining it
        // keeps the native identity valid until completion or window retirement.
        let Some(object) = (unsafe { Retained::retain(native_session.cast::<AnyObject>()) }) else {
            return false;
        };
        let sequence = unsafe { objc2::msg_send![&*object, draggingSequenceNumber] };
        if let Some(current) = self.native_session.get() {
            return Retained::as_ptr(&current.object) == Retained::as_ptr(&object)
                && current.sequence == sequence;
        }
        self.native_session
            .set(MacNativeDragSession { object, sequence })
            .is_ok()
    }

    fn matches_sequence(&self, sequence: NSInteger) -> bool {
        self.native_session
            .get()
            .is_some_and(|native| native.sequence == sequence)
    }

    fn matches_native_session(&self, native_session: *mut AnyObject, sequence: NSInteger) -> bool {
        self.native_session.get().is_some_and(|native| {
            Retained::as_ptr(&native.object)
                .cast_mut()
                .cast::<AnyObject>()
                == native_session
                && native.sequence == sequence
        })
    }
}

pub(super) fn update_left_mouse_down_event(
    state: &mut MacWindowState,
    native_event: *mut AnyObject,
    event: &PlatformInput,
) {
    match event {
        PlatformInput::MouseDown(event) if event.button == MouseButton::Left => {
            // SAFETY: AppKit owns native_event for this callback. Retain it so the source
            // session can start after the pointer has left the originating view.
            state.last_left_mouse_down_event =
                unsafe { Retained::retain(native_event.cast::<AnyObject>()) };
        }
        PlatformInput::MouseUp(event) if event.button == MouseButton::Left => {
            state.last_left_mouse_down_event = None;
        }
        _ => {}
    }
}

pub(super) fn start_external_drag(
    window_state: &Arc<Mutex<MacWindowState>>,
    payload: &ExternalDragPayload,
    session: PlatformExternalDragSession,
) -> bool {
    let ExternalDragPayload::Files(paths) = payload;
    if paths.entries().is_empty() || MainThreadMarker::new().is_none() {
        return false;
    }

    let (native_view, native_window, mouse_down_event, handle) = {
        let state = window_state.lock();
        if state.is_closed() || state.interaction_is_quiesced() || state.external_drag.is_some() {
            return false;
        }
        let Some(event) = state.last_left_mouse_down_event.clone() else {
            return false;
        };
        // SAFETY: MacWindowState owns both AppKit objects. The retained snapshots outlive
        // reentrant native callbacks, including callbacks that retire the window.
        let (Some(native_view), Some(native_window)) = (unsafe {
            (
                Retained::retain(state.native_view.as_ptr().cast::<AnyObject>()),
                Retained::retain(state.native_window.cast::<AnyObject>()),
            )
        }) else {
            return false;
        };
        (native_view, native_window, event, state.handle)
    };

    // SAFETY: This is the AppKit main thread. All Objective-C objects are retained for
    // their calls, and the window-state mutex is released before native drag tracking.
    unsafe {
        let location: objc2_foundation::NSPoint =
            objc2::msg_send![&*mouse_down_event, locationInWindow];
        // AppKit keeps this frame's distance from the event's location as the drag image's
        // offset from the cursor, so it has to stay anchored on `event`.
        let frame = NSRect::new(
            objc2_foundation::NSPoint::new(location.x - 16., location.y - 16.),
            NSSize::new(32., 32.),
        );
        let dragging_items = NSMutableArray::<NSDraggingItem>::new();
        for (path, is_directory) in paths.entries() {
            // Preserve non-UTF-8 paths.
            let Ok(path_bytes) = CString::new(path.as_os_str().as_bytes()) else {
                log::warn!("start_external_drag skipped path containing an interior nul byte");
                continue;
            };
            let path_bytes = NonNull::new_unchecked(path_bytes.as_ptr().cast_mut());
            let url = NSURL::fileURLWithFileSystemRepresentation_isDirectory_relativeToURL(
                path_bytes,
                *is_directory,
                None,
            );
            let pasteboard_writer = ProtocolObject::<dyn NSPasteboardWriting>::from_ref(&*url);
            let item = NSDraggingItem::initWithPasteboardWriter(
                NSDraggingItem::alloc(),
                pasteboard_writer,
            );

            // Resolve drag images lazily via `imageComponentsProvider` (Apple's
            // recommendation for large item counts), and by file *type* rather than
            // `iconForFile:`, which can synchronously hit LaunchServices, network
            // mounts, or iCloud for every selected path and beachball drag startup.
            let is_directory = *is_directory;
            let extension = path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(String::from);
            let provider = RcBlock::new(move || {
                let component = NSDraggingImageComponent::draggingImageComponentWithKey(
                    NSDraggingImageComponentIconKey,
                );
                let workspace = NSWorkspace::sharedWorkspace();
                let content_type = if is_directory {
                    UTTypeFolder.retain()
                } else {
                    extension
                        .as_ref()
                        .and_then(|extension| {
                            UTType::typeWithFilenameExtension(&NSString::from_str(extension))
                        })
                        .unwrap_or_else(|| UTTypeData.retain())
                };
                let icon = workspace.iconForContentType(&content_type);
                component.setContents(Some(&icon));
                // Component frames are relative to the item's dragging frame.
                component.setFrame(NSRect::new(
                    objc2_foundation::NSPoint::new(0., 0.),
                    NSSize::new(32., 32.),
                ));
                let components = NSArray::from_slice(&[&*component]);
                NonNull::new_unchecked(Retained::autorelease_return(components))
            });
            item.setDraggingFrame(frame);
            item.setImageComponentsProvider(Some(&provider));
            dragging_items.addObject(&item);
        }
        if dragging_items.count() == 0 {
            return false;
        }

        let native_window_ptr = Retained::as_ptr(&native_window)
            .cast_mut()
            .cast::<AnyObject>();
        if !native_window_matches_handle(native_window_ptr, handle) {
            return false;
        }
        let source = {
            let mut state = window_state.lock();
            if state.is_closed()
                || state.interaction_is_quiesced()
                || state.external_drag.is_some()
                || state
                    .last_left_mouse_down_event
                    .as_ref()
                    .is_none_or(|event| {
                        Retained::as_ptr(event) != Retained::as_ptr(&mouse_down_event)
                    })
            {
                return false;
            }
            state.synthetic_drag_counter += 1;
            let source = Rc::new(MacExternalDrag {
                session,
                mouse_down_event,
                native_session: OnceCell::new(),
                synthetic_drag_counter: state.synthetic_drag_counter,
            });
            state.external_drag = Some(source.clone());
            source
        };
        let native_session: *mut AnyObject = objc2::msg_send![
            &*native_view,
            beginDraggingSessionWithItems: &*dragging_items,
            event: &*source.mouse_down_event,
            source: &*native_window
        ];
        let started = source.bind_native_session(native_session.cast());
        if !started {
            let retired = {
                let mut state = window_state.lock();
                if state
                    .external_drag
                    .as_ref()
                    .is_some_and(|current| Rc::ptr_eq(current, &source))
                {
                    state.external_drag.take()
                } else {
                    None
                }
            };
            drop(retired);
        }
        started
    }
}

pub(super) fn is_drag_from_this_window(this: &AnyObject, dragging_info: *mut AnyObject) -> bool {
    // SAFETY: AppKit supplies NSDraggingInfo for the live destination callback.
    let source: *mut AnyObject =
        unsafe { objc2::msg_send![&*dragging_info.cast::<AnyObject>(), draggingSource] };
    source.cast::<AnyObject>().cast_const() == this as *const AnyObject
}

fn admit_file_drop(
    current: &mut Option<NSInteger>,
    sequence: NSInteger,
    event: &FileDropEvent,
) -> bool {
    match event {
        FileDropEvent::Entered { .. } => {
            *current = Some(sequence);
            true
        }
        FileDropEvent::Exited | FileDropEvent::Ended => {
            if *current != Some(sequence) {
                return false;
            }
            *current = None;
            true
        }
        _ => *current == Some(sequence),
    }
}

pub(super) fn send_file_drop_event(
    window_state: Arc<Mutex<MacWindowState>>,
    dragging_info: *mut AnyObject,
    event: FileDropEvent,
) -> bool {
    // SAFETY: AppKit supplies a live NSDraggingInfo during each destination callback.
    let (native_source, sequence): (*mut AnyObject, NSInteger) = unsafe {
        let info = &*dragging_info.cast::<AnyObject>();
        (
            objc2::msg_send![info, draggingSource],
            objc2::msg_send![info, draggingSequenceNumber],
        )
    };
    let (source_session, event_callback) = {
        let mut state = window_state.lock();
        if state.is_closed() || state.interaction_is_quiesced() {
            return false;
        }
        let source_session = if native_source.cast::<AnyObject>() == state.native_window {
            let Some(source) = state
                .external_drag
                .as_ref()
                .filter(|source| source.matches_sequence(sequence))
            else {
                return false;
            };
            Some(source.session.clone())
        } else {
            None
        };
        if !admit_file_drop(&mut state.file_drop_sequence, sequence, &event) {
            return false;
        }
        (source_session, state.event_callback.clone())
    };
    if let Some(session) = source_session {
        session.dispatch(event);
    } else {
        event_callback.dispatch(PlatformInput::FileDrop(event));
    }
    true
}

pub(super) extern "C" fn dragging_session_source_operation_mask(
    this: &AnyObject,
    _: Sel,
    native_session: *mut AnyObject,
    context: NSDraggingContext,
) -> NSDragOperation {
    // SAFETY: AppKit invokes this selector on the registered GPUI window, whose
    // WINDOW_STATE_IVAR refers to its exact owning MacWindowState.
    let window_state = unsafe { get_window_state(this) };
    let source = {
        let state = window_state.lock();
        if state.is_closed() || state.interaction_is_quiesced() {
            return NSDragOperation::None;
        }
        state.external_drag.clone()
    };
    let Some(source) = source else {
        return NSDragOperation::None;
    };
    if !unsafe { source.bind_native_session(native_session) } {
        return NSDragOperation::None;
    }
    match context {
        NSDraggingContext::OutsideApplication => NSDragOperation::Copy,
        NSDraggingContext::WithinApplication => NSDragOperation::Copy | NSDragOperation::Move,
        _ => NSDragOperation::Copy | NSDragOperation::Move,
    }
}

pub(super) extern "C" fn dragging_session_will_begin(
    this: &AnyObject,
    _: Sel,
    native_session: *mut AnyObject,
    _: NSPoint,
) {
    // SAFETY: AppKit invokes this selector on the registered GPUI window, whose
    // WINDOW_STATE_IVAR refers to its exact owning MacWindowState.
    let window_state = unsafe { get_window_state(this) };
    let source = {
        let state = window_state.lock();
        if state.is_closed() || state.interaction_is_quiesced() {
            return;
        }
        state.external_drag.clone()
    };
    if let Some(source) = source {
        unsafe { source.bind_native_session(native_session) };
    }
}

pub(super) extern "C" fn dragging_session_ended(
    this: &AnyObject,
    _: Sel,
    native_session: *mut AnyObject,
    _: NSPoint,
    _: NSDragOperation,
) {
    // SAFETY: AppKit invokes this selector on the registered GPUI window, whose
    // WINDOW_STATE_IVAR refers to its exact owning MacWindowState.
    let window_state = unsafe { get_window_state(this) };
    // SAFETY: AppKit supplies the live session being completed on its main thread.
    let sequence: NSInteger =
        unsafe { objc2::msg_send![&*native_session.cast::<AnyObject>(), draggingSequenceNumber] };
    let (source, admitted) = {
        let mut state = window_state.lock();
        let Some(source) = state
            .external_drag
            .as_ref()
            .filter(|source| source.matches_native_session(native_session, sequence))
            .cloned()
        else {
            return;
        };
        if state
            .last_left_mouse_down_event
            .as_ref()
            .is_some_and(|event| {
                Retained::as_ptr(event) == Retained::as_ptr(&source.mouse_down_event)
            })
        {
            state.last_left_mouse_down_event = None;
        }
        if state.synthetic_drag_counter == source.synthetic_drag_counter {
            state.synthetic_drag_counter += 1;
        }
        admit_file_drop(
            &mut state.file_drop_sequence,
            sequence,
            &FileDropEvent::Ended,
        );
        (
            state.external_drag.take(),
            !state.is_closed() && !state.interaction_is_quiesced(),
        )
    };
    if admitted && let Some(source) = source {
        source.session.dispatch(FileDropEvent::Ended);
    }
}

pub(super) fn synthetic_drag_button_is_pressed(
    button: Option<MouseButton>,
    pressed: NSUInteger,
) -> bool {
    let bit = match button {
        Some(MouseButton::Left) => 0,
        Some(MouseButton::Right) => 1,
        Some(MouseButton::Middle) => 2,
        Some(MouseButton::Navigate(open_gpui::NavigationDirection::Back)) => 3,
        Some(MouseButton::Navigate(open_gpui::NavigationDirection::Forward)) => 4,
        None => return false,
    };
    pressed & (1 << bit) != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui::{ExternalPaths, NavigationDirection, point, px};

    fn entered() -> FileDropEvent {
        FileDropEvent::Entered {
            position: point(px(12.), px(24.)),
            paths: ExternalPaths::default(),
        }
    }

    #[test]
    fn source_sequence_can_leave_and_reenter_before_submission() {
        let mut sequence = None;
        assert!(admit_file_drop(&mut sequence, 7, &entered()));
        assert!(admit_file_drop(&mut sequence, 7, &FileDropEvent::Exited));
        assert_eq!(sequence, None);
        assert!(admit_file_drop(&mut sequence, 7, &entered()));
        assert!(admit_file_drop(
            &mut sequence,
            7,
            &FileDropEvent::Pending {
                position: point(px(20.), px(30.)),
            },
        ));
        assert!(admit_file_drop(
            &mut sequence,
            7,
            &FileDropEvent::Submit {
                position: point(px(20.), px(30.)),
            },
        ));
        assert!(admit_file_drop(&mut sequence, 7, &FileDropEvent::Exited));
        assert_eq!(sequence, None);
    }

    #[test]
    fn delayed_native_terminal_cannot_clear_a_newer_destination_sequence() {
        let mut sequence = None;
        assert!(admit_file_drop(&mut sequence, 7, &entered()));
        assert!(admit_file_drop(&mut sequence, 8, &entered()));
        for event in [
            FileDropEvent::Pending {
                position: point(px(20.), px(30.)),
            },
            FileDropEvent::Submit {
                position: point(px(20.), px(30.)),
            },
            FileDropEvent::Exited,
            FileDropEvent::Ended,
        ] {
            assert!(!admit_file_drop(&mut sequence, 7, &event));
            assert_eq!(sequence, Some(8));
        }
        assert!(admit_file_drop(&mut sequence, 8, &FileDropEvent::Ended));
        assert!(!admit_file_drop(&mut sequence, 8, &FileDropEvent::Ended));
    }

    #[test]
    fn native_submission_requires_an_entered_destination() {
        let mut sequence = None;
        assert!(!admit_file_drop(
            &mut sequence,
            7,
            &FileDropEvent::Submit {
                position: point(px(20.), px(30.)),
            },
        ));
        assert!(!admit_file_drop(&mut sequence, 7, &FileDropEvent::Exited));
        assert_eq!(sequence, None);
    }

    #[test]
    fn synthetic_drag_stops_when_its_original_button_is_released() {
        for (button, bit) in [
            (MouseButton::Left, 0),
            (MouseButton::Right, 1),
            (MouseButton::Middle, 2),
            (MouseButton::Navigate(NavigationDirection::Back), 3),
            (MouseButton::Navigate(NavigationDirection::Forward), 4),
        ] {
            assert!(synthetic_drag_button_is_pressed(Some(button), 1 << bit));
            assert!(!synthetic_drag_button_is_pressed(Some(button), 0));
            assert!(!synthetic_drag_button_is_pressed(
                Some(button),
                0b1_1111 & !(1 << bit),
            ));
        }
        assert!(!synthetic_drag_button_is_pressed(None, 0b1_1111));
    }
}
