use super::{
    live_undock::{
        DockLiveUndockDragGeneration, DockLiveUndockEffect, DockLiveUndockFact,
        DockLiveUndockIdentity, DockLiveUndockPayloadLeaseReceipt, DockLiveUndockPhysicalPoint,
        DockLiveUndockPresentationLeaseGeneration, DockLiveUndockPromotionDestination,
        DockLiveUndockPromotionToken, DockLiveUndockRouteFeedback, DockLiveUndockSourceSnapshot,
        DockLiveUndockTrigger,
    },
    owner::{DockSurfaceChangeCategory, DockSurfaceOwner, DockSurfaceTransition},
    payload_recovery::{
        DockPayloadRecoveryAuthority, DockPayloadRecoveryCommitError,
        DockPayloadRecoveryDisposition, DockPayloadRecoveryPrepareError, DockPayloadRecoveryReason,
        DockPayloadRecoveryRegistry, DockPayloadRecoveryRestoreError,
    },
    window_session::DockSurfaceWindowSession,
};
use crate::{
    DockController, DockFloatingContainer, DockGraph, DockGraphDropTarget, DockItemId, DockNode,
    DockNodeId, DockOp, DockSpaceId, DockViewportRuntimeHandle, DockWorkspace, SplitAxis,
    locked_drop_identity::DockLockedPayloadIdentity,
    workspace_drop_transaction::DockWorkspaceDropPayload,
};
use open_gpui::{AppContext as _, Bounds, EntityId, WindowId, point, px, size};

const OWNER_REVISION: u64 = 7;

struct PayloadFixture {
    graph: DockGraph,
    payload: DockLockedPayloadIdentity,
    source_space: DockSpaceId,
    primary_space: DockSpaceId,
    primary_tabs: DockNodeId,
}

#[derive(Clone, Copy)]
struct LivePayloadProof {
    identity: DockLiveUndockIdentity,
    lease: DockLiveUndockPayloadLeaseReceipt,
}

fn space(id: &str) -> DockSpaceId {
    DockSpaceId::from(id)
}

fn item(id: &str) -> DockItemId {
    DockItemId::from(id)
}

fn tabs(graph: &mut DockGraph, items: &[&str]) -> DockNodeId {
    let items: Vec<_> = items.iter().copied().map(item).collect();
    graph.insert_node(DockNode::Tabs {
        selected: items.first().cloned(),
        items,
    })
}

fn bounds(origin: f32) -> Bounds<open_gpui::Pixels> {
    Bounds::new(point(px(origin), px(origin)), size(px(240.0), px(160.0)))
}

fn item_fixture() -> PayloadFixture {
    let source_space = space("detached");
    let primary_space = space("main");
    let mut graph = DockGraph::new();
    let source_tabs = tabs(&mut graph, &["payload", "source-peer"]);
    let primary_tabs = tabs(&mut graph, &["home"]);
    graph.set_root(source_space.clone(), source_tabs);
    graph.set_root(primary_space.clone(), primary_tabs);
    let payload_item = item("payload");
    let payload = DockLockedPayloadIdentity::capture(
        &graph,
        &source_space,
        DockWorkspaceDropPayload::Item {
            source_tabs,
            item: &payload_item,
        },
    )
    .expect("item payload should lock");
    PayloadFixture {
        graph,
        payload,
        source_space,
        primary_space,
        primary_tabs,
    }
}

fn tabs_fixture() -> PayloadFixture {
    let source_space = space("detached");
    let primary_space = space("main");
    let mut graph = DockGraph::new();
    let source_tabs = tabs(&mut graph, &["payload-a", "payload-b"]);
    let primary_tabs = tabs(&mut graph, &["home"]);
    graph.set_root(source_space.clone(), source_tabs);
    graph.set_root(primary_space.clone(), primary_tabs);
    let payload = DockLockedPayloadIdentity::capture(
        &graph,
        &source_space,
        DockWorkspaceDropPayload::Tabs { source_tabs },
    )
    .expect("tabs payload should lock");
    PayloadFixture {
        graph,
        payload,
        source_space,
        primary_space,
        primary_tabs,
    }
}

fn floating_fixture(split: bool) -> PayloadFixture {
    let source_space = space("detached");
    let primary_space = space("main");
    let mut graph = DockGraph::new();
    let source_root = tabs(&mut graph, &["source-root"]);
    let primary_tabs = tabs(&mut graph, &["home"]);
    let child_root = if split {
        let left = tabs(&mut graph, &["payload-a"]);
        let right = tabs(&mut graph, &["payload-b"]);
        graph.insert_node(DockNode::Split {
            axis: SplitAxis::Horizontal,
            children: vec![left, right],
            fractions: vec![0.5, 0.5],
        })
    } else {
        tabs(&mut graph, &["payload-a", "payload-b"])
    };
    let floating = graph.insert_node(DockNode::Floating { child: child_root });
    graph.set_root(source_space.clone(), source_root);
    graph.set_root(primary_space.clone(), primary_tabs);
    graph
        .floating_containers_mut(source_space.clone())
        .push(DockFloatingContainer {
            node: floating,
            bounds: bounds(0.0),
        });
    let payload = DockLockedPayloadIdentity::capture(
        &graph,
        &source_space,
        DockWorkspaceDropPayload::Floating { floating },
    )
    .expect("floating payload should lock");
    PayloadFixture {
        graph,
        payload,
        source_space,
        primary_space,
        primary_tabs,
    }
}

