#![cfg(feature = "command")]

use std::sync::{Arc, Mutex};

use open_gpui::{App, KeyBinding, KeyContext, Keymap, TestAppContext, actions};
use open_gpui_command::{
    CommandAvailabilityMap, CommandContribution, CommandDescriptor, CommandIconDescriptor,
    CommandKeyBinding, CommandKeyBindingProjection, CommandKeyBindingRegistry,
    CommandRegistrySnapshot, GpuiCommandActionMap,
};
use open_gpui_devtools::{
    DevtoolsDomainKind, DevtoolsProbe, DevtoolsTargetKind, ProbeId, ProbeSnapshotError,
    SnapshotKind, command,
};

actions!(
    devtools_command_adapter_tests,
    [OpenWorkspace, SaveWorkspace, BadWorkspace, HiddenWorkspace]
);

fn command_keybinding_projection_fixture(cx: &App) -> CommandKeyBindingProjection {
    let mut registry = CommandKeyBindingRegistry::new();
    registry.register(
        "core",
        [
            CommandKeyBinding::new("workspace.open", "ctrl-p").context("Workspace"),
            CommandKeyBinding::new("workspace.save", "ctrl-p").context("Workspace"),
            CommandKeyBinding::new("workspace.missing", "ctrl-m"),
            CommandKeyBinding::new("workspace.bad", "ctrl-b").context("Workspace &&"),
        ],
    );
    let actions = GpuiCommandActionMap::new()
        .action("workspace.open", OpenWorkspace)
        .action("workspace.save", SaveWorkspace)
        .action("workspace.bad", BadWorkspace);

    registry.project(&actions, cx)
}

#[test]
fn command_registry_adapter_projects_metadata_and_sanitizes() {
    let snapshot = CommandRegistrySnapshot::new(
        "commands-v1",
        [CommandContribution::new(
            CommandDescriptor::new("workspace.open", "Open Workspace")
                .icon(CommandIconDescriptor::new("folder-open").fallback_label("Open"))
                .group("Workspace")
                .keyword("C:\\Users\\Frank\\token.txt")
                .shortcut("Ctrl+O")
                .disabled_reason("token=raw-secret")
                .tooltip("Open workspace for alice@example.com")
                .accessibility_description("Opens a workspace")
                .when("account == alice@example.com")
                .menu_path(["File", "Open Recent"]),
        )
        .source("extension alice@example.com")],
    );

    let envelope =
        command::command_registry_snapshot_envelope(ProbeId::new("command").unwrap(), &snapshot);
    let capture =
        command::command_registry_capture(ProbeId::new("command.registry").unwrap(), &snapshot);
    let serialized = serde_json::to_string(&envelope).unwrap();

    assert_eq!(envelope.kind, SnapshotKind::Command);
    assert_eq!(capture.targets.targets[0].kind, DevtoolsTargetKind::Runtime);
    assert_eq!(capture.domains[0].kind, DevtoolsDomainKind::Command);
    assert_eq!(capture.snapshots[0].kind, SnapshotKind::Command);
    assert!(serialized.contains("Command registry"));
    assert!(serialized.contains("workspace.open"));
    assert!(serialized.contains("folder-open"));
    assert!(!serialized.contains("alice@example.com"), "{serialized}");
    assert!(!serialized.contains("raw-secret"), "{serialized}");
    assert!(!serialized.contains("Frank"), "{serialized}");
    assert!(serialized.contains("[redacted"));
}

