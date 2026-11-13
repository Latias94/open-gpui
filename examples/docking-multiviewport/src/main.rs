use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    rc::Rc,
    time::Duration,
};

use anyhow::{Context as _, Result, ensure};
use open_gpui::{
    AnyWindowHandle, App, Bounds, Context, Entity, Global, IntoElement, ParentElement, Render,
    Styled, TitlebarOptions, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size,
};
use open_gpui_docking::{
    dockspace::{
        close::CloseDecision,
        runtime::{DockspaceDocumentBootstrap, DockspaceDocumentId},
    },
    native::NativeReport,
    persistence::{DockHostDocument, DockSurfaceKeyBinding, DockWorkspaceDocument},
    prelude::{
        DockAnchor, DockPane, DockPlacement, DockPolicy, DockSurface, DockspaceAxis,
        DockspaceLayout, DockspaceNode, DockspaceRootLayout, DockspaceSession,
        DockspaceSurfaceLayout, GpuiDockspaceConfig, ItemId, RootId, SurfaceId,
    },
};
use open_gpui_platform::application;

#[cfg(test)]
mod tests;

const MAIN: SurfaceId = SurfaceId::new(1);
const SECONDARY: SurfaceId = SurfaceId::new(2);
const MAIN_ROOT: RootId = RootId::new(1);
const SECONDARY_ROOT: RootId = RootId::new(2);
const SNAPSHOT_DEBOUNCE: Duration = Duration::from_millis(250);

#[derive(Clone, Copy, PartialEq, Eq)]
enum WorkspaceKind {
    Shared,
    Isolated,
}

impl WorkspaceKind {
    fn name(self) -> &'static str {
        match self {
            Self::Shared => "shared",
            Self::Isolated => "isolated",
        }
    }

    fn document_id(self) -> DockspaceDocumentId {
        match self {
            Self::Shared => DockspaceDocumentId::from_bytes([0x4d; 16]),
            Self::Isolated => DockspaceDocumentId::from_bytes([0x49; 16]),
        }
    }

    fn pane_keys(self) -> &'static [&'static str] {
        match self {
            Self::Shared => &["project", "editor", "inspector", "preview"],
            Self::Isolated => &["activity", "notes"],
        }
    }

    fn policy(self) -> DockPolicy {
        let mut policy = DockPolicy::default();
        policy.set_allow_contained_floating(true);
        policy.set_allow_native_surfaces(self == Self::Shared);
        policy
    }
}

struct Workspace {
    kind: WorkspaceKind,
    dock: DockSurface,
    panels: Vec<Entity<ExamplePanel>>,
    roots: RefCell<BTreeMap<SurfaceId, AnyWindowHandle>>,
    snapshot_export_pending: Cell<bool>,
    last_snapshot: RefCell<Option<Vec<u8>>>,
}

struct DemoApp {
    shared: Rc<Workspace>,
    isolated: Rc<Workspace>,
}

impl Global for DemoApp {}

impl DemoApp {
    fn workspace(&self, kind: WorkspaceKind) -> Rc<Workspace> {
        match kind {
            WorkspaceKind::Shared => self.shared.clone(),
            WorkspaceKind::Isolated => self.isolated.clone(),
        }
    }
}

struct ExamplePanel {
    workspace: WorkspaceKind,
    title: &'static str,
    accent: u32,
    lines: &'static [&'static str],
    edits: usize,
}