fn live_payload_proof(authority: u64) -> LivePayloadProof {
    let mut window_session = DockSurfaceWindowSession::new(EntityId::from(authority));
    let opening = window_session
        .reserve_opening()
        .expect("test primary should reserve");
    let window_lease = window_session
        .commit_opening(opening, WindowId::from(authority + 100))
        .expect("test primary should activate");
    let mut live_undock = super::live_undock::DockLiveUndockSession::new();
    let source = DockLiveUndockSourceSnapshot::new(WindowId::from(authority + 200), OWNER_REVISION);
    let trigger = DockLiveUndockTrigger::new(
        DockLiveUndockDragGeneration::new(authority + 1)
            .expect("the test drag generation should be non-zero"),
        source,
        DockLiveUndockRouteFeedback::Desktop,
        DockLiveUndockPhysicalPoint::new(50, 50),
    )
    .expect("desktop should be an eligible live-undock route");
    let identity = live_undock
        .apply(DockLiveUndockFact::Trigger {
            lease: window_lease,
            trigger,
        })
        .into_iter()
        .find_map(|effect| match effect {
            DockLiveUndockEffect::OpenProvisional { identity, .. } => Some(identity),
            _ => None,
        })
        .expect("the accepted trigger should reserve one provisional opening");
    let presentation_generation = DockLiveUndockPresentationLeaseGeneration::new(authority + 1)
        .expect("test presentation generation should be non-zero");
    let lease = DockLiveUndockPayloadLeaseReceipt::for_test(
        identity,
        source,
        presentation_generation,
        WindowId::from(authority + 300),
    );
    LivePayloadProof { identity, lease }
}

fn presentation_authority(live: LivePayloadProof) -> DockPayloadRecoveryAuthority {
    DockPayloadRecoveryAuthority::presentation_lease(live.lease)
}

fn committed_destination_authority(live: LivePayloadProof) -> DockPayloadRecoveryAuthority {
    DockPayloadRecoveryAuthority::committed_destination(
        live.identity,
        DockLiveUndockPromotionToken::new(1).expect("test promotion token should be non-zero"),
        DockLiveUndockPromotionDestination::SameWindowDesktop {
            window_id: live.lease.destination_window(),
        },
    )
}

fn move_payload_to_lost_space(fixture: &mut PayloadFixture, lost_space: &DockSpaceId) {
    match &fixture.payload {
        DockLockedPayloadIdentity::Item { item, .. } => fixture
            .graph
            .apply_op_checked(&DockOp::MoveItem {
                source_space: fixture.source_space.clone(),
                item: item.clone(),
                target_space: lost_space.clone(),
                target: DockGraphDropTarget::empty_space(),
            })
            .expect("item promotion should commit"),
        DockLockedPayloadIdentity::Tabs { source_tabs, .. } => fixture
            .graph
            .apply_op_checked(&DockOp::MoveTabs {
                source_space: fixture.source_space.clone(),
                source_tabs: *source_tabs,
                target_space: lost_space.clone(),
                target: DockGraphDropTarget::empty_space(),
            })
            .expect("tabs promotion should commit"),
        DockLockedPayloadIdentity::Floating { floating, .. } => fixture
            .graph
            .apply_op_checked(&DockOp::MoveFloating {
                source_space: fixture.source_space.clone(),
                floating: *floating,
                target_space: lost_space.clone(),
                target: DockGraphDropTarget::empty_space(),
            })
            .expect("floating promotion should commit"),
    };
}

fn commit_lost_viewport_recovery(
    registry: &mut DockPayloadRecoveryRegistry,
    fixture: &PayloadFixture,
    live: LivePayloadProof,
) -> super::payload_recovery::DockPayloadRecoveryCommitReceipt {
    let prepared = registry
        .prepare(
            &fixture.graph,
            OWNER_REVISION,
            committed_destination_authority(live),
            &fixture.payload,
            DockPayloadRecoveryReason::LostViewportRecovery,
        )
        .expect("lost payload should prepare");
    registry
        .commit(
            &fixture.graph,
            OWNER_REVISION,
            &fixture.primary_space,
            true,
            &prepared,
        )
        .expect("lost payload should commit")
}

fn prepare(
    registry: &mut DockPayloadRecoveryRegistry,
    fixture: &PayloadFixture,
    live: LivePayloadProof,
) -> super::payload_recovery::DockPayloadRecoveryPrepared {
    registry
        .prepare(
            &fixture.graph,
            OWNER_REVISION,
            presentation_authority(live),
            &fixture.payload,
            DockPayloadRecoveryReason::PreCommitOrphan,
        )
        .expect("payload should prepare")
}

fn merge_back(fixture: &mut PayloadFixture) {
    match &fixture.payload {
        DockLockedPayloadIdentity::Item { item, .. } => fixture
            .graph
            .apply_op_checked(&DockOp::MoveItem {
                source_space: fixture.source_space.clone(),
                item: item.clone(),
                target_space: fixture.primary_space.clone(),
                target: DockGraphDropTarget::center(fixture.primary_tabs),
            })
            .expect("item merge-back should apply"),
        DockLockedPayloadIdentity::Tabs { source_tabs, .. } => fixture
            .graph
            .apply_op_checked(&DockOp::MoveTabs {
                source_space: fixture.source_space.clone(),
                source_tabs: *source_tabs,
                target_space: fixture.primary_space.clone(),
                target: DockGraphDropTarget::center(fixture.primary_tabs),
            })
            .expect("tabs merge-back should apply"),
        DockLockedPayloadIdentity::Floating { .. } => fixture
            .graph
            .merge_space_floating_forest_into(&fixture.source_space, &fixture.primary_space),
    };
}