#[open_gpui::test]
fn command_keybinding_projection_adapter_reports_diagnostics_and_conflicts(
    cx: &mut TestAppContext,
) {
    let projection = cx.update(|app| command_keybinding_projection_fixture(app));
    let snapshot = command::command_keybinding_projection_probe_snapshot(&projection);
    let projection_probe =
        command::command_keybinding_projection_snapshot_probe("command.keybindings", move || {
            Ok(snapshot.clone())
        })
        .unwrap();
    let envelope = command::command_keybinding_projection_snapshot_envelope(
        ProbeId::new("command.keybindings").unwrap(),
        &projection,
    );
    let capture = command::command_keybinding_projection_capture(
        ProbeId::new("command.keybindings.capture").unwrap(),
        &projection,
    );
    let probe_snapshot = projection_probe.snapshot().unwrap();
    let serialized = serde_json::to_string(&envelope).unwrap();

    assert_eq!(projection.projected_entries().len(), 2);
    assert_eq!(projection.conflicts().len(), 1);
    assert_eq!(projection.diagnostics().len(), 2);
    assert_eq!(envelope.kind, SnapshotKind::Command);
    assert_eq!(probe_snapshot.kind, SnapshotKind::Command);
    assert_eq!(capture.domains[0].kind, DevtoolsDomainKind::Command);
    assert_eq!(capture.snapshots[0].kind, SnapshotKind::Command);
    let summary = capture.domains[0].summary.as_ref().unwrap();
    assert_eq!(summary["key_binding_count"], 2);
    assert!(summary.get("installed_key_binding_count").is_none());
    assert_eq!(
        envelope.tree.nodes[0].payload.as_ref().unwrap()["key_binding_count"],
        2
    );
    assert!(!serialized.contains("installed_key_binding_count"));
    assert!(serialized.contains("\"conflict_count\":1"));
    assert!(serialized.contains("\"diagnostic_count\":2"));
    assert!(serialized.contains("missing-action"));
    assert!(serialized.contains("invalid-context"));
    assert!(serialized.contains("workspace.open"));
    assert!(serialized.contains("workspace.save"));
}

#[open_gpui::test]
fn command_keybinding_probe_reads_republished_snapshots_and_propagates_errors(
    cx: &mut TestAppContext,
) {
    let (first, second) = cx.update(|app| {
        let first = command_keybinding_projection_fixture(app);
        let mut registry = CommandKeyBindingRegistry::new();
        registry.register(
            "core",
            [CommandKeyBinding::new("workspace.save", "ctrl-s").context("Workspace")],
        );
        let actions = GpuiCommandActionMap::new().action("workspace.save", SaveWorkspace);
        let second = registry.project(&actions, app);
        (
            command::command_keybinding_projection_probe_snapshot(&first),
            command::command_keybinding_projection_probe_snapshot(&second),
        )
    });
    let published = Arc::new(Mutex::new(Ok(first.clone())));
    let provider = Arc::clone(&published);
    let probe = command::command_keybinding_projection_snapshot_probe("command.live", move || {
        provider
            .lock()
            .map_err(|_| {
                ProbeSnapshotError::CollectionFailed("command snapshot lock poisoned".to_owned())
            })?
            .clone()
    })
    .unwrap();

    let before = probe.snapshot().unwrap();
    assert_eq!(&before.tree, first.tree());
    assert_eq!(
        before.tree.nodes[0].payload.as_ref().unwrap()["conflict_count"],
        1
    );

    *published.lock().unwrap() = Ok(second.clone());
    let after = probe.snapshot().unwrap();
    assert_eq!(before.probe_id, after.probe_id);
    assert_eq!(&after.tree, second.tree());
    assert_ne!(before.tree, after.tree);
    let summary = after.tree.nodes[0].payload.as_ref().unwrap();
    assert_eq!(summary["key_binding_count"], 1);
    assert_eq!(summary["conflict_count"], 0);
    assert_eq!(summary["diagnostic_count"], 0);
    assert!(serde_json::to_string(&after).unwrap().contains("ctrl-s"));

    let error = ProbeSnapshotError::CollectionFailed("command producer unavailable".to_owned());
    *published.lock().unwrap() = Err(error.clone());
    assert_eq!(probe.snapshot(), Err(error));
}

