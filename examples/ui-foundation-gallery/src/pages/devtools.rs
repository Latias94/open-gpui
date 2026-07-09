//! Devtools inspector gallery page.

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicU64, Ordering},
};

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
    DevtoolsCapture, DevtoolsDomainId, DevtoolsDomainKind, DevtoolsEventKind, DevtoolsEventRecord,
    DevtoolsEventRecorder, DevtoolsInspectorState, DevtoolsRegistry, DevtoolsSession,
    DevtoolsSessionError, DevtoolsSessionExport, DevtoolsSessionFrame, DevtoolsTargetId,
    DevtoolsTargetKind, DevtoolsTargetSnapshot, DevtoolsWorkbench, DevtoolsWorkbenchRefreshStatus,
    ProbeId, SnapshotCollection, SnapshotDiagnostic, SnapshotKind,
    adapters::sanitize_sensitive_text, command as devtools_command, form, gpui, motion, resource,
    ui_components,
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
    "open_gpui_devtools::DevtoolsCapture",
    "open_gpui_devtools::DevtoolsInspectorState",
    "open_gpui_devtools::DevtoolsInspector",
    "open_gpui_devtools::DevtoolsTargetId",
    "open_gpui_devtools::DevtoolsDomainId",
    "open_gpui_devtools::DevtoolsEventRecorder",
    "open_gpui_devtools::SnapshotEnvelope",
    "open_gpui_devtools::SnapshotKind",
    "open_gpui_devtools::SnapshotRedactionSummary",
    "open_gpui_devtools::command::command_registry_snapshot_probe",
    "open_gpui_devtools::command::command_keybinding_projection_snapshot_probe",
    "open_gpui_devtools::command::command_keymap_resolution_snapshot_probe",
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

/// Allowlisted shell facts that Gallery contributes to its live DevTools workbench.
#[derive(Clone, Debug, PartialEq)]
pub struct GalleryDevtoolsLiveFacts {
    active_page: String,
    viewport_width_px: f32,
    shell_mode: String,
    density: String,
    control_size: String,
}

impl GalleryDevtoolsLiveFacts {
    /// Creates sanitized Gallery shell facts for DevTools capture.
    pub fn new(
        active_page: impl AsRef<str>,
        viewport_width_px: f32,
        shell_mode: impl AsRef<str>,
        density: impl AsRef<str>,
        control_size: impl AsRef<str>,
    ) -> Self {
        Self {
            active_page: sanitize_sensitive_text(active_page.as_ref()),
            viewport_width_px,
            shell_mode: sanitize_sensitive_text(shell_mode.as_ref()),
            density: sanitize_sensitive_text(density.as_ref()),
            control_size: sanitize_sensitive_text(control_size.as_ref()),
        }
    }

    fn default_devtools_page() -> Self {
        Self::new("devtools", 1040.0, "desktop", "comfortable", "md")
    }
}

/// Latest user-visible Gallery DevTools workbench refresh outcome.
pub type GalleryDevtoolsRefreshStatus = DevtoolsWorkbenchRefreshStatus;

/// Latest user-visible selection retention outcome after a Gallery DevTools refresh.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GalleryDevtoolsSelectionStatus {
    /// No event selection was active before refresh.
    None,
    /// The exact event identity remained selected after refresh.
    Preserved,
    /// The previous event identity disappeared and inspector state selected another visible event.
    Remapped,
}

impl GalleryDevtoolsSelectionStatus {
    /// Returns the stable status label used by tests and UI selectors.
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Preserved => "selection-preserved",
            Self::Remapped => "selection-remapped",
        }
    }
}

/// Shell-owned Gallery DevTools session and bounded history owner.
pub struct GalleryDevtoolsWorkbench {
    workbench: DevtoolsWorkbench,
    live_facts: Arc<Mutex<GalleryDevtoolsLiveFacts>>,
    selection_status: GalleryDevtoolsSelectionStatus,
}

impl std::fmt::Debug for GalleryDevtoolsWorkbench {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GalleryDevtoolsWorkbench")
            .field("session_id", &self.workbench.session_id())
            .field("current_generation", &self.current_generation())
            .field("retained_frames", &self.retained_frames())
            .field("refresh_status", &self.refresh_status())
            .field("selection_status", &self.selection_status)
            .finish()
    }
}