#[test]
fn original_item_tabs_floating_and_split_floating_remain_recoverable() {
    let fixtures = [
        item_fixture(),
        tabs_fixture(),
        floating_fixture(false),
        floating_fixture(true),
    ];

    for (index, fixture) in fixtures.into_iter().enumerate() {
        let live = live_payload_proof(1_000 + index as u64 * 10);
        let mut registry = DockPayloadRecoveryRegistry::new();
        let prepared = prepare(&mut registry, &fixture, live);
        assert!(registry.can_commit(&fixture.graph, OWNER_REVISION, &prepared,));
        let receipt = registry
            .commit(
                &fixture.graph,
                OWNER_REVISION,
                &fixture.primary_space,
                true,
                &prepared,
            )
            .expect("original payload should commit");
        assert_eq!(
            receipt.disposition(),
            DockPayloadRecoveryDisposition::VisibleRecoveryEntry,
            "an original detached location still needs a visible recovery entry"
        );
        assert_eq!(registry.visible_records().count(), 1);
    }
}

#[test]
fn merge_back_uniquely_rehomes_every_payload_shape_into_live_primary_space() {
    let mut fixtures = [
        item_fixture(),
        tabs_fixture(),
        floating_fixture(false),
        floating_fixture(true),
    ];

    for (index, fixture) in fixtures.iter_mut().enumerate() {
        merge_back(fixture);
        let live = live_payload_proof(2_000 + index as u64 * 10);
        let mut registry = DockPayloadRecoveryRegistry::new();
        let prepared = prepare(&mut registry, fixture, live);
        let receipt = registry
            .commit(
                &fixture.graph,
                OWNER_REVISION,
                &fixture.primary_space,
                true,
                &prepared,
            )
            .expect("unique merge-back should commit");
        assert_eq!(
            receipt.disposition(),
            DockPayloadRecoveryDisposition::AlreadyRehomed
        );
        assert_eq!(registry.visible_records().count(), 0);
        assert!(
            registry
                .record(receipt)
                .is_some_and(|record| record.was_rehomed())
        );
    }
}

#[test]
fn floating_subtree_survives_wrapper_retirement_and_tabs_flattening() {
    let mut split_fixture = floating_fixture(true);
    let split_floating = match &split_fixture.payload {
        DockLockedPayloadIdentity::Floating { floating, .. } => *floating,
        _ => unreachable!("split floating fixture must contain a floating payload"),
    };
    let lost_space = space("lost-viewport");
    split_fixture
        .graph
        .apply_op_checked(&DockOp::MoveFloating {
            source_space: split_fixture.source_space.clone(),
            floating: split_floating,
            target_space: lost_space.clone(),
            target: DockGraphDropTarget::empty_space(),
        })
        .expect("postcommit tear-off should retire the floating wrapper");
    let split_live = live_payload_proof(2_100);
    let mut split_registry = DockPayloadRecoveryRegistry::new();
    let split_prepared = prepare(&mut split_registry, &split_fixture, split_live);
    assert!(split_registry.can_commit(&split_fixture.graph, OWNER_REVISION, &split_prepared,));

    let mut tabs_fixture = floating_fixture(false);
    let tabs_floating = match &tabs_fixture.payload {
        DockLockedPayloadIdentity::Floating { floating, .. } => *floating,
        _ => unreachable!("floating fixture must contain a floating payload"),
    };
    tabs_fixture
        .graph
        .apply_op_checked(&DockOp::MoveFloating {
            source_space: tabs_fixture.source_space.clone(),
            floating: tabs_floating,
            target_space: lost_space.clone(),
            target: DockGraphDropTarget::empty_space(),
        })
        .expect("postcommit tear-off should install the child tabs as a space root");
    let detached_tabs = tabs_fixture
        .graph
        .root(&lost_space)
        .expect("the detached child tabs should own the lost viewport space");
    tabs_fixture
        .graph
        .apply_op_checked(&DockOp::MoveTabs {
            source_space: lost_space,
            source_tabs: detached_tabs,
            target_space: tabs_fixture.primary_space.clone(),
            target: DockGraphDropTarget::center(tabs_fixture.primary_tabs),
        })
        .expect("viewport merge-back should flatten the payload into primary tabs");
    let tabs_live = live_payload_proof(2_200);
    let mut tabs_registry = DockPayloadRecoveryRegistry::new();
    let tabs_prepared = prepare(&mut tabs_registry, &tabs_fixture, tabs_live);
    let receipt = tabs_registry
        .commit(
            &tabs_fixture.graph,
            OWNER_REVISION,
            &tabs_fixture.primary_space,
            true,
            &tabs_prepared,
        )
        .expect("flattened floating items should remain uniquely recoverable");
    assert_eq!(
        receipt.disposition(),
        DockPayloadRecoveryDisposition::AlreadyRehomed
    );
}

#[test]
fn missing_item_tabs_and_floating_payloads_fail_closed() {
    let mut fixtures = [item_fixture(), tabs_fixture(), floating_fixture(false)];

    for (index, fixture) in fixtures.iter_mut().enumerate() {
        match &fixture.payload {
            DockLockedPayloadIdentity::Item { .. } | DockLockedPayloadIdentity::Tabs { .. } => {
                fixture.graph.remove_root(&fixture.source_space);
            }
            DockLockedPayloadIdentity::Floating { .. } => {
                fixture
                    .graph
                    .floating_containers_mut(fixture.source_space.clone())
                    .clear();
            }
        }
        let live = live_payload_proof(3_000 + index as u64 * 10);
        let mut registry = DockPayloadRecoveryRegistry::new();
        assert_eq!(
            registry.prepare(
                &fixture.graph,
                OWNER_REVISION,
                presentation_authority(live),
                &fixture.payload,
                DockPayloadRecoveryReason::PreCommitOrphan,
            ),
            Err(DockPayloadRecoveryPrepareError::PayloadMissing)
        );
    }
}

