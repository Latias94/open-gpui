use crate::{
    DockPanelPlacement, DockSurface, DockSurfaceChangeCategory, DockSurfaceChangeEvent,
    DockSurfacePanelError, DockSurfaceSnapshot,
};
use open_gpui::{
    App, AppContext as _, Bounds, IntoElement, Render, Subscription, Window, div, point, px, size,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};

struct TestPanel;

impl Render for TestPanel {
    fn render(
        &mut self,
        _window: &mut Window,
        _cx: &mut open_gpui::Context<Self>,
    ) -> impl IntoElement {
        div()
    }
}

fn test_panel(cx: &mut App) -> open_gpui::AnyView {
    cx.new(|_| TestPanel).into()
}

fn test_surface(cx: &mut App) -> DockSurface {
    DockSurface::builder("main")
        .panel_placements([
            DockPanelPlacement::center("editor"),
            DockPanelPlacement::right_rail("inspector").selected(),
            DockPanelPlacement::stacked_with("terminal", "inspector"),
        ])
        .panel_factory("editor", "Editor", test_panel)
        .panel_factory("inspector", "Inspector", test_panel)
        .panel_factory("terminal", "Terminal", test_panel)
        .allow_floating(true)
        .build(cx)
        .expect("surface layout should validate")
}

fn collect_changes(
    surface: &DockSurface,
    changes: Rc<RefCell<Vec<DockSurfaceChangeEvent>>>,
    cx: &mut App,
) -> Subscription {
    surface.subscribe_changes(cx, move |event, _| {
        changes.borrow_mut().push(event.clone());
    })
}