impl GalleryDevtoolsWorkbench {
    /// Creates a live Gallery DevTools workbench seeded with two deterministic frames.
    pub fn new(initial_facts: GalleryDevtoolsLiveFacts) -> Self {
        let live_facts = Arc::new(Mutex::new(initial_facts.clone()));
        let session = devtools_gallery_session_with_facts(Arc::clone(&live_facts));
        let workbench = DevtoolsWorkbench::from_session(session);
        let mut workbench = Self {
            workbench,
            live_facts,
            selection_status: GalleryDevtoolsSelectionStatus::None,
        };
        workbench
            .refresh_with_facts(initial_facts.clone())
            .expect("gallery devtools workbench first refresh succeeds");
        workbench
            .refresh_with_facts(initial_facts)
            .expect("gallery devtools workbench second refresh succeeds");
        workbench.workbench.mark_idle();
        workbench
    }

    /// Returns the inspector state for the current shell-owned frame.
    pub fn inspector_state(&self) -> DevtoolsInspectorState {
        self.workbench.inspector_state()
    }

    /// Refreshes the shell-owned session with new allowlisted Gallery facts.
    pub fn refresh_with_facts(
        &mut self,
        facts: GalleryDevtoolsLiveFacts,
    ) -> Result<DevtoolsSessionFrame, DevtoolsSessionError> {
        self.set_live_facts(facts);
        self.workbench.refresh()
    }

    /// Records the event selection retention outcome after a controller refresh.
    pub fn set_selection_status(&mut self, status: GalleryDevtoolsSelectionStatus) {
        self.selection_status = status;
    }

    /// Returns the latest refresh status.
    pub fn refresh_status(&self) -> GalleryDevtoolsRefreshStatus {
        self.workbench.refresh_status()
    }

    /// Returns the latest selection status.
    pub const fn selection_status(&self) -> GalleryDevtoolsSelectionStatus {
        self.selection_status
    }

    /// Returns the retained frame count.
    pub fn retained_frames(&self) -> usize {
        self.workbench.retained_frames()
    }

    /// Returns the configured session history limit.
    pub fn history_limit(&self) -> usize {
        self.workbench.history_limit()
    }

    /// Returns the current generation, if a frame exists.
    pub fn current_generation(&self) -> Option<u64> {
        self.workbench.current_generation()
    }

    /// Returns the previous generation, if one is attached to the current frame.
    pub fn previous_generation(&self) -> Option<u64> {
        self.workbench.previous_generation()
    }

    /// Returns the current diff row count.
    pub fn diff_row_count(&self) -> usize {
        self.workbench.diff_row_count()
    }

    /// Returns the current diff state label.
    pub fn diff_state_label(&self) -> &'static str {
        self.workbench.diff_state_label()
    }

    /// Returns the latest sanitized refresh error, if one exists.
    pub fn last_error(&self) -> Option<&str> {
        self.workbench.last_error()
    }

    fn set_live_facts(&self, facts: GalleryDevtoolsLiveFacts) {
        let mut live_facts = self
            .live_facts
            .lock()
            .expect("gallery devtools live facts lock is not poisoned");
        *live_facts = facts;
    }
}

/// Returns the deterministic devtools inspector state used by the gallery.
pub fn devtools_gallery_state() -> DevtoolsInspectorState {
    DevtoolsInspectorState::from_session_frame(devtools_gallery_session_frame())
}

/// Returns the deterministic target/domain/event capture used by the gallery.
pub fn devtools_gallery_capture() -> DevtoolsCapture {
    devtools_gallery_session_frame().capture
}

/// Returns the latest deterministic session frame used by the gallery workbench.
pub fn devtools_gallery_session_frame() -> DevtoolsSessionFrame {
    let mut session = devtools_gallery_session();
    session
        .refresh()
        .expect("gallery devtools session first refresh succeeds");
    session
        .refresh()
        .expect("gallery devtools session second refresh succeeds")
}

/// Returns a sanitized deterministic two-frame session export for offline replay tests.
pub fn devtools_gallery_session_export() -> DevtoolsSessionExport {
    let mut session = devtools_gallery_session();
    session
        .refresh()
        .expect("gallery devtools session first refresh succeeds");
    session
        .refresh()
        .expect("gallery devtools session second refresh succeeds");
    session.export()
}

