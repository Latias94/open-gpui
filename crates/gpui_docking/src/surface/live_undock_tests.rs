use super::{
    apply_surface_shutdown_close_effects, finish_live_undock_open_failure,
    finish_live_undock_open_return,
    live_undock::{
        DockLiveUndockCancelReason, DockLiveUndockDragGeneration, DockLiveUndockEffect,
        DockLiveUndockFact, DockLiveUndockOpenFailureOutcome, DockLiveUndockOpenRequest,
        DockLiveUndockOpenReturnOutcome, DockLiveUndockPhase, DockLiveUndockRouteFeedback,
        DockLiveUndockSession, DockLiveUndockSourceSnapshot, DockLiveUndockTrigger,
    },
    prepare_surface_shutdown, reduce_live_undock_fact,
    window_session::{
        DockSurfaceWindowSession, DockSurfaceWindowSessionDependencyId,
        DockSurfaceWindowSessionLease, DockSurfaceWindowSessionPhase,
        DockSurfaceWindowSessionShutdownReason,
    },
};
use crate::{DockHost, DockSurfacePrimaryWindowOpenOutcome, DockViewportWindowRole};
use open_gpui::{
    AnyWindowHandle, AppContext as _, Empty, EntityId, PlatformWindowCreationCapabilities,
    QuitMode, WindowCreationSupport, WindowId, WindowInitialPresentationOrder, WindowOptions,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn active_window_session(
    authority: u64,
    anchor: u64,
) -> (
    DockSurfaceWindowSession,
    super::window_session::DockSurfaceWindowSessionLease,
) {
    let mut session = DockSurfaceWindowSession::new(EntityId::from(authority));
    let opening = session.reserve_opening().expect("G1 should reserve");
    let lease = session
        .commit_opening(opening, WindowId::from(anchor))
        .expect("G1 should activate");
    (session, lease)
}

mod reducer_tests {
    use super::{DockHost, active_window_session};
    use crate::{
        DockGraph, DockItemId, DockNode, DockSpaceId,
        locked_drop_identity::DockLockedPayloadIdentity,
        surface::{
            live_undock::*,
            payload_recovery::{
                DockPayloadRecoveryAuthority, DockPayloadRecoveryCommitReceipt,
                DockPayloadRecoveryReason, DockPayloadRecoveryRegistry,
            },
        },
    };
    use open_gpui::{AnyWindowHandle, WindowHandle, WindowId};

    fn drag_generation(value: u64) -> DockLiveUndockDragGeneration {
        DockLiveUndockDragGeneration::new(value).expect("test drag generations are non-zero")
    }

    fn lease_generation(value: u64) -> DockLiveUndockPresentationLeaseGeneration {
        DockLiveUndockPresentationLeaseGeneration::new(value)
            .expect("test presentation-lease generations are non-zero")
    }

    fn placement_generation(value: u64) -> DockLiveUndockPlacementGeneration {
        DockLiveUndockPlacementGeneration::new(value)
            .expect("test placement generations are non-zero")
    }

    fn fake_window(value: u64) -> AnyWindowHandle {
        WindowHandle::<DockHost>::new(WindowId::from(value)).into()
    }

    fn admitted_runtime(
        identity: DockLiveUndockIdentity,
        window: AnyWindowHandle,
    ) -> crate::DockViewportProvisionalOpenAttemptCompletion {
        crate::DockViewportProvisionalOpenAttemptCompletion::admitted_for_test(
            window.window_id(),
            identity.opening(),
        )
    }

    fn source_for(identity: DockLiveUndockIdentity) -> DockLiveUndockSourceSnapshot {
        let generation = identity.drag_generation().get();
        DockLiveUndockSourceSnapshot::new(WindowId::from(9_000 + generation), generation)
    }

    fn source_native_terminal(
        identity: DockLiveUndockIdentity,
        source: DockLiveUndockSourceSnapshot,
    ) -> DockLiveUndockSourceNativeTerminalReceipt {
        DockLiveUndockSourceNativeTerminalReceipt::from_native_terminal(
            identity,
            source,
            source.window_id(),
        )
        .expect("the exact source window should mint terminal evidence")
    }

    pub(super) fn trigger_for(generation: u64) -> DockLiveUndockTrigger {
        DockLiveUndockTrigger::new(
            drag_generation(generation),
            DockLiveUndockSourceSnapshot::new(WindowId::from(9_000 + generation), generation),
            DockLiveUndockRouteFeedback::Desktop,
        )
        .expect("desktop is an eligible live-undock trigger route")
    }

    pub(super) fn start_request(
        session: &mut DockLiveUndockSession,
        lease: super::super::window_session::DockSurfaceWindowSessionLease,
        generation: u64,
    ) -> DockLiveUndockOpenRequest {
        let effects = session.apply(DockLiveUndockFact::Trigger {
            lease,
            trigger: trigger_for(generation),
        });
        let mut opening = effects.into_iter().filter_map(|effect| match effect {
            DockLiveUndockEffect::OpenProvisional { identity, request } => {
                assert_eq!(identity, request.identity());
                Some(request)
            }
            _ => None,
        });
        let request = opening.next().expect("the trigger should open once");
        assert!(opening.next().is_none(), "the trigger must open only once");
        request
    }

    fn start(
        session: &mut DockLiveUndockSession,
        lease: super::super::window_session::DockSurfaceWindowSessionLease,
        generation: u64,
    ) -> DockLiveUndockIdentity {
        start_request(session, lease, generation).identity()
    }

    fn admit(
        session: &mut DockLiveUndockSession,
        identity: DockLiveUndockIdentity,
        window: AnyWindowHandle,
    ) {
        let effects = session.apply(DockLiveUndockFact::OpeningReturned {
            identity,
            window,
            binding: DockLiveUndockOpeningBinding::ExactGated,
            runtime: admitted_runtime(identity, window),
        });
        assert!(matches!(
            effects.as_slice(),
            [DockLiveUndockEffect::ProvisionalAdmitted {
                identity: current,
                window: current_window,
                ..
            }] if *current == identity && *current_window == window
        ));
    }

    fn assert_release_placement_request(
        effects: &DockLiveUndockEffects,
        identity: DockLiveUndockIdentity,
        window: AnyWindowHandle,
        release: DockLiveUndockReleaseLock,
    ) {
        assert!(matches!(
            effects.as_slice(),
            [
                DockLiveUndockEffect::RetireSourceTransportProxy {
                    identity: retired_identity,
                },
                DockLiveUndockEffect::RequestReleasePlacement {
                    identity: current,
                    window: current_window,
                    release: current_release,
                },
            ] if *retired_identity == identity
                && *current == identity
                && *current_window == window
                && *current_release == release
        ));
    }

    fn activate_payload(
        session: &mut DockLiveUndockSession,
        identity: DockLiveUndockIdentity,
        source: DockLiveUndockSourceSnapshot,
        presentation_lease: DockLiveUndockPresentationLeaseGeneration,
        window: AnyWindowHandle,
    ) -> DockLiveUndockPayloadMountReceipt {
        let lease = DockLiveUndockPayloadLeaseReceipt::for_test(
            identity,
            source,
            presentation_lease,
            window.window_id(),
        );
        let effects = session.apply(DockLiveUndockFact::PresentationLeaseActivated {
            identity,
            receipt: lease,
        });
        assert!(matches!(
            effects.as_slice(),
            [DockLiveUndockEffect::CommitSourceProxy { .. }]
        ));
        let proxy = DockLiveUndockSourceProxyReceipt::for_test(lease, 101)
            .expect("the source proxy must commit in a non-zero GPUI frame");
        let effects = session.apply(DockLiveUndockFact::SourceProxyCommitted {
            identity,
            receipt: proxy,
        });
        assert!(matches!(
            effects.as_slice(),
            [DockLiveUndockEffect::MountAndExposePayload { .. }]
        ));
        let mount = DockLiveUndockPayloadMountReceipt::for_test(proxy, 102)
            .expect("the payload mount must commit in a non-zero GPUI frame");
        let effects = session.apply(DockLiveUndockFact::PayloadMounted {
            identity,
            receipt: mount,
        });
        assert!(matches!(
            effects.as_slice(),
            [DockLiveUndockEffect::ObservePayloadPresentation { .. }]
        ));
        mount
    }

    fn observe_nonempty_visible(
        session: &mut DockLiveUndockSession,
        identity: DockLiveUndockIdentity,
        mount: DockLiveUndockPayloadMountReceipt,
        frame_generation: u64,
    ) {
        let preflight = DockLiveUndockPayloadPresentationReceipt::for_test(mount, frame_generation)
            .expect("the payload preflight frame must be non-zero");
        let effects = session.apply(DockLiveUndockFact::PayloadPresented {
            identity,
            receipt: preflight,
        });
        assert!(matches!(
            effects.as_slice(),
            [DockLiveUndockEffect::ArmExactReveal { .. }]
        ));
        let reveal_frame =
            DockLiveUndockPayloadPresentationReceipt::for_test(mount, frame_generation + 1);
        let reveal = DockLiveUndockRevealReceipt::for_test(
            preflight,
            reveal_frame.expect("the reveal frame remains non-zero"),
        )
        .expect("the exact later payload frame should satisfy test reveal authority");
        assert!(matches!(
            session
                .apply(DockLiveUndockFact::RevealObserved {
                    identity,
                    observation: DockLiveUndockRevealObservation::Visible(reveal),
                })
                .as_slice(),
            [DockLiveUndockEffect::RetireFrozenSourceVisual { .. }]
        ));
    }

    fn desktop_release(generation: DockLiveUndockPlacementGeneration) -> DockLiveUndockReleaseLock {
        DockLiveUndockReleaseLock::new(
            DockLiveUndockPhysicalPoint::new(800, 500),
            DockLiveUndockRouteFeedback::Desktop,
            DockLiveUndockPhysicalBounds::new(DockLiveUndockPhysicalPoint::new(760, 470), 640, 480)
                .expect("test release bounds are non-empty"),
            generation,
        )
    }

    fn prepare_token(
        effects: DockLiveUndockEffects,
    ) -> (
        DockLiveUndockPromotionToken,
        DockLiveUndockPromotionDestination,
    ) {
        effects
            .into_iter()
            .find_map(|effect| match effect {
                DockLiveUndockEffect::PreparePromotion {
                    token, destination, ..
                } => Some((token, destination)),
                _ => None,
            })
            .expect("the exact readiness boundary should request one promotion preflight")
    }

    fn semantics_receipt(
        identity: DockLiveUndockIdentity,
        token: DockLiveUndockPromotionToken,
        destination: DockLiveUndockPromotionDestination,
    ) -> DockLiveUndockDestinationSemanticsReceipt {
        DockLiveUndockDestinationSemanticsReceipt::for_test(identity, token, destination)
    }

    fn interaction_receipt(
        semantics: DockLiveUndockDestinationSemanticsReceipt,
    ) -> DockLiveUndockDestinationInteractionReceipt {
        DockLiveUndockDestinationInteractionReceipt::for_test(semantics)
    }

    fn restoration_request(
        effects: &DockLiveUndockEffects,
    ) -> (
        DockLiveUndockSourceSnapshot,
        DockLiveUndockPayloadLeaseReceipt,
        bool,
    ) {
        effects
            .as_slice()
            .iter()
            .find_map(|effect| match effect {
                DockLiveUndockEffect::RestoreSource {
                    source,
                    payload_lease,
                    restore_focus,
                    ..
                } => Some((*source, *payload_lease, *restore_focus)),
                _ => None,
            })
            .expect("the transition should request one exact source restoration")
    }

    fn orphan_recovery_request(
        effects: &DockLiveUndockEffects,
    ) -> (DockLiveUndockPayloadLeaseReceipt, Option<AnyWindowHandle>) {
        effects
            .as_slice()
            .iter()
            .find_map(|effect| match effect {
                DockLiveUndockEffect::RecoverOrphanedPayloadTopology {
                    payload_lease,
                    provisional,
                    ..
                } => Some((*payload_lease, *provisional)),
                _ => None,
            })
            .expect("the transition should request one exact orphan recovery")
    }

    fn committed_payload_recovery(
        authority: DockPayloadRecoveryAuthority,
        reason: DockPayloadRecoveryReason,
    ) -> DockPayloadRecoveryCommitReceipt {
        let source_space = DockSpaceId::from("reducer-recovery-source");
        let payload_item = DockItemId::from("payload");
        let mut graph = DockGraph::new();
        let source_tabs = graph.insert_node(DockNode::Tabs {
            selected: Some(payload_item.clone()),
            items: vec![payload_item.clone()],
        });
        graph.set_root(source_space.clone(), source_tabs);
        let payload_identity = DockLockedPayloadIdentity::Item {
            source_space: source_space.clone(),
            source_tabs,
            item: payload_item,
        };
        let owner_revision = authority
            .presentation()
            .map_or(1, DockLiveUndockPayloadLeaseReceipt::surface_revision);
        let mut registry = DockPayloadRecoveryRegistry::new();
        let prepared = registry
            .prepare(&graph, owner_revision, authority, &payload_identity, reason)
            .expect("the reducer fixture must prepare a real payload recovery");
        let recovery = registry
            .commit(&graph, owner_revision, &source_space, false, &prepared)
            .expect("the reducer fixture must commit a real payload recovery");
        recovery
    }

    fn committed_orphan_recovery(
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
    ) -> DockLiveUndockOrphanRecoveryReceipt {
        DockLiveUndockOrphanRecoveryReceipt::for_test(committed_payload_recovery(
            DockPayloadRecoveryAuthority::presentation_lease(payload_lease),
            DockPayloadRecoveryReason::PreCommitOrphan,
        ))
        .expect("orphan recovery must retain presentation authority")
    }

    fn committed_destination_recovery(
        authority: DockPayloadRecoveryAuthority,
    ) -> DockLiveUndockCommittedDestinationRecoveryReceipt {
        DockLiveUndockCommittedDestinationRecoveryReceipt::new(committed_payload_recovery(
            authority,
            DockPayloadRecoveryReason::LostViewportRecovery,
        ))
        .expect("committed recovery must retain durable promotion authority")
    }

    fn committed_destination_recovery_request(
        effects: &DockLiveUndockEffects,
    ) -> (
        DockPayloadRecoveryAuthority,
        DockLiveUndockPromotionToken,
        DockLiveUndockPromotionDestination,
    ) {
        effects
            .as_slice()
            .iter()
            .find_map(|effect| match effect {
                DockLiveUndockEffect::RecoverCommittedDestinationTopology {
                    authority,
                    token,
                    destination,
                    ..
                } => Some((*authority, *token, *destination)),
                _ => None,
            })
            .expect("the transition should request one exact committed destination recovery")
    }

    fn acknowledge_source_restoration(
        session: &mut DockLiveUndockSession,
        identity: DockLiveUndockIdentity,
        payload_lease: DockLiveUndockPayloadLeaseReceipt,
    ) -> DockLiveUndockEffects {
        let receipt = match session.phase() {
            DockLiveUndockPhase::Compensating => {
                DockLiveUndockSourceRestorationReceipt::source_unchanged_for_test(payload_lease)
            }
            DockLiveUndockPhase::Restoring => {
                DockLiveUndockSourceRestorationReceipt::source_presented_after_release_for_test(
                    payload_lease,
                    9_001,
                )
                .expect("the restored source frame should be non-zero")
            }
            phase => panic!("source restoration cannot be acknowledged in phase {phase:?}"),
        };
        session.apply(DockLiveUndockFact::SourceRestorationCommitted { identity, receipt })
    }

    fn prepare_desktop(
        session: &mut DockLiveUndockSession,
        identity: DockLiveUndockIdentity,
        window: AnyWindowHandle,
        placement: DockLiveUndockPlacementGeneration,
    ) -> DockLiveUndockPromotionToken {
        let source = source_for(identity);
        let mount = activate_payload(session, identity, source, lease_generation(3), window);
        observe_nonempty_visible(session, identity, mount, 91);
        assert!(
            session
                .apply(DockLiveUndockFact::PlacementObserved {
                    identity,
                    window_id: window.window_id(),
                    generation: placement,
                    outcome: DockLiveUndockPlacementOutcome::Exact,
                })
                .is_empty()
        );
        let (token, destination) =
            prepare_token(session.apply(DockLiveUndockFact::ReleaseLocked {
                identity,
                release: desktop_release(placement),
            }));
        assert_eq!(
            destination,
            DockLiveUndockPromotionDestination::SameWindowDesktop {
                window_id: window.window_id()
            }
        );
        token
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum TestPresentationFailureStage {
        PayloadLeaseClaim,
        RetainedVisualTicket,
        RehostPreparation,
        SourceProxyReplay,
        DestinationExposureFinish,
        PayloadPresentationObservation,
        ExactRevealTicket,
    }

    fn progress_to_presentation_failure_stage(
        session: &mut DockLiveUndockSession,
        identity: DockLiveUndockIdentity,
        window: AnyWindowHandle,
        stage: TestPresentationFailureStage,
    ) -> (
        DockLiveUndockPresentationFailure,
        Option<DockLiveUndockPayloadLeaseReceipt>,
    ) {
        match stage {
            TestPresentationFailureStage::PayloadLeaseClaim => {
                return (DockLiveUndockPresentationFailure::PayloadLeaseClaim, None);
            }
            TestPresentationFailureStage::RetainedVisualTicket => {
                return (
                    DockLiveUndockPresentationFailure::RetainedVisualTicket,
                    None,
                );
            }
            TestPresentationFailureStage::RehostPreparation => {
                return (DockLiveUndockPresentationFailure::RehostPreparation, None);
            }
            TestPresentationFailureStage::SourceProxyReplay
            | TestPresentationFailureStage::DestinationExposureFinish
            | TestPresentationFailureStage::PayloadPresentationObservation
            | TestPresentationFailureStage::ExactRevealTicket => {}
        }

        let lease = DockLiveUndockPayloadLeaseReceipt::for_test(
            identity,
            source_for(identity),
            lease_generation(30),
            window.window_id(),
        );
        assert!(matches!(
            session
                .apply(DockLiveUndockFact::PresentationLeaseActivated {
                    identity,
                    receipt: lease,
                })
                .as_slice(),
            [DockLiveUndockEffect::CommitSourceProxy { .. }]
        ));
        if stage == TestPresentationFailureStage::SourceProxyReplay {
            return (
                DockLiveUndockPresentationFailure::SourceProxyReplay { lease },
                Some(lease),
            );
        }

        let proxy = DockLiveUndockSourceProxyReceipt::for_test(lease, 301)
            .expect("the test source proxy frame is non-zero");
        assert!(matches!(
            session
                .apply(DockLiveUndockFact::SourceProxyCommitted {
                    identity,
                    receipt: proxy,
                })
                .as_slice(),
            [DockLiveUndockEffect::MountAndExposePayload { .. }]
        ));
        if stage == TestPresentationFailureStage::DestinationExposureFinish {
            return (
                DockLiveUndockPresentationFailure::DestinationExposureFinish { proxy },
                Some(lease),
            );
        }

        let mount = DockLiveUndockPayloadMountReceipt::for_test(proxy, 302)
            .expect("the test payload mount frame is non-zero");
        assert!(matches!(
            session
                .apply(DockLiveUndockFact::PayloadMounted {
                    identity,
                    receipt: mount,
                })
                .as_slice(),
            [DockLiveUndockEffect::ObservePayloadPresentation { .. }]
        ));
        if stage == TestPresentationFailureStage::PayloadPresentationObservation {
            return (
                DockLiveUndockPresentationFailure::PayloadPresentationObservation { mount },
                Some(lease),
            );
        }

        let presentation = DockLiveUndockPayloadPresentationReceipt::for_test(mount, 303)
            .expect("the test payload presentation frame is non-zero");
        assert!(matches!(
            session
                .apply(DockLiveUndockFact::PayloadPresented {
                    identity,
                    receipt: presentation,
                })
                .as_slice(),
            [DockLiveUndockEffect::ArmExactReveal { .. }]
        ));
        assert_eq!(
            stage,
            TestPresentationFailureStage::ExactRevealTicket,
            "the helper must cover every presentation failure stage"
        );
        (
            DockLiveUndockPresentationFailure::ExactRevealTicket { presentation },
            Some(lease),
        )
    }

    #[test]
    fn payload_lease_constructor_derives_generation_from_rehost_projection() {
        use open_gpui::{
            WindowProvisionalSession, retained_visual::TicketIdentity,
            view_presentation_window::RehostProjection,
        };

        let _constructor: fn(
            DockLiveUndockIdentity,
            DockLiveUndockSourceSnapshot,
            u64,
            TicketIdentity,
            &RehostProjection,
            &WindowProvisionalSession,
        ) -> Option<DockLiveUndockPayloadLeaseReceipt> = DockLiveUndockPayloadLeaseReceipt::new;
    }

    #[test]
    fn every_current_presentation_stage_failure_waits_for_exact_restoration_when_required() {
        let stages = [
            TestPresentationFailureStage::PayloadLeaseClaim,
            TestPresentationFailureStage::RetainedVisualTicket,
            TestPresentationFailureStage::RehostPreparation,
            TestPresentationFailureStage::SourceProxyReplay,
            TestPresentationFailureStage::DestinationExposureFinish,
            TestPresentationFailureStage::PayloadPresentationObservation,
            TestPresentationFailureStage::ExactRevealTicket,
        ];

        for (offset, stage) in stages.into_iter().enumerate() {
            let (_, lease) = active_window_session(500 + offset as u64, 600 + offset as u64);
            let mut session = DockLiveUndockSession::new();
            let identity = start(&mut session, lease, 1);
            let window = fake_window(700 + offset as u64);
            admit(&mut session, identity, window);
            let (failure, payload_lease) =
                progress_to_presentation_failure_stage(&mut session, identity, window, stage);

            let restore_effects =
                session.apply(DockLiveUndockFact::PresentationStageFailed { identity, failure });
            assert!(matches!(
                restore_effects.as_slice().first(),
                Some(DockLiveUndockEffect::RetireSourceTransportProxy {
                    identity: current,
                }) if *current == identity
            ));
            let effects = if let Some(payload_lease) = payload_lease {
                let (source, requested_lease, _) = restoration_request(&restore_effects);
                assert_eq!(source, payload_lease.source());
                assert_eq!(requested_lease, payload_lease);
                assert!(!restore_effects.as_slice().iter().any(|effect| matches!(
                    effect,
                    DockLiveUndockEffect::ProvisionalRetirementRequired { .. }
                        | DockLiveUndockEffect::PublishTerminal { .. }
                )));
                let expected_phase = if stage == TestPresentationFailureStage::SourceProxyReplay {
                    DockLiveUndockPhase::Compensating
                } else {
                    DockLiveUndockPhase::Restoring
                };
                assert_eq!(session.phase(), expected_phase);
                acknowledge_source_restoration(&mut session, identity, payload_lease)
            } else {
                assert!(
                    !restore_effects
                        .as_slice()
                        .iter()
                        .any(|effect| matches!(effect, DockLiveUndockEffect::RestoreSource { .. }))
                );
                restore_effects
            };
            assert!(effects.as_slice().iter().any(|effect| matches!(
                effect,
                DockLiveUndockEffect::PublishTerminal {
                    result: DockLiveUndockTerminalResult::Restored(
                        DockLiveUndockRestoreReason::PresentationFailed(current)
                    ),
                    ..
                } if *current == failure
            )));
            assert!(effects.as_slice().iter().any(|effect| matches!(
                effect,
                DockLiveUndockEffect::ProvisionalRetirementRequired {
                    reason: DockLiveUndockRetirementReason::SourceRestored(
                        DockLiveUndockRestoreReason::PresentationFailed(current)
                    ),
                    ..
                } if *current == failure
            )));
            assert!(!effects.as_slice().iter().any(|effect| matches!(
                effect,
                DockLiveUndockEffect::CommitPreparedPromotion { .. }
                    | DockLiveUndockEffect::DestinationSemanticsCommitRequired { .. }
            )));
            assert_eq!(session.phase(), DockLiveUndockPhase::Retiring);
        }
    }

    #[test]
    fn completed_or_stale_presentation_failures_cannot_regress_the_pipeline() {
        let (_, lease) = active_window_session(520, 620);
        let mut stale_session = DockLiveUndockSession::new();
        let stale_identity = start(&mut stale_session, lease, 1);

        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 2);
        let window = fake_window(720);
        assert!(
            session
                .apply(DockLiveUndockFact::PresentationStageFailed {
                    identity,
                    failure: DockLiveUndockPresentationFailure::PayloadLeaseClaim,
                })
                .is_empty(),
            "presentation work cannot fail before the provisional is admitted"
        );
        admit(&mut session, identity, window);
        assert!(
            session
                .apply(DockLiveUndockFact::PresentationStageFailed {
                    identity: stale_identity,
                    failure: DockLiveUndockPresentationFailure::PayloadLeaseClaim,
                })
                .is_empty(),
            "a previous drag generation cannot fail the active presentation pipeline"
        );

        let payload_lease = DockLiveUndockPayloadLeaseReceipt::for_test(
            identity,
            source_for(identity),
            lease_generation(31),
            window.window_id(),
        );
        session.apply(DockLiveUndockFact::PresentationLeaseActivated {
            identity,
            receipt: payload_lease,
        });
        for failure in [
            DockLiveUndockPresentationFailure::PayloadLeaseClaim,
            DockLiveUndockPresentationFailure::RetainedVisualTicket,
            DockLiveUndockPresentationFailure::RehostPreparation,
        ] {
            assert!(
                session
                    .apply(DockLiveUndockFact::PresentationStageFailed { identity, failure })
                    .is_empty(),
                "a completed early stage cannot report a late failure"
            );
        }

        let proxy = DockLiveUndockSourceProxyReceipt::for_test(payload_lease, 311)
            .expect("the test source proxy frame is non-zero");
        session.apply(DockLiveUndockFact::SourceProxyCommitted {
            identity,
            receipt: proxy,
        });
        assert!(
            session
                .apply(DockLiveUndockFact::PresentationStageFailed {
                    identity,
                    failure: DockLiveUndockPresentationFailure::SourceProxyReplay {
                        lease: payload_lease,
                    },
                })
                .is_empty()
        );

        let mount = DockLiveUndockPayloadMountReceipt::for_test(proxy, 312)
            .expect("the test payload mount frame is non-zero");
        session.apply(DockLiveUndockFact::PayloadMounted {
            identity,
            receipt: mount,
        });
        assert!(
            session
                .apply(DockLiveUndockFact::PresentationStageFailed {
                    identity,
                    failure: DockLiveUndockPresentationFailure::DestinationExposureFinish { proxy },
                })
                .is_empty()
        );

        let preflight = DockLiveUndockPayloadPresentationReceipt::for_test(mount, 313)
            .expect("the test payload presentation frame is non-zero");
        session.apply(DockLiveUndockFact::PayloadPresented {
            identity,
            receipt: preflight,
        });
        assert!(
            session
                .apply(DockLiveUndockFact::PresentationStageFailed {
                    identity,
                    failure: DockLiveUndockPresentationFailure::PayloadPresentationObservation {
                        mount,
                    },
                })
                .is_empty()
        );

        let replacement = DockLiveUndockPayloadPresentationReceipt::for_test(mount, 314)
            .expect("the replacement preflight frame is non-zero");
        session.apply(DockLiveUndockFact::PayloadPresented {
            identity,
            receipt: replacement,
        });
        assert!(
            session
                .apply(DockLiveUndockFact::PresentationStageFailed {
                    identity,
                    failure: DockLiveUndockPresentationFailure::ExactRevealTicket {
                        presentation: preflight,
                    },
                })
                .is_empty(),
            "an old reveal ticket cannot fail its replacement preflight"
        );

        let reveal_frame = DockLiveUndockPayloadPresentationReceipt::for_test(mount, 315)
            .expect("the test reveal frame is non-zero");
        let reveal = DockLiveUndockRevealReceipt::for_test(replacement, reveal_frame)
            .expect("the test reveal is exact");
        session.apply(DockLiveUndockFact::RevealObserved {
            identity,
            observation: DockLiveUndockRevealObservation::Visible(reveal),
        });
        assert!(
            session
                .apply(DockLiveUndockFact::PresentationStageFailed {
                    identity,
                    failure: DockLiveUndockPresentationFailure::ExactRevealTicket {
                        presentation: replacement,
                    },
                })
                .is_empty()
        );

        let placement = placement_generation(32);
        session.apply(DockLiveUndockFact::PlacementObserved {
            identity,
            window_id: window.window_id(),
            generation: placement,
            outcome: DockLiveUndockPlacementOutcome::Exact,
        });
        let (token, _) = prepare_token(session.apply(DockLiveUndockFact::ReleaseLocked {
            identity,
            release: desktop_release(placement),
        }));
        session.apply(DockLiveUndockFact::PromotionPrepared { identity, token });
        session.apply(DockLiveUndockFact::DurableSwapCommitted { identity, token });
        assert!(
            session
                .apply(DockLiveUndockFact::PresentationStageFailed {
                    identity,
                    failure: DockLiveUndockPresentationFailure::ExactRevealTicket {
                        presentation: replacement,
                    },
                })
                .is_empty(),
            "a presentation callback cannot roll back a durable destination"
        );
    }

    #[test]
    fn trigger_is_consumed_once_and_stale_generation_facts_are_inert() {
        let (_, lease) = active_window_session(101, 201);
        let mut session = DockLiveUndockSession::new();
        let first = start(&mut session, lease, 5);

        assert!(
            DockLiveUndockTrigger::new(
                drag_generation(6),
                DockLiveUndockSourceSnapshot::new(WindowId::from(9_006), 6),
                DockLiveUndockRouteFeedback::Host(DockLiveUndockHostTarget::new(
                    WindowId::from(400),
                    1,
                )),
            )
            .is_none()
        );

        assert!(
            session
                .apply(DockLiveUndockFact::Trigger {
                    lease,
                    trigger: DockLiveUndockTrigger::new(
                        drag_generation(5),
                        DockLiveUndockSourceSnapshot::new(WindowId::from(9_005), 5),
                        DockLiveUndockRouteFeedback::Desktop,
                    )
                    .expect("desktop is eligible"),
                })
                .is_empty()
        );
        assert!(
            session
                .apply(DockLiveUndockFact::Trigger {
                    lease,
                    trigger: DockLiveUndockTrigger::new(
                        drag_generation(4),
                        DockLiveUndockSourceSnapshot::new(WindowId::from(9_004), 4),
                        DockLiveUndockRouteFeedback::Desktop,
                    )
                    .expect("desktop is eligible"),
                })
                .is_empty()
        );
        let effects = session.apply(DockLiveUndockFact::OpeningFailed { identity: first });
        assert!(
            effects
                .as_slice()
                .iter()
                .any(|effect| matches!(effect, DockLiveUndockEffect::OpeningFailed { .. }))
        );
        assert!(
            !effects
                .as_slice()
                .iter()
                .any(|effect| matches!(effect, DockLiveUndockEffect::PublishTerminal { .. }))
        );
        let terminal = session.apply(DockLiveUndockFact::ReleaseLocked {
            identity: first,
            release: desktop_release(placement_generation(1)),
        });
        assert!(terminal.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                result: DockLiveUndockTerminalResult::Restored(
                    DockLiveUndockRestoreReason::ProvisionalTerminal
                ),
                ..
            }
        )));
        assert!(
            session
                .apply(DockLiveUndockFact::Trigger {
                    lease,
                    trigger: DockLiveUndockTrigger::new(
                        drag_generation(5),
                        DockLiveUndockSourceSnapshot::new(WindowId::from(9_005), 5),
                        DockLiveUndockRouteFeedback::Desktop,
                    )
                    .expect("desktop is eligible"),
                })
                .is_empty(),
            "opening failure must not re-arm the same drag generation"
        );

        let second = start(&mut session, lease, 6);
        assert!(
            session
                .apply(DockLiveUndockFact::RouteObserved {
                    identity: first,
                    route: DockLiveUndockRouteFeedback::Desktop,
                })
                .is_empty(),
            "a previous generation cannot mutate its replacement"
        );
        assert!(matches!(
            session
                .apply(DockLiveUndockFact::RouteObserved {
                    identity: second,
                    route: DockLiveUndockRouteFeedback::OpaqueBarrier,
                })
                .as_slice(),
            [DockLiveUndockEffect::RouteFeedbackChanged { .. }]
        ));
    }

    #[test]
    fn shutdown_race_retires_a_late_open_without_a_second_terminal() {
        let (_, lease) = active_window_session(102, 202);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let first = session.apply(DockLiveUndockFact::ShutdownRequested { lease });
        assert!(first.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                result: DockLiveUndockTerminalResult::Restored(
                    DockLiveUndockRestoreReason::Shutdown
                ),
                ..
            }
        )));
        let dependency = first
            .as_slice()
            .iter()
            .find_map(|effect| match effect {
                DockLiveUndockEffect::ShutdownFrozen(snapshot) => Some(snapshot.dependency()),
                _ => None,
            })
            .expect("shutdown must claim one exact retirement dependency");
        assert_eq!(session.phase(), DockLiveUndockPhase::Retiring);

        let repeated = session.apply(DockLiveUndockFact::ShutdownRequested { lease });
        assert!(
            repeated
                .as_slice()
                .iter()
                .any(|effect| matches!(effect, DockLiveUndockEffect::ShutdownFrozen(_)))
        );
        assert!(
            !repeated
                .as_slice()
                .iter()
                .any(|effect| matches!(effect, DockLiveUndockEffect::PublishTerminal { .. }))
        );

        let window = fake_window(302);
        let late = session.apply(DockLiveUndockFact::OpeningReturned {
            identity,
            window,
            binding: DockLiveUndockOpeningBinding::ExactGated,
            runtime: admitted_runtime(identity, window),
        });
        assert!(late.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ProvisionalRetirementRequired {
                window: Some(current),
                dependency: Some(_),
                reason: DockLiveUndockRetirementReason::Shutdown,
                ..
            } if *current == window
        )));
        assert!(
            !late
                .as_slice()
                .iter()
                .any(|effect| matches!(effect, DockLiveUndockEffect::PublishTerminal { .. }))
        );

        assert!(matches!(
            session
                .apply(DockLiveUndockFact::ShutdownDependencyTransferred {
                    identity,
                    dependency,
                })
                .as_slice(),
            [DockLiveUndockEffect::ShutdownDependencyTransferred {
                dependency: current,
                ..
            }] if *current == dependency
        ));
        assert!(
            session
                .apply(DockLiveUndockFact::ShutdownRequested { lease })
                .is_empty(),
            "a transferred shutdown dependency cannot be reclaimed"
        );
        assert!(matches!(
            session
                .apply(DockLiveUndockFact::WindowTerminal {
                    identity,
                    window_id: window.window_id(),
                })
                .as_slice(),
            [DockLiveUndockEffect::WindowTerminalSettled(outcome)]
                if outcome.dependency().is_none()
        ));
    }

    #[test]
    fn shutdown_snapshot_survives_until_source_restoration_acknowledges() {
        let (_, lease) = active_window_session(503, 603);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(703);
        admit(&mut session, identity, window);
        activate_payload(
            &mut session,
            identity,
            source_for(identity),
            lease_generation(32),
            window,
        );

        let effects = session.apply(DockLiveUndockFact::ShutdownRequested { lease });
        let snapshot = effects
            .as_slice()
            .iter()
            .find_map(|effect| match effect {
                DockLiveUndockEffect::ShutdownFrozen(snapshot) => Some(*snapshot),
                _ => None,
            })
            .expect("shutdown should freeze the exact live provisional");
        let (_, payload_lease, restore_focus) = restoration_request(&effects);
        assert!(!restore_focus);
        assert_eq!(session.phase(), DockLiveUndockPhase::Restoring);
        assert_eq!(session.current_identity(), Some(identity));
        assert_eq!(session.shutdown_snapshot(lease), Some(snapshot));
        assert!(!effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ProvisionalRetirementRequired { .. }
                | DockLiveUndockEffect::PublishTerminal { .. }
        )));

        let repeated = session.apply(DockLiveUndockFact::ShutdownRequested { lease });
        assert!(matches!(
            repeated.as_slice(),
            [
                DockLiveUndockEffect::ShutdownFrozen(current),
                DockLiveUndockEffect::ShutdownSourceRestorationRequired {
                    identity: current_identity,
                    source,
                    payload_lease: current_payload_lease,
                },
            ] if *current == snapshot
                && *current_identity == identity
                && *source == payload_lease.source()
                && *current_payload_lease == payload_lease
        ));
        let committed = acknowledge_source_restoration(&mut session, identity, payload_lease);
        assert!(
            !committed
                .as_slice()
                .iter()
                .any(|effect| matches!(effect, DockLiveUndockEffect::RestoreSourceFocus { .. }))
        );
        assert!(committed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ProvisionalRetirementRequired {
                dependency: Some(dependency),
                reason: DockLiveUndockRetirementReason::Shutdown,
                ..
            } if *dependency == snapshot.dependency()
        )));
        assert!(committed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                result: DockLiveUndockTerminalResult::Restored(
                    DockLiveUndockRestoreReason::Shutdown
                ),
                ..
            }
        )));
        assert_eq!(session.phase(), DockLiveUndockPhase::Retiring);
    }

    #[test]
    fn shutdown_after_failed_opening_claims_and_settles_the_exact_dependency() {
        let (_, lease) = active_window_session(504, 604);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);

        let failed = session.apply(DockLiveUndockFact::OpeningFailed { identity });
        assert!(failed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::OpeningFailed {
                identity: current,
                dependency: None,
            } if *current == identity
        )));

        let shutdown = session.apply(DockLiveUndockFact::ShutdownRequested { lease });
        assert!(matches!(
            shutdown.as_slice(),
            [
                DockLiveUndockEffect::ShutdownFrozen(snapshot),
                DockLiveUndockEffect::SettleShutdownDependency {
                    identity: current,
                    dependency,
                },
                DockLiveUndockEffect::PublishTerminal {
                    identity: terminal_identity,
                    result: DockLiveUndockTerminalResult::Restored(
                        DockLiveUndockRestoreReason::Shutdown
                    ),
                },
            ] if snapshot.identity() == identity
                && snapshot.window().is_none()
                && *current == identity
                && *terminal_identity == identity
                && *dependency == snapshot.dependency()
        ));
        assert_eq!(session.phase(), DockLiveUndockPhase::Idle);
        assert_eq!(session.shutdown_snapshot(lease), None);
    }

    #[test]
    fn shutdown_forces_terminal_source_restoration_and_settles_its_dependency() {
        let (_, lease) = active_window_session(505, 605);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(705);
        admit(&mut session, identity, window);
        activate_payload(
            &mut session,
            identity,
            source_for(identity),
            lease_generation(33),
            window,
        );

        let terminal = session.apply(DockLiveUndockFact::WindowTerminal {
            identity,
            window_id: window.window_id(),
        });
        assert!(terminal.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::RouteFeedbackChanged {
                identity: current,
                route: DockLiveUndockRouteFeedback::Unavailable,
            } if *current == identity
        )));
        assert!(
            !terminal
                .as_slice()
                .iter()
                .any(|effect| matches!(effect, DockLiveUndockEffect::RestoreSource { .. }))
        );

        let restore = session.apply(DockLiveUndockFact::Cancel {
            identity,
            reason: DockLiveUndockCancelReason::Escape,
        });
        let (_, payload_lease, _) = restoration_request(&restore);
        assert_eq!(session.phase(), DockLiveUndockPhase::Restoring);

        let shutdown = session.apply(DockLiveUndockFact::ShutdownRequested { lease });
        let snapshot = shutdown
            .as_slice()
            .iter()
            .find_map(|effect| match effect {
                DockLiveUndockEffect::ShutdownFrozen(snapshot) => Some(*snapshot),
                _ => None,
            })
            .expect("shutdown should freeze terminal source restoration");
        assert!(matches!(
            shutdown.as_slice(),
            [
                DockLiveUndockEffect::ShutdownFrozen(current),
                DockLiveUndockEffect::ShutdownSourceRestorationRequired {
                    identity: current_identity,
                    source,
                    payload_lease: current_payload_lease,
                },
            ] if *current == snapshot
                && *current_identity == identity
                && *source == payload_lease.source()
                && *current_payload_lease == payload_lease
        ));
        assert_eq!(snapshot.window(), None);
        assert_eq!(session.shutdown_snapshot(lease), Some(snapshot));

        let committed = acknowledge_source_restoration(&mut session, identity, payload_lease);
        assert!(matches!(
            committed.as_slice(),
            [
                DockLiveUndockEffect::SettleShutdownDependency {
                    identity: current,
                    dependency,
                },
                DockLiveUndockEffect::PublishTerminal {
                    identity: terminal_identity,
                    result: DockLiveUndockTerminalResult::Restored(
                        DockLiveUndockRestoreReason::Shutdown
                    ),
                },
            ] if *current == identity
                && *terminal_identity == identity
                && *dependency == snapshot.dependency()
        ));
        assert_eq!(session.phase(), DockLiveUndockPhase::Idle);
    }

    #[test]
    fn payload_mount_waits_for_the_exact_source_proxy_barrier() {
        let (_, lease) = active_window_session(103, 203);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(303);
        admit(&mut session, identity, window);
        let source = source_for(identity);
        let presentation_lease = lease_generation(2);
        let payload_lease = DockLiveUndockPayloadLeaseReceipt::for_test(
            identity,
            source,
            presentation_lease,
            window.window_id(),
        );
        let exact_proxy = DockLiveUndockSourceProxyReceipt::for_test(payload_lease, 42)
            .expect("the exact proxy frame is non-zero");
        let exact_mount = DockLiveUndockPayloadMountReceipt::for_test(exact_proxy, 43)
            .expect("the exact mount frame is non-zero");

        assert!(
            session
                .apply(DockLiveUndockFact::PayloadMounted {
                    identity,
                    receipt: exact_mount,
                })
                .is_empty()
        );
        let activation = session.apply(DockLiveUndockFact::PresentationLeaseActivated {
            identity,
            receipt: payload_lease,
        });
        assert!(matches!(
            activation.as_slice(),
            [DockLiveUndockEffect::CommitSourceProxy { .. }]
        ));
        assert!(
            session
                .apply(DockLiveUndockFact::PayloadMounted {
                    identity,
                    receipt: exact_mount,
                })
                .is_empty()
        );
        assert!(
            DockLiveUndockSourceProxyReceipt::for_test(payload_lease, 0).is_none(),
            "zero is not a committed GPUI source-proxy frame"
        );
        assert!(
            DockLiveUndockSourceProxyReceipt::for_test(payload_lease, source.scene_generation(),)
                .is_some(),
            "equal numeric values from Dock scene and GPUI frame domains remain independent"
        );
        let wrong_lease = DockLiveUndockPayloadLeaseReceipt::for_test(
            identity,
            source,
            lease_generation(3),
            window.window_id(),
        );
        assert!(
            session
                .apply(DockLiveUndockFact::SourceProxyCommitted {
                    identity,
                    receipt: DockLiveUndockSourceProxyReceipt::for_test(wrong_lease, 41)
                        .expect("the wrong exact lease remains a representable receipt"),
                })
                .is_empty(),
            "a different presentation lease cannot satisfy the proxy barrier"
        );
        let wrong_source_lease = DockLiveUndockPayloadLeaseReceipt::for_test(
            identity,
            DockLiveUndockSourceSnapshot::new(WindowId::from(99_999), 41),
            presentation_lease,
            window.window_id(),
        );
        assert!(
            session
                .apply(DockLiveUndockFact::SourceProxyCommitted {
                    identity,
                    receipt: DockLiveUndockSourceProxyReceipt::for_test(wrong_source_lease, 42)
                        .expect("a later frame from the wrong source remains representable"),
                })
                .is_empty(),
            "a different source window cannot satisfy the proxy barrier"
        );
        assert!(matches!(
            session
                .apply(DockLiveUndockFact::SourceProxyCommitted {
                    identity,
                    receipt: exact_proxy,
                })
                .as_slice(),
            [DockLiveUndockEffect::MountAndExposePayload { .. }]
        ));
        assert!(matches!(
            session
                .apply(DockLiveUndockFact::PayloadMounted {
                    identity,
                    receipt: exact_mount,
                })
                .as_slice(),
            [DockLiveUndockEffect::ObservePayloadPresentation { .. }]
        ));
    }

    #[test]
    fn desktop_release_before_readiness_waits_for_same_window_reveal_and_placement() {
        let (_, lease) = active_window_session(104, 204);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(304);
        admit(&mut session, identity, window);
        let placement = placement_generation(8);
        let release = desktop_release(placement);
        let effects = session.apply(DockLiveUndockFact::ReleaseLocked { identity, release });
        assert_release_placement_request(&effects, identity, window, release);
        assert!(
            session
                .apply(DockLiveUndockFact::ReleaseLocked { identity, release })
                .is_empty(),
            "the first release locks the exact hit and placement generation"
        );

        let source = source_for(identity);
        let mount = activate_payload(&mut session, identity, source, lease_generation(3), window);
        observe_nonempty_visible(&mut session, identity, mount, 92);
        let effects = session.apply(DockLiveUndockFact::PlacementObserved {
            identity,
            window_id: window.window_id(),
            generation: placement,
            outcome: DockLiveUndockPlacementOutcome::Adjusted,
        });
        assert!(effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PreparePromotion { release, .. }
                if release.point() == DockLiveUndockPhysicalPoint::new(800, 500)
                    && release.desired_bounds()
                        == DockLiveUndockPhysicalBounds::new(
                            DockLiveUndockPhysicalPoint::new(760, 470),
                            640,
                            480,
                        )
                        .expect("test release bounds are non-empty")
                    && release.placement_generation() == placement
        )));
        let (token, destination) = prepare_token(effects);
        assert_eq!(token.get(), 1);
        assert_eq!(
            destination,
            DockLiveUndockPromotionDestination::SameWindowDesktop {
                window_id: window.window_id()
            }
        );
    }

    #[test]
    fn stale_reveal_while_moving_retires_provisional_and_marks_route_unavailable() {
        let (_, lease) = active_window_session(104, 214);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(314);
        admit(&mut session, identity, window);
        let mount = activate_payload(
            &mut session,
            identity,
            source_for(identity),
            lease_generation(13),
            window,
        );
        let preflight = DockLiveUndockPayloadPresentationReceipt::for_test(mount, 92)
            .expect("the current reveal preflight must be non-zero");
        assert!(matches!(
            session
                .apply(DockLiveUndockFact::PayloadPresented {
                    identity,
                    receipt: preflight,
                })
                .as_slice(),
            [DockLiveUndockEffect::ArmExactReveal { .. }]
        ));
        let reveal_frame = DockLiveUndockPayloadPresentationReceipt::for_test(mount, 93)
            .expect("the stale reveal observation must retain an exact payload frame");

        let effects = session.apply(DockLiveUndockFact::RevealObserved {
            identity,
            observation: DockLiveUndockRevealObservation::failed(
                reveal_frame,
                DockLiveUndockRevealOutcome::Stale,
            ),
        });

        assert!(effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ProvisionalRetirementRequired {
                identity: current,
                reason: DockLiveUndockRetirementReason::PresentationUnavailable,
                ..
            } if *current == identity
        )));
        assert!(effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::RouteFeedbackChanged {
                identity: current,
                route: DockLiveUndockRouteFeedback::Unavailable,
            } if *current == identity
        )));
        assert_eq!(session.phase(), DockLiveUndockPhase::Bound);
    }

    #[test]
    fn stale_reveal_after_desktop_release_restores_the_source() {
        let (_, lease) = active_window_session(104, 215);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(315);
        admit(&mut session, identity, window);
        let mount = activate_payload(
            &mut session,
            identity,
            source_for(identity),
            lease_generation(14),
            window,
        );
        let preflight = DockLiveUndockPayloadPresentationReceipt::for_test(mount, 94)
            .expect("the current reveal preflight must be non-zero");
        session.apply(DockLiveUndockFact::PayloadPresented {
            identity,
            receipt: preflight,
        });
        let release = desktop_release(placement_generation(18));
        assert_release_placement_request(
            &session.apply(DockLiveUndockFact::ReleaseLocked { identity, release }),
            identity,
            window,
            release,
        );
        let reveal_frame = DockLiveUndockPayloadPresentationReceipt::for_test(mount, 95)
            .expect("the stale reveal observation must retain an exact payload frame");

        let effects = session.apply(DockLiveUndockFact::RevealObserved {
            identity,
            observation: DockLiveUndockRevealObservation::failed(
                reveal_frame,
                DockLiveUndockRevealOutcome::Stale,
            ),
        });

        assert!(effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::RestoreSource {
                identity: current,
                restore_focus: true,
                ..
            } if *current == identity
        )));
        assert_eq!(session.phase(), DockLiveUndockPhase::Restoring);
    }

    #[test]
    fn release_before_open_return_requests_exact_placement_once_on_admission() {
        let (_, lease) = active_window_session(104, 205);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(305);
        let release = desktop_release(placement_generation(9));

        assert!(matches!(
            session
                .apply(DockLiveUndockFact::ReleaseLocked { identity, release })
                .as_slice(),
            [DockLiveUndockEffect::RetireSourceTransportProxy {
                identity: current,
            }] if *current == identity
        ));
        assert!(
            session
                .apply(DockLiveUndockFact::ReleaseLocked { identity, release })
                .is_empty(),
            "replayed release facts must remain inert"
        );

        let effects = session.apply(DockLiveUndockFact::OpeningReturned {
            identity,
            window,
            binding: DockLiveUndockOpeningBinding::ExactGated,
            runtime: admitted_runtime(identity, window),
        });
        assert!(matches!(
            effects.as_slice(),
            [
                DockLiveUndockEffect::ProvisionalAdmitted {
                    identity: admitted_identity,
                    window: admitted_window,
                    ..
                },
                DockLiveUndockEffect::RequestReleasePlacement {
                    identity: placement_identity,
                    window: placement_window,
                    release: requested_release,
                },
            ] if *admitted_identity == identity
                && *admitted_window == window
                && *placement_identity == identity
                && *placement_window == window
                && *requested_release == release
        ));
        assert!(
            session
                .apply(DockLiveUndockFact::OpeningReturned {
                    identity,
                    window,
                    binding: DockLiveUndockOpeningBinding::ExactGated,
                    runtime: admitted_runtime(identity, window),
                })
                .is_empty(),
            "an opening return cannot issue the placement twice"
        );
    }

    #[test]
    fn rejected_placement_and_release_timeout_restore_the_source() {
        let (_, lease) = active_window_session(105, 205);
        let mut rejected = DockLiveUndockSession::new();
        let rejected_identity = start(&mut rejected, lease, 1);
        let rejected_window = fake_window(305);
        admit(&mut rejected, rejected_identity, rejected_window);
        let rejected_mount = activate_payload(
            &mut rejected,
            rejected_identity,
            source_for(rejected_identity),
            lease_generation(4),
            rejected_window,
        );
        observe_nonempty_visible(&mut rejected, rejected_identity, rejected_mount, 93);
        let placement = placement_generation(9);
        let release = desktop_release(placement);
        let effects = rejected.apply(DockLiveUndockFact::ReleaseLocked {
            identity: rejected_identity,
            release,
        });
        assert_release_placement_request(&effects, rejected_identity, rejected_window, release);
        let effects = rejected.apply(DockLiveUndockFact::PlacementObserved {
            identity: rejected_identity,
            window_id: rejected_window.window_id(),
            generation: placement,
            outcome: DockLiveUndockPlacementOutcome::Rejected,
        });
        let (_, rejected_lease, _) = restoration_request(&effects);
        assert!(!effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ProvisionalRetirementRequired { .. }
                | DockLiveUndockEffect::PublishTerminal { .. }
        )));
        let committed =
            acknowledge_source_restoration(&mut rejected, rejected_identity, rejected_lease);
        assert!(committed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                result: DockLiveUndockTerminalResult::Restored(
                    DockLiveUndockRestoreReason::PlacementFailed(
                        DockLiveUndockPlacementOutcome::Rejected
                    )
                ),
                ..
            }
        )));

        let mut timed_out = DockLiveUndockSession::new();
        let timeout_identity = start(&mut timed_out, lease, 2);
        let timeout_window = fake_window(306);
        admit(&mut timed_out, timeout_identity, timeout_window);
        activate_payload(
            &mut timed_out,
            timeout_identity,
            source_for(timeout_identity),
            lease_generation(5),
            timeout_window,
        );
        let timeout_placement = placement_generation(10);
        let timeout_release = desktop_release(timeout_placement);
        let effects = timed_out.apply(DockLiveUndockFact::ReleaseLocked {
            identity: timeout_identity,
            release: timeout_release,
        });
        assert_release_placement_request(
            &effects,
            timeout_identity,
            timeout_window,
            timeout_release,
        );
        let effects = timed_out.apply(DockLiveUndockFact::ReleaseDeadlineExpired {
            identity: timeout_identity,
            placement_generation: timeout_placement,
        });
        let (_, timeout_lease, _) = restoration_request(&effects);
        assert!(!effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal { .. }
                | DockLiveUndockEffect::ProvisionalRetirementRequired { .. }
        )));
        let committed =
            acknowledge_source_restoration(&mut timed_out, timeout_identity, timeout_lease);
        assert!(committed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                result: DockLiveUndockTerminalResult::Restored(
                    DockLiveUndockRestoreReason::ReleaseDeadlineExpired
                ),
                ..
            }
        )));
    }

    #[test]
    fn promotion_preflight_failure_restores_before_any_durable_swap() {
        let (_, lease) = active_window_session(106, 206);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(307);
        admit(&mut session, identity, window);
        let token = prepare_desktop(&mut session, identity, window, placement_generation(11));
        let effects =
            session.apply(DockLiveUndockFact::PromotionPreparationFailed { identity, token });
        let (_, payload_lease, _) = restoration_request(&effects);
        assert!(!effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal { .. }
                | DockLiveUndockEffect::ProvisionalRetirementRequired { .. }
        )));
        let committed = acknowledge_source_restoration(&mut session, identity, payload_lease);
        assert!(committed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                result: DockLiveUndockTerminalResult::Restored(
                    DockLiveUndockRestoreReason::PromotionPreparationFailed
                ),
                ..
            }
        )));
        assert!(!effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::CommitPreparedPromotion { .. }
                | DockLiveUndockEffect::DestinationSemanticsCommitRequired { .. }
        )));
    }

    #[test]
    fn promotion_commit_preflight_failure_restores_before_any_durable_swap() {
        let (_, lease) = active_window_session(113, 213);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(313);
        admit(&mut session, identity, window);
        let token = prepare_desktop(&mut session, identity, window, placement_generation(19));
        assert!(matches!(
            session
                .apply(DockLiveUndockFact::PromotionPrepared { identity, token })
                .as_slice(),
            [DockLiveUndockEffect::CommitPreparedPromotion { .. }]
        ));

        let effects =
            session.apply(DockLiveUndockFact::PromotionPreparationFailed { identity, token });
        let (_, payload_lease, _) = restoration_request(&effects);
        assert!(!effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal { .. }
                | DockLiveUndockEffect::ProvisionalRetirementRequired { .. }
                | DockLiveUndockEffect::DestinationSemanticsCommitRequired { .. }
        )));
        let committed = acknowledge_source_restoration(&mut session, identity, payload_lease);
        assert!(committed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                result: DockLiveUndockTerminalResult::Restored(
                    DockLiveUndockRestoreReason::PromotionPreparationFailed
                ),
                ..
            }
        )));
    }

    #[test]
    fn destination_terminal_before_durable_swap_restores_without_committing() {
        let (_, lease) = active_window_session(112, 212);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(312);
        admit(&mut session, identity, window);
        let token = prepare_desktop(&mut session, identity, window, placement_generation(16));
        assert!(matches!(
            session
                .apply(DockLiveUndockFact::PromotionPrepared { identity, token })
                .as_slice(),
            [DockLiveUndockEffect::CommitPreparedPromotion { .. }]
        ));

        let effects = session.apply(DockLiveUndockFact::DestinationTerminal {
            identity,
            window_id: window.window_id(),
        });
        let (_, payload_lease, _) = restoration_request(&effects);
        assert!(!effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal { .. }
                | DockLiveUndockEffect::ProvisionalRetirementRequired { .. }
        )));
        let committed = acknowledge_source_restoration(&mut session, identity, payload_lease);
        assert!(committed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                result: DockLiveUndockTerminalResult::Restored(
                    DockLiveUndockRestoreReason::DestinationTerminalBeforeCommit
                ),
                ..
            }
        )));
        assert!(!effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::RecoverCommittedDestinationTopology { .. }
                | DockLiveUndockEffect::DestinationSemanticsCommitRequired { .. }
        )));
    }

    #[test]
    fn source_or_payload_terminal_after_release_aborts_until_the_swap_is_durable() {
        let (_, lease) = active_window_session(113, 213);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(313);
        admit(&mut session, identity, window);
        let _token = prepare_desktop(&mut session, identity, window, placement_generation(17));

        let effects = session.apply(DockLiveUndockFact::Cancel {
            identity,
            reason: DockLiveUndockCancelReason::PayloadClosed,
        });
        let (_, payload_lease, _) = restoration_request(&effects);
        assert!(!effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal { .. }
                | DockLiveUndockEffect::ProvisionalRetirementRequired { .. }
        )));
        let committed = acknowledge_source_restoration(&mut session, identity, payload_lease);
        assert!(committed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                result: DockLiveUndockTerminalResult::Restored(
                    DockLiveUndockRestoreReason::Cancelled(
                        DockLiveUndockCancelReason::PayloadClosed
                    )
                ),
                ..
            }
        )));
    }

    #[test]
    fn destination_terminal_after_durable_swap_never_rolls_back_the_drag() {
        let (_, lease) = active_window_session(107, 207);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(308);
        admit(&mut session, identity, window);
        let token = prepare_desktop(&mut session, identity, window, placement_generation(12));
        assert!(matches!(
            session
                .apply(DockLiveUndockFact::PromotionPrepared { identity, token })
                .as_slice(),
            [DockLiveUndockEffect::CommitPreparedPromotion { .. }]
        ));
        assert!(matches!(
            session
                .apply(DockLiveUndockFact::DurableSwapCommitted { identity, token })
                .as_slice(),
            [DockLiveUndockEffect::DestinationSemanticsCommitRequired { .. }]
        ));

        let durable_lease = DockLiveUndockPayloadLeaseReceipt::for_test(
            identity,
            source_for(identity),
            lease_generation(3),
            window.window_id(),
        );
        let stale_restoration =
            DockLiveUndockSourceRestorationReceipt::source_presented_after_release_for_test(
                durable_lease,
                9_201,
            )
            .expect("late restoration evidence remains representable");
        assert!(
            session
                .apply(DockLiveUndockFact::SourceRestorationCommitted {
                    identity,
                    receipt: stale_restoration,
                })
                .is_empty(),
            "restoration acknowledgement cannot roll back a durable destination"
        );
        assert!(
            session
                .apply(DockLiveUndockFact::SourceRestorationDeferred {
                    identity,
                    source: source_for(identity),
                    payload_lease: durable_lease,
                })
                .is_empty(),
            "restoration failure cannot roll back a durable destination"
        );

        let effects = session.apply(DockLiveUndockFact::WindowTerminal {
            identity,
            window_id: window.window_id(),
        });
        let (authority, recovery_token, destination) =
            committed_destination_recovery_request(&effects);
        assert_eq!(recovery_token, token);
        assert_eq!(destination.window_id(), window.window_id());
        assert!(matches!(
            effects.as_slice(),
            [
                DockLiveUndockEffect::WindowTerminalSettled(_),
                DockLiveUndockEffect::RecoverCommittedDestinationTopology { .. }
            ]
        ));
        assert!(
            !effects
                .as_slice()
                .iter()
                .any(|effect| matches!(effect, DockLiveUndockEffect::RestoreSource { .. }))
        );
        assert_eq!(
            session.phase(),
            DockLiveUndockPhase::RecoveringCommittedDestination
        );
        let semantics = semantics_receipt(
            identity,
            token,
            DockLiveUndockPromotionDestination::SameWindowDesktop {
                window_id: window.window_id(),
            },
        );
        assert!(
            session
                .apply(DockLiveUndockFact::DestinationSemanticsCommitted {
                    identity,
                    receipt: semantics,
                })
                .is_empty()
        );

        let stale_token = DockLiveUndockPromotionToken::new(token.get() + 1)
            .expect("the stale recovery token remains non-zero");
        assert!(
            session
                .apply(DockLiveUndockFact::CommittedDestinationRecoveryCommitted {
                    identity,
                    receipt: committed_destination_recovery(
                        DockPayloadRecoveryAuthority::durable_promotion(
                            identity,
                            stale_token,
                            destination,
                        ),
                    ),
                },)
                .is_empty(),
            "a recovery receipt for another promotion cannot retire this destination"
        );

        let recovery = committed_destination_recovery(authority);
        assert!(matches!(
            session
                .apply(
                    DockLiveUndockFact::CommittedDestinationRecoveryCommitted {
                        identity,
                        receipt: recovery,
                    },
                )
                .as_slice(),
            [DockLiveUndockEffect::PublishTerminal {
                identity: current,
                result: DockLiveUndockTerminalResult::DestinationLostAfterCommit(
                    DockLiveUndockPromotionDestination::SameWindowDesktop { .. }
                ),
            }] if *current == identity
        ));
        assert!(
            session
                .apply(DockLiveUndockFact::CommittedDestinationRecoveryCommitted {
                    identity,
                    receipt: recovery,
                },)
                .is_empty(),
            "a duplicate recovery receipt cannot publish another terminal result"
        );
        assert_eq!(session.phase(), DockLiveUndockPhase::Idle);
    }

    #[test]
    fn same_hwnd_promotion_ungates_only_after_destination_interaction_admission() {
        let (_, lease) = active_window_session(108, 208);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(309);
        admit(&mut session, identity, window);
        let token = prepare_desktop(&mut session, identity, window, placement_generation(13));
        let stale_token = DockLiveUndockPromotionToken::new(token.get() + 1)
            .expect("the stale test token remains non-zero");
        let destination = DockLiveUndockPromotionDestination::SameWindowDesktop {
            window_id: window.window_id(),
        };
        let semantics = semantics_receipt(identity, token, destination);
        let stale_semantics = semantics_receipt(identity, stale_token, destination);
        let interaction = interaction_receipt(semantics);
        assert!(
            session
                .apply(DockLiveUndockFact::PromotionPrepared {
                    identity,
                    token: stale_token,
                })
                .is_empty()
        );
        session.apply(DockLiveUndockFact::PromotionPrepared { identity, token });
        session.apply(DockLiveUndockFact::DurableSwapCommitted { identity, token });
        assert!(
            session
                .apply(DockLiveUndockFact::DestinationSemanticsCommitted {
                    identity,
                    receipt: stale_semantics,
                })
                .is_empty()
        );

        let effects = session.apply(DockLiveUndockFact::DestinationSemanticsCommitted {
            identity,
            receipt: semantics,
        });
        assert!(matches!(
            effects.as_slice(),
            [DockLiveUndockEffect::DestinationInteractionAdmissionRequired {
                identity: current,
                semantics: current_semantics,
            }] if *current == identity && *current_semantics == semantics
        ));
        assert_eq!(session.phase(), DockLiveUndockPhase::Bound);
        assert!(
            session
                .apply(DockLiveUndockFact::DestinationInteractionAdmitted {
                    identity,
                    receipt: interaction_receipt(stale_semantics),
                })
                .is_empty(),
            "a stale admission cannot open the interaction gate"
        );
        assert!(
            session
                .apply(DockLiveUndockFact::DestinationSemanticsCommitted {
                    identity,
                    receipt: semantics,
                })
                .is_empty(),
            "a duplicate semantics acknowledgement cannot complete promotion"
        );

        let effects = session.apply(DockLiveUndockFact::DestinationInteractionAdmitted {
            identity,
            receipt: interaction,
        });
        assert!(matches!(
            effects.as_slice(),
            [
                DockLiveUndockEffect::DestinationInteractionReady {
                    identity: current,
                    interaction: current_interaction,
                    destination: DockLiveUndockPromotionDestination::SameWindowDesktop { .. },
                },
                DockLiveUndockEffect::PublishTerminal {
                    result: DockLiveUndockTerminalResult::Committed(
                        DockLiveUndockPromotionDestination::SameWindowDesktop { .. }
                    ),
                    ..
                }
            ] if *current == identity && *current_interaction == interaction
        ));
        assert!(!effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ProvisionalRetirementRequired { .. }
        )));
        assert_eq!(session.phase(), DockLiveUndockPhase::Idle);
        assert!(
            session
                .apply(DockLiveUndockFact::DestinationInteractionAdmitted {
                    identity,
                    receipt: interaction,
                })
                .is_empty(),
            "a duplicate admission cannot publish another terminal"
        );
    }

    #[test]
    fn destination_semantics_commit_failure_enters_committed_destination_recovery() {
        let (_, lease) = active_window_session(126, 226);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(326);
        admit(&mut session, identity, window);
        let token = prepare_desktop(&mut session, identity, window, placement_generation(29));
        let destination = DockLiveUndockPromotionDestination::SameWindowDesktop {
            window_id: window.window_id(),
        };
        session.apply(DockLiveUndockFact::PromotionPrepared { identity, token });
        session.apply(DockLiveUndockFact::DurableSwapCommitted { identity, token });

        let stale_token = DockLiveUndockPromotionToken::new(token.get() + 1)
            .expect("the stale test token remains non-zero");
        assert!(
            session
                .apply(DockLiveUndockFact::DestinationSemanticsCommitFailed {
                    identity,
                    token: stale_token,
                    destination,
                })
                .is_empty(),
            "a stale failure cannot recover the current durable destination"
        );

        let effects = session.apply(DockLiveUndockFact::DestinationSemanticsCommitFailed {
            identity,
            token,
            destination,
        });
        assert!(matches!(
            effects.as_slice(),
            [DockLiveUndockEffect::RecoverCommittedDestinationTopology { .. }]
        ));
        assert_eq!(
            session.phase(),
            DockLiveUndockPhase::RecoveringCommittedDestination
        );
    }

    #[test]
    fn destination_interaction_admission_failure_never_publishes_committed() {
        let (_, lease) = active_window_session(116, 216);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(316);
        admit(&mut session, identity, window);
        let token = prepare_desktop(&mut session, identity, window, placement_generation(19));
        let stale_token = DockLiveUndockPromotionToken::new(token.get() + 1)
            .expect("the stale test token remains non-zero");
        let destination = DockLiveUndockPromotionDestination::SameWindowDesktop {
            window_id: window.window_id(),
        };
        let semantics = semantics_receipt(identity, token, destination);
        let stale_semantics = semantics_receipt(identity, stale_token, destination);
        session.apply(DockLiveUndockFact::PromotionPrepared { identity, token });
        session.apply(DockLiveUndockFact::DurableSwapCommitted { identity, token });
        assert!(matches!(
            session
                .apply(DockLiveUndockFact::DestinationSemanticsCommitted {
                    identity,
                    receipt: semantics,
                })
                .as_slice(),
            [DockLiveUndockEffect::DestinationInteractionAdmissionRequired { .. }]
        ));
        assert!(
            session
                .apply(DockLiveUndockFact::DestinationInteractionAdmissionFailed {
                    identity,
                    semantics: stale_semantics,
                })
                .is_empty()
        );

        let effects = session.apply(DockLiveUndockFact::DestinationInteractionAdmissionFailed {
            identity,
            semantics,
        });
        assert!(matches!(
            effects.as_slice(),
            [DockLiveUndockEffect::RecoverCommittedDestinationTopology { .. }]
        ));
        assert!(!effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::DestinationInteractionReady { .. }
                | DockLiveUndockEffect::PublishTerminal {
                    result: DockLiveUndockTerminalResult::Committed(_),
                    ..
                }
        )));
        assert_eq!(
            session.phase(),
            DockLiveUndockPhase::RecoveringCommittedDestination
        );

        let (authority, recovery_token, recovery_destination) =
            committed_destination_recovery_request(&effects);
        assert_eq!(recovery_token, token);
        assert_eq!(recovery_destination, destination);
        let committed = session.apply(DockLiveUndockFact::CommittedDestinationRecoveryCommitted {
            identity,
            receipt: committed_destination_recovery(authority),
        });
        assert!(matches!(
            committed.as_slice(),
            [DockLiveUndockEffect::RetireCommittedSameWindowDestination {
                identity: current,
                token: current_token,
                window_id,
            }] if *current == identity
                && *current_token == token
                && *window_id == window.window_id()
        ));
        assert_eq!(
            session.phase(),
            DockLiveUndockPhase::RecoveringCommittedDestination
        );

        let terminal = session.apply(DockLiveUndockFact::WindowTerminal {
            identity,
            window_id: window.window_id(),
        });
        assert!(terminal.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                result: DockLiveUndockTerminalResult::DestinationLostAfterCommit(
                    DockLiveUndockPromotionDestination::SameWindowDesktop { .. }
                ),
                ..
            }
        )));
        assert_eq!(session.phase(), DockLiveUndockPhase::Idle);
    }

    #[test]
    fn shutdown_committed_same_window_recovery_keeps_dependency_until_cleanup_receipt() {
        let (_, lease) = active_window_session(117, 217);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(317);
        admit(&mut session, identity, window);
        let token = prepare_desktop(&mut session, identity, window, placement_generation(20));
        let destination = DockLiveUndockPromotionDestination::SameWindowDesktop {
            window_id: window.window_id(),
        };
        session.apply(DockLiveUndockFact::PromotionPrepared { identity, token });
        session.apply(DockLiveUndockFact::DurableSwapCommitted { identity, token });

        let shutdown = session.apply(DockLiveUndockFact::ShutdownRequested { lease });
        let dependency = shutdown
            .as_slice()
            .iter()
            .find_map(|effect| match effect {
                DockLiveUndockEffect::ShutdownFrozen(snapshot) => Some(snapshot.dependency()),
                _ => None,
            })
            .expect("shutdown must claim committed-destination cleanup authority");
        let (authority, current_token, current_destination) = shutdown
            .as_slice()
            .iter()
            .find_map(|effect| match effect {
                DockLiveUndockEffect::ShutdownCommittedDestinationRecoveryRequired {
                    authority,
                    token,
                    destination,
                    ..
                } => Some((*authority, *token, *destination)),
                _ => None,
            })
            .expect("shutdown must use the dedicated committed-recovery executor");
        assert_eq!(current_token, token);
        assert_eq!(current_destination, destination);
        assert_eq!(
            session.phase(),
            DockLiveUndockPhase::RecoveringCommittedDestination
        );

        let terminal = session.apply(DockLiveUndockFact::WindowTerminal {
            identity,
            window_id: window.window_id(),
        });
        assert!(matches!(
            terminal.as_slice(),
            [DockLiveUndockEffect::WindowTerminalSettled(outcome)]
                if outcome.dependency().is_none()
        ));
        assert_eq!(
            session
                .shutdown_snapshot(lease)
                .map(|snapshot| snapshot.dependency()),
            Some(dependency)
        );

        let completed = session.apply(DockLiveUndockFact::CommittedDestinationRecoveryCommitted {
            identity,
            receipt: committed_destination_recovery(authority),
        });
        assert!(completed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::SettleShutdownDependency {
                identity: current,
                dependency: current_dependency,
            } if *current == identity && *current_dependency == dependency
        )));
        assert!(completed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                identity: current,
                result: DockLiveUndockTerminalResult::DestinationLostAfterCommit(
                    DockLiveUndockPromotionDestination::SameWindowDesktop { .. }
                ),
            } if *current == identity
        )));
        assert_eq!(session.phase(), DockLiveUndockPhase::Idle);
    }

    #[test]
    fn shutdown_committed_destination_recovery_failure_finishes_with_typed_failure_terminal() {
        let (_, lease) = active_window_session(130, 230);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(430);
        admit(&mut session, identity, window);
        let token = prepare_desktop(&mut session, identity, window, placement_generation(32));
        let destination = DockLiveUndockPromotionDestination::SameWindowDesktop {
            window_id: window.window_id(),
        };
        session.apply(DockLiveUndockFact::PromotionPrepared { identity, token });
        session.apply(DockLiveUndockFact::DurableSwapCommitted { identity, token });

        let shutdown = session.apply(DockLiveUndockFact::ShutdownRequested { lease });
        let dependency = shutdown
            .as_slice()
            .iter()
            .find_map(|effect| match effect {
                DockLiveUndockEffect::ShutdownFrozen(snapshot) => Some(snapshot.dependency()),
                _ => None,
            })
            .expect("shutdown must retain exact committed-recovery cleanup authority");
        let authority =
            DockPayloadRecoveryAuthority::durable_promotion(identity, token, destination);

        let failure = DockLiveUndockCommittedDestinationRecoveryFailure::PreparationRejected;
        let failed = session.apply(
            DockLiveUndockFact::ShutdownCommittedDestinationRecoveryFailed {
                identity,
                authority,
                token,
                destination,
                failure,
            },
        );
        let publish_index = failed
            .as_slice()
            .iter()
            .position(|effect| {
                matches!(
                    effect,
                    DockLiveUndockEffect::PublishTerminal {
                        identity: current_identity,
                        result: DockLiveUndockTerminalResult::ShutdownCleanupFailed(
                            DockLiveUndockShutdownFailure::CommittedDestinationRecovery(
                                current_failure,
                            ),
                        ),
                    } if *current_identity == identity && *current_failure == failure
                )
            })
            .expect("shutdown failure must publish a typed terminal result");
        let fail_dependency_index = failed
            .as_slice()
            .iter()
            .position(|effect| {
                matches!(
                    effect,
                    DockLiveUndockEffect::FailShutdownDependency {
                        identity: current_identity,
                        dependency: current_dependency,
                        failure: DockLiveUndockShutdownFailure::CommittedDestinationRecovery(
                            current_failure,
                        ),
                    } if *current_identity == identity
                        && *current_dependency == dependency
                        && *current_failure == failure
                )
            })
            .expect("shutdown failure must terminate its exact dependency");
        assert!(publish_index < fail_dependency_index);
        assert!(!failed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::SettleShutdownDependency { .. }
        )));
        assert_eq!(session.phase(), DockLiveUndockPhase::ShutdownCleanupFailed);
        assert!(session.shutdown_snapshot(lease).is_none());

        assert!(
            session
                .apply(DockLiveUndockFact::ShutdownRequested { lease })
                .is_empty(),
            "a terminal committed-recovery failure must not re-arm recovery or publish success"
        );
        assert_eq!(session.phase(), DockLiveUndockPhase::ShutdownCleanupFailed);
    }

    #[test]
    fn host_destination_recovery_receipt_is_the_terminal_cleanup_authority() {
        let (_, lease) = active_window_session(129, 229);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        session.apply(DockLiveUndockFact::OpeningFailed { identity });
        let target = DockLiveUndockHostTarget::new(WindowId::from(429), 31);
        let release = DockLiveUndockReleaseLock::new(
            DockLiveUndockPhysicalPoint::new(430, 330),
            DockLiveUndockRouteFeedback::Host(target),
            DockLiveUndockPhysicalBounds::new(DockLiveUndockPhysicalPoint::new(390, 300), 640, 480)
                .expect("test release bounds are non-empty"),
            placement_generation(31),
        );
        let (token, destination) =
            prepare_token(session.apply(DockLiveUndockFact::ReleaseLocked { identity, release }));
        session.apply(DockLiveUndockFact::PromotionPrepared { identity, token });
        session.apply(DockLiveUndockFact::DurableSwapCommitted { identity, token });
        let semantics = semantics_receipt(identity, token, destination);
        session.apply(DockLiveUndockFact::DestinationSemanticsCommitted {
            identity,
            receipt: semantics,
        });
        let recovery = session.apply(DockLiveUndockFact::DestinationInteractionAdmissionFailed {
            identity,
            semantics,
        });
        let (authority, recovery_token, recovery_destination) =
            committed_destination_recovery_request(&recovery);
        assert_eq!(recovery_token, token);
        assert_eq!(recovery_destination, destination);

        let terminal = session.apply(DockLiveUndockFact::CommittedDestinationRecoveryCommitted {
            identity,
            receipt: committed_destination_recovery(authority),
        });
        assert!(terminal.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                identity: current,
                result: DockLiveUndockTerminalResult::DestinationLostAfterCommit(
                    DockLiveUndockPromotionDestination::Host(current_target)
                ),
            } if *current == identity && *current_target == target
        )));
        assert_eq!(session.phase(), DockLiveUndockPhase::Idle);
    }

    #[test]
    fn host_return_can_commit_before_the_unseen_provisional_finishes_opening() {
        let (_, lease) = active_window_session(109, 209);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let target = DockLiveUndockHostTarget::new(WindowId::from(409), 22);
        let release = DockLiveUndockReleaseLock::new(
            DockLiveUndockPhysicalPoint::new(400, 300),
            DockLiveUndockRouteFeedback::Host(target),
            DockLiveUndockPhysicalBounds::new(DockLiveUndockPhysicalPoint::new(360, 270), 640, 480)
                .expect("test release bounds are non-empty"),
            placement_generation(14),
        );
        let (token, destination) =
            prepare_token(session.apply(DockLiveUndockFact::ReleaseLocked { identity, release }));
        assert_eq!(
            destination,
            DockLiveUndockPromotionDestination::Host(target)
        );
        session.apply(DockLiveUndockFact::PromotionPrepared { identity, token });
        assert!(matches!(
            session
                .apply(DockLiveUndockFact::DurableSwapCommitted { identity, token })
                .as_slice(),
            [
                DockLiveUndockEffect::ApplyCommittedHostWindowEffects {
                    identity: current_identity,
                    token: current_token,
                    destination: current_destination,
                },
                DockLiveUndockEffect::DestinationSemanticsCommitRequired {
                    identity: semantics_identity,
                    token: semantics_token,
                    destination: semantics_destination,
                },
            ] if *current_identity == identity
                && *current_token == token
                && *current_destination == destination
                && *semantics_identity == identity
                && *semantics_token == token
                && *semantics_destination == destination
        ));
        let semantics = semantics_receipt(identity, token, destination);
        let semantics_effects = session.apply(DockLiveUndockFact::DestinationSemanticsCommitted {
            identity,
            receipt: semantics,
        });
        assert!(matches!(
            semantics_effects.as_slice(),
            [DockLiveUndockEffect::DestinationInteractionAdmissionRequired { .. }]
        ));
        let effects = session.apply(DockLiveUndockFact::DestinationInteractionAdmitted {
            identity,
            receipt: interaction_receipt(semantics),
        });
        assert!(effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ProvisionalRetirementRequired {
                window: None,
                reason: DockLiveUndockRetirementReason::HostCommitted,
                ..
            }
        )));
        assert!(effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                result: DockLiveUndockTerminalResult::Committed(
                    DockLiveUndockPromotionDestination::Host(current)
                ),
                ..
            } if *current == target
        )));
        assert_eq!(session.phase(), DockLiveUndockPhase::Retiring);

        let late_window = fake_window(310);
        let late = session.apply(DockLiveUndockFact::OpeningReturned {
            identity,
            window: late_window,
            binding: DockLiveUndockOpeningBinding::ExactGated,
            runtime: admitted_runtime(identity, late_window),
        });
        assert!(late.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ProvisionalRetirementRequired {
                window: Some(current),
                reason: DockLiveUndockRetirementReason::HostCommitted,
                ..
            } if *current == late_window
        )));
        assert!(
            !late
                .as_slice()
                .iter()
                .any(|effect| matches!(effect, DockLiveUndockEffect::PublishTerminal { .. }))
        );
    }

    #[test]
    fn host_release_can_commit_after_the_provisional_opening_becomes_unavailable() {
        let (_, lease) = active_window_session(111, 211);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let unavailable = session.apply(DockLiveUndockFact::OpeningFailed { identity });
        assert!(unavailable.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::RouteFeedbackChanged {
                route: DockLiveUndockRouteFeedback::Unavailable,
                ..
            }
        )));
        assert!(
            !unavailable
                .as_slice()
                .iter()
                .any(|effect| matches!(effect, DockLiveUndockEffect::PublishTerminal { .. }))
        );

        assert!(
            session
                .apply(DockLiveUndockFact::RouteObserved {
                    identity,
                    route: DockLiveUndockRouteFeedback::Desktop,
                })
                .is_empty(),
            "an unavailable provisional cannot publish a usable desktop route again"
        );

        let target = DockLiveUndockHostTarget::new(WindowId::from(411), 23);
        assert!(matches!(
            session
                .apply(DockLiveUndockFact::RouteObserved {
                    identity,
                    route: DockLiveUndockRouteFeedback::Host(target),
                })
                .as_slice(),
            [DockLiveUndockEffect::RouteFeedbackChanged {
                route: DockLiveUndockRouteFeedback::Host(current),
                ..
            }] if *current == target
        ));
        let release = DockLiveUndockReleaseLock::new(
            DockLiveUndockPhysicalPoint::new(410, 310),
            DockLiveUndockRouteFeedback::Host(target),
            DockLiveUndockPhysicalBounds::new(DockLiveUndockPhysicalPoint::new(370, 280), 640, 480)
                .expect("test release bounds are non-empty"),
            placement_generation(15),
        );
        let (token, destination) =
            prepare_token(session.apply(DockLiveUndockFact::ReleaseLocked { identity, release }));
        assert_eq!(
            destination,
            DockLiveUndockPromotionDestination::Host(target)
        );
        session.apply(DockLiveUndockFact::PromotionPrepared { identity, token });
        session.apply(DockLiveUndockFact::DurableSwapCommitted { identity, token });
        let semantics = semantics_receipt(identity, token, destination);
        let semantics_effects = session.apply(DockLiveUndockFact::DestinationSemanticsCommitted {
            identity,
            receipt: semantics,
        });
        assert!(matches!(
            semantics_effects.as_slice(),
            [DockLiveUndockEffect::DestinationInteractionAdmissionRequired { .. }]
        ));
        let effects = session.apply(DockLiveUndockFact::DestinationInteractionAdmitted {
            identity,
            receipt: interaction_receipt(semantics),
        });
        assert!(effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                result: DockLiveUndockTerminalResult::Committed(
                    DockLiveUndockPromotionDestination::Host(current)
                ),
                ..
            } if *current == target
        )));
        assert!(
            !effects
                .as_slice()
                .iter()
                .any(|effect| matches!(effect, DockLiveUndockEffect::RestoreSource { .. }))
        );
        assert_eq!(session.phase(), DockLiveUndockPhase::Idle);
    }

    #[test]
    fn host_durable_swap_retires_a_late_open_without_mounting_duplicate_payload() {
        let (_, lease) = active_window_session(114, 214);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let target = DockLiveUndockHostTarget::new(WindowId::from(414), 24);
        let release = DockLiveUndockReleaseLock::new(
            DockLiveUndockPhysicalPoint::new(420, 320),
            DockLiveUndockRouteFeedback::Host(target),
            DockLiveUndockPhysicalBounds::new(DockLiveUndockPhysicalPoint::new(380, 290), 640, 480)
                .expect("test release bounds are non-empty"),
            placement_generation(18),
        );
        let (token, _) =
            prepare_token(session.apply(DockLiveUndockFact::ReleaseLocked { identity, release }));
        session.apply(DockLiveUndockFact::PromotionPrepared { identity, token });
        session.apply(DockLiveUndockFact::DurableSwapCommitted { identity, token });

        let late_window = fake_window(314);
        let late = session.apply(DockLiveUndockFact::OpeningReturned {
            identity,
            window: late_window,
            binding: DockLiveUndockOpeningBinding::ExactGated,
            runtime: admitted_runtime(identity, late_window),
        });
        assert!(late.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ProvisionalRetirementRequired {
                window: Some(current),
                reason: DockLiveUndockRetirementReason::HostDestinationSelected,
                ..
            } if *current == late_window
        )));
        assert!(
            !late
                .as_slice()
                .iter()
                .any(|effect| matches!(effect, DockLiveUndockEffect::ProvisionalAdmitted { .. }))
        );
        assert!(
            session
                .apply(DockLiveUndockFact::PresentationLeaseActivated {
                    identity,
                    receipt: DockLiveUndockPayloadLeaseReceipt::for_test(
                        identity,
                        source_for(identity),
                        lease_generation(7),
                        late_window.window_id(),
                    ),
                })
                .is_empty(),
            "a durable Host destination cannot remount the payload into the provisional"
        );
    }

    #[test]
    fn a_trigger_deferred_by_retirement_is_replayable_after_terminal_cleanup() {
        let (_, lease) = active_window_session(115, 215);
        let mut session = DockLiveUndockSession::new();
        let first = start(&mut session, lease, 1);
        let first_window = fake_window(315);
        admit(&mut session, first, first_window);
        session.apply(DockLiveUndockFact::Cancel {
            identity: first,
            reason: DockLiveUndockCancelReason::Escape,
        });
        assert_eq!(session.phase(), DockLiveUndockPhase::Retiring);

        let deferred = session.apply(DockLiveUndockFact::Trigger {
            lease,
            trigger: trigger_for(2),
        });
        assert!(matches!(
            deferred.as_slice(),
            [DockLiveUndockEffect::TriggerDeferred {
                drag_generation: current_generation,
            }] if *current_generation == drag_generation(2)
        ));
        session.apply(DockLiveUndockFact::WindowTerminal {
            identity: first,
            window_id: first_window.window_id(),
        });
        assert_eq!(session.phase(), DockLiveUndockPhase::Idle);

        let replay = session.apply(DockLiveUndockFact::Trigger {
            lease,
            trigger: trigger_for(2),
        });
        assert!(
            replay
                .as_slice()
                .iter()
                .any(|effect| matches!(effect, DockLiveUndockEffect::OpenProvisional { .. }))
        );
    }

    #[test]
    fn stale_presentation_and_placement_observations_cannot_regress_readiness() {
        let (_, lease) = active_window_session(116, 216);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(316);
        admit(&mut session, identity, window);
        let mount = activate_payload(
            &mut session,
            identity,
            source_for(identity),
            lease_generation(8),
            window,
        );
        observe_nonempty_visible(&mut session, identity, mount, 11);
        assert!(
            session
                .apply(DockLiveUndockFact::PayloadPresented {
                    identity,
                    receipt: DockLiveUndockPayloadPresentationReceipt::for_test(mount, 10)
                        .expect("the stale payload frame remains representable"),
                })
                .is_empty()
        );
        let current_placement = placement_generation(20);
        session.apply(DockLiveUndockFact::PlacementObserved {
            identity,
            window_id: window.window_id(),
            generation: current_placement,
            outcome: DockLiveUndockPlacementOutcome::Exact,
        });
        assert!(
            session
                .apply(DockLiveUndockFact::PlacementObserved {
                    identity,
                    window_id: window.window_id(),
                    generation: placement_generation(19),
                    outcome: DockLiveUndockPlacementOutcome::Rejected,
                })
                .is_empty()
        );
        assert!(
            session
                .apply(DockLiveUndockFact::ReleaseLocked {
                    identity,
                    release: desktop_release(current_placement),
                })
                .as_slice()
                .iter()
                .any(|effect| matches!(effect, DockLiveUndockEffect::PreparePromotion { .. }))
        );
    }

    #[test]
    fn superseded_successful_reveal_cannot_fail_or_open_the_current_preflight() {
        let (_, lease) = active_window_session(117, 217);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(317);
        admit(&mut session, identity, window);
        let mount = activate_payload(
            &mut session,
            identity,
            source_for(identity),
            lease_generation(9),
            window,
        );
        let first = DockLiveUndockPayloadPresentationReceipt::for_test(mount, 11)
            .expect("first payload preflight should be representable");
        let second = DockLiveUndockPayloadPresentationReceipt::for_test(mount, 12)
            .expect("replacement payload preflight should be representable");
        assert!(matches!(
            session
                .apply(DockLiveUndockFact::PayloadPresented {
                    identity,
                    receipt: first,
                })
                .as_slice(),
            [DockLiveUndockEffect::ArmExactReveal { .. }]
        ));
        assert!(matches!(
            session
                .apply(DockLiveUndockFact::PayloadPresented {
                    identity,
                    receipt: second,
                })
                .as_slice(),
            [DockLiveUndockEffect::ArmExactReveal { .. }]
        ));
        let old_reveal_frame = DockLiveUndockPayloadPresentationReceipt::for_test(mount, 13)
            .expect("old reveal frame should remain representable");
        let old_reveal = DockLiveUndockRevealReceipt::for_test(first, old_reveal_frame)
            .expect("old reveal is internally exact for its own preflight");
        assert!(
            session
                .apply(DockLiveUndockFact::RevealObserved {
                    identity,
                    observation: DockLiveUndockRevealObservation::Visible(old_reveal),
                })
                .is_empty(),
            "a superseded success must be inert rather than treated as reveal failure"
        );

        let placement = placement_generation(21);
        session.apply(DockLiveUndockFact::PlacementObserved {
            identity,
            window_id: window.window_id(),
            generation: placement,
            outcome: DockLiveUndockPlacementOutcome::Exact,
        });
        assert!(
            matches!(
                session
                    .apply(DockLiveUndockFact::ReleaseLocked {
                        identity,
                        release: desktop_release(placement),
                    })
                    .as_slice(),
                [DockLiveUndockEffect::RetireSourceTransportProxy {
                    identity: current,
                }] if *current == identity
            ),
            "the obsolete reveal cannot open promotion for the replacement preflight",
        );
        assert_eq!(session.phase(), DockLiveUndockPhase::Bound);
    }

    #[test]
    fn cancel_is_idempotent_and_source_deactivation_never_restores_focus() {
        let (_, lease) = active_window_session(110, 210);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(311);
        admit(&mut session, identity, window);
        let source = source_for(identity);
        let presentation_lease = lease_generation(6);
        let activation = session.apply(DockLiveUndockFact::PresentationLeaseActivated {
            identity,
            receipt: DockLiveUndockPayloadLeaseReceipt::for_test(
                identity,
                source,
                presentation_lease,
                window.window_id(),
            ),
        });
        assert!(matches!(
            activation.as_slice(),
            [DockLiveUndockEffect::CommitSourceProxy { .. }]
        ));

        let effects = session.apply(DockLiveUndockFact::Cancel {
            identity,
            reason: DockLiveUndockCancelReason::SourceDeactivated,
        });
        assert!(effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::RestoreSource {
                restore_focus: false,
                ..
            }
        )));
        assert!(!effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal { .. }
                | DockLiveUndockEffect::ProvisionalRetirementRequired { .. }
        )));
        assert_eq!(session.phase(), DockLiveUndockPhase::Compensating);
        assert!(
            session
                .apply(DockLiveUndockFact::Cancel {
                    identity,
                    reason: DockLiveUndockCancelReason::Escape,
                })
                .is_empty()
        );
        let (_, payload_lease, restore_focus) = restoration_request(&effects);
        assert!(!restore_focus);
        let released_receipt =
            DockLiveUndockSourceRestorationReceipt::source_presented_after_release_for_test(
                payload_lease,
                9_000,
            )
            .expect("after-release evidence should be representable");
        assert!(
            session
                .apply(DockLiveUndockFact::SourceRestorationCommitted {
                    identity,
                    receipt: released_receipt,
                })
                .is_empty(),
            "compensation cannot accept an after-release presentation receipt"
        );
        assert_eq!(session.phase(), DockLiveUndockPhase::Compensating);
        let committed = acknowledge_source_restoration(&mut session, identity, payload_lease);
        assert!(committed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                result: DockLiveUndockTerminalResult::Restored(
                    DockLiveUndockRestoreReason::Cancelled(
                        DockLiveUndockCancelReason::SourceDeactivated
                    )
                ),
                ..
            }
        )));
    }

    #[test]
    fn source_restoration_requires_exact_ack_and_source_terminal_proof() {
        let (_, lease) = active_window_session(119, 219);
        let mut stale_session = DockLiveUndockSession::new();
        let stale_identity = start(&mut stale_session, lease, 2);

        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(319);
        let source = source_for(identity);
        admit(&mut session, identity, window);
        activate_payload(&mut session, identity, source, lease_generation(20), window);

        let restore = session.apply(DockLiveUndockFact::Cancel {
            identity,
            reason: DockLiveUndockCancelReason::Escape,
        });
        let (_, payload_lease, _) = restoration_request(&restore);
        assert_eq!(session.phase(), DockLiveUndockPhase::Restoring);
        assert!(!restore.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ProvisionalRetirementRequired { .. }
                | DockLiveUndockEffect::PublishTerminal { .. }
        )));

        let exact =
            DockLiveUndockSourceRestorationReceipt::source_presented_after_release_for_test(
                payload_lease,
                9_101,
            )
            .expect("the exact restored source frame should be representable");
        assert!(
            session
                .apply(DockLiveUndockFact::SourceRestorationCommitted {
                    identity,
                    receipt: DockLiveUndockSourceRestorationReceipt::source_unchanged_for_test(
                        payload_lease,
                    ),
                })
                .is_empty(),
            "after-release restoration cannot accept an unchanged-source receipt"
        );
        assert!(
            session
                .apply(DockLiveUndockFact::SourceRestorationCommitted {
                    identity: stale_identity,
                    receipt: exact,
                })
                .is_empty(),
            "a stale drag identity cannot acknowledge the current restoration"
        );

        let stale_source_lease = DockLiveUndockPayloadLeaseReceipt::for_test(
            identity,
            DockLiveUndockSourceSnapshot::new(WindowId::from(99_001), source.scene_generation()),
            payload_lease.lease_generation(),
            window.window_id(),
        );
        let stale_source =
            DockLiveUndockSourceRestorationReceipt::source_presented_after_release_for_test(
                stale_source_lease,
                9_102,
            )
            .expect("stale source evidence remains representable");
        assert!(
            session
                .apply(DockLiveUndockFact::SourceRestorationCommitted {
                    identity,
                    receipt: stale_source,
                })
                .is_empty(),
            "a stale source snapshot cannot acknowledge the current restoration"
        );

        let stale_payload_lease = DockLiveUndockPayloadLeaseReceipt::for_test(
            identity,
            source,
            lease_generation(21),
            window.window_id(),
        );
        let stale_lease =
            DockLiveUndockSourceRestorationReceipt::source_presented_after_release_for_test(
                stale_payload_lease,
                9_103,
            )
            .expect("stale lease evidence remains representable");
        assert!(
            session
                .apply(DockLiveUndockFact::SourceRestorationCommitted {
                    identity,
                    receipt: stale_lease,
                })
                .is_empty(),
            "a stale payload lease cannot acknowledge the current restoration"
        );
        assert_eq!(session.phase(), DockLiveUndockPhase::Restoring);

        let committed = session.apply(DockLiveUndockFact::SourceRestorationCommitted {
            identity,
            receipt: exact,
        });
        assert_eq!(
            committed
                .as_slice()
                .iter()
                .filter(|effect| matches!(effect, DockLiveUndockEffect::RestoreSourceFocus { .. }))
                .count(),
            1,
            "an accepted focus-restoring receipt must emit exactly one activation effect"
        );
        assert!(committed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::RestoreSourceFocus {
                identity: current,
                source: current_source,
                payload_lease: current_lease,
            } if *current == identity
                && *current_source == source
                && *current_lease == payload_lease
        )));
        assert!(committed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ProvisionalRetirementRequired {
                reason: DockLiveUndockRetirementReason::SourceRestored(
                    DockLiveUndockRestoreReason::Cancelled(DockLiveUndockCancelReason::Escape)
                ),
                ..
            }
        )));
        assert!(committed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                result: DockLiveUndockTerminalResult::Restored(
                    DockLiveUndockRestoreReason::Cancelled(DockLiveUndockCancelReason::Escape)
                ),
                ..
            }
        )));

        let mut failed = DockLiveUndockSession::new();
        let failed_identity = start(&mut failed, lease, 3);
        let failed_window = fake_window(320);
        let failed_source = source_for(failed_identity);
        admit(&mut failed, failed_identity, failed_window);
        activate_payload(
            &mut failed,
            failed_identity,
            failed_source,
            lease_generation(22),
            failed_window,
        );
        let restore = failed.apply(DockLiveUndockFact::Cancel {
            identity: failed_identity,
            reason: DockLiveUndockCancelReason::Escape,
        });
        let (_, failed_lease, _) = restoration_request(&restore);
        let stale_failed_lease = DockLiveUndockPayloadLeaseReceipt::for_test(
            failed_identity,
            failed_source,
            lease_generation(23),
            failed_window.window_id(),
        );
        assert!(
            failed
                .apply(DockLiveUndockFact::SourceRestorationDeferred {
                    identity: failed_identity,
                    source: failed_source,
                    payload_lease: stale_failed_lease,
                })
                .is_empty(),
            "a stale deferred failure cannot affect the current restoration"
        );
        let deferred = failed.apply(DockLiveUndockFact::SourceRestorationDeferred {
            identity: failed_identity,
            source: failed_source,
            payload_lease: failed_lease,
        });
        assert!(deferred.is_empty());
        assert_eq!(failed.phase(), DockLiveUndockPhase::Restoring);

        assert!(matches!(
            failed
                .apply(DockLiveUndockFact::SourceRestorationRetryElapsed {
                    identity: failed_identity,
                    source: failed_source,
                    payload_lease: failed_lease,
                })
                .as_slice(),
            [DockLiveUndockEffect::RestoreSource {
                identity: current,
                payload_lease: current_lease,
                ..
            }] if *current == failed_identity && *current_lease == failed_lease
        ));

        assert!(
            failed
                .apply(DockLiveUndockFact::Cancel {
                    identity: failed_identity,
                    reason: DockLiveUndockCancelReason::SourceClosed,
                })
                .is_empty(),
            "logical source close cannot prove native source authority is terminal"
        );
        assert_eq!(failed.phase(), DockLiveUndockPhase::Restoring);

        let recovery = failed.apply(DockLiveUndockFact::SourceWindowNativeTerminal {
            receipt: source_native_terminal(failed_identity, failed_source),
        });
        assert!(matches!(
            recovery.as_slice(),
            [DockLiveUndockEffect::RecoverOrphanedPayloadTopology {
                identity: current,
                payload_lease: current_lease,
                ..
            }] if *current == failed_identity && *current_lease == failed_lease
        ));
        assert!(!recovery.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ProvisionalRetirementRequired { .. }
                | DockLiveUndockEffect::PublishTerminal { .. }
        )));
        assert_eq!(failed.phase(), DockLiveUndockPhase::RecoveringOrphan);

        let exact_recovery = committed_orphan_recovery(failed_lease);
        assert!(
            failed
                .apply(DockLiveUndockFact::OrphanRecoveryCommitted {
                    identity: stale_identity,
                    receipt: exact_recovery,
                })
                .is_empty(),
            "a stale identity cannot acknowledge the current orphan recovery"
        );
        assert!(
            failed
                .apply(DockLiveUndockFact::OrphanRecoveryFailed {
                    identity: failed_identity,
                    receipt: committed_orphan_recovery(stale_failed_lease),
                })
                .is_empty(),
            "a stale payload lease cannot fail the current orphan recovery"
        );
        assert_eq!(failed.phase(), DockLiveUndockPhase::RecoveringOrphan);

        assert!(matches!(
            failed
                .apply(DockLiveUndockFact::WindowTerminal {
                    identity: failed_identity,
                    window_id: failed_window.window_id(),
                })
                .as_slice(),
            [DockLiveUndockEffect::WindowTerminalSettled(outcome)]
                if outcome.dependency().is_none()
        ));
        assert_eq!(failed.phase(), DockLiveUndockPhase::RecoveringOrphan);

        let lost = failed.apply(DockLiveUndockFact::OrphanRecoveryFailed {
            identity: failed_identity,
            receipt: exact_recovery,
        });
        assert!(lost.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                result: DockLiveUndockTerminalResult::SourceLostBeforeCommit,
                ..
            }
        )));
        assert_eq!(failed.phase(), DockLiveUndockPhase::Idle);
    }

    #[test]
    fn source_native_terminal_recovers_migrated_payload_without_restoring_the_dead_source() {
        let (_, lease) = active_window_session(118, 218);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(318);
        let source = source_for(identity);
        let presentation_lease = lease_generation(19);
        admit(&mut session, identity, window);
        activate_payload(&mut session, identity, source, presentation_lease, window);

        let logical_close = session.apply(DockLiveUndockFact::Cancel {
            identity,
            reason: DockLiveUndockCancelReason::SourceClosed,
        });
        assert!(matches!(
            logical_close.as_slice(),
            [
                DockLiveUndockEffect::RetireSourceTransportProxy {
                    identity: retired_identity,
                },
                DockLiveUndockEffect::RestoreSource {
                    identity: current,
                    restore_focus: false,
                    ..
                }
            ] if *retired_identity == identity && *current == identity
        ));
        assert_eq!(session.phase(), DockLiveUndockPhase::Restoring);

        let effects = session.apply(DockLiveUndockFact::SourceWindowNativeTerminal {
            receipt: source_native_terminal(identity, source),
        });
        assert!(matches!(
            effects.as_slice(),
            [DockLiveUndockEffect::RecoverOrphanedPayloadTopology {
                identity: current,
                payload_lease,
                provisional: Some(current_window),
            }] if *current == identity
                && payload_lease.source() == source
                && payload_lease.lease_generation() == presentation_lease
                && *current_window == window
        ));
        assert!(!effects.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ProvisionalRetirementRequired { .. }
                | DockLiveUndockEffect::PublishTerminal { .. }
        )));
        let (payload_lease, provisional) = orphan_recovery_request(&effects);
        assert_eq!(provisional, Some(window));
        assert_eq!(session.phase(), DockLiveUndockPhase::RecoveringOrphan);

        assert!(
            session
                .apply(DockLiveUndockFact::SourceWindowNativeTerminal {
                    receipt: source_native_terminal(identity, source),
                })
                .is_empty(),
            "source-loss recovery must be idempotent while acknowledgement is pending"
        );

        let shutdown = session.apply(DockLiveUndockFact::ShutdownRequested { lease });
        let snapshot = shutdown
            .as_slice()
            .iter()
            .find_map(|effect| match effect {
                DockLiveUndockEffect::ShutdownFrozen(snapshot) => Some(*snapshot),
                _ => None,
            })
            .expect("shutdown should retain the orphan-recovery dependency");
        assert_eq!(session.phase(), DockLiveUndockPhase::RecoveringOrphan);
        assert_eq!(session.shutdown_snapshot(lease), Some(snapshot));
        assert!(matches!(
            shutdown.as_slice(),
            [
                DockLiveUndockEffect::ShutdownFrozen(current),
                DockLiveUndockEffect::ShutdownOrphanRecoveryRequired {
                    identity: current_identity,
                    payload_lease: current_payload_lease,
                    provisional: Some(current_window),
                },
            ] if *current == snapshot
                && *current_identity == identity
                && *current_payload_lease == payload_lease
                && *current_window == window
        ));
        assert!(!shutdown.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ProvisionalRetirementRequired { .. }
                | DockLiveUndockEffect::PublishTerminal { .. }
        )));

        let committed = session.apply(DockLiveUndockFact::OrphanRecoveryCommitted {
            identity,
            receipt: committed_orphan_recovery(payload_lease),
        });
        assert!(committed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ProvisionalRetirementRequired {
                identity: current,
                dependency: Some(dependency),
                binding: Some(DockLiveUndockOpeningBinding::ExactGated),
                reason: DockLiveUndockRetirementReason::SourceLost,
                ..
            } if *current == identity && *dependency == snapshot.dependency()
        )));
        assert!(committed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                identity: current,
                result: DockLiveUndockTerminalResult::SourceLostBeforeCommit,
            } if *current == identity
        )));
        assert_eq!(session.phase(), DockLiveUndockPhase::Retiring);

        assert!(
            session
                .apply(DockLiveUndockFact::OrphanRecoveryFailed {
                    identity,
                    receipt: committed_orphan_recovery(payload_lease),
                })
                .is_empty(),
            "a late recovery outcome cannot publish a second terminal result"
        );
        assert!(matches!(
            session
                .apply(DockLiveUndockFact::WindowTerminal {
                    identity,
                    window_id: window.window_id(),
                })
                .as_slice(),
            [DockLiveUndockEffect::WindowTerminalSettled(_)]
        ));
        assert_eq!(session.phase(), DockLiveUndockPhase::Idle);
    }

    #[test]
    fn shutdown_forces_terminal_orphan_recovery_and_settles_its_dependency() {
        let (_, lease) = active_window_session(506, 606);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(706);
        let source = source_for(identity);
        admit(&mut session, identity, window);
        activate_payload(&mut session, identity, source, lease_generation(34), window);

        let recovery = session.apply(DockLiveUndockFact::SourceWindowNativeTerminal {
            receipt: source_native_terminal(identity, source),
        });
        let (payload_lease, provisional) = orphan_recovery_request(&recovery);
        assert_eq!(provisional, Some(window));
        assert_eq!(session.phase(), DockLiveUndockPhase::RecoveringOrphan);

        assert!(matches!(
            session
                .apply(DockLiveUndockFact::WindowTerminal {
                    identity,
                    window_id: window.window_id(),
                })
                .as_slice(),
            [DockLiveUndockEffect::WindowTerminalSettled(outcome)]
                if outcome.dependency().is_none()
        ));
        assert_eq!(session.phase(), DockLiveUndockPhase::RecoveringOrphan);

        let shutdown = session.apply(DockLiveUndockFact::ShutdownRequested { lease });
        let snapshot = shutdown
            .as_slice()
            .iter()
            .find_map(|effect| match effect {
                DockLiveUndockEffect::ShutdownFrozen(snapshot) => Some(*snapshot),
                _ => None,
            })
            .expect("shutdown should freeze terminal orphan recovery");
        assert!(matches!(
            shutdown.as_slice(),
            [
                DockLiveUndockEffect::ShutdownFrozen(current),
                DockLiveUndockEffect::ShutdownOrphanRecoveryRequired {
                    identity: current_identity,
                    payload_lease: current_payload_lease,
                    provisional: None,
                },
            ] if *current == snapshot
                && *current_identity == identity
                && *current_payload_lease == payload_lease
        ));
        assert_eq!(snapshot.window(), None);
        assert_eq!(session.shutdown_snapshot(lease), Some(snapshot));

        let committed = session.apply(DockLiveUndockFact::OrphanRecoveryCommitted {
            identity,
            receipt: committed_orphan_recovery(payload_lease),
        });
        assert!(matches!(
            committed.as_slice(),
            [
                DockLiveUndockEffect::SettleShutdownDependency {
                    identity: current,
                    dependency,
                },
                DockLiveUndockEffect::PublishTerminal {
                    identity: terminal_identity,
                    result: DockLiveUndockTerminalResult::SourceLostBeforeCommit,
                },
            ] if *current == identity
                && *terminal_identity == identity
                && *dependency == snapshot.dependency()
        ));
        assert_eq!(session.phase(), DockLiveUndockPhase::Idle);
    }

    #[test]
    fn shutdown_orphan_cleanup_receipt_is_terminal_without_retry() {
        let (_, lease) = active_window_session(507, 607);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(707);
        let source = source_for(identity);
        admit(&mut session, identity, window);
        activate_payload(&mut session, identity, source, lease_generation(35), window);

        let recovery = session.apply(DockLiveUndockFact::SourceWindowNativeTerminal {
            receipt: source_native_terminal(identity, source),
        });
        let (payload_lease, _) = orphan_recovery_request(&recovery);
        let _ = session.apply(DockLiveUndockFact::WindowTerminal {
            identity,
            window_id: window.window_id(),
        });
        let shutdown = session.apply(DockLiveUndockFact::ShutdownRequested { lease });
        let dependency = shutdown
            .as_slice()
            .iter()
            .find_map(|effect| match effect {
                DockLiveUndockEffect::ShutdownFrozen(snapshot) => Some(snapshot.dependency()),
                _ => None,
            })
            .expect("shutdown should claim the orphan-recovery dependency");

        let terminal = session.apply(DockLiveUndockFact::ShutdownOrphanCleanupCompleted {
            receipt: DockLiveUndockOrphanCleanupReceipt::for_test(payload_lease),
        });
        assert!(matches!(
            terminal.as_slice(),
            [
                DockLiveUndockEffect::SettleShutdownDependency {
                    identity: current,
                    dependency: current_dependency,
                },
                DockLiveUndockEffect::PublishTerminal {
                    identity: terminal_identity,
                    result: DockLiveUndockTerminalResult::SourceLostBeforeCommit,
                },
            ] if *current == identity
                && *terminal_identity == identity
                && *current_dependency == dependency
        ));
        assert_eq!(session.phase(), DockLiveUndockPhase::Idle);
    }

    #[test]
    fn shutdown_window_terminal_cannot_consume_pending_orphan_cleanup_dependency() {
        let (_, lease) = active_window_session(508, 608);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(708);
        let source = source_for(identity);
        admit(&mut session, identity, window);
        activate_payload(&mut session, identity, source, lease_generation(36), window);

        let restore = session.apply(DockLiveUndockFact::Cancel {
            identity,
            reason: DockLiveUndockCancelReason::CaptureLost,
        });
        let (_, payload_lease, _) = restoration_request(&restore);
        let shutdown = session.apply(DockLiveUndockFact::ShutdownRequested { lease });
        let dependency = shutdown
            .as_slice()
            .iter()
            .find_map(|effect| match effect {
                DockLiveUndockEffect::ShutdownFrozen(snapshot) => Some(snapshot.dependency()),
                _ => None,
            })
            .expect("shutdown must claim cleanup authority before the window terminates");

        assert!(matches!(
            session
                .apply(DockLiveUndockFact::WindowTerminal {
                    identity,
                    window_id: window.window_id(),
                })
                .as_slice(),
            [DockLiveUndockEffect::WindowTerminalSettled(outcome)]
                if outcome.dependency().is_none()
        ));
        assert_eq!(
            session
                .shutdown_snapshot(lease)
                .map(|snapshot| snapshot.dependency()),
            Some(dependency)
        );

        let recovery = session.apply(DockLiveUndockFact::SourceWindowNativeTerminal {
            receipt: source_native_terminal(identity, source),
        });
        assert!(matches!(
            recovery.as_slice(),
            [DockLiveUndockEffect::ShutdownOrphanRecoveryRequired {
                identity: current,
                payload_lease: current_lease,
                provisional: None,
            }] if *current == identity && *current_lease == payload_lease
        ));
        assert_eq!(session.phase(), DockLiveUndockPhase::RecoveringOrphan);
        assert_eq!(
            session
                .shutdown_snapshot(lease)
                .map(|snapshot| snapshot.dependency()),
            Some(dependency)
        );
    }

    #[test]
    fn shutdown_orphan_cleanup_failure_finishes_with_typed_failure_terminal() {
        let (_, lease) = active_window_session(509, 609);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(709);
        let source = source_for(identity);
        admit(&mut session, identity, window);
        activate_payload(&mut session, identity, source, lease_generation(37), window);

        let recovery = session.apply(DockLiveUndockFact::SourceWindowNativeTerminal {
            receipt: source_native_terminal(identity, source),
        });
        let (payload_lease, _) = orphan_recovery_request(&recovery);
        let shutdown = session.apply(DockLiveUndockFact::ShutdownRequested { lease });
        let dependency = shutdown
            .as_slice()
            .iter()
            .find_map(|effect| match effect {
                DockLiveUndockEffect::ShutdownFrozen(snapshot) => Some(snapshot.dependency()),
                _ => None,
            })
            .expect("shutdown must retain one exact orphan-cleanup dependency");

        let failure = DockLiveUndockOrphanCleanupFailure::PreflightRejected;
        let failed = session.apply(DockLiveUndockFact::ShutdownOrphanCleanupFailed {
            identity,
            payload_lease,
            failure,
        });
        assert!(failed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                identity: current_identity,
                result: DockLiveUndockTerminalResult::ShutdownCleanupFailed(
                    DockLiveUndockShutdownFailure::OrphanCleanup(current_failure),
                ),
            } if *current_identity == identity && *current_failure == failure
        )));
        assert!(failed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::FailShutdownDependency {
                identity: current_identity,
                dependency: current_dependency,
                failure: DockLiveUndockShutdownFailure::OrphanCleanup(current_failure),
            } if *current_identity == identity
                && *current_dependency == dependency
                && *current_failure == failure
        )));
        assert!(!failed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::SettleShutdownDependency { .. }
        )));
        assert_eq!(session.phase(), DockLiveUndockPhase::ShutdownCleanupFailed);
        assert!(session.shutdown_snapshot(lease).is_none());

        assert!(
            session
                .apply(DockLiveUndockFact::ShutdownRequested { lease })
                .is_empty(),
            "a terminal cleanup failure must not arm retries or publish a false terminal"
        );
        assert_eq!(session.phase(), DockLiveUndockPhase::ShutdownCleanupFailed);
    }

    #[test]
    fn shutdown_revokes_pending_source_focus_before_restoration_acknowledges() {
        let (_, lease) = active_window_session(120, 220);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(321);
        let source = source_for(identity);
        admit(&mut session, identity, window);
        activate_payload(&mut session, identity, source, lease_generation(24), window);

        let restore = session.apply(DockLiveUndockFact::Cancel {
            identity,
            reason: DockLiveUndockCancelReason::Escape,
        });
        let (_, payload_lease, restore_focus) = restoration_request(&restore);
        assert!(restore_focus);
        assert_eq!(session.phase(), DockLiveUndockPhase::Restoring);

        let shutdown = session.apply(DockLiveUndockFact::ShutdownRequested { lease });
        assert!(shutdown.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::ShutdownSourceRestorationRequired {
                identity: current,
                payload_lease: current_lease,
                ..
            } if *current == identity && *current_lease == payload_lease
        )));

        let committed = acknowledge_source_restoration(&mut session, identity, payload_lease);
        assert!(
            !committed
                .as_slice()
                .iter()
                .any(|effect| matches!(effect, DockLiveUndockEffect::RestoreSourceFocus { .. }))
        );
        assert!(committed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                result: DockLiveUndockTerminalResult::Restored(
                    DockLiveUndockRestoreReason::Shutdown
                ),
                ..
            }
        )));
    }

    #[test]
    fn source_host_presentation_loss_transfers_restoration_to_orphan_recovery() {
        let (_, lease) = active_window_session(121, 221);
        let mut session = DockLiveUndockSession::new();
        let identity = start(&mut session, lease, 1);
        let window = fake_window(322);
        let source = source_for(identity);
        admit(&mut session, identity, window);
        activate_payload(&mut session, identity, source, lease_generation(25), window);

        let restore = session.apply(DockLiveUndockFact::Cancel {
            identity,
            reason: DockLiveUndockCancelReason::Escape,
        });
        let (_, payload_lease, _) = restoration_request(&restore);
        assert_eq!(session.phase(), DockLiveUndockPhase::Restoring);

        let recovery = session.apply(DockLiveUndockFact::PresentationAuthorityLost {
            receipt: DockLiveUndockPresentationAuthorityLossReceipt::for_test(
                payload_lease,
                DockLiveUndockPresentationAuthorityLoss::SourceHostPresentationLost,
            ),
        });
        assert!(matches!(
            recovery.as_slice(),
            [DockLiveUndockEffect::RecoverOrphanedPayloadTopology {
                identity: current,
                payload_lease: current_lease,
                provisional: Some(current_window),
            }] if *current == identity
                && *current_lease == payload_lease
                && *current_window == window
        ));
        assert_eq!(session.phase(), DockLiveUndockPhase::RecoveringOrphan);

        let committed = session.apply(DockLiveUndockFact::OrphanRecoveryCommitted {
            identity,
            receipt: committed_orphan_recovery(payload_lease),
        });
        assert!(committed.as_slice().iter().any(|effect| matches!(
            effect,
            DockLiveUndockEffect::PublishTerminal {
                result: DockLiveUndockTerminalResult::PresentationAuthorityLostBeforeCommit(
                    DockLiveUndockPresentationAuthorityLoss::SourceHostPresentationLost
                ),
                ..
            }
        )));
    }
}