impl Render for ExamplePanel {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let workspace = cx.global::<DemoApp>().workspace(self.workspace);
        let snapshot_bytes = workspace
            .last_snapshot
            .borrow()
            .as_ref()
            .map_or(0, Vec::len);
        div()
            .flex()
            .flex_col()
            .size_full()
            .gap_3()
            .p_4()
            .bg(rgb(0xffffff))
            .text_color(rgb(0x111827))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(div().w(px(4.0)).h(px(28.0)).bg(rgb(self.accent)))
                    .child(div().text_lg().child(self.title)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .children(self.lines.iter().map(|line| {
                        div()
                            .px_2()
                            .py_1()
                            .bg(rgb(0xf3f4f6))
                            .text_color(rgb(0x253041))
                            .child(*line)
                    })),
            )
            .child(format!("Local View edits: {}", self.edits))
            .child(
                div()
                    .id("edit-retained-view")
                    .px_3()
                    .py_2()
                    .bg(rgb(0xe2edfc))
                    .on_click(cx.listener(|panel, _, _, cx| {
                        panel.edits += 1;
                        cx.notify();
                    }))
                    .child("Edit this retained View"),
            )
            .when(self.workspace == WorkspaceKind::Shared, |element| {
                element
                    .child(
                        div()
                            .id("select-editor")
                            .px_3()
                            .py_2()
                            .bg(rgb(0xede9fe))
                            .on_click(cx.listener(|panel, _, _, cx| {
                                let workspace = cx.global::<DemoApp>().workspace(panel.workspace);
                                if let Err(error) = workspace.select_editor(cx) {
                                    log::warn!("editor selection failed: {error:#}");
                                }
                            }))
                            .child("Select Editor"),
                    )
                    .child(
                        div()
                            .id("move-preview")
                            .px_3()
                            .py_2()
                            .bg(rgb(0xfef3c7))
                            .on_click(cx.listener(|panel, _, _, cx| {
                                let workspace = cx.global::<DemoApp>().workspace(panel.workspace);
                                match workspace.move_preview(cx) {
                                    Ok(()) => workspace.request_snapshot(cx),
                                    Err(error) => log::warn!("preview move failed: {error:#}"),
                                }
                            }))
                            .child("Move Preview between Editor and secondary root"),
                    )
            })
            .child(
                div()
                    .id("export-snapshot")
                    .px_3()
                    .py_2()
                    .bg(rgb(0xdcfce7))
                    .on_click(cx.listener(|panel, _, _, cx| {
                        let workspace = cx.global::<DemoApp>().workspace(panel.workspace);
                        workspace.request_snapshot(cx);
                    }))
                    .child("Export workspace snapshot"),
            )
            .child(format!("Last exported document: {snapshot_bytes} bytes"))
    }
}

fn panel(
    workspace: WorkspaceKind,
    title: &'static str,
    accent: u32,
    lines: &'static [&'static str],
    cx: &mut App,
) -> Entity<ExamplePanel> {
    cx.new(|_| ExamplePanel {
        workspace,
        title,
        accent,
        lines,
        edits: 0,
    })
}

fn retain_workspace(
    kind: WorkspaceKind,
    session: DockspaceSession,
    panels: Vec<(ItemId, Entity<ExamplePanel>)>,
    cx: &mut App,
) -> Result<Rc<Workspace>> {
    let panes = panels
        .iter()
        .map(|(item, view)| DockPane::new(*item, view.read(cx).title, view.clone()))
        .collect::<Vec<_>>();
    let dock = DockSurface::new_managed(session, panes, GpuiDockspaceConfig::default(), cx)?;
    dock.set_close_handler(
        move |dock, plan, cx| {
            if let Err(error) = dock.update(
                |frame| {
                    let pending = frame
                        .pending_close_decisions(plan.request())
                        .collect::<Vec<_>>();
                    for item in pending {
                        frame.resolve_close(plan.request(), item.token(), CloseDecision::Allow)?;
                    }
                    Ok(())
                },
                cx,
            ) {
                log::warn!("{} demo pane close failed: {error:#}", kind.name());
            }
        },
        cx,
    )?;
    Ok(Rc::new(Workspace {
        kind,
        dock,
        panels: panels.into_iter().map(|(_, view)| view).collect(),
        roots: RefCell::default(),
        snapshot_export_pending: Cell::new(false),
        last_snapshot: RefCell::default(),
    }))
}