#[test]
fn unresolved_payload_failures_commit_visible_diagnostic_records() {
    let mut missing = item_fixture();
    missing.graph.remove_root(&missing.source_space);
    let missing_live = live_payload_proof(3_100);
    let mut missing_registry = DockPayloadRecoveryRegistry::new();
    let missing_prepared = missing_registry
        .prepare_unresolved(
            &missing.graph,
            OWNER_REVISION,
            presentation_authority(missing_live),
            &missing.payload,
            DockPayloadRecoveryReason::PreCommitOrphan,
            DockPayloadRecoveryPrepareError::PayloadMissing,
        )
        .expect("a missing payload should prepare one unresolved diagnostic");
    let missing_receipt = missing_registry
        .commit(
            &missing.graph,
            OWNER_REVISION,
            &missing.primary_space,
            true,
            &missing_prepared,
        )
        .expect("unchanged missing evidence should commit");
    assert_eq!(
        missing_receipt.disposition(),
        DockPayloadRecoveryDisposition::Unresolved
    );
    assert_eq!(missing_registry.visible_records().count(), 1);
}

#[test]
fn unresolved_graph_evidence_cannot_commit_after_payload_reappears() {
    let fixture = item_fixture();
    let mut missing_graph = fixture.graph.clone();
    missing_graph.remove_root(&fixture.source_space);
    let live = live_payload_proof(3_300);
    let mut registry = DockPayloadRecoveryRegistry::new();
    let prepared = registry
        .prepare_unresolved(
            &missing_graph,
            OWNER_REVISION,
            presentation_authority(live),
            &fixture.payload,
            DockPayloadRecoveryReason::PreCommitOrphan,
            DockPayloadRecoveryPrepareError::PayloadMissing,
        )
        .expect("missing evidence should prepare");

    assert!(!registry.can_commit(&fixture.graph, OWNER_REVISION, &prepared));
    assert_eq!(
        registry.commit(
            &fixture.graph,
            OWNER_REVISION,
            &fixture.primary_space,
            true,
            &prepared,
        ),
        Err(DockPayloadRecoveryCommitError::PayloadSurvivalChanged)
    );
}

#[test]
fn duplicate_item_tabs_and_floating_matches_are_ambiguous() {
    let fixtures = [item_fixture(), tabs_fixture(), floating_fixture(false)];

    for (index, fixture) in fixtures.into_iter().enumerate() {
        let mut graph = DockGraph::new();
        match &fixture.payload {
            DockLockedPayloadIdentity::Item { item: payload, .. } => {
                for candidate_space in [space("candidate-a"), space("candidate-b")] {
                    let candidate = graph.insert_node(DockNode::Tabs {
                        items: vec![payload.clone()],
                        selected: Some(payload.clone()),
                    });
                    graph.set_root(candidate_space, candidate);
                }
            }
            DockLockedPayloadIdentity::Tabs { ordered_items, .. } => {
                for (candidate_space, prefix) in [
                    (space("candidate-a"), item("prefix-a")),
                    (space("candidate-b"), item("prefix-b")),
                ] {
                    let mut items = vec![prefix];
                    items.extend(ordered_items.iter().cloned());
                    let candidate = graph.insert_node(DockNode::Tabs {
                        selected: items.first().cloned(),
                        items,
                    });
                    graph.set_root(candidate_space, candidate);
                }
            }
            DockLockedPayloadIdentity::Floating { ordered_items, .. } => {
                for (candidate_space, origin) in
                    [(space("candidate-a"), 0.0), (space("candidate-b"), 20.0)]
                {
                    let child = graph.insert_node(DockNode::Tabs {
                        items: ordered_items.clone(),
                        selected: ordered_items.first().cloned(),
                    });
                    let floating = graph.insert_node(DockNode::Floating { child });
                    graph
                        .floating_containers_mut(candidate_space)
                        .push(DockFloatingContainer {
                            node: floating,
                            bounds: bounds(origin),
                        });
                }
            }
        }
        let live = live_payload_proof(4_000 + index as u64 * 10);
        let mut registry = DockPayloadRecoveryRegistry::new();
        assert_eq!(
            registry.prepare(
                &graph,
                OWNER_REVISION,
                presentation_authority(live),
                &fixture.payload,
                DockPayloadRecoveryReason::PreCommitOrphan,
            ),
            Err(DockPayloadRecoveryPrepareError::PayloadAmbiguous)
        );
    }
}