#[test]
fn triggered_opening_is_generation_bound_and_single_in_flight() {
    let (_, lease) = active_window_session(1, 7);
    let mut live = DockLiveUndockSession::new();

    let request = reducer_tests::start_request(&mut live, lease, 1);
    assert_eq!(request.key().lease(), lease);
    assert_eq!(request.key().generation(), 1);
    assert_eq!(request.provisional_session().snapshot().generation(), 1);
    assert_eq!(live.phase(), DockLiveUndockPhase::Opening);
    assert!(matches!(
        live.apply(DockLiveUndockFact::Trigger {
            lease,
            trigger: reducer_tests::trigger_for(2),
        })
        .as_slice(),
        [DockLiveUndockEffect::TriggerDeferred { drag_generation }]
            if drag_generation.get() == 2
    ));
}

#[test]
fn shutdown_dependency_survives_until_a_cancelled_pending_open_settles() {
    let (_, lease) = active_window_session(1, 7);
    let mut live = DockLiveUndockSession::new();
    let request = reducer_tests::start_request(&mut live, lease, 1);

    let (frozen, effects) = live.freeze_for_shutdown(lease).into_parts();
    let frozen = frozen.expect("the pending opening must freeze into shutdown");
    assert!(effects.as_slice().iter().any(
        |effect| matches!(effect, DockLiveUndockEffect::ShutdownFrozen(current) if *current == frozen)
    ));
    assert_eq!(
        frozen.dependency(),
        DockSurfaceWindowSessionDependencyId::live_undock(request.key().generation())
    );
    assert_eq!(frozen.window(), None);
    assert_eq!(live.phase(), DockLiveUndockPhase::Retiring);
    let (second_frozen, second_effects) = live.freeze_for_shutdown(lease).into_parts();
    assert_eq!(second_frozen, Some(frozen));
    assert!(second_effects.as_slice().iter().any(
        |effect| matches!(effect, DockLiveUndockEffect::ShutdownFrozen(current) if *current == frozen)
    ));

    let (failure, failure_effects) = live.fail_opening(request.key()).into_parts();
    assert_eq!(
        failure,
        DockLiveUndockOpenFailureOutcome::SettleDependency {
            lease,
            dependency: frozen.dependency(),
        }
    );
    assert!(matches!(
        failure_effects.as_slice(),
        [DockLiveUndockEffect::OpeningFailed {
            dependency: Some(dependency),
            ..
        }] if *dependency == frozen.dependency()
    ));
    assert_eq!(live.phase(), DockLiveUndockPhase::Idle);
    let (stale, stale_effects) = live.fail_opening(request.key()).into_parts();
    assert_eq!(stale, DockLiveUndockOpenFailureOutcome::Stale);
    assert!(stale_effects.is_empty());
}