fn build_shared_workspace(cx: &mut App, secondary: bool) -> Result<Rc<Workspace>> {
    let kind = WorkspaceKind::Shared;
    let mut bootstrap = DockspaceDocumentBootstrap::new(kind.document_id());
    let project = bootstrap.ensure_item("project")?;
    let editor = bootstrap.ensure_item("editor")?;
    let inspector = bootstrap.ensure_item("inspector")?;
    let preview = bootstrap.ensure_item("preview")?;
    let editor_tabs = if secondary {
        vec![editor]
    } else {
        vec![editor, preview]
    };
    let content = DockspaceNode::split(
        DockspaceAxis::Horizontal,
        [
            (DockspaceNode::tabs([project]), 0.25),
            (DockspaceNode::central_tabs(editor_tabs), 0.49),
            (DockspaceNode::tabs([inspector]), 0.26),
        ],
    )?;
    let mut surfaces = vec![DockspaceSurfaceLayout::new(
        MAIN,
        DockspaceRootLayout::new(MAIN_ROOT, content),
    )];
    if secondary {
        surfaces.push(DockspaceSurfaceLayout::new(
            SECONDARY,
            DockspaceRootLayout::new(SECONDARY_ROOT, DockspaceNode::central_tabs([preview])),
        ));
    }
    let session = DockspaceSession::from_persistent_layout(
        DockspaceLayout::new(surfaces)?,
        kind.policy(),
        bootstrap,
    )?;
    retain_workspace(
        kind,
        session,
        vec![
            (
                project,
                panel(kind, "Project", 0x2563eb, &["src", "examples", "docs"], cx),
            ),
            (
                editor,
                panel(
                    kind,
                    "Editor",
                    0x7c3aed,
                    &[
                        "One DockspaceSession owns both shared roots",
                        "Application Entities retain local edits across moves",
                    ],
                    cx,
                ),
            ),
            (
                inspector,
                panel(
                    kind,
                    "Inspector",
                    0x0f766e,
                    &[
                        "Native policy and backend capabilities are separate",
                        "Unsupported backends keep Preview beside Editor",
                    ],
                    cx,
                ),
            ),
            (
                preview,
                panel(
                    kind,
                    "Preview",
                    0xb45309,
                    &[
                        "This View can move between shared roots",
                        "Edit it before moving to observe retained state",
                    ],
                    cx,
                ),
            ),
        ],
        cx,
    )
}

fn build_isolated_workspace(cx: &mut App) -> Result<Rc<Workspace>> {
    let kind = WorkspaceKind::Isolated;
    let mut bootstrap = DockspaceDocumentBootstrap::new(kind.document_id());
    let activity = bootstrap.ensure_item("activity")?;
    let notes = bootstrap.ensure_item("notes")?;
    let content = DockspaceNode::split(
        DockspaceAxis::Horizontal,
        [
            (DockspaceNode::central_tabs([activity]), 0.68),
            (DockspaceNode::tabs([notes]), 0.32),
        ],
    )?;
    let session = DockspaceSession::from_persistent_layout(
        DockspaceLayout::new([DockspaceSurfaceLayout::new(
            MAIN,
            DockspaceRootLayout::new(MAIN_ROOT, content),
        )])?,
        kind.policy(),
        bootstrap,
    )?;
    retain_workspace(
        kind,
        session,
        vec![
            (
                activity,
                panel(
                    kind,
                    "Activity",
                    0xdc2626,
                    &[
                        "Independent DockspaceSession",
                        "Independent native window group",
                    ],
                    cx,
                ),
            ),
            (
                notes,
                panel(
                    kind,
                    "Notes",
                    0x0891b2,
                    &[
                        "This workspace also uses logical surface 1",
                        "Closing the shared anchor leaves this workspace running",
                    ],
                    cx,
                ),
            ),
        ],
        cx,
    )
}

impl Workspace {
    fn open_root(
        &self,
        surface: SurfaceId,
        anchor: bool,
        options: WindowOptions,
        cx: &mut App,
    ) -> Result<AnyWindowHandle> {
        let window = self.dock.open_native_root(surface, anchor, options, cx)?;
        self.roots.borrow_mut().insert(surface, window);
        Ok(window)
    }

    fn item(&self, key: &str) -> Result<ItemId> {
        self.dock
            .item_id_for_external_key(key)
            .with_context(|| format!("workspace has no registered pane key {key:?}"))
    }

    fn select_editor(&self, cx: &mut App) -> Result<()> {
        ensure!(
            !self.dock.is_update_pending(),
            "a physical frame still owns the update"
        );
        let editor = self.item("editor")?;
        self.dock.update(
            |frame| {
                frame.select_item_current(editor)?;
                Ok(())
            },
            cx,
        )?;
        ensure!(
            self.dock
                .with_view(|view| view.item(editor).is_some_and(|item| item.is_selected())),
            "the committed model did not select Editor"
        );
        log::info!("Editor selected; native activation and focus require host observations");
        Ok(())
    }