#[test]
fn revision_change_and_graph_node_aba_invalidate_prepared_proof() {
    let mut fixture = item_fixture();
    let live = live_payload_proof(5_000);
    let mut registry = DockPayloadRecoveryRegistry::new();
    let revision_bound = prepare(&mut registry, &fixture, live);
    assert!(!registry.can_commit(&fixture.graph, OWNER_REVISION + 1, &revision_bound,));
    assert_eq!(
        registry.commit(
            &fixture.graph,
            OWNER_REVISION + 1,
            &fixture.primary_space,
            true,
            &revision_bound,
        ),
        Err(DockPayloadRecoveryCommitError::OwnerRevisionChanged)
    );

    let original_tabs = fixture.payload.source_node();
    let graph_bound = prepare(&mut registry, &fixture, live);
    let payload_item = match &fixture.payload {
        DockLockedPayloadIdentity::Item { item, .. } => item.clone(),
        _ => unreachable!("item fixture must contain an item payload"),
    };
    fixture
        .graph
        .apply_op_checked(&DockOp::MoveItem {
            source_space: fixture.source_space.clone(),
            item: payload_item.clone(),
            target_space: fixture.primary_space.clone(),
            target: DockGraphDropTarget::center(fixture.primary_tabs),
        })
        .expect("payload should move away");
    fixture
        .graph
        .apply_op_checked(&DockOp::CloseItem {
            space: fixture.source_space.clone(),
            item: item("source-peer"),
        })
        .expect("the old source tabs should be retired before logical re-creation");
    fixture
        .graph
        .apply_op_checked(&DockOp::MoveItem {
            source_space: fixture.primary_space.clone(),
            item: payload_item.clone(),
            target_space: fixture.source_space.clone(),
            target: DockGraphDropTarget::empty_space(),
        })
        .expect("payload should return through a new tabs generation");
    let replacement_tabs = fixture
        .graph
        .find_item_in_space(&fixture.source_space, &payload_item)
        .expect("payload should be back in its logical source space")
        .0;
    assert_ne!(replacement_tabs, original_tabs);
    assert!(!registry.can_commit(&fixture.graph, OWNER_REVISION, &graph_bound,));
    assert_eq!(
        registry.commit(
            &fixture.graph,
            OWNER_REVISION,
            &fixture.primary_space,
            true,
            &graph_bound,
        ),
        Err(DockPayloadRecoveryCommitError::PayloadSurvivalChanged)
    );
}

#[test]
fn newer_prepare_and_first_commit_make_older_or_duplicate_tokens_stale() {
    let fixture = tabs_fixture();
    let live = live_payload_proof(6_000);
    let mut registry = DockPayloadRecoveryRegistry::new();
    let first = prepare(&mut registry, &fixture, live);
    let second = prepare(&mut registry, &fixture, live);
    assert!(second.generation().get() > first.generation().get());
    assert!(!registry.can_commit(&fixture.graph, OWNER_REVISION, &first));
    assert_eq!(
        registry.commit(
            &fixture.graph,
            OWNER_REVISION,
            &fixture.primary_space,
            true,
            &first,
        ),
        Err(DockPayloadRecoveryCommitError::StalePreparedToken)
    );

    let duplicate = second.clone();
    registry
        .commit(
            &fixture.graph,
            OWNER_REVISION,
            &fixture.primary_space,
            true,
            &second,
        )
        .expect("latest token should commit once");
    assert_eq!(
        registry.commit(
            &fixture.graph,
            OWNER_REVISION,
            &fixture.primary_space,
            true,
            &duplicate,
        ),
        Err(DockPayloadRecoveryCommitError::StalePreparedToken)
    );
}

#[test]
fn committed_receipt_has_registry_provenance_and_lost_viewport_reuses_protocol() {
    let fixture = floating_fixture(true);
    let live = live_payload_proof(7_000);
    let mut registry = DockPayloadRecoveryRegistry::new();
    let prepared = registry
        .prepare(
            &fixture.graph,
            OWNER_REVISION,
            committed_destination_authority(live),
            &fixture.payload,
            DockPayloadRecoveryReason::LostViewportRecovery,
        )
        .expect("lost viewport should use the same preparation protocol");
    assert_eq!(prepared.owner_revision(), OWNER_REVISION);
    assert_eq!(prepared.live_identity(), live.identity);
    assert_eq!(prepared.authority(), committed_destination_authority(live));
    assert_eq!(
        prepared.reason(),
        DockPayloadRecoveryReason::LostViewportRecovery
    );
    let receipt = registry
        .commit(
            &fixture.graph,
            OWNER_REVISION,
            &fixture.primary_space,
            true,
            &prepared,
        )
        .expect("lost viewport recovery should commit");
    assert_eq!(receipt.live_identity(), live.identity);
    assert_eq!(receipt.authority(), committed_destination_authority(live));
    let record = registry
        .record(receipt)
        .expect("only the committing registry should retain the receipt record");
    assert_eq!(record.receipt(), receipt);
    assert_eq!(record.base_owner_revision(), OWNER_REVISION);
    assert_eq!(
        record.reason(),
        DockPayloadRecoveryReason::LostViewportRecovery
    );
    assert_eq!(record.payload_identity(), &fixture.payload);
    assert_eq!(record.disposition(), receipt.disposition());
    assert!(!record.was_rehomed());
    assert!(DockPayloadRecoveryRegistry::new().record(receipt).is_none());
    assert_eq!(
        registry.prepare(
            &fixture.graph,
            OWNER_REVISION,
            committed_destination_authority(live),
            &fixture.payload,
            DockPayloadRecoveryReason::LostViewportRecovery,
        ),
        Err(DockPayloadRecoveryPrepareError::PayloadAlreadyCommitted)
    );
}

#[test]
fn recovery_authority_kind_must_match_the_recovery_phase() {
    let fixture = item_fixture();
    let first = live_payload_proof(7_100);
    let mut registry = DockPayloadRecoveryRegistry::new();
    assert_eq!(
        registry.prepare(
            &fixture.graph,
            OWNER_REVISION,
            presentation_authority(first),
            &fixture.payload,
            DockPayloadRecoveryReason::LostViewportRecovery,
        ),
        Err(DockPayloadRecoveryPrepareError::AuthorityReasonMismatch)
    );
    assert_eq!(
        registry.prepare(
            &fixture.graph,
            OWNER_REVISION,
            committed_destination_authority(first),
            &fixture.payload,
            DockPayloadRecoveryReason::PreCommitOrphan,
        ),
        Err(DockPayloadRecoveryPrepareError::AuthorityReasonMismatch)
    );
    let prepared = prepare(&mut registry, &fixture, first);
    assert_eq!(prepared.generation().get(), 1);
}