fn enable_provisional_test_windows(cx: &mut open_gpui::TestAppContext) {
    cx.set_platform_window_creation_capabilities(PlatformWindowCreationCapabilities {
        focus_on_appearing: WindowCreationSupport::Supported,
        transient_for: WindowCreationSupport::Supported,
        provisional_presentation: WindowCreationSupport::Supported,
        initial_presentation_order: WindowInitialPresentationOrder::BeforeVisibility,
    });
}

fn provisional_options(request: &super::live_undock::DockLiveUndockOpenRequest) -> WindowOptions {
    WindowOptions {
        show: true,
        focus_on_appearing: false,
        provisional_session: Some(request.provisional_session().clone()),
        ..Default::default()
    }
}

fn begin_triggered_live_undock_opening(
    surface: &crate::DockSurface,
    lease: DockSurfaceWindowSessionLease,
    source_window: WindowId,
    cx: &mut open_gpui::App,
) -> DockLiveUndockOpenRequest {
    let trigger = DockLiveUndockTrigger::new(
        DockLiveUndockDragGeneration::new(1).expect("the test drag generation should be non-zero"),
        DockLiveUndockSourceSnapshot::new(source_window, 1),
        DockLiveUndockRouteFeedback::Desktop,
    )
    .expect("desktop should be an eligible live-undock route");
    reduce_live_undock_fact(
        surface.owner(),
        DockLiveUndockFact::Trigger { lease, trigger },
        cx,
    )
    .expect("the exact active surface lease should accept the trigger")
    .into_iter()
    .find_map(|effect| match effect {
        DockLiveUndockEffect::OpenProvisional { request, .. } => Some(request),
        _ => None,
    })
    .expect("the accepted trigger should reserve one provisional opening")
}