fn devtools_gallery_session() -> DevtoolsSession {
    devtools_gallery_session_with_facts(Arc::new(Mutex::new(
        GalleryDevtoolsLiveFacts::default_devtools_page(),
    )))
}

fn devtools_gallery_session_with_facts(
    live_facts: Arc<Mutex<GalleryDevtoolsLiveFacts>>,
) -> DevtoolsSession {
    let mut registry = DevtoolsRegistry::default();
    let refresh_index = Arc::new(AtomicU64::new(0));
    let provider_refresh_index = Arc::clone(&refresh_index);
    let provider_live_facts = Arc::clone(&live_facts);
    registry
        .register_capture_provider_fn("gallery.devtools", move || {
            let refresh_index = provider_refresh_index.fetch_add(1, Ordering::SeqCst) + 1;
            let live_facts = provider_live_facts
                .lock()
                .map_err(|_| {
                    open_gpui_devtools::ProbeSnapshotError::CollectionFailed(
                        "gallery live facts lock poisoned".to_owned(),
                    )
                })?
                .clone();
            Ok(devtools_gallery_provider_capture(
                refresh_index,
                &live_facts,
            ))
        })
        .expect("unique gallery devtools capture provider");

    DevtoolsSession::new("gallery.devtools", registry).with_history_limit(4)
}

fn devtools_gallery_provider_capture(
    refresh_index: u64,
    live_facts: &GalleryDevtoolsLiveFacts,
) -> DevtoolsCapture {
    let collection = devtools_gallery_legacy_collection();
    let base_capture = DevtoolsCapture::from_snapshot_collection(collection);
    let gpui_capture = gpui::gpui_runtime_capture(&gallery_gpui_runtime_sample(refresh_index));
    let shell_target_id =
        DevtoolsTargetId::from_parts(["gallery", "shell", live_facts.active_page.as_str()]);
    let shell_domain_id = DevtoolsDomainId::from_parts(["gallery", "shell", "live"]);
    let shell_payload = gallery_shell_live_payload(refresh_index, live_facts);
    let timeline_probe_id = ProbeId::new("timeline.motion-frame").expect("valid timeline probe id");
    let timeline_target_id = DevtoolsTargetId::from_probe_id(&timeline_probe_id);
    let timeline_domain_id =
        DevtoolsDomainId::from_probe_snapshot(&timeline_probe_id, &SnapshotKind::Timeline);
    let mut recorder = DevtoolsEventRecorder::new("gallery.devtools", "Gallery DevTools", 16);
    recorder.record(
        DevtoolsEventRecord::new(
            "gallery.motion-frame-demand",
            "Gallery motion frame demand",
            DevtoolsEventKind::Instant,
        )
        .target_id(timeline_target_id)
        .domain_id(timeline_domain_id)
        .timestamp_ms(40 + refresh_index)
        .with_payload(serde_json::json!({
            "page": "devtools",
            "source": "ui-foundation-gallery",
            "refresh_index": refresh_index,
            "needs_frame": refresh_index % 2 == 0,
        })),
    );
    recorder.record(
        DevtoolsEventRecord::new(
            "gallery.shell-live-facts",
            "Gallery shell live facts",
            DevtoolsEventKind::Instant,
        )
        .target_id(shell_target_id.clone())
        .domain_id(shell_domain_id.clone())
        .timestamp_ms(80 + refresh_index)
        .with_payload(shell_payload.clone()),
    );
    let event_batch = recorder.snapshot();
    let mut targets = base_capture.targets.targets;
    targets.extend(gpui_capture.targets.targets);
    targets.push(
        DevtoolsTargetSnapshot::new(
            shell_target_id.clone(),
            DevtoolsTargetKind::App,
            "Gallery shell",
        )
        .with_metadata(shell_payload.clone()),
    );
    let mut domains = base_capture.domains;
    domains.extend(gpui_capture.domains);
    domains.push(
        open_gpui_devtools::DevtoolsDomainSnapshot::new(
            shell_domain_id,
            shell_target_id,
            DevtoolsDomainKind::Custom("gallery-shell".to_owned()),
            "Gallery shell live facts",
        )
        .with_summary(shell_payload),
    );
    let mut events = event_batch.events;
    events.extend(gpui_capture.events);
    let mut snapshots = base_capture.snapshots;
    snapshots.extend(gpui_capture.snapshots);
    let mut diagnostics = base_capture.diagnostics;
    diagnostics.extend(gpui_capture.diagnostics);

    DevtoolsCapture::new(
        open_gpui_devtools::DevtoolsTargetTree::new(targets),
        domains,
        events,
        snapshots,
        diagnostics,
    )
}