#[open_gpui::test]
fn owner_commit_records_panel_lifecycle_without_publishing_revision_early(
    cx: &mut open_gpui::TestAppContext,
) {
    let fixture = item_fixture();
    let controller = cx.new(|_| {
        DockController::new(DockWorkspace::new(
            fixture.primary_space.clone(),
            fixture.graph.clone(),
        ))
    });
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let owner = cx.new(|cx| {
        DockSurfaceOwner::new(
            controller,
            runtime,
            fixture.primary_space.clone(),
            cx.entity_id(),
        )
    });

    let (receipt, revision_before_finish, event) = cx.update_entity(&owner, |owner, owner_cx| {
        let opening = owner
            .window_session_mut()
            .reserve_opening()
            .expect("primary should reserve");
        let primary_lease = owner
            .window_session_mut()
            .commit_opening(opening, WindowId::from(8_001))
            .expect("primary should activate");
        let source = DockLiveUndockSourceSnapshot::new(WindowId::from(8_002), 0);
        let trigger = DockLiveUndockTrigger::new(
            DockLiveUndockDragGeneration::new(1)
                .expect("the test drag generation should be non-zero"),
            source,
            DockLiveUndockRouteFeedback::Desktop,
            DockLiveUndockPhysicalPoint::new(50, 50),
        )
        .expect("desktop should be an eligible live-undock route");
        let request = owner
            .reduce_live_undock_fact(DockLiveUndockFact::Trigger {
                lease: primary_lease,
                trigger,
            })
            .expect("the exact active surface lease should accept the trigger")
            .into_iter()
            .find_map(|effect| match effect {
                DockLiveUndockEffect::OpenProvisional { request, .. } => Some(request),
                _ => None,
            })
            .expect("the accepted trigger should reserve one provisional opening");
        let identity = request.identity();
        let payload_lease = DockLiveUndockPayloadLeaseReceipt::for_test(
            identity,
            source,
            DockLiveUndockPresentationLeaseGeneration::new(1)
                .expect("test lease generation should be non-zero"),
            WindowId::from(8_003),
        );
        let prepared = owner
            .prepare_payload_recovery(
                DockPayloadRecoveryAuthority::presentation_lease(payload_lease),
                &fixture.payload,
                DockPayloadRecoveryReason::PreCommitOrphan,
                owner_cx,
            )
            .expect("owner should reserve recovery");
        assert!(owner.can_commit_payload_recovery(&prepared, owner_cx));
        let transaction = owner.begin_root_transaction();
        let receipt = owner
            .commit_payload_recovery(transaction, &prepared, owner_cx)
            .expect("owner should commit inside the active root transaction");
        let revision_before_finish = owner.revision();
        let event = owner
            .finish_root_transaction(transaction, owner_cx)
            .expect("payload recovery should publish one categorized change");
        (receipt, revision_before_finish, event)
    });

    assert_eq!(revision_before_finish, 0);
    assert_eq!(event.revision(), 1);
    assert_eq!(
        event.categories(),
        &[DockSurfaceChangeCategory::PanelLifecycle]
    );
    assert!(event.transitions().is_empty());
    assert_eq!(
        receipt.disposition(),
        DockPayloadRecoveryDisposition::VisibleRecoveryEntry
    );
}