#[open_gpui::test]
fn runtime_registration_rejection_is_reported_and_converges_without_bound_owner(
    cx: &mut open_gpui::TestAppContext,
) {
    enable_provisional_test_windows(cx);
    let closed = Rc::new(RefCell::new(Vec::new()));
    let (surface, anchor, runtime, lease) = cx.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        let closed = closed.clone();
        cx.on_window_closed(move |_, window_id| closed.borrow_mut().push(window_id))
            .detach();
        let surface = crate::DockSurface::builder("main")
            .allow_platform_viewports(true)
            .build(cx)
            .expect("the surface should validate");
        let anchor = match surface.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the primary should open, got {outcome:?}"),
        };
        let lease = cx.read_entity(surface.owner(), |owner, _| {
            owner
                .window_session()
                .active_lease()
                .expect("the primary should activate one exact surface lease")
        });
        let runtime = surface.viewport_runtime(cx);
        (surface, anchor, runtime, lease)
    });
    cx.run_until_parked();

    let (error, immediate_phase) = cx.update(|cx| {
        runtime.reject_next_provisional_registration_for_test();
        let request = begin_triggered_live_undock_opening(&surface, lease, anchor.window_id(), cx);
        let identity = request.identity();
        let error = runtime
            .open_triggered_live_undock_provisional_viewport(
                surface.primary_space().clone(),
                WindowOptions::default(),
                &request,
                cx,
            )
            .expect_err("runtime rejection must be reported to the opening executor");
        reduce_live_undock_fact(
            surface.owner(),
            DockLiveUndockFact::Cancel {
                identity,
                reason: DockLiveUndockCancelReason::CaptureLost,
            },
            cx,
        );
        let phase = cx.read_entity(surface.owner(), |owner, _| owner.live_undock_phase());
        (error.to_string(), phase)
    });

    assert!(
        error.contains("RuntimeRegistrationRejected"),
        "the explicit runtime rejection outcome should reach the caller: {error}"
    );
    assert!(matches!(
        immediate_phase,
        DockLiveUndockPhase::Compensating
            | DockLiveUndockPhase::Restoring
            | DockLiveUndockPhase::Retiring
            | DockLiveUndockPhase::Idle
    ));

    cx.run_until_parked();

    assert!(cx.windows().contains(&anchor));
    assert_eq!(cx.windows().len(), 1);
    assert_eq!(closed.borrow().len(), 1);
    assert_ne!(closed.borrow()[0], anchor.window_id());
    assert_eq!(
        runtime.windows_for_surface(lease),
        vec![(DockViewportWindowRole::PrimaryAnchor, anchor)]
    );
    assert_eq!(
        cx.update(|cx| cx.read_entity(surface.owner(), |owner, _| owner.live_undock_phase())),
        DockLiveUndockPhase::Idle
    );
    assert_eq!(
        cx.update(|cx| surface.window_session_status(cx).phase()),
        DockSurfaceWindowSessionPhase::Active
    );
}

