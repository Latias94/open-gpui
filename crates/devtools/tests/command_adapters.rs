#![cfg(feature = "command")]

use open_gpui::{KeyBinding, KeyContext, Keymap, actions};
use open_gpui_command::{
    CommandAvailabilityMap, CommandContribution, CommandDescriptor, CommandIconDescriptor,
    CommandKeyBinding, CommandKeyBindingProjection, CommandKeyBindingRegistry,
    CommandRegistrySnapshot, GpuiCommandActionMap,
};
use open_gpui_devtools::{
    DevtoolsDomainKind, DevtoolsProbe, DevtoolsTargetKind, ProbeId, SnapshotKind, command,
};

actions!(
    devtools_command_adapter_tests,
    [OpenWorkspace, SaveWorkspace, BadWorkspace, HiddenWorkspace]
);

fn command_keybinding_projection_fixture() -> CommandKeyBindingProjection {
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

    registry.project(&actions)
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

#[test]
fn command_keybinding_projection_adapter_reports_diagnostics_and_conflicts() {
    let projection = command_keybinding_projection_fixture();
    let projection_probe = command::command_keybinding_projection_snapshot_probe(
        "command.keybindings",
        command_keybinding_projection_fixture,
    )
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
            &availability,
            &keymap,
            &contexts,
        )
        .unwrap();
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

    let disabled = actions
        .resolve_keymap_sequence(
            "ctrl-k ctrl-s",
            &registry,
            &availability,
            &keymap,
            &contexts,
        )
        .unwrap();
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

    let hidden = actions
        .resolve_keymap_sequence(
            "ctrl-k ctrl-h",
            &registry,
            &availability,
            &keymap,
            &contexts,
        )
        .unwrap();
    let hidden_serialized =
        serde_json::to_string(command::command_keymap_resolution_probe_snapshot(&hidden).tree())
            .unwrap();

    assert!(hidden_serialized.contains("\"state\":\"hidden\""));
    assert!(!hidden_serialized.contains("\"Hidden\""));
}