    fn move_preview(&self, cx: &mut App) -> Result<()> {
        ensure!(
            !self.dock.is_update_pending(),
            "a physical frame still owns the update"
        );
        let editor = self.item("editor")?;
        let preview = self.item("preview")?;
        let (editor_surface, preview_surface) = self.dock.with_view(|view| {
            (
                view.item(editor).map(|item| item.surface()),
                view.item(preview).map(|item| item.surface()),
            )
        });
        let editor_surface = editor_surface.context("Editor is closed")?;
        let (placement, destination) = if preview_surface == Some(editor_surface) {
            ensure!(
                editor_surface != SECONDARY,
                "Editor already occupies the secondary surface"
            );
            let secondary = self
                .roots
                .borrow()
                .get(&SECONDARY)
                .copied()
                .context("the backend has no open secondary root")?;
            ensure!(
                cx.windows().contains(&secondary),
                "the secondary window is closed"
            );
            let root = self
                .dock
                .with_view(|view| {
                    view.surface(SECONDARY)
                        .and_then(|surface| surface.main_root())
                        .map(|root| root.id())
                })
                .context("the secondary logical root is unavailable")?;
            (DockPlacement::Center(DockAnchor::Central(root)), SECONDARY)
        } else {
            (
                DockPlacement::Center(DockAnchor::Item(editor)),
                editor_surface,
            )
        };
        self.dock.update(
            |frame| {
                if preview_surface.is_some() {
                    frame.dock_item_current(preview, placement)?;
                } else {
                    frame.open_item_current(preview, placement)?;
                }
                frame.select_item_current(preview)?;
                Ok(())
            },
            cx,
        )?;
        ensure!(
            self.dock
                .with_view(|view| view.item(preview).map(|item| item.surface()))
                == Some(destination),
            "the committed model rejected the Preview move"
        );
        log::info!(
            "Preview moved to logical surface {} with its original Entity",
            destination.get()
        );
        Ok(())
    }

    fn request_snapshot(self: &Rc<Self>, cx: &mut App) {
        if self.snapshot_export_pending.replace(true) {
            return;
        }
        let workspace = self.clone();
        cx.spawn(async move |cx| {
            cx.background_executor().timer(SNAPSHOT_DEBOUNCE).await;
            cx.update(|cx| {
                workspace.snapshot_export_pending.set(false);
                if let Err(error) = workspace.export_snapshot(cx) {
                    log::warn!(
                        "{} snapshot export failed: {error:#}",
                        workspace.kind.name()
                    );
                }
                workspace.log_native_observations(cx);
                cx.refresh_windows();
            });
        })
        .detach();
    }

    fn export_snapshot(&self, cx: &App) -> Result<()> {
        let host = DockHostDocument {
            surfaces: self.dock.with_view(|view| {
                view.surfaces()
                    .map(|surface| DockSurfaceKeyBinding {
                        space_key: match surface.id() {
                            MAIN => "main".to_owned(),
                            SECONDARY => "preview-window".to_owned(),
                            surface => format!("surface-{}", surface.get()),
                        },
                        surface: surface.id(),
                        logical_placement: None,
                        central_policy: None,
                    })
                    .collect()
            }),
            migration: None,
        };
        let encoded = self.dock.capture_document(host)?.to_json()?;
        let kind = self.kind;
        let restored =
            DockWorkspaceDocument::restore_json(&encoded, kind.policy(), |document, key| {
                document == kind.document_id() && kind.pane_keys().contains(&key)
            })?;
        log::info!(
            "{} snapshot exported: bytes={} surfaces={} saved_surface_preferences={} retained_view_edits={}; document decode validated without opening native windows",
            kind.name(),
            encoded.len(),
            restored.session.view().surfaces().count(),
            restored.session.surface_presentation_preferences().count(),
            self.panels
                .iter()
                .map(|panel| panel.read(cx).edits)
                .sum::<usize>(),
        );
        *self.last_snapshot.borrow_mut() = Some(encoded);
        Ok(())
    }