#[open_gpui::test]
fn builder_time_provisional_freeze_retries_close_after_registry_commit(
    cx: &mut open_gpui::TestAppContext,
) {
    enable_provisional_test_windows(cx);
    let closed = Rc::new(RefCell::new(Vec::new()));
    cx.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        let closed = closed.clone();
        cx.on_window_closed(move |_, window_id| closed.borrow_mut().push(window_id))
            .detach();
    });

    let (surface, anchor, provisional) = cx.update(|cx| {
        let surface = crate::DockSurface::builder("main")
            .allow_platform_viewports(true)
            .build(cx)
            .expect("the surface should validate");
        let anchor = match surface.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the primary should open, got {outcome:?}"),
        };
        let lease = cx.read_entity(surface.owner(), |owner, _| {
            owner
                .window_session()
                .active_lease()
                .expect("the primary should activate one exact surface lease")
        });
        let request = begin_triggered_live_undock_opening(&surface, lease, anchor.window_id(), cx);
        let opening = request.key();
        let runtime = surface.viewport_runtime(cx);
        let builder_runtime = runtime.clone();
        let builder_owner = surface.owner().clone();
        let attempt_slot = Rc::new(Cell::new(None));
        let builder_attempt_slot = attempt_slot.clone();
        let provisional: AnyWindowHandle = cx
            .open_window(provisional_options(&request), move |window, cx| {
                let attempt = builder_runtime
                    .begin_live_undock_provisional_open_attempt(window.window_handle(), opening)
                    .expect("the builder must register Runtime ownership before returning a view");
                builder_attempt_slot.set(Some(attempt));
                let effects = prepare_surface_shutdown(
                    &builder_owner,
                    lease,
                    DockSurfaceWindowSessionShutdownReason::AnchorCloseRequested,
                    cx,
                )
                .expect("the active surface should freeze the builder-time opening");
                apply_surface_shutdown_close_effects(&builder_owner, effects, cx);
                cx.new(|_| Empty)
            })
            .expect("the cancelled opening may still return one committed HWND")
            .into();
        let attempt = attempt_slot
            .take()
            .expect("the builder must publish its exact Runtime attempt");
        let completion =
            runtime.complete_live_undock_provisional_open_attempt(attempt, opening, false);
        assert!(matches!(
            completion,
            crate::DockViewportProvisionalOpenAttemptCompletion::ShutdownOwned(_)
        ));
        assert!(matches!(
            finish_live_undock_open_return(
                surface.owner(),
                opening,
                provisional,
                completion,
                cx,
            ),
            DockLiveUndockOpenReturnOutcome::Retire {
                lease: current,
                dependency: Some(_),
                binding_valid: true,
            } if current == lease
        ));
        (surface, anchor, provisional)
    });

    cx.run_until_parked();

    assert!(!cx.windows().contains(&provisional));
    assert!(!cx.windows().contains(&anchor));
    assert_eq!(
        closed.borrow().as_slice(),
        [provisional.window_id(), anchor.window_id()],
        "the post-commit retry must close the provisional before the anchor"
    );
    let status = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(status.phase(), DockSurfaceWindowSessionPhase::Closed);
    assert_eq!(status.pending_terminal_ticket_count(), 0);
    assert_eq!(status.runtime_empty(), Some(true));
}