#[open_gpui::test]
fn owner_lost_and_restore_transactions_publish_exact_named_transitions(
    cx: &mut open_gpui::TestAppContext,
) {
    let mut fixture = item_fixture();
    let lost_space = space("owner-lost-viewport");
    move_payload_to_lost_space(&mut fixture, &lost_space);
    let controller = cx.new(|_| {
        DockController::new(DockWorkspace::new(
            fixture.primary_space.clone(),
            fixture.graph.clone(),
        ))
    });
    let runtime = DockViewportRuntimeHandle::new(controller.clone());
    let owner = cx.new(|cx| {
        DockSurfaceOwner::new(
            controller.clone(),
            runtime,
            fixture.primary_space.clone(),
            cx.entity_id(),
        )
    });

    let (recovery, action, lost_event) = cx.update_entity(&owner, |owner, owner_cx| {
        let opening = owner
            .window_session_mut()
            .reserve_opening()
            .expect("primary should reserve");
        let primary_lease = owner
            .window_session_mut()
            .commit_opening(opening, WindowId::from(8_501))
            .expect("primary should activate");
        let source = DockLiveUndockSourceSnapshot::new(WindowId::from(8_502), 0);
        let trigger = DockLiveUndockTrigger::new(
            DockLiveUndockDragGeneration::new(1)
                .expect("the test drag generation should be non-zero"),
            source,
            DockLiveUndockRouteFeedback::Desktop,
            DockLiveUndockPhysicalPoint::new(50, 50),
        )
        .expect("desktop should be an eligible live-undock route");
        let identity = owner
            .reduce_live_undock_fact(DockLiveUndockFact::Trigger {
                lease: primary_lease,
                trigger,
            })
            .expect("the exact primary lease should admit live undock")
            .into_iter()
            .find_map(|effect| match effect {
                DockLiveUndockEffect::OpenProvisional { identity, .. } => Some(identity),
                _ => None,
            })
            .expect("live undock should allocate one identity");
        let authority = DockPayloadRecoveryAuthority::committed_destination(
            identity,
            DockLiveUndockPromotionToken::new(1).expect("test promotion token should be non-zero"),
            DockLiveUndockPromotionDestination::SameWindowDesktop {
                window_id: WindowId::from(8_503),
            },
        );
        let prepared = owner
            .prepare_payload_recovery(
                authority,
                &fixture.payload,
                DockPayloadRecoveryReason::LostViewportRecovery,
                owner_cx,
            )
            .expect("lost viewport should reserve recovery");
        let transaction = owner.begin_root_transaction();
        let recovery = owner
            .commit_payload_recovery(transaction, &prepared, owner_cx)
            .expect("lost viewport should commit its record");
        let lost_event = owner
            .finish_root_transaction(transaction, owner_cx)
            .expect("lost viewport should publish one revision");
        let action = owner
            .payload_recovery_restore_action(recovery)
            .expect("the current anchor should own one restore action");
        (recovery, action, lost_event)
    });

    assert_eq!(lost_event.revision(), 1);
    assert_eq!(
        lost_event.categories(),
        &[
            DockSurfaceChangeCategory::PanelLifecycle,
            DockSurfaceChangeCategory::ViewportTopology,
        ]
    );
    assert_eq!(
        lost_event.transitions(),
        &[DockSurfaceTransition::ViewportLostAfterPromotion]
    );

    cx.update_entity(&owner, |owner, owner_cx| {
        let (first, _) = owner
            .reserve_payload_recovery_restore(action, owner_cx)
            .expect("the first Restore activation should reserve the executor");
        assert!(
            matches!(
                owner.reserve_payload_recovery_restore(action, owner_cx),
                Err(DockPayloadRecoveryRestoreError::AlreadyInFlight)
            ),
            "duplicate mouse and accessibility activations must share one Restore execution",
        );
        assert!(
            owner.cancel_payload_recovery_execution(first),
            "the exact reserved Restore execution should be cancellable",
        );
        let (next, _) = owner
            .reserve_payload_recovery_restore(action, owner_cx)
            .expect("the recovery record should accept a later fresh execution");
        assert!(
            next.sequence() > first.sequence(),
            "a later Restore attempt must receive a fresh execution identity",
        );
        assert!(owner.cancel_payload_recovery_execution(next));
    });

    let (restored, restored_event) = cx.update_entity(&owner, |owner, owner_cx| {
        let transaction = owner.begin_root_transaction();
        let restored = owner
            .restore_payload_recovery(transaction, action, owner_cx)
            .expect("the exact restore action should commit");
        let event = owner
            .finish_root_transaction(transaction, owner_cx)
            .expect("Restore should publish one revision");
        (restored, event)
    });
    assert_eq!(restored.recovery(), recovery);
    assert_eq!(restored_event.revision(), 2);
    assert_eq!(
        restored_event.categories(),
        &[
            DockSurfaceChangeCategory::Layout,
            DockSurfaceChangeCategory::Selection,
            DockSurfaceChangeCategory::PanelLifecycle,
        ]
    );
    assert_eq!(
        restored_event.transitions(),
        &[DockSurfaceTransition::ViewportRecovered]
    );
    cx.read_entity(&controller, |controller, _| {
        assert!(
            controller
                .graph()
                .find_item_in_space(&fixture.primary_space, &item("payload"))
                .is_some()
        );
        assert!(
            controller
                .graph()
                .find_item_in_space(&lost_space, &item("payload"))
                .is_none()
        );
    });

    let (stale, stale_event, revision) = cx.update_entity(&owner, |owner, owner_cx| {
        let transaction = owner.begin_root_transaction();
        let stale = owner.restore_payload_recovery(transaction, action, owner_cx);
        let event = owner.finish_root_transaction(transaction, owner_cx);
        (stale, event, owner.revision())
    });
    assert!(matches!(
        stale,
        Err(DockPayloadRecoveryRestoreError::StaleAction)
    ));
    assert!(stale_event.is_none());
    assert_eq!(revision, 2);
}

#[test]
fn restore_rehomes_lost_item_and_consumes_the_exact_recovery_action() {
    let mut fixture = item_fixture();
    let lost_space = space("lost-viewport");
    move_payload_to_lost_space(&mut fixture, &lost_space);
    let graph_before_prepare = format!("{:?}", fixture.graph);
    let live = live_payload_proof(8_100);
    let anchor_lease = live.identity.opening().lease();
    let mut registry = DockPayloadRecoveryRegistry::new();
    let recovery = commit_lost_viewport_recovery(&mut registry, &fixture, live);
    let action = registry
        .restore_action(recovery, anchor_lease)
        .expect("a visible lost-viewport record should expose one exact restore action");

    let prepared = registry
        .prepare_restore(
            &fixture.graph,
            OWNER_REVISION,
            &fixture.primary_space,
            Some(anchor_lease),
            action,
        )
        .expect("the exact active anchor should prepare restore");
    assert_eq!(
        format!("{:?}", fixture.graph),
        graph_before_prepare,
        "restore preparation must not mutate the authoritative graph"
    );
    assert!(registry.can_commit_restore(
        &fixture.graph,
        OWNER_REVISION,
        Some(anchor_lease),
        &prepared,
    ));

    fixture.graph = prepared.projected_graph().clone();
    let restored = registry.commit_prepared_restore(prepared);
    assert_eq!(restored.recovery(), recovery);
    assert_eq!(registry.visible_records().count(), 0);
    assert_eq!(
        fixture
            .graph
            .find_item_in_space(&fixture.primary_space, &item("payload"))
            .map(|(tabs, _)| tabs),
        Some(fixture.primary_tabs)
    );
    assert!(
        fixture
            .graph
            .find_item_in_space(&lost_space, &item("payload"))
            .is_none()
    );
    assert!(matches!(
        registry.prepare_restore(
            &fixture.graph,
            OWNER_REVISION,
            &fixture.primary_space,
            Some(anchor_lease),
            action,
        ),
        Err(DockPayloadRecoveryRestoreError::StaleAction)
    ));
}

