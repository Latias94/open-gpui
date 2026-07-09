//! DevTools adapters for `open-gpui-command` registry and keybinding facts.

use std::collections::BTreeSet;

use open_gpui_command::{
    CommandContribution, CommandDescriptor, CommandKeyBindingConflict, CommandKeyBindingDiagnostic,
    CommandKeyBindingDiagnosticKind, CommandKeyBindingProjectedEntry, CommandKeyBindingProjection,
    CommandKeymapCommandState, CommandKeymapResolution, CommandKeymapResolvedCommand,
    CommandRegistrySnapshot,
};

use crate::{
    ProbeId, ProbeSnapshotError, SnapshotEnvelope, SnapshotKind, SnapshotNode, SnapshotProbe,
    SnapshotProbeSnapshot, SnapshotRedactionSummary, SnapshotTree,
    adapters::snapshot_node_with_payload,
};

/// Converts a command registry snapshot into a DevTools tree.
pub fn command_registry_probe_snapshot(
    snapshot: &CommandRegistrySnapshot,
) -> SnapshotProbeSnapshot {
    SnapshotProbeSnapshot::new(command_registry_tree(snapshot))
        .with_redaction(SnapshotRedactionSummary::default())
}

/// Converts a command registry snapshot into a DevTools envelope.
pub fn command_registry_snapshot_envelope(
    probe_id: ProbeId,
    snapshot: &CommandRegistrySnapshot,
) -> SnapshotEnvelope {
    SnapshotEnvelope::new(
        probe_id,
        SnapshotKind::Command,
        command_registry_tree(snapshot),
    )
    .with_redaction(SnapshotRedactionSummary::default())
}

/// Builds a closure-backed command registry snapshot probe.
pub fn command_registry_snapshot_probe<F>(
    id: impl Into<String>,
    snapshot: F,
) -> Result<
    SnapshotProbe<impl Fn() -> Result<SnapshotProbeSnapshot, ProbeSnapshotError> + Send + Sync>,
    ProbeSnapshotError,
>
where
    F: Fn() -> CommandRegistrySnapshot + Send + Sync + 'static,
{
    SnapshotProbe::new(id, SnapshotKind::Command, move || {
        Ok(command_registry_probe_snapshot(&snapshot()))
    })
}

/// Converts command keybinding projection facts into a DevTools tree.
pub fn command_keybinding_projection_probe_snapshot(
    projection: &CommandKeyBindingProjection,
) -> SnapshotProbeSnapshot {
    SnapshotProbeSnapshot::new(command_keybinding_projection_tree(projection))
        .with_redaction(SnapshotRedactionSummary::default())
}

/// Converts command keybinding projection facts into a DevTools envelope.
pub fn command_keybinding_projection_envelope(
    probe_id: ProbeId,
    projection: &CommandKeyBindingProjection,
) -> SnapshotEnvelope {
    SnapshotEnvelope::new(
        probe_id,
        SnapshotKind::Command,
        command_keybinding_projection_tree(projection),
    )
    .with_redaction(SnapshotRedactionSummary::default())
}

/// Builds a closure-backed command keybinding projection probe.
pub fn command_keybinding_projection_probe<F>(
    id: impl Into<String>,
    projection: F,
) -> Result<
    SnapshotProbe<impl Fn() -> Result<SnapshotProbeSnapshot, ProbeSnapshotError> + Send + Sync>,
    ProbeSnapshotError,
>
where
    F: Fn() -> CommandKeyBindingProjection + Send + Sync + 'static,
{
    SnapshotProbe::new(id, SnapshotKind::Command, move || {
        Ok(command_keybinding_projection_probe_snapshot(&projection()))
    })
}

/// Converts one command keymap resolution into a DevTools tree.
pub fn command_keymap_resolution_probe_snapshot(
    resolution: &CommandKeymapResolution,
) -> SnapshotProbeSnapshot {
    SnapshotProbeSnapshot::new(command_keymap_resolution_tree(resolution))
        .with_redaction(SnapshotRedactionSummary::default())
}

/// Converts one command keymap resolution into a DevTools envelope.
pub fn command_keymap_resolution_envelope(
    probe_id: ProbeId,
    resolution: &CommandKeymapResolution,
) -> SnapshotEnvelope {
    SnapshotEnvelope::new(
        probe_id,
        SnapshotKind::Command,
        command_keymap_resolution_tree(resolution),
    )
    .with_redaction(SnapshotRedactionSummary::default())
}

/// Builds a closure-backed command keymap resolution probe.
pub fn command_keymap_resolution_probe<F>(
    id: impl Into<String>,
    resolution: F,
) -> Result<
    SnapshotProbe<impl Fn() -> Result<SnapshotProbeSnapshot, ProbeSnapshotError> + Send + Sync>,
    ProbeSnapshotError,
>
where
    F: Fn() -> CommandKeymapResolution + Send + Sync + 'static,
{
    SnapshotProbe::new(id, SnapshotKind::Command, move || {
        Ok(command_keymap_resolution_probe_snapshot(&resolution()))
    })
}