#[open_gpui::test]
fn frozen_admission_before_provisional_builder_survives_close_observer_panic(
    cx: &mut open_gpui::TestAppContext,
) {
    enable_provisional_test_windows(cx);
    let closed = Rc::new(RefCell::new(Vec::new()));
    let (surface, anchor, runtime, lease) = cx.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        let closed = closed.clone();
        cx.on_window_closed(move |_, window_id| closed.borrow_mut().push(window_id))
            .detach();
        let surface = crate::DockSurface::builder("main")
            .allow_platform_viewports(true)
            .build(cx)
            .expect("the surface should validate");
        let anchor = match surface.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the primary should open, got {outcome:?}"),
        };
        let lease = cx.read_entity(surface.owner(), |owner, _| {
            owner
                .window_session()
                .active_lease()
                .expect("the primary should activate one exact surface lease")
        });
        let runtime = surface.viewport_runtime(cx);
        (surface, anchor, runtime, lease)
    });
    cx.run_until_parked();

    let panic_observed = Rc::new(Cell::new(false));
    let panic_observer = panic_observed.clone();
    let close = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        cx.update(|cx| {
            let anchor_id = anchor.window_id();
            cx.on_window_closed(move |_, window_id| {
                if window_id != anchor_id && !panic_observer.replace(true) {
                    panic!("injected provisional close observer panic");
                }
            })
            .detach();
            let request =
                begin_triggered_live_undock_opening(&surface, lease, anchor.window_id(), cx);
            let owner = surface.owner().clone();
            runtime.install_live_undock_provisional_builder_hook_for_test(move |cx| {
                let effects = prepare_surface_shutdown(
                    &owner,
                    lease,
                    DockSurfaceWindowSessionShutdownReason::AnchorCloseRequested,
                    cx,
                )
                .expect("the builder race should freeze the active surface");
                apply_surface_shutdown_close_effects(&owner, effects, cx);
            });
            let _ = runtime.open_triggered_live_undock_provisional_viewport(
                surface.primary_space().clone(),
                WindowOptions::default(),
                &request,
                cx,
            );
        });
    }));

    assert!(panic_observed.get(), "the injected close observer must run");
    assert!(
        close.is_err(),
        "GPUI must rethrow the injected observer panic"
    );
    cx.run_until_parked();

    assert_eq!(
        closed.borrow().last(),
        Some(&anchor.window_id()),
        "the provisional child must close before the surface anchor"
    );
    assert_eq!(closed.borrow().len(), 2);
    assert!(!cx.windows().contains(&anchor));
    assert!(runtime.windows_for_surface(lease).is_empty());
    let status = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(status.phase(), DockSurfaceWindowSessionPhase::Closed);
    assert_eq!(status.pending_terminal_ticket_count(), 0);
    assert_eq!(status.runtime_empty(), Some(true));
}