#[open_gpui::test]
fn surface_clones_share_monotonic_revision_and_root_command_boundaries(
    cx: &mut open_gpui::TestAppContext,
) {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let observed_changes = changes.clone();

    let (surface, surface_clone, subscription, first_revision, second_revision) = cx.update(|cx| {
        let surface = test_surface(cx);
        let surface_clone = surface.clone();
        let subscription = collect_changes(&surface, observed_changes, cx);

        assert_eq!(surface.revision(cx), 0);
        assert_eq!(surface_clone.revision(cx), 0);

        let first = surface
            .select_panel("terminal", cx)
            .expect("terminal selection should succeed");
        assert!(first.changed());
        let first_revision = surface_clone.revision(cx);

        let second = surface_clone
            .select_panel("inspector", cx)
            .expect("inspector selection should succeed");
        assert!(second.changed());
        let second_revision = surface.revision(cx);

        let unchanged = surface
            .select_panel("inspector", cx)
            .expect("selecting the selected panel should be valid");
        assert!(!unchanged.changed());
        assert!(matches!(
            surface.select_panel("missing", cx),
            Err(DockSurfacePanelError::PanelUnavailable { .. })
        ));
        assert_eq!(surface.revision(cx), second_revision);

        (
            surface,
            surface_clone,
            subscription,
            first_revision,
            second_revision,
        )
    });

    cx.run_until_parked();

    assert_eq!((first_revision, second_revision), (1, 2));
    assert_eq!(
        cx.read(|cx| (surface.revision(cx), surface_clone.revision(cx))),
        (2, 2)
    );
    let changes = changes.borrow();
    assert_eq!(changes.len(), 2);
    assert_eq!(
        changes
            .iter()
            .map(DockSurfaceChangeEvent::revision)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert!(
        changes
            .iter()
            .all(|event| event.categories() == [DockSurfaceChangeCategory::Selection].as_slice())
    );

    drop(subscription);
}

#[open_gpui::test]
fn change_event_callback_can_start_a_new_root_transaction(cx: &mut open_gpui::TestAppContext) {
    let revisions = Rc::new(RefCell::new(Vec::new()));
    let observed_revisions = revisions.clone();

    let (surface, subscription) = cx.update(|cx| {
        let surface = test_surface(cx);
        let surface_for_callback = surface.clone();
        let subscription = surface.subscribe_changes(cx, move |event, cx| {
            observed_revisions.borrow_mut().push(event.revision());
            if event.revision() == 1 {
                surface_for_callback
                    .select_panel("inspector", cx)
                    .expect("event callback should be able to start a new root transaction");
            }
        });
        surface
            .select_panel("terminal", cx)
            .expect("initial selection should succeed");
        (surface, subscription)
    });

    cx.run_until_parked();

    assert_eq!(revisions.borrow().as_slice(), &[1, 2]);
    assert_eq!(cx.read(|cx| surface.revision(cx)), 2);
    drop(subscription);
}

#[open_gpui::test]
fn panel_root_commands_emit_one_deduplicated_category_event(cx: &mut open_gpui::TestAppContext) {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let observed_changes = changes.clone();

    let (_surface, subscription, revision) = cx.update(|cx| {
        let surface = test_surface(cx);
        let subscription = collect_changes(&surface, observed_changes, cx);

        assert!(
            surface
                .select_panel("terminal", cx)
                .expect("terminal selection should succeed")
                .changed()
        );
        assert!(
            surface
                .close_panel("terminal", cx)
                .expect("terminal close should succeed")
                .changed()
        );
        assert!(
            surface
                .open_panel("terminal", cx)
                .expect("terminal reopen should succeed")
                .changed()
        );
        assert!(
            surface
                .float_panel_in_window(
                    "terminal",
                    Bounds::new(point(px(24.0), px(32.0)), size(px(280.0), px(160.0))),
                    cx,
                )
                .expect("terminal float should succeed")
                .changed()
        );
        assert!(
            surface
                .dock_panel_at(
                    DockPanelPlacement::stacked_with("terminal", "inspector"),
                    cx
                )
                .expect("terminal dock-back should succeed")
                .changed()
        );

        let revision = surface.revision(cx);
        (surface, subscription, revision)
    });

    cx.run_until_parked();

    assert_eq!(revision, 5);
    let expected_categories = [
        vec![DockSurfaceChangeCategory::Selection],
        vec![
            DockSurfaceChangeCategory::Layout,
            DockSurfaceChangeCategory::Selection,
            DockSurfaceChangeCategory::PanelLifecycle,
        ],
        vec![
            DockSurfaceChangeCategory::Layout,
            DockSurfaceChangeCategory::Selection,
            DockSurfaceChangeCategory::PanelLifecycle,
        ],
        vec![
            DockSurfaceChangeCategory::Layout,
            DockSurfaceChangeCategory::Selection,
        ],
        vec![
            DockSurfaceChangeCategory::Layout,
            DockSurfaceChangeCategory::Selection,
        ],
    ];
    let changes = changes.borrow();
    assert_eq!(changes.len(), expected_categories.len());
    for (index, (event, expected)) in changes.iter().zip(expected_categories).enumerate() {
        assert_eq!(event.revision(), index as u64 + 1);
        assert_eq!(event.categories(), expected.as_slice());
    }

    drop(subscription);
}

#[open_gpui::test]
fn snapshot_pairs_current_revision_layout_and_placement_and_accepts_legacy_json(
    cx: &mut open_gpui::TestAppContext,
) {
    let (snapshot, revision, layout, viewport_placement, legacy_snapshot) = cx.update(|cx| {
        let surface = test_surface(cx);
        assert!(
            surface
                .select_panel("terminal", cx)
                .expect("terminal selection should succeed")
                .changed()
        );

        let snapshot = surface.export_snapshot(cx);
        let mut legacy_json =
            serde_json::to_value(&snapshot).expect("snapshot should serialize to JSON");
        legacy_json
            .as_object_mut()
            .expect("snapshot JSON should be an object")
            .remove("revision");
        let legacy_snapshot: DockSurfaceSnapshot =
            serde_json::from_value(legacy_json).expect("legacy snapshot JSON should deserialize");

        (
            snapshot,
            surface.revision(cx),
            surface.export_layout(cx),
            surface.export_viewport_placement(cx),
            legacy_snapshot,
        )
    });

    assert_eq!(snapshot.revision(), revision);
    assert_eq!(snapshot.layout(), &layout);
    assert_eq!(snapshot.viewport_placement(), &viewport_placement);
    assert_eq!(legacy_snapshot.revision(), 0);
    assert_eq!(legacy_snapshot.layout(), snapshot.layout());
    assert_eq!(
        legacy_snapshot.viewport_placement(),
        snapshot.viewport_placement()
    );
}

#[open_gpui::test]
fn change_events_are_metadata_only_and_subscription_drop_stops_observation(
    cx: &mut open_gpui::TestAppContext,
) {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let observed_changes = changes.clone();

    let (surface, subscription) = cx.update(|cx| {
        let surface = test_surface(cx);
        let subscription = collect_changes(&surface, observed_changes, cx);
        assert!(
            surface
                .close_panel("terminal", cx)
                .expect("terminal close should succeed")
                .changed()
        );
        (surface, subscription)
    });
    cx.run_until_parked();

    assert_eq!(changes.borrow().len(), 1);
    let event_debug = format!("{:?}", changes.borrow()[0]);
    assert!(!event_debug.contains("terminal"));
    assert!(!event_debug.contains("Terminal"));

    drop(subscription);
    let revision = cx.update(|cx| {
        assert!(
            surface
                .open_panel("terminal", cx)
                .expect("terminal reopen should succeed")
                .changed()
        );
        surface.revision(cx)
    });
    cx.run_until_parked();

    assert_eq!(revision, 2);
    assert_eq!(changes.borrow().len(), 1);
}

#[open_gpui::test]
fn caller_controls_debounce_and_snapshot_export_with_fake_clock(
    cx: &mut open_gpui::TestAppContext,
) {
    const DEBOUNCE: Duration = Duration::from_millis(50);

    let export_pending = Rc::new(Cell::new(false));
    let export_count = Rc::new(Cell::new(0));
    let exported_snapshots = Rc::new(RefCell::new(Vec::new()));

    let pending_for_events = export_pending.clone();
    let count_for_events = export_count.clone();
    let snapshots_for_events = exported_snapshots.clone();
    let (surface, subscription) = cx.update(|cx| {
        let surface = test_surface(cx);
        let surface_for_events = surface.clone();
        let subscription = surface.subscribe_changes(cx, move |_event, cx| {
            if pending_for_events.replace(true) {
                return;
            }

            let surface = surface_for_events.clone();
            let export_pending = pending_for_events.clone();
            let export_count = count_for_events.clone();
            let exported_snapshots = snapshots_for_events.clone();
            cx.spawn(async move |cx| {
                cx.background_executor().timer(DEBOUNCE).await;
                cx.update(|cx| {
                    exported_snapshots
                        .borrow_mut()
                        .push(surface.export_snapshot(cx));
                    export_count.set(export_count.get() + 1);
                    export_pending.set(false);
                });
            })
            .detach();
        });

        assert!(
            surface
                .select_panel("terminal", cx)
                .expect("terminal selection should succeed")
                .changed()
        );
        assert!(
            surface
                .select_panel("inspector", cx)
                .expect("inspector selection should succeed")
                .changed()
        );

        (surface, subscription)
    });

    cx.run_until_parked();
    assert_eq!(export_count.get(), 0);
    assert!(exported_snapshots.borrow().is_empty());

    cx.executor()
        .advance_clock(DEBOUNCE - Duration::from_millis(1));
    cx.run_until_parked();
    assert_eq!(export_count.get(), 0);

    cx.executor().advance_clock(Duration::from_millis(1));
    cx.run_until_parked();
    assert_eq!(export_count.get(), 1);
    assert_eq!(exported_snapshots.borrow().len(), 1);
    assert_eq!(
        exported_snapshots.borrow()[0].revision(),
        cx.read(|cx| surface.revision(cx))
    );

    drop(subscription);
}
