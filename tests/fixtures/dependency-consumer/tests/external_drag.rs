//! Ordinary external-drag consumers without test-support or an application runtime.
//!
//! Payload metadata and event conversions are pure. Listener, platform and session interfaces
//! are checked in uninvoked closures; they do not establish native drag or dispatch behavior.

use open_gpui::{
    AnyDrag, App, AppContext as _, DragStartGeometry, Empty, ExternalDragPayload,
    ExternalDragPayloadSource, ExternalPaths, FileDragPaths, FileDropEvent, InputEvent,
    InteractiveElement as _, Interactivity, Modifiers, MouseButton, MouseExitEvent,
    PlatformExternalDragSession, PlatformInput, PlatformWindow, StatefulInteractiveElement as _,
    TargetedEvent, Window, div, point, px,
};
use std::path::PathBuf;

#[test]
fn ordinary_file_payload_preserves_caller_supplied_directory_metadata() {
    let entries = [
        (PathBuf::from("drag-source/report.txt"), false),
        (PathBuf::from("drag-source/folder.with.extension"), true),
    ];
    let payload = ExternalDragPayload::Files(FileDragPaths::new(entries.clone()));
    let ExternalDragPayload::Files(paths) = &payload;
    assert_eq!(paths.entries(), entries.as_slice());
    assert_eq!(payload.clone(), payload);
}

#[test]
fn ordinary_exit_events_distinguish_window_leave_from_session_completion() {
    let position = point(px(24.0), px(36.0));
    let paths = ExternalPaths(
        [PathBuf::from("drag-source/report.txt")]
            .into_iter()
            .collect(),
    );
    let entered = FileDropEvent::Entered {
        position,
        paths: paths.clone(),
    }
    .to_platform_input();
    let PlatformInput::FileDrop(FileDropEvent::Entered {
        position: entered_position,
        paths: entered_paths,
    }) = entered
    else {
        panic!("file entry must retain its public platform variant");
    };
    assert_eq!(entered_position, position);
    assert_eq!(entered_paths.paths(), paths.paths());
    assert!(matches!(
        FileDropEvent::Exited.to_platform_input(),
        PlatformInput::FileDrop(FileDropEvent::Exited)
    ));
    assert!(matches!(
        FileDropEvent::Ended.to_platform_input(),
        PlatformInput::FileDrop(FileDropEvent::Ended)
    ));

    let exit = MouseExitEvent {
        position,
        pressed_button: Some(MouseButton::Left),
        modifiers: Modifiers {
            shift: true,
            ..Modifiers::none()
        },
    }
    .to_platform_input();
    let PlatformInput::MouseExited(exit) = exit else {
        panic!("mouse exit must retain its public platform variant");
    };
    assert_eq!(exit.position, position);
    assert_eq!(exit.pressed_button, Some(MouseButton::Left));
    assert!(exit.modifiers.shift);
}

#[test]
fn ordinary_listener_and_platform_session_interfaces_type_check_without_a_runtime() {
    let _listeners_type_only = || {
        let listener =
            |_: &PathBuf, _: &DragStartGeometry, _: &mut Window, cx: &mut App| cx.new(|_| Empty);
        let resolver = |path: &PathBuf, _: &mut Window, _: &mut App| {
            Some(ExternalDragPayload::Files(FileDragPaths::new([(
                path.clone(),
                false,
            )])))
        };
        let mouse_exit = |event: &TargetedEvent<MouseExitEvent>, _: &mut Window, _: &mut App| {
            let _: &MouseExitEvent = event.window_event();
        };
        let file_exit = |event: &TargetedEvent<FileDropEvent>, _: &mut Window, _: &mut App| {
            let _: &FileDropEvent = event.window_event();
        };

        let _combined = div()
            .id("external-drag-combined")
            .on_drag_with_external(PathBuf::from("drag-source/report.txt"), listener, resolver)
            .on_mouse_exit(mouse_exit)
            .on_file_drop_exit(file_exit);
        let mut separate = div()
            .id("external-drag-separate")
            .on_drag(PathBuf::from("drag-source/report.txt"), listener)
            .external_drag_payload(resolver);
        separate.interactivity().on_mouse_exit(mouse_exit);
        separate.interactivity().on_file_drop_exit(file_exit);

        let mut imperative = Interactivity::default();
        imperative.on_drag_with_external(
            PathBuf::from("drag-source/report.txt"),
            listener,
            resolver,
        );
    };
    let _: fn(&dyn PlatformWindow) -> bool = |window| window.can_start_external_drag();
    let _: fn(&dyn PlatformWindow, &ExternalDragPayload, PlatformExternalDragSession) -> bool =
        |window, payload, session| window.start_external_drag(payload, session);
    let _: fn(&PlatformExternalDragSession, FileDropEvent) = PlatformExternalDragSession::dispatch;
    let _: fn(&PlatformExternalDragSession) -> PlatformExternalDragSession = Clone::clone;
    let _: fn(&App) -> bool = App::has_active_platform_drag;
    let _drag_source_type_only = |drag: &AnyDrag| {
        let _: &Option<ExternalDragPayloadSource> = &drag.external_payload_source;
    };
}
