use std::{cell::Cell, rc::Rc};

use open_gpui::{
    Context, FocusHandle, InteractiveElement, IntoElement, KeyContext, Keymap, OperatingSystem,
    ParentElement, PlatformInfo, PlatformTarget, Render, TestAppContext, VisualContext, Window,
    actions, div,
};
use open_gpui_command::{
    CommandCenter, CommandContribution, CommandDescriptor, CommandKeyBinding,
    CommandKeyBindingDiagnosticKind, command_shortcut_label, parse_command_key_sequence,
};

actions!(runtime_keybindings, [OpenWorkspace, SaveWorkspace]);

struct Workspace {
    focus: FocusHandle,
    dispatched: Rc<Cell<usize>>,
}

impl Render for Workspace {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let dispatched = self.dispatched.clone();
        div()
            .key_context("Workspace")
            .track_focus(&self.focus)
            .on_action(move |_: &OpenWorkspace, _, _| {
                dispatched.set(dispatched.get() + 1);
            })
            .child("Workspace")
    }
}

fn assert_command_workflow(
    cx: &mut TestAppContext,
    os: OperatingSystem,
    shortcut: &str,
    normalized: &str,
) {
    cx.set_platform_info(PlatformInfo {
        os,
        target: PlatformTarget::Web,
    });
    let mut center = CommandCenter::new("runtime-shortcuts");
    center
        .register_source(
            "workspace",
            "core",
            [CommandContribution::new(CommandDescriptor::new(
                "workspace.open",
                "Open Workspace",
            ))],
        )
        .unwrap();
    center
        .register_action("workspace.open", OpenWorkspace)
        .set_key_contexts([
            KeyContext::new_with_defaults(os),
            KeyContext::parse("Workspace").unwrap(),
        ])
        .register_key_bindings(
            "workspace-shortcuts",
            [CommandKeyBinding::new(
                "workspace.open",
                String::from("secondary-k secondary-o"),
            )],
        );

    let mut keymap = Keymap::default();
    cx.update(|cx| {
        let report = center.install_key_bindings(&mut keymap, cx);
        assert!(report.is_clean());
        assert_eq!(report.installed_count(), 1);
        assert_eq!(
            report.projection().projected_entries()[0].keystrokes(),
            shortcut
        );
        assert_eq!(
            command_shortcut_label(&report.key_bindings()[0], os),
            shortcut
        );
        assert!(center.install_key_bindings_in_app(cx).is_clean());
    });

    let snapshot = center.snapshot_for_keymap(&keymap, os);
    assert_eq!(
        snapshot
            .descriptor("workspace.open")
            .unwrap()
            .shortcut_ref(),
        Some(shortcut)
    );
    assert!(
        center
            .shortcut_diagnostics_for_keymap(&keymap, os)
            .is_empty()
    );

    let sequence = String::from("secondary-k secondary-o");
    let input = parse_command_key_sequence(&sequence, os).unwrap();
    let pending = center.resolve_key_input_for_keymap(&input[..1], &keymap, os);
    assert!(pending.is_pending());
    assert!(pending.matched_commands().is_empty());
    assert_eq!(pending.pending_commands()[0].shortcut(), shortcut);
    let matched = cx
        .read(|cx| center.resolve_key_sequence_for_keymap(&sequence, &keymap, cx))
        .unwrap();
    assert!(!matched.is_pending());
    assert_eq!(matched.input_label(), normalized);
    assert_eq!(
        matched.primary_dispatchable_command().unwrap().command_id(),
        "workspace.open"
    );

    let dispatched = Rc::new(Cell::new(0));
    let (view, cx) = cx.add_window_view(|_, cx| Workspace {
        focus: cx.focus_handle(),
        dispatched: dispatched.clone(),
    });
    cx.update_window_entity(&view, |view, window, cx| view.focus.focus(window, cx));
    cx.update(|window, cx| {
        window.draw(cx).clear(cx);
        assert_eq!(
            center
                .snapshot_for_window(window, cx)
                .descriptor("workspace.open")
                .unwrap()
                .shortcut_ref(),
            Some(shortcut)
        );
        assert!(
            center
                .shortcut_diagnostics_for_window(window, cx)
                .is_empty()
        );
    });

    cx.simulate_keystrokes("secondary-k");
    assert_eq!(dispatched.get(), 0, "a chord prefix must not dispatch");
    cx.simulate_keystrokes("secondary-o");
    assert_eq!(
        dispatched.get(),
        1,
        "the completed chord must dispatch once"
    );
}

#[open_gpui::test]
fn mac_command_workflow_uses_runtime_identity(cx: &mut TestAppContext) {
    assert_command_workflow(cx, OperatingSystem::Mac, "⌘K ⌘O", "cmd-k cmd-o");
}

#[open_gpui::test]
fn windows_command_workflow_uses_runtime_identity(cx: &mut TestAppContext) {
    assert_command_workflow(
        cx,
        OperatingSystem::Windows,
        "ctrl-K ctrl-O",
        "ctrl-k ctrl-o",
    );
}

#[open_gpui::test]
fn linux_command_workflow_uses_runtime_identity(cx: &mut TestAppContext) {
    assert_command_workflow(cx, OperatingSystem::Linux, "ctrl-K ctrl-O", "ctrl-k ctrl-o");
}

#[open_gpui::test]
fn command_projection_diagnoses_host_specific_alias_conflicts(cx: &mut TestAppContext) {
    for (os, shortcut, conflict_count) in [
        (OperatingSystem::Mac, "⌘P", 0),
        (OperatingSystem::Windows, "ctrl-P", 1),
        (OperatingSystem::Linux, "ctrl-P", 1),
    ] {
        cx.set_platform_info(PlatformInfo {
            os,
            target: PlatformTarget::Web,
        });
        let mut center = CommandCenter::new("alias-conflicts");
        center
            .register_action("workspace.open", OpenWorkspace)
            .register_action("workspace.save", SaveWorkspace)
            .register_key_bindings(
                "defaults",
                [
                    CommandKeyBinding::new("workspace.open", "secondary-p"),
                    CommandKeyBinding::new("workspace.save", "ctrl-p"),
                    CommandKeyBinding::new("workspace.open", "secondary-x-y"),
                ],
            );
        let projection = cx.read(|cx| center.key_binding_projection(cx));
        assert_eq!(projection.projected_entries()[0].keystrokes(), shortcut);
        assert_eq!(projection.conflicts().len(), conflict_count);
        assert_eq!(projection.diagnostics().len(), 1);
        assert_eq!(
            projection.diagnostics()[0].kind(),
            CommandKeyBindingDiagnosticKind::InvalidKeystrokes
        );
        assert_eq!(projection.diagnostics()[0].keystrokes(), "secondary-x-y");
        if let Some(conflict) = projection.conflicts().first() {
            assert_eq!(conflict.keystrokes(), shortcut);
        }
    }
}