#[open_gpui::test]
fn shutdown_owned_invalid_binding_waits_for_child_native_terminal(
    cx: &mut open_gpui::TestAppContext,
) {
    enable_provisional_test_windows(cx);
    let closed = Rc::new(RefCell::new(Vec::new()));
    let (surface, anchor, provisional, identity, completion) = cx.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        let closed = closed.clone();
        cx.on_window_closed(move |_, window_id| closed.borrow_mut().push(window_id))
            .detach();
        let surface = crate::DockSurface::builder("main")
            .allow_platform_viewports(true)
            .build(cx)
            .expect("the surface should validate");
        let anchor = match surface.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the primary should open, got {outcome:?}"),
        };
        let lease = cx.read_entity(surface.owner(), |owner, _| {
            owner
                .window_session()
                .active_lease()
                .expect("the primary should activate one exact surface lease")
        });
        let request = begin_triggered_live_undock_opening(&surface, lease, anchor.window_id(), cx);
        let identity = request.identity();
        let opening = request.key();
        let runtime = surface.viewport_runtime(cx);
        let builder_runtime = runtime.clone();
        let builder_owner = surface.owner().clone();
        let attempt_slot = Rc::new(Cell::new(None));
        let builder_attempt_slot = attempt_slot.clone();
        let provisional: AnyWindowHandle = cx
            .open_window(provisional_options(&request), move |window, cx| {
                let attempt = builder_runtime
                    .begin_live_undock_provisional_open_attempt(window.window_handle(), opening)
                    .expect("the builder must register exact Runtime ownership");
                builder_attempt_slot.set(Some(attempt));
                let effects = prepare_surface_shutdown(
                    &builder_owner,
                    lease,
                    DockSurfaceWindowSessionShutdownReason::AnchorCloseRequested,
                    cx,
                )
                .expect("the active surface should freeze the builder-time opening");
                apply_surface_shutdown_close_effects(&builder_owner, effects, cx);
                cx.new(|_| Empty)
            })
            .expect("the frozen opening may still return one committed window")
            .into();
        let attempt = attempt_slot
            .take()
            .expect("the builder must publish its exact Runtime attempt");
        let completion =
            runtime.complete_live_undock_provisional_open_attempt(attempt, opening, false);
        assert!(matches!(
            completion,
            crate::DockViewportProvisionalOpenAttemptCompletion::ShutdownOwned(_)
        ));
        (surface, anchor, provisional, identity, completion)
    });
    let provisional_terminal = cx.hold_window_native_terminal(provisional);

    cx.update(|cx| {
        let effects = reduce_live_undock_fact(
            surface.owner(),
            DockLiveUndockFact::OpeningReturned {
                identity,
                window: provisional,
                binding: super::live_undock::DockLiveUndockOpeningBinding::Invalid,
                runtime: completion,
            },
            cx,
        )
        .expect("the retiring opening must consume its late return");
        let live_runtime = cx.read_entity(surface.owner(), |owner, _| owner.live_undock_runtime());
        live_runtime.enqueue_effects(effects, cx);
    });
    cx.run_until_parked();

    assert!(!cx.windows().contains(&provisional));
    assert!(
        cx.windows().contains(&anchor),
        "the anchor must remain live while the child native terminal is held"
    );
    assert_eq!(closed.borrow().as_slice(), [provisional.window_id()]);
    let waiting = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(waiting.phase(), DockSurfaceWindowSessionPhase::ShuttingDown);
    assert_eq!(waiting.pending_terminal_ticket_count(), 2);

    assert!(provisional_terminal.release());
    cx.run_until_parked();

    assert!(!cx.windows().contains(&anchor));
    assert_eq!(
        closed.borrow().as_slice(),
        [provisional.window_id(), anchor.window_id()]
    );
    let closed_status = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(closed_status.phase(), DockSurfaceWindowSessionPhase::Closed);
    assert_eq!(closed_status.pending_terminal_ticket_count(), 0);
    assert_eq!(closed_status.runtime_empty(), Some(true));
}

