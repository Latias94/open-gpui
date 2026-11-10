use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use open_gpui::{
    Context, FocusHandle, InteractiveElement, IntoElement, KeyContext, Keymap, OperatingSystem,
    ParentElement, PlatformInfo, PlatformTarget, Render, Styled, TestAppContext, VisualContext,
    Window, actions, div,
};
use open_gpui_command::{
    CommandCenter, CommandContribution, CommandDescriptor, CommandKeyBinding,
    CommandKeyBindingPatch,
};
use open_gpui_ui_components::{
    Command, CommandKeyBindingCaptureState, CommandKeyBindingEditorFilter,
    CommandKeyBindingEditorPreviewState, CommandPaletteController, CommandShortcutInspectorState,
    TextInput, gpui_adapter::init_text_input,
};

actions!(runtime_palette_keybindings, [OpenWorkspace, SaveWorkspace]);

struct BindingCaptureInput {
    capture: Rc<RefCell<CommandKeyBindingCaptureState>>,
}

impl Render for BindingCaptureInput {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let capture = self.capture.clone();
        let value = capture.borrow().raw_sequence().to_owned();
        div().size_full().child(
            TextInput::new("runtime-binding-capture", "Keybinding sequence")
                .value(value)
                .on_change(move |sequence, _, cx| {
                    *capture.borrow_mut() = CommandKeyBindingCaptureState::from_sequence(
                        sequence,
                        cx.platform_info().os,
                    );
                }),
        )
    }
}

fn capture_from_editable_input(
    cx: &mut TestAppContext,
    sequence: &str,
) -> CommandKeyBindingCaptureState {
    let capture = Rc::new(RefCell::new(cx.read(|cx| {
        CommandKeyBindingCaptureState::from_sequence("", cx.platform_info().os)
    })));
    let (_, input_cx) = cx.add_window_view(|_, _| BindingCaptureInput {
        capture: capture.clone(),
    });
    input_cx.update(|window, cx| window.draw(cx).clear(cx));
    let input = input_cx
        .debug_bounds("text-input:runtime-binding-capture:root")
        .expect("the editable capture control must render");
    input_cx.simulate_click(input.center(), Default::default());
    input_cx.simulate_input(sequence);
    input_cx.update(|window, cx| window.draw(cx).clear(cx));
    let captured = capture.borrow().clone();
    assert_eq!(
        captured.raw_sequence(),
        sequence,
        "capture must run through on_change"
    );
    captured
}

struct PaletteWorkspace {
    center: CommandCenter,
    controller: CommandPaletteController,
    focus: FocusHandle,
    dispatched: Rc<Cell<usize>>,
}

impl Render for PaletteWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let projection = self
            .controller
            .projection_for_window(&self.center, window, cx);
        let dispatched = self.dispatched.clone();
        div()
            .key_context("Workspace")
            .track_focus(&self.focus)
            .on_action(move |_: &OpenWorkspace, _, _| {
                dispatched.set(dispatched.get() + 1);
            })
            .child(
                Command::new("runtime-palette", "Workspace commands")
                    .palette_projection(&projection),
            )
    }
}