#[open_gpui::test]
fn command_keymap_resolution_adapter_reports_pending_and_matched_commands(cx: &mut TestAppContext) {
    let registry = CommandRegistrySnapshot::new(
        "keymap-v1",
        [
            CommandContribution::new(CommandDescriptor::new("workspace.open", "Open Workspace")),
            CommandContribution::new(CommandDescriptor::new("workspace.save", "Save Workspace")),
            CommandContribution::new(CommandDescriptor::new(
                "workspace.hidden",
                "Hidden Workspace",
            )),
        ],
    );
    let actions = GpuiCommandActionMap::new()
        .action("workspace.open", OpenWorkspace)
        .action("workspace.save", SaveWorkspace)
        .action("workspace.hidden", HiddenWorkspace);
    let mut keymap = Keymap::default();
    keymap.add_bindings([
        KeyBinding::new("ctrl-k ctrl-o", OpenWorkspace, Some("Workspace")),
        KeyBinding::new("ctrl-k ctrl-s", SaveWorkspace, Some("Workspace")),
        KeyBinding::new("ctrl-k ctrl-h", HiddenWorkspace, Some("Workspace")),
    ]);
    let contexts = [KeyContext::parse("Workspace").unwrap()];
    let availability = CommandAvailabilityMap::new()
        .disabled("workspace.save", "token=raw-disabled")
        .hidden("workspace.hidden");

    let pending = cx.update(|app| {
        actions
            .resolve_keymap_sequence(
                "ctrl-k",
                &registry,
                &CommandAvailabilityMap::new(),
                &keymap,
                &contexts,
                app,
            )
            .unwrap()
    });
    let pending_snapshot = command::command_keymap_resolution_probe_snapshot(&pending);
    let pending_serialized = serde_json::to_string(pending_snapshot.tree()).unwrap();

    assert!(pending.is_pending());
    assert!(pending_serialized.contains("\"pending\":true"));
    assert!(pending_serialized.contains("Pending commands"));
    assert!(pending_serialized.contains("workspace.open"));
    assert!(pending_serialized.contains("workspace.save"));

    let matched_resolution = cx.update(|app| {
        actions
            .resolve_keymap_sequence(
                "ctrl-k ctrl-o",
                &registry,
                &availability,
                &keymap,
                &contexts,
                app,
            )
            .unwrap()
    });
    let matched_probe =
        command::command_keymap_resolution_snapshot_probe("command.keys", move || {
            matched_resolution.clone()
        })
        .unwrap();
    let matched = matched_probe.snapshot().unwrap();
    let matched_serialized = serde_json::to_string(&matched).unwrap();

    assert_eq!(matched.kind, SnapshotKind::Command);
    assert!(matched_serialized.contains("\"primary_command\":\"workspace.open\""));
    assert!(matched_serialized.contains("\"dispatchable\":true"));

    let disabled = cx.update(|app| {
        actions
            .resolve_keymap_sequence(
                "ctrl-k ctrl-s",
                &registry,
                &availability,
                &keymap,
                &contexts,
                app,
            )
            .unwrap()
    });
    let disabled_envelope = command::command_keymap_resolution_snapshot_envelope(
        ProbeId::new("command.disabled").unwrap(),
        &disabled,
    );
    let disabled_capture = command::command_keymap_resolution_capture(
        ProbeId::new("command.disabled.capture").unwrap(),
        &disabled,
    );
    let disabled_serialized = serde_json::to_string(&disabled_envelope).unwrap();

    assert_eq!(disabled_envelope.kind, SnapshotKind::Command);
    assert_eq!(
        disabled_capture.domains[0].kind,
        DevtoolsDomainKind::Command
    );
    assert_eq!(
        disabled_capture.targets.targets[0].kind,
        DevtoolsTargetKind::Runtime
    );
    assert!(disabled_serialized.contains("\"state\":\"disabled\""));
    assert!(disabled_serialized.contains("\"reason\":\"token=[redacted]\""));
    assert!(!disabled_serialized.contains("raw-disabled"));
    assert!(!disabled_serialized.contains("\"Disabled\""));

    let hidden = cx.update(|app| {
        actions
            .resolve_keymap_sequence(
                "ctrl-k ctrl-h",
                &registry,
                &availability,
                &keymap,
                &contexts,
                app,
            )
            .unwrap()
    });
    let hidden_serialized =
        serde_json::to_string(command::command_keymap_resolution_probe_snapshot(&hidden).tree())
            .unwrap();

    assert!(hidden_serialized.contains("\"state\":\"hidden\""));
    assert!(!hidden_serialized.contains("\"Hidden\""));
}