fn command_registry_tree(snapshot: &CommandRegistrySnapshot) -> SnapshotTree {
    let source_count = snapshot
        .contributions()
        .iter()
        .filter_map(CommandContribution::source_ref)
        .collect::<BTreeSet<_>>()
        .len();
    let mut root = snapshot_node_with_payload(
        ["command", "registry"],
        "Command registry",
        serde_json::json!({
            "revision": snapshot.revision(),
            "command_count": snapshot.len(),
            "source_count": source_count,
            "empty": snapshot.is_empty(),
        }),
    );

    for (index, contribution) in snapshot.contributions().iter().enumerate() {
        root = root.with_child(command_contribution_node(index, contribution));
    }

    SnapshotTree::new([root])
}

fn command_contribution_node(index: usize, contribution: &CommandContribution) -> SnapshotNode {
    let index_label = index.to_string();
    let descriptor = contribution.descriptor();
    snapshot_node_with_payload(
        ["command", "registry", index_label.as_str(), descriptor.id()],
        descriptor.label(),
        serde_json::json!({
            "id": descriptor.id(),
            "label": descriptor.label(),
            "source": contribution.source_ref(),
            "icon": command_icon_payload(descriptor),
            "group": descriptor.group_ref(),
            "keywords": descriptor.keywords_ref(),
            "shortcut": descriptor.shortcut_ref(),
            "disabled": descriptor.disabled_state(),
            "disabled_reason": descriptor.disabled_reason_ref(),
            "tooltip": descriptor.tooltip_ref(),
            "accessibility_description": descriptor.accessibility_description_ref(),
            "when": descriptor.when_ref(),
            "menu_path": descriptor.menu_path_ref(),
        }),
    )
}

fn command_icon_payload(descriptor: &CommandDescriptor) -> Option<serde_json::Value> {
    descriptor.icon_ref().map(|icon| {
        serde_json::json!({
            "name": icon.name(),
            "fallback_label": icon.fallback_label_ref(),
        })
    })
}

fn command_keybinding_projection_tree(projection: &CommandKeyBindingProjection) -> SnapshotTree {
    let mut root = snapshot_node_with_payload(
        ["command", "keybindings"],
        "Command keybindings",
        serde_json::json!({
            "projected_binding_count": projection.projected_entries().len(),
            "installed_key_binding_count": projection.key_bindings().len(),
            "diagnostic_count": projection.diagnostics().len(),
            "conflict_count": projection.conflicts().len(),
            "clean": projection.is_clean(),
            "has_conflicts": projection.has_conflicts(),
            "strictly_clean": projection.is_strictly_clean(),
        }),
    );

    root = root.with_child(projected_bindings_node(projection.projected_entries()));
    root = root.with_child(keybinding_diagnostics_node(projection.diagnostics()));
    root = root.with_child(keybinding_conflicts_node(projection.conflicts()));

    SnapshotTree::new([root])
}

fn projected_bindings_node(entries: &[CommandKeyBindingProjectedEntry]) -> SnapshotNode {
    let mut root = snapshot_node_with_payload(
        ["command", "keybindings", "projected"],
        "Projected bindings",
        serde_json::json!({ "count": entries.len() }),
    );

    for (index, entry) in entries.iter().enumerate() {
        let index_label = index.to_string();
        root = root.with_child(snapshot_node_with_payload(
            [
                "command",
                "keybindings",
                "projected",
                index_label.as_str(),
                entry.command_id(),
            ],
            entry.command_id(),
            serde_json::json!({
                "source": entry.source_id().as_str(),
                "command_id": entry.command_id(),
                "keystrokes": entry.keystrokes(),
                "raw_keystrokes": entry.raw_keystrokes(),
                "context": entry.context_ref(),
                "raw_context": entry.raw_context_ref(),
            }),
        ));
    }

    root
}

fn keybinding_diagnostics_node(diagnostics: &[CommandKeyBindingDiagnostic]) -> SnapshotNode {
    let mut root = snapshot_node_with_payload(
        ["command", "keybindings", "diagnostics"],
        "Projection diagnostics",
        serde_json::json!({ "count": diagnostics.len() }),
    );

    for (index, diagnostic) in diagnostics.iter().enumerate() {
        root = root.with_child(keybinding_diagnostic_node(index, diagnostic));
    }

    root
}

fn keybinding_diagnostic_node(
    index: usize,
    diagnostic: &CommandKeyBindingDiagnostic,
) -> SnapshotNode {
    let index_label = index.to_string();
    let kind = keybinding_diagnostic_kind_label(diagnostic.kind());
    snapshot_node_with_payload(
        [
            "command",
            "keybindings",
            "diagnostics",
            index_label.as_str(),
            diagnostic.command_id(),
        ],
        format!("{kind} {}", diagnostic.command_id()),
        serde_json::json!({
            "kind": kind,
            "source": diagnostic.source_id().as_str(),
            "command_id": diagnostic.command_id(),
            "keystrokes": diagnostic.keystrokes(),
            "context": diagnostic.context_ref(),
            "message": diagnostic.message(),
        }),
    )
}