#[test]
fn restore_moves_only_the_payload_range_after_tabs_were_flattened_with_a_peer() {
    let mut fixture = tabs_fixture();
    let lost_space = space("lost-tabs-viewport");
    move_payload_to_lost_space(&mut fixture, &lost_space);
    let lost_tabs = fixture
        .graph
        .root(&lost_space)
        .expect("promoted tabs should become the lost-space root");
    fixture
        .graph
        .apply_op_checked(&DockOp::OpenItem {
            space: lost_space.clone(),
            target_tabs: Some(lost_tabs),
            item: item("unrelated-peer"),
            insert_index: None,
        })
        .expect("the lost viewport should accept an unrelated peer");

    let live = live_payload_proof(8_200);
    let anchor_lease = live.identity.opening().lease();
    let mut registry = DockPayloadRecoveryRegistry::new();
    let recovery = commit_lost_viewport_recovery(&mut registry, &fixture, live);
    let action = registry
        .restore_action(recovery, anchor_lease)
        .expect("lost tabs should expose restore");
    let prepared = registry
        .prepare_restore(
            &fixture.graph,
            OWNER_REVISION,
            &fixture.primary_space,
            Some(anchor_lease),
            action,
        )
        .expect("flattened tabs should project into the primary recovery group");
    fixture.graph = prepared.projected_graph().clone();
    registry.commit_prepared_restore(prepared);

    let Some(DockNode::Tabs { items, .. }) = fixture.graph.node(fixture.primary_tabs) else {
        panic!("primary recovery target should remain tabs");
    };
    assert_eq!(items, &[item("home"), item("payload-a"), item("payload-b")]);
    assert_eq!(
        fixture.graph.collect_items_in_space(&lost_space),
        vec![item("unrelated-peer")],
        "Restore must not move a peer that joined the destination tabs later"
    );
}

#[test]
fn restore_flattens_a_split_floating_payload_into_the_primary_recovery_group() {
    let mut fixture = floating_fixture(true);
    let lost_space = space("lost-floating-viewport");
    move_payload_to_lost_space(&mut fixture, &lost_space);
    let live = live_payload_proof(8_300);
    let anchor_lease = live.identity.opening().lease();
    let mut registry = DockPayloadRecoveryRegistry::new();
    let recovery = commit_lost_viewport_recovery(&mut registry, &fixture, live);
    let action = registry
        .restore_action(recovery, anchor_lease)
        .expect("lost floating payload should expose restore");
    let prepared = registry
        .prepare_restore(
            &fixture.graph,
            OWNER_REVISION,
            &fixture.primary_space,
            Some(anchor_lease),
            action,
        )
        .expect("split floating payload should prepare as ordered items");
    fixture.graph = prepared.projected_graph().clone();
    registry.commit_prepared_restore(prepared);

    fixture
        .graph
        .validate()
        .expect("the restored graph should remain canonical");
    let Some(DockNode::Tabs { items, .. }) = fixture.graph.node(fixture.primary_tabs) else {
        panic!("primary recovery target should remain tabs");
    };
    assert_eq!(items, &[item("home"), item("payload-a"), item("payload-b")]);
    assert!(fixture.graph.collect_items_in_space(&lost_space).is_empty());
}

#[test]
fn restore_rejects_an_inactive_anchor_and_a_location_changed_after_preflight() {
    let mut fixture = item_fixture();
    let lost_space = space("lost-stale-viewport");
    move_payload_to_lost_space(&mut fixture, &lost_space);
    let live = live_payload_proof(8_400);
    let anchor_lease = live.identity.opening().lease();
    let mut registry = DockPayloadRecoveryRegistry::new();
    let recovery = commit_lost_viewport_recovery(&mut registry, &fixture, live);
    let action = registry
        .restore_action(recovery, anchor_lease)
        .expect("lost payload should expose restore");
    assert!(matches!(
        registry.prepare_restore(
            &fixture.graph,
            OWNER_REVISION,
            &fixture.primary_space,
            None,
            action,
        ),
        Err(DockPayloadRecoveryRestoreError::AnchorUnavailable)
    ));

    let prepared = registry
        .prepare_restore(
            &fixture.graph,
            OWNER_REVISION,
            &fixture.primary_space,
            Some(anchor_lease),
            action,
        )
        .expect("current action should prepare before the graph changes");
    fixture
        .graph
        .apply_op_checked(&DockOp::MoveItem {
            source_space: lost_space,
            item: item("payload"),
            target_space: fixture.source_space.clone(),
            target: DockGraphDropTarget::center(
                fixture
                    .graph
                    .root(&fixture.source_space)
                    .expect("source peer should retain its tabs"),
            ),
        })
        .expect("test should move the payload after restore preflight");
    assert!(!registry.can_commit_restore(
        &fixture.graph,
        OWNER_REVISION,
        Some(anchor_lease),
        &prepared,
    ));
    assert_eq!(registry.visible_records().count(), 1);
}