    fn log_native_observations(&self, cx: &mut App) {
        for report in self.dock.take_native_reports() {
            match report {
                NativeReport::Submission { binding, snapshot } => log::info!(
                    "{} submission: binding={binding:?} outcome={:?} accepted={:?} submitted={:?}",
                    self.kind.name(),
                    snapshot.outcome(),
                    snapshot.accepted_frame_generation(),
                    snapshot.submitted_frame_generation(),
                ),
                NativeReport::OpenFailed { stage, message, .. } => {
                    log::warn!(
                        "{} native open failed at {stage:?}: {message}",
                        self.kind.name()
                    );
                }
                NativeReport::Unsupported { operation } => {
                    log::warn!(
                        "{} native operation unsupported: {operation}",
                        self.kind.name()
                    );
                }
                report => log::info!("{} native observation: {report:?}", self.kind.name()),
            }
        }
        for window in cx.windows() {
            let Some(binding) = self.dock.native_binding_for_window(window.window_id()) else {
                continue;
            };
            if let Err(error) = window.update(cx, |_, window, _| {
                log::info!(
                    "{} native window: binding={binding:?} capabilities={:?} presentation={:?}",
                    self.kind.name(),
                    window.window_capabilities(),
                    window.presentation_facts(),
                );
            }) {
                log::warn!("native window observation unavailable: {error}");
            }
        }
    }
}

fn main_window_options(cx: &App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(1040.0), px(680.0)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some("Open GPUI Docking Multiviewport".into()),
            appears_transparent: false,
            traffic_light_position: None,
        }),
        ..Default::default()
    }
}

fn secondary_window_options(cx: &App) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(560.0), px(540.0)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        focus_on_appearing: false,
        titlebar: Some(TitlebarOptions {
            title: Some("Open GPUI Docking Secondary".into()),
            appears_transparent: false,
            traffic_light_position: None,
        }),
        ..Default::default()
    }
}

fn isolated_window_options(cx: &App) -> WindowOptions {
    let mut bounds = Bounds::centered(None, size(px(620.0), px(440.0)), cx);
    bounds.origin.x += px(260.0);
    bounds.origin.y += px(80.0);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        focus_on_appearing: false,
        titlebar: Some(TitlebarOptions {
            title: Some("Open GPUI Docking Isolated Workspace".into()),
            appears_transparent: false,
            traffic_light_position: None,
        }),
        ..Default::default()
    }
}

fn open(cx: &mut App) -> Result<()> {
    let secondary = cx.viewport_capabilities().platform_viewport_windows;
    let shared = build_shared_workspace(cx, secondary)?;
    let isolated = build_isolated_workspace(cx)?;
    cx.set_global(DemoApp {
        shared: shared.clone(),
        isolated: isolated.clone(),
    });
    let primary = shared.open_root(MAIN, true, main_window_options(cx), cx)?;
    let isolated_primary = isolated.open_root(MAIN, true, isolated_window_options(cx), cx)?;
    ensure!(
        primary != isolated_primary,
        "independent workspaces need distinct windows"
    );
    if secondary {
        if let Err(error) = shared.open_root(SECONDARY, false, secondary_window_options(cx), cx) {
            log::warn!("secondary native root did not open: {error:#}");
            if let Err(error) = shared.move_preview(cx) {
                log::warn!(
                    "Preview could not return to Editor yet: {error:#}; use Move Preview to retry"
                );
            }
        }
    } else {
        log::info!("platform viewport windows are unavailable; Preview remains in Editor tabs");
    }
    shared.request_snapshot(cx);
    isolated.request_snapshot(cx);

    cx.spawn(async move |cx| {
        cx.background_executor()
            .timer(Duration::from_millis(100))
            .await;
        cx.update(|cx| {
            if let Err(error) = shared.select_editor(cx) {
                log::warn!(
                    "startup Editor selection failed: {error:#}; use Select Editor to retry"
                );
            }
        });
    })
    .detach();
    cx.activate(true);
    Ok(())
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    application().run(|cx| {
        if let Err(error) = open(cx) {
            eprintln!("docking-multiviewport startup failed: {error:#}");
            std::process::exit(1);
        }
    });
}