#[open_gpui::test]
fn shutdown_stale_open_return_waits_for_child_native_terminal(cx: &mut open_gpui::TestAppContext) {
    enable_provisional_test_windows(cx);
    let closed = Rc::new(RefCell::new(Vec::new()));
    let (surface, anchor, provisional, opening, completion, lease) = cx.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        let closed = closed.clone();
        cx.on_window_closed(move |_, window_id| closed.borrow_mut().push(window_id))
            .detach();
        let surface = crate::DockSurface::builder("main")
            .allow_platform_viewports(true)
            .build(cx)
            .expect("the surface should validate");
        let anchor = match surface.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the primary should open, got {outcome:?}"),
        };
        let lease = cx.read_entity(surface.owner(), |owner, _| {
            owner
                .window_session()
                .active_lease()
                .expect("the primary should activate one exact surface lease")
        });
        let request = begin_triggered_live_undock_opening(&surface, lease, anchor.window_id(), cx);
        let opening = request.key();
        let runtime = surface.viewport_runtime(cx);
        let builder_runtime = runtime.clone();
        let builder_owner = surface.owner().clone();
        let attempt_slot = Rc::new(Cell::new(None));
        let builder_attempt_slot = attempt_slot.clone();
        let provisional: AnyWindowHandle = cx
            .open_window(provisional_options(&request), move |window, cx| {
                let attempt = builder_runtime
                    .begin_live_undock_provisional_open_attempt(window.window_handle(), opening)
                    .expect("the builder must register exact Runtime ownership");
                builder_attempt_slot.set(Some(attempt));
                let effects = prepare_surface_shutdown(
                    &builder_owner,
                    lease,
                    DockSurfaceWindowSessionShutdownReason::AnchorCloseRequested,
                    cx,
                )
                .expect("the active surface should freeze the builder-time opening");
                apply_surface_shutdown_close_effects(&builder_owner, effects, cx);
                assert!(matches!(
                    finish_live_undock_open_failure(&builder_owner, opening, cx),
                    DockLiveUndockOpenFailureOutcome::SettleDependency {
                        lease: current,
                        ..
                    } if current == lease
                ));
                cx.new(|_| Empty)
            })
            .expect("the stale opening may still return one committed window")
            .into();
        let completion = runtime.complete_live_undock_provisional_open_attempt(
            attempt_slot
                .take()
                .expect("the builder must publish its exact Runtime attempt"),
            opening,
            false,
        );
        assert!(matches!(
            completion,
            crate::DockViewportProvisionalOpenAttemptCompletion::ShutdownOwned(_)
        ));
        (surface, anchor, provisional, opening, completion, lease)
    });
    let provisional_terminal = cx.hold_window_native_terminal(provisional);

    assert_eq!(
        cx.update(|cx| finish_live_undock_open_return(
            surface.owner(),
            opening,
            provisional,
            completion,
            cx,
        )),
        DockLiveUndockOpenReturnOutcome::Stale
    );
    cx.run_until_parked();

    assert!(!cx.windows().contains(&provisional));
    assert!(
        cx.windows().contains(&anchor),
        "the stale child return must retain the anchor until native terminal"
    );
    assert_eq!(closed.borrow().as_slice(), [provisional.window_id()]);
    let waiting = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(waiting.phase(), DockSurfaceWindowSessionPhase::ShuttingDown);
    assert_eq!(waiting.pending_terminal_ticket_count(), 2);

    assert!(provisional_terminal.release());
    cx.run_until_parked();

    assert!(!cx.windows().contains(&anchor));
    assert_eq!(
        closed.borrow().as_slice(),
        [provisional.window_id(), anchor.window_id()]
    );
    let runtime = cx.update(|cx| surface.viewport_runtime(cx));
    assert!(runtime.windows_for_surface(lease).is_empty());
    let closed_status = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(closed_status.phase(), DockSurfaceWindowSessionPhase::Closed);
    assert_eq!(closed_status.pending_terminal_ticket_count(), 0);
    assert_eq!(closed_status.runtime_empty(), Some(true));
}

#[open_gpui::test]
fn frozen_provisional_builder_initial_close_aborts_exact_runtime_record(
    cx: &mut open_gpui::TestAppContext,
) {
    enable_provisional_test_windows(cx);
    let (surface, anchor, runtime, lease) = cx.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        let surface = crate::DockSurface::builder("main")
            .allow_platform_viewports(true)
            .build(cx)
            .expect("the surface should validate");
        let anchor = match surface.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the primary should open, got {outcome:?}"),
        };
        let lease = cx.read_entity(surface.owner(), |owner, _| {
            owner
                .window_session()
                .active_lease()
                .expect("the primary should activate one exact surface lease")
        });
        let runtime = surface.viewport_runtime(cx);
        (surface, anchor, runtime, lease)
    });
    cx.run_until_parked();
    cx.close_next_window_during_initial_presentation();

    let error = cx.update(|cx| {
        let request = begin_triggered_live_undock_opening(&surface, lease, anchor.window_id(), cx);
        let owner = surface.owner().clone();
        runtime.install_live_undock_provisional_builder_hook_for_test(move |cx| {
            let effects = prepare_surface_shutdown(
                &owner,
                lease,
                DockSurfaceWindowSessionShutdownReason::AnchorCloseRequested,
                cx,
            )
            .expect("the builder race should freeze the active surface");
            apply_surface_shutdown_close_effects(&owner, effects, cx);
        });
        runtime
            .open_triggered_live_undock_provisional_viewport(
                surface.primary_space().clone(),
                WindowOptions::default(),
                &request,
                cx,
            )
            .expect_err("the injected initial close must reject the provisional window")
            .to_string()
    });

    assert!(
        error.contains("closed") || error.contains("initial presentation"),
        "the platform initial close should reach the caller: {error}"
    );
    cx.run_until_parked();

    assert!(!cx.windows().contains(&anchor));
    assert!(runtime.windows_for_surface(lease).is_empty());
    assert_eq!(
        cx.update(|cx| cx.read_entity(surface.owner(), |owner, _| owner.live_undock_phase())),
        DockLiveUndockPhase::Idle
    );
    let status = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(status.phase(), DockSurfaceWindowSessionPhase::Closed);
    assert_eq!(status.pending_terminal_ticket_count(), 0);
    assert_eq!(status.runtime_empty(), Some(true));
}

#[open_gpui::test]
fn stale_open_return_compensates_exact_runtime_retirement(cx: &mut open_gpui::TestAppContext) {
    enable_provisional_test_windows(cx);
    let closed = Rc::new(RefCell::new(Vec::new()));
    let (surface, anchor, provisional_id, lease) = cx.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        let closed = closed.clone();
        cx.on_window_closed(move |_, window_id| closed.borrow_mut().push(window_id))
            .detach();
        let surface = crate::DockSurface::builder("main")
            .allow_platform_viewports(true)
            .build(cx)
            .expect("the surface should validate");
        let anchor = match surface.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the primary should open, got {outcome:?}"),
        };
        let lease = cx.read_entity(surface.owner(), |owner, _| {
            owner
                .window_session()
                .active_lease()
                .expect("the primary should activate one exact surface lease")
        });
        let request = begin_triggered_live_undock_opening(&surface, lease, anchor.window_id(), cx);
        let identity = request.identity();
        let opening = request.key();
        let runtime = surface.viewport_runtime(cx);
        let builder_runtime = runtime.clone();
        let builder_owner = surface.owner().clone();
        let attempt_slot = Rc::new(Cell::new(None));
        let builder_attempt_slot = attempt_slot.clone();
        let provisional: AnyWindowHandle = cx
            .open_window(provisional_options(&request), move |window, cx| {
                let attempt = builder_runtime
                    .begin_live_undock_provisional_open_attempt(window.window_handle(), opening)
                    .expect("the builder must register one exact Runtime attempt");
                builder_attempt_slot.set(Some(attempt));
                finish_live_undock_open_failure(&builder_owner, opening, cx);
                reduce_live_undock_fact(
                    &builder_owner,
                    DockLiveUndockFact::Cancel {
                        identity,
                        reason: DockLiveUndockCancelReason::CaptureLost,
                    },
                    cx,
                );
                cx.new(|_| Empty)
            })
            .expect("the stale opening may still return one committed HWND")
            .into();
        let completion = runtime.complete_live_undock_provisional_open_attempt(
            attempt_slot
                .take()
                .expect("the builder must publish its exact Runtime attempt"),
            opening,
            false,
        );
        assert!(matches!(
            completion,
            crate::DockViewportProvisionalOpenAttemptCompletion::RetirementRequired(_)
        ));
        assert_eq!(
            finish_live_undock_open_return(surface.owner(), opening, provisional, completion, cx,),
            DockLiveUndockOpenReturnOutcome::Stale
        );
        let provisional_id = provisional.window_id();
        (surface, anchor, provisional_id, lease)
    });

    cx.run_until_parked();

    assert!(cx.windows().contains(&anchor));
    assert!(
        !cx.windows()
            .iter()
            .any(|window| window.window_id() == provisional_id)
    );
    assert_eq!(closed.borrow().as_slice(), [provisional_id]);
    let (windows, ownership) = cx.update(|cx| {
        let runtime = surface.viewport_runtime(cx);
        (
            runtime.windows_for_surface(lease),
            runtime.runtime_status().window_ownership,
        )
    });
    assert!(windows.iter().any(|(role, window)| {
        *role == DockViewportWindowRole::PrimaryAnchor && *window == anchor
    }));
    assert!(windows.iter().any(|(role, window)| {
        matches!(role, DockViewportWindowRole::ProvisionalViewport(_))
            && window.window_id() == provisional_id
    }));
    assert_eq!(ownership.opening_window_count, 0);
    assert_eq!(ownership.active_window_count, 1);
    assert_eq!(ownership.retiring_window_count, 1);
}

#[open_gpui::test]
fn cancelled_pending_open_failure_releases_shutdown_dependency(cx: &mut open_gpui::TestAppContext) {
    enable_provisional_test_windows(cx);
    let (surface, anchor) = cx.update(|cx| {
        cx.set_quit_mode(QuitMode::Explicit);
        let surface = crate::DockSurface::builder("main")
            .allow_platform_viewports(true)
            .build(cx)
            .expect("the surface should validate");
        let anchor = match surface.open_primary_window(WindowOptions::default(), cx) {
            DockSurfacePrimaryWindowOpenOutcome::Opened(opened) => opened.window(),
            outcome => panic!("the primary should open, got {outcome:?}"),
        };
        let lease = cx.read_entity(surface.owner(), |owner, _| {
            owner
                .window_session()
                .active_lease()
                .expect("the primary should activate one exact surface lease")
        });
        let request = begin_triggered_live_undock_opening(&surface, lease, anchor.window_id(), cx);
        let effects = prepare_surface_shutdown(
            surface.owner(),
            lease,
            DockSurfaceWindowSessionShutdownReason::AnchorCloseRequested,
            cx,
        )
        .expect("the active surface should begin shutdown");
        apply_surface_shutdown_close_effects(surface.owner(), effects, cx);
        assert!(cx.windows().contains(&anchor));
        assert!(matches!(
            finish_live_undock_open_failure(surface.owner(), request.key(), cx),
            DockLiveUndockOpenFailureOutcome::SettleDependency {
                lease: current,
                ..
            } if current == lease
        ));
        (surface, anchor)
    });

    cx.run_until_parked();

    assert!(!cx.windows().contains(&anchor));
    let status = cx.update(|cx| surface.window_session_status(cx));
    assert_eq!(status.phase(), DockSurfaceWindowSessionPhase::Closed);
    assert_eq!(status.pending_terminal_ticket_count(), 0);
    assert_eq!(status.runtime_empty(), Some(true));
}