fn keybinding_conflicts_node(conflicts: &[CommandKeyBindingConflict]) -> SnapshotNode {
    let mut root = snapshot_node_with_payload(
        ["command", "keybindings", "conflicts"],
        "Keybinding conflicts",
        serde_json::json!({ "count": conflicts.len() }),
    );

    for (index, conflict) in conflicts.iter().enumerate() {
        root = root.with_child(keybinding_conflict_node(index, conflict));
    }

    root
}

fn keybinding_conflict_node(index: usize, conflict: &CommandKeyBindingConflict) -> SnapshotNode {
    let index_label = index.to_string();
    let mut node = snapshot_node_with_payload(
        [
            "command",
            "keybindings",
            "conflicts",
            index_label.as_str(),
            conflict.keystrokes(),
        ],
        format!("Conflict {}", conflict.keystrokes()),
        serde_json::json!({
            "keystrokes": conflict.keystrokes(),
            "context": conflict.context_ref(),
            "entry_count": conflict.entries().len(),
        }),
    );

    for (entry_index, entry) in conflict.entries().iter().enumerate() {
        let entry_index_label = entry_index.to_string();
        node = node.with_child(snapshot_node_with_payload(
            [
                "command",
                "keybindings",
                "conflicts",
                index_label.as_str(),
                "entries",
                entry_index_label.as_str(),
                entry.command_id(),
            ],
            entry.command_id(),
            serde_json::json!({
                "source": entry.source_id().as_str(),
                "command_id": entry.command_id(),
            }),
        ));
    }

    node
}

fn command_keymap_resolution_tree(resolution: &CommandKeymapResolution) -> SnapshotTree {
    let primary_command = resolution
        .primary_command()
        .map(CommandKeymapResolvedCommand::command_id);
    let primary_dispatchable_command = resolution
        .primary_dispatchable_command()
        .map(CommandKeymapResolvedCommand::command_id);
    let mut root = snapshot_node_with_payload(
        ["command", "keymap-resolution"],
        "Command keymap resolution",
        serde_json::json!({
            "input": resolution.input(),
            "input_label": resolution.input_label(),
            "pending": resolution.is_pending(),
            "matched_count": resolution.matched_commands().len(),
            "pending_count": resolution.pending_commands().len(),
            "has_pending_commands": resolution.has_pending_commands(),
            "primary_command": primary_command,
            "primary_dispatchable_command": primary_dispatchable_command,
        }),
    );

    root = root.with_child(resolved_commands_node(
        ["command", "keymap-resolution", "matched"],
        "Matched commands",
        "matched",
        resolution.matched_commands(),
    ));
    root = root.with_child(resolved_commands_node(
        ["command", "keymap-resolution", "pending"],
        "Pending commands",
        "pending",
        resolution.pending_commands(),
    ));

    SnapshotTree::new([root])
}

fn resolved_commands_node<I, S>(
    id_parts: I,
    label: impl AsRef<str>,
    group: &'static str,
    commands: &[CommandKeymapResolvedCommand],
) -> SnapshotNode
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut root = snapshot_node_with_payload(
        id_parts,
        label,
        serde_json::json!({ "count": commands.len() }),
    );

    for (index, command) in commands.iter().enumerate() {
        root = root.with_child(resolved_command_node(group, index, command));
    }

    root
}

fn resolved_command_node(
    group: &'static str,
    index: usize,
    command: &CommandKeymapResolvedCommand,
) -> SnapshotNode {
    let index_label = index.to_string();
    let state = command_state_label(command.state());
    snapshot_node_with_payload(
        [
            "command",
            "keymap-resolution",
            group,
            index_label.as_str(),
            command.command_id(),
        ],
        command.command_id(),
        serde_json::json!({
            "command_id": command.command_id(),
            "shortcut": command.shortcut(),
            "state": state,
            "reason": command.state().reason_ref(),
            "dispatchable": command.is_dispatchable(),
        }),
    )
}

fn keybinding_diagnostic_kind_label(kind: CommandKeyBindingDiagnosticKind) -> &'static str {
    match kind {
        CommandKeyBindingDiagnosticKind::MissingAction => "missing-action",
        CommandKeyBindingDiagnosticKind::InvalidKeystrokes => "invalid-keystrokes",
        CommandKeyBindingDiagnosticKind::InvalidContext => "invalid-context",
    }
}

fn command_state_label(state: &CommandKeymapCommandState) -> &'static str {
    match state {
        CommandKeymapCommandState::Dispatchable => "dispatchable",
        CommandKeymapCommandState::MissingCommand => "missing-command",
        CommandKeymapCommandState::Disabled { .. } => "disabled",
        CommandKeymapCommandState::Hidden => "hidden",
    }
}
