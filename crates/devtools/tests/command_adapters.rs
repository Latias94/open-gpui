#![cfg(feature = "command")]

use open_gpui::{KeyBinding, KeyContext, Keymap, actions};
use open_gpui_command::{
    CommandAvailabilityMap, CommandContribution, CommandDescriptor, CommandIconDescriptor,
    CommandKeyBinding, CommandKeyBindingRegistry, CommandRegistrySnapshot, GpuiCommandActionMap,
};
use open_gpui_devtools::{DevtoolsProbe, ProbeId, SnapshotKind, command};

actions!(
    devtools_command_adapter_tests,
    [OpenWorkspace, SaveWorkspace, BadWorkspace]
);

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
    let serialized = serde_json::to_string(&envelope).unwrap();

    assert_eq!(envelope.kind, SnapshotKind::Command);
    assert!(serialized.contains("Command registry"));
    assert!(serialized.contains("workspace.open"));
    assert!(serialized.contains("folder-open"));
    assert!(!serialized.contains("alice@example.com"), "{serialized}");
    assert!(!serialized.contains("raw-secret"), "{serialized}");
    assert!(!serialized.contains("Frank"), "{serialized}");
    assert!(serialized.contains("[redacted"));
}

#[test]
fn command_keybinding_projection_adapter_reports_diagnostics_and_conflicts() {
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

    let projection = registry.project(&actions);
    let snapshot = command::command_keybinding_projection_probe_snapshot(&projection);
    let serialized = serde_json::to_string(snapshot.tree()).unwrap();

    assert_eq!(projection.projected_entries().len(), 2);
    assert_eq!(projection.conflicts().len(), 1);
    assert_eq!(projection.diagnostics().len(), 2);
    assert!(serialized.contains("\"conflict_count\":1"));
    assert!(serialized.contains("\"diagnostic_count\":2"));
    assert!(serialized.contains("missing-action"));
    assert!(serialized.contains("invalid-context"));
    assert!(serialized.contains("workspace.open"));
    assert!(serialized.contains("workspace.save"));
}

#[test]
fn command_keymap_resolution_adapter_reports_pending_and_matched_commands() {
    let registry = CommandRegistrySnapshot::new(
        "keymap-v1",
        [
            CommandContribution::new(CommandDescriptor::new("workspace.open", "Open Workspace")),
            CommandContribution::new(CommandDescriptor::new("workspace.save", "Save Workspace")),
        ],
    );
    let actions = GpuiCommandActionMap::new()
        .action("workspace.open", OpenWorkspace)
        .action("workspace.save", SaveWorkspace);
    let mut keymap = Keymap::default();
    keymap.add_bindings([
        KeyBinding::new("ctrl-k ctrl-o", OpenWorkspace, Some("Workspace")),
        KeyBinding::new("ctrl-k ctrl-s", SaveWorkspace, Some("Workspace")),
    ]);
    let contexts = [KeyContext::parse("Workspace").unwrap()];

    let pending = actions
        .resolve_keymap_sequence(
            "ctrl-k",
            &registry,
            &CommandAvailabilityMap::new(),
            &keymap,
            &contexts,
        )
        .unwrap();
    let pending_snapshot = command::command_keymap_resolution_probe_snapshot(&pending);
    let pending_serialized = serde_json::to_string(pending_snapshot.tree()).unwrap();

    assert!(pending.is_pending());
    assert!(pending_serialized.contains("\"pending\":true"));
    assert!(pending_serialized.contains("Pending commands"));
    assert!(pending_serialized.contains("workspace.open"));
    assert!(pending_serialized.contains("workspace.save"));

    let matched_resolution = actions
        .resolve_keymap_sequence(
            "ctrl-k ctrl-o",
            &registry,
            &CommandAvailabilityMap::new(),
            &keymap,
            &contexts,
        )
        .unwrap();
    let matched_probe = command::command_keymap_resolution_probe("command.keys", move || {
        matched_resolution.clone()
    })
    .unwrap();
    let matched = matched_probe.snapshot().unwrap();
    let matched_serialized = serde_json::to_string(&matched).unwrap();

    assert_eq!(matched.kind, SnapshotKind::Command);
    assert!(matched_serialized.contains("\"primary_command\":\"workspace.open\""));
    assert!(matched_serialized.contains("\"dispatchable\":true"));
}