fn gallery_shell_live_payload(
    refresh_index: u64,
    live_facts: &GalleryDevtoolsLiveFacts,
) -> serde_json::Value {
    serde_json::json!({
        "refresh_index": refresh_index,
        "active_page": live_facts.active_page,
        "viewport_width_px": live_facts.viewport_width_px,
        "shell_mode": live_facts.shell_mode,
        "density": live_facts.density,
        "control_size": live_facts.control_size,
    })
}

/// Returns the deterministic snapshot collection used by the gallery.
pub fn devtools_gallery_collection() -> SnapshotCollection {
    devtools_gallery_capture().snapshot_collection()
}

fn devtools_gallery_legacy_collection() -> SnapshotCollection {
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
            devtools_command::command_keybinding_projection_snapshot_probe(
                "command.keybindings",
                command_keybinding_projection_sample,
            )
            .expect("valid command keybinding probe"),
        )
        .expect("unique command keybinding probe");
    registry
        .register(
            devtools_command::command_keymap_resolution_snapshot_probe(
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

fn gallery_gpui_runtime_sample(refresh_index: u64) -> gpui::GpuiRuntimeSnapshot {
    gpui::GpuiRuntimeSnapshot {
        runtime_id: "gallery".to_owned(),
        generation: refresh_index,
        windows: vec![gpui::GpuiRuntimeWindowSnapshot {
            window_id: 1,
            display_id: Some("gallery-display".to_owned()),
            active: true,
            focused: true,
            bounds: Some(gpui::GpuiRuntimeRectSnapshot {
                origin: gpui::GpuiRuntimePointSnapshot { x: 0.0, y: 0.0 },
                size: gpui::GpuiRuntimeSizeSnapshot {
                    width: 1024.0,
                    height: 768.0,
                },
            }),
            content_size: Some(gpui::GpuiRuntimeSizeSnapshot {
                width: 1008.0,
                height: 720.0,
            }),
            scale_factor: Some(1.0),
        }],
        focus: Some(gpui::GpuiRuntimeFocusSnapshot {
            active_window_id: Some(1),
            focused_window_id: Some(1),
            focus_scope_count: 3,
            focus_handle_count: 9,
        }),
        input: Some(gpui::GpuiRuntimeInputSnapshot {
            key_down_count: refresh_index,
            pointer_event_count: refresh_index.saturating_add(1),
            scroll_event_count: 1,
            text_input_event_count: 0,
            ime_event_count: 0,
            clipboard_event_count: 0,
            last_event_kind: Some("refresh".to_owned()),
        }),
        frame: Some(gpui::GpuiRuntimeFrameSnapshot {
            requested_frames: refresh_index.saturating_add(2),
            painted_frames: refresh_index.saturating_add(1),
            animation_frame_count: 1,
            last_frame_duration_ms: Some(16.0 + refresh_index as f32),
            last_presented_generation: Some(refresh_index),
        }),
        scroll_viewports: vec![gpui::GpuiRuntimeScrollSnapshot::from_scroll_viewport(
            gallery_scroll_viewport_sample(),
        )],
        diagnostics: Vec::new(),
    }
}

fn unmounted_framework_diagnostics() -> Vec<SnapshotDiagnostic> {
    vec![SnapshotDiagnostic::new(
        ProbeId::new("docking").unwrap(),
        "runtime.unavailable",
        "docking runtime is not mounted in this gallery page",
    )]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn devtools_gallery_state_exposes_redacted_snapshots_and_diagnostics() {
        let state = devtools_gallery_state();
        let rows = state.snapshot_rows();

        assert_eq!(rows.len(), 11);
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
        assert!(
            rows.iter()
                .any(|row| row.probe_id.as_str() == "gpui.runtime.gallery"
                    && row.kind_label == "gpui-runtime")
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
        assert_eq!(state.diagnostics().len(), 1);
        assert_eq!(state.target_rows().len(), 15);
        let event_state = devtools_gallery_state().with_filter("motion-frame-demand");
        assert!(event_state.event_rows().iter().any(|row| {
            row.event_id == "gallery.motion-frame-demand" && row.kind_label == "instant"
        }));
    }
}
