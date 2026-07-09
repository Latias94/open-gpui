//! Devtools inspector gallery page.

use open_gpui::{
    KeyBinding, KeyContext, Keymap, ScrollViewportChangeSource, ScrollViewportSnapshot, actions,
    bounds, point, px, size,
};
use open_gpui_command::{
    CommandAvailabilityMap, CommandContribution, CommandDescriptor, CommandIconDescriptor,
    CommandKeyBinding, CommandKeyBindingProjection, CommandKeyBindingRegistry,
    CommandKeymapResolution, CommandRegistrySnapshot, GpuiCommandActionMap,
};
use open_gpui_devtools::{
    DevtoolsInspectorState, DevtoolsRegistry, ProbeId, SnapshotCollection, SnapshotDiagnostic,
    SnapshotKind, command as devtools_command, form, gpui, motion, resource, ui_components,
};
use open_gpui_motion::{MotionFrameDemand, MotionFrameReason};
use open_gpui_resource::PaginatedResourceSnapshotView;
use open_gpui_ui_components::{COMPONENT_A11Y_EVIDENCE, ThemeSnapshot};

use super::components::{form_devtools_dogfood_snapshot, resource_devtools_dogfood_snapshots};

/// Page title.
pub const TITLE: &str = "DevTools";
/// Page summary.
pub const SUMMARY: &str = "Read-only local inspection over redacted snapshot probes.";
/// Foundation signals exercised by this page.
pub const SIGNALS: &[&str] = &[
    "open_gpui_devtools::DevtoolsRegistry",
    "open_gpui_devtools::DevtoolsInspectorState",
    "open_gpui_devtools::DevtoolsInspector",
    "open_gpui_devtools::SnapshotEnvelope",
    "open_gpui_devtools::SnapshotKind",
    "open_gpui_devtools::SnapshotRedactionSummary",
    "open_gpui_devtools::command::command_registry_snapshot_probe",
    "open_gpui_devtools::command::command_keybinding_projection_probe",
    "open_gpui_devtools::command::command_keymap_resolution_probe",
    "open_gpui_devtools::form::form_snapshot_probe",
    "open_gpui_devtools::gpui::scroll_viewport_layout_probe_snapshot",
    "open_gpui_devtools::resource::resource_snapshot_probe",
    "open_gpui_devtools::ui_components::theme_probe_snapshot",
    "open_gpui_devtools::ui_components::a11y_evidence_probe_snapshot",
    "open_gpui_devtools::motion::motion_frame_demand_probe_snapshot",
    "open_gpui_devtools::motion::motion_frame_demand_timeline_probe_snapshot",
];

actions!(
    gallery_devtools_command,
    [OpenCommandPalette, SaveWorkspace, ToggleDevtools]
);

/// Returns the deterministic devtools inspector state used by the gallery.
pub fn devtools_gallery_state() -> DevtoolsInspectorState {
    DevtoolsInspectorState::new(devtools_gallery_collection())
}

/// Returns the deterministic snapshot collection used by the gallery.
pub fn devtools_gallery_collection() -> SnapshotCollection {
    let mut registry = DevtoolsRegistry::default();
    let form_snapshot = form_devtools_dogfood_snapshot();
    let resource_snapshots = resource_devtools_dogfood_snapshots();
    let resource_snapshot = resource_snapshots.resource;
    let mutation_snapshot = resource_snapshots.mutation;

    registry
        .register_snapshot_probe("accessibility", SnapshotKind::Accessibility, || {
            Ok(ui_components::a11y_evidence_probe_snapshot(
                COMPONENT_A11Y_EVIDENCE,
            ))
        })
        .expect("unique accessibility probe");
    registry
        .register(
            devtools_command::command_registry_snapshot_probe(
                "command.registry",
                command_registry_sample,
            )
            .expect("valid command registry probe"),
        )
        .expect("unique command registry probe");
    registry
        .register(
            devtools_command::command_keybinding_projection_probe(
                "command.keybindings",
                command_keybinding_projection_sample,
            )
            .expect("valid command keybinding probe"),
        )
        .expect("unique command keybinding probe");
    registry
        .register(
            devtools_command::command_keymap_resolution_probe(
                "command.keymap",
                command_keymap_resolution_sample,
            )
            .expect("valid command keymap probe"),
        )
        .expect("unique command keymap probe");
    registry
        .register(
            form::form_snapshot_probe("form", move || form_snapshot.clone())
                .expect("valid form probe"),
        )
        .expect("unique form probe");
    registry
        .register_snapshot_probe("layout.scroll-viewport", SnapshotKind::Layout, || {
            Ok(gpui::scroll_viewport_layout_probe_snapshot(
                gallery_scroll_viewport_sample(),
            ))
        })
        .expect("unique layout scroll viewport probe");
    registry
        .register_snapshot_probe("motion", SnapshotKind::Motion, || {
            Ok(motion::motion_frame_demand_probe_snapshot(
                MotionFrameDemand::NeedsFrame(MotionFrameReason::UpdateRender),
            ))
        })
        .expect("unique motion probe");
    registry
        .register_snapshot_probe("timeline.motion-frame", SnapshotKind::Timeline, || {
            Ok(motion::motion_frame_demand_timeline_probe_snapshot(
                MotionFrameDemand::NeedsFrame(MotionFrameReason::UpdateRender),
            ))
        })
        .expect("unique motion timeline probe");
    registry
        .register(
            resource::resource_snapshot_probe(
                "resource",
                move || vec![resource_snapshot.clone()],
                move || vec![mutation_snapshot.clone()],
                Vec::<PaginatedResourceSnapshotView>::new,
            )
            .expect("valid resource probe"),
        )
        .expect("unique resource probe");
    registry
        .register_snapshot_probe("theme", SnapshotKind::Theme, || {
            Ok(ui_components::theme_probe_snapshot(ThemeSnapshot::light()))
        })
        .expect("unique theme probe");

    let mut collection = registry.collect();
    collection
        .diagnostics
        .extend(unmounted_framework_diagnostics());
    collection
}