fn assert_palette_workflow(
    cx: &mut TestAppContext,
    os: OperatingSystem,
    shortcut: &str,
    normalized: &str,
    platform_shortcut: &str,
    platform_normalized: &str,
) {
    cx.set_platform_info(PlatformInfo {
        os,
        target: PlatformTarget::Web,
    });
    cx.update(init_text_input);

    let capture = capture_from_editable_input(cx, "secondary-k secondary-o");
    assert!(capture.is_valid());
    assert_eq!(capture.raw_sequence(), "secondary-k secondary-o");
    assert_eq!(capture.input_label(), Some(normalized));
    let platform_capture = CommandKeyBindingCaptureState::from_sequence("cmd-shift-s", os);
    assert_eq!(platform_capture.input_label(), Some(platform_normalized));
    let invalid = CommandKeyBindingCaptureState::from_sequence("secondary-x-y", os);
    assert!(!invalid.is_valid());
    assert!(invalid.error().unwrap().contains("secondary-x-y"));

    let mut center = CommandCenter::new("runtime-palette");
    center
        .register_source(
            "workspace",
            "core",
            [
                CommandContribution::new(
                    CommandDescriptor::new("workspace.open", "Open Workspace").group("Workspace"),
                ),
                CommandContribution::new(
                    CommandDescriptor::new("workspace.save", "Save Workspace").group("Workspace"),
                ),
            ],
        )
        .unwrap();
    center
        .register_action("workspace.open", OpenWorkspace)
        .register_action("workspace.save", SaveWorkspace)
        .set_key_contexts([
            KeyContext::new_with_defaults(os),
            KeyContext::parse("Workspace").unwrap(),
        ])
        .register_key_bindings(
            "defaults",
            [CommandKeyBinding::new("workspace.open", "secondary-z")],
        );

    let before = cx.read(|cx| center.key_binding_projection(cx));
    let target = before.projected_entries()[0].edit_target();
    let captured_binding = CommandKeyBinding::new(
        "workspace.open",
        capture.input_label().expect("valid capture"),
    );
    let preview = cx.read(|cx| {
        center.preview_key_binding_patch(
            CommandKeyBindingPatch::replace(target, captured_binding.clone()),
            cx,
        )
    });
    let editor = CommandKeyBindingEditorPreviewState::from_patch_preview(
        &preview,
        CommandKeyBindingEditorFilter::new(),
    );
    assert!(editor.is_strictly_clean());
    assert_eq!(editor.editor().rows()[0].keystrokes(), shortcut);
    assert_eq!(editor.editor().rows()[0].raw_keystrokes(), normalized);
    assert_eq!(
        center.key_binding_registry().entries()[0]
            .binding()
            .keystrokes(),
        "secondary-z"
    );

    center.register_key_bindings(
        "defaults",
        [
            captured_binding,
            CommandKeyBinding::new("workspace.save", platform_capture.input_label().unwrap()),
        ],
    );
    let mut keymap = Keymap::default();
    cx.update(|cx| {
        assert!(center.install_key_bindings(&mut keymap, cx).is_clean());
        assert!(center.install_key_bindings_in_app(cx).is_clean());
    });
    let mut controller = CommandPaletteController::new();
    let update = controller
        .set_query_for_keymap(&mut center, "", &keymap, os)
        .unwrap();
    assert!(
        update
            .palette_projection()
            .shortcut_diagnostics()
            .is_empty()
    );
    let state = Command::new("projected-palette", "Workspace commands")
        .palette_projection(update.palette_projection())
        .state();
    assert_eq!(
        state
            .group_items(0)
            .map(|item| (item.value(), item.shortcut()))
            .collect::<Vec<_>>(),
        [
            ("workspace.open", Some(shortcut)),
            ("workspace.save", Some(platform_shortcut)),
        ]
    );

    let pending = cx
        .read(|cx| {
            controller.preflight_key_sequence_for_keymap(&center, "secondary-k", &keymap, cx)
        })
        .unwrap();
    let inspector = CommandShortcutInspectorState::from_preflight(&pending);
    assert!(inspector.is_pending());
    assert_eq!(inspector.pending_commands()[0].shortcut(), shortcut);
    let matched = cx
        .read(|cx| {
            controller.preflight_key_sequence_for_keymap(
                &center,
                capture.input_label().unwrap(),
                &keymap,
                cx,
            )
        })
        .unwrap();
    assert_eq!(matched.input_label(), normalized);
    assert_eq!(
        matched.primary_dispatchable_command_id(),
        Some("workspace.open")
    );

    let dispatched = Rc::new(Cell::new(0));
    let (view, cx) = cx.add_window_view(|_, cx| PaletteWorkspace {
        center,
        controller,
        focus: cx.focus_handle(),
        dispatched: dispatched.clone(),
    });
    cx.update_window_entity(&view, |view, window, cx| view.focus.focus(window, cx));
    cx.update(|window, cx| window.draw(cx).clear(cx));
    assert!(
        cx.debug_bounds("listbox:runtime-palette-listbox:option:workspace.open")
            .is_some()
    );
    cx.update_window_entity(&view, |view, window, cx| {
        let projection = view
            .controller
            .projection_for_window(&view.center, window, cx);
        assert!(projection.shortcut_diagnostics().is_empty());
        let state = Command::new("window-palette", "Workspace commands")
            .palette_projection(&projection)
            .state();
        assert_eq!(
            state.group_items(0).next().unwrap().shortcut(),
            Some(shortcut)
        );
    });
    cx.simulate_keystrokes("secondary-k");
    assert_eq!(dispatched.get(), 0);
    cx.simulate_keystrokes("secondary-o");
    assert_eq!(dispatched.get(), 1);
}

#[open_gpui::test]
fn mac_palette_capture_preview_and_chord_dispatch(cx: &mut TestAppContext) {
    assert_palette_workflow(
        cx,
        OperatingSystem::Mac,
        "⌘K ⌘O",
        "cmd-k cmd-o",
        "⌘⇧S",
        "cmd-shift-s",
    );
}

#[open_gpui::test]
fn windows_palette_capture_preview_and_chord_dispatch(cx: &mut TestAppContext) {
    assert_palette_workflow(
        cx,
        OperatingSystem::Windows,
        "ctrl-K ctrl-O",
        "ctrl-k ctrl-o",
        "⊞shift-S",
        "win-shift-s",
    );
}

#[open_gpui::test]
fn linux_palette_capture_preview_and_chord_dispatch(cx: &mut TestAppContext) {
    assert_palette_workflow(
        cx,
        OperatingSystem::Linux,
        "ctrl-K ctrl-O",
        "ctrl-k ctrl-o",
        "❖shift-S",
        "super-shift-s",
    );
}