fn command_registry_sample() -> CommandRegistrySnapshot {
    CommandRegistrySnapshot::new(
        "gallery-devtools-command-registry-v1",
        [
            CommandContribution::new(
                CommandDescriptor::new("gallery.command_palette.open", "Open Command Palette")
                    .icon(CommandIconDescriptor::new("command").fallback_label("Command"))
                    .group("Navigation")
                    .keyword("palette")
                    .shortcut("Ctrl+K")
                    .accessibility_description("Opens the gallery command palette")
                    .menu_path(["View", "Command Palette"]),
            )
            .source("gallery.core"),
            CommandContribution::new(
                CommandDescriptor::new("gallery.workspace.save", "Save Workspace")
                    .group("Workspace")
                    .keyword("persist")
                    .shortcut("Ctrl+S")
                    .tooltip("Save the active gallery workspace"),
            )
            .source("gallery.core"),
            CommandContribution::new(
                CommandDescriptor::new("gallery.devtools.toggle", "Toggle DevTools")
                    .group("Diagnostics")
                    .keyword("inspect")
                    .shortcut("Ctrl+K Ctrl+D"),
            )
            .source("gallery.devtools"),
        ],
    )
}

fn command_keybinding_projection_sample() -> CommandKeyBindingProjection {
    let mut registry = CommandKeyBindingRegistry::new();
    registry.register(
        "gallery-defaults",
        [
            CommandKeyBinding::new("gallery.command_palette.open", "ctrl-k").context("Gallery"),
            CommandKeyBinding::new("gallery.devtools.toggle", "ctrl-k").context("Gallery"),
            CommandKeyBinding::new("gallery.command_palette.missing", "ctrl-m"),
            CommandKeyBinding::new("gallery.workspace.save", "ctrl-s").context("Gallery &&"),
        ],
    );

    registry.project(&command_action_map())
}

fn command_keymap_resolution_sample() -> CommandKeymapResolution {
    let registry = command_registry_sample();
    let actions = command_action_map();
    let mut keymap = Keymap::default();
    keymap.add_bindings([
        KeyBinding::new("ctrl-k ctrl-p", OpenCommandPalette, Some("Gallery")),
        KeyBinding::new("ctrl-k ctrl-d", ToggleDevtools, Some("Gallery")),
    ]);
    let contexts = [KeyContext::parse("Gallery").expect("valid gallery key context")];

    actions
        .resolve_keymap_sequence(
            "ctrl-k",
            &registry,
            &CommandAvailabilityMap::new(),
            &keymap,
            &contexts,
        )
        .expect("gallery command keymap sample should resolve")
}

fn command_action_map() -> GpuiCommandActionMap {
    GpuiCommandActionMap::new()
        .action("gallery.command_palette.open", OpenCommandPalette)
        .action("gallery.workspace.save", SaveWorkspace)
        .action("gallery.devtools.toggle", ToggleDevtools)
}

fn gallery_scroll_viewport_sample() -> ScrollViewportSnapshot {
    ScrollViewportSnapshot::new(
        42,
        ScrollViewportChangeSource::InitialLayout,
        bounds(point(px(12.0), px(24.0)), size(px(640.0), px(360.0))),
        point(px(8.0), px(16.0)),
        point(px(80.0), px(160.0)),
        size(px(960.0), px(720.0)),
    )
}

fn unmounted_framework_diagnostics() -> Vec<SnapshotDiagnostic> {
    vec![
        gpui::scroll_viewport_unavailable_diagnostic(ProbeId::new("scroll").unwrap()),
        SnapshotDiagnostic::new(
            ProbeId::new("docking").unwrap(),
            "runtime.unavailable",
            "docking runtime is not mounted in this gallery page",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devtools_gallery_state_exposes_redacted_snapshots_and_diagnostics() {
        let state = devtools_gallery_state();
        let rows = state.snapshot_rows();

        assert_eq!(rows.len(), 10);
        assert!(rows.iter().any(
            |row| row.probe_id.as_str() == "accessibility" && row.kind_label == "accessibility"
        ));
        assert!(
            rows.iter()
                .any(|row| row.probe_id.as_str() == "command.registry"
                    && row.category_label == "command"
                    && row.kind_label == "command")
        );
        assert!(
            rows.iter()
                .any(|row| row.probe_id.as_str() == "command.keybindings")
        );
        assert!(
            rows.iter()
                .any(|row| row.probe_id.as_str() == "command.keymap")
        );
        assert!(rows.iter().any(|row| row.probe_id.as_str() == "form"
            && row.kind_label == "form"
            && row.redacted_values == 5));
        assert!(
            rows.iter()
                .any(|row| row.probe_id.as_str() == "layout.scroll-viewport"
                    && row.category_label == "layout"
                    && row.kind_label == "layout")
        );
        assert!(rows.iter().any(|row| row.probe_id.as_str() == "resource"
            && row.kind_label == "resource"
            && row.redacted_values == 2));
        assert!(rows.iter().any(|row| row.probe_id.as_str() == "motion"));
        assert!(
            rows.iter()
                .any(|row| row.probe_id.as_str() == "timeline.motion-frame"
                    && row.category_label == "timeline"
                    && row.kind_label == "timeline")
        );
        assert!(rows.iter().any(|row| row.probe_id.as_str() == "theme"));
        assert_eq!(state.diagnostics().len(), 2);
    }
}
