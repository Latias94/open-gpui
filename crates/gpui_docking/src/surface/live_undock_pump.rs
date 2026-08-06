use super::DockSurfaceOwner;
use open_gpui::{Entity, WeakEntity};
use std::{cell::RefCell, collections::VecDeque, fmt, rc::Rc};

/// One unit of work serialized by the live-undock effect pump.
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockPumpCommand<Fact, Effects> {
    Fact(Fact),
    Effects(Effects),
}

/// Result of one attempt to drain queued live-undock work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DockLiveUndockPumpDrainOutcome {
    Drained { command_count: usize },
    AlreadyDraining,
}

/// Atomic result of offering one command to the live-undock pump.
///
/// `Accepted` proves that either the returned permit retains the owner for a new deferred drain,
/// or an existing permit/current drain already covers the command. `OwnerUnavailable` returns the
/// exact command without ever placing it in the queue.
#[must_use = "inspect accepted live-undock work and schedule any returned drain permit"]
pub(crate) enum DockLiveUndockEnqueueResult<Fact, Effects> {
    Accepted {
        drain_permit: Option<DockLiveUndockDrainPermit<Fact, Effects>>,
    },
    OwnerUnavailable(DockLiveUndockPumpCommand<Fact, Effects>),
}

impl<Fact, Effects> fmt::Debug for DockLiveUndockEnqueueResult<Fact, Effects>
where
    Fact: fmt::Debug,
    Effects: fmt::Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Accepted { drain_permit } => formatter
                .debug_struct("Accepted")
                .field("drain_permit", drain_permit)
                .finish(),
            Self::OwnerUnavailable(command) => formatter
                .debug_tuple("OwnerUnavailable")
                .field(command)
                .finish(),
        }
    }
}

struct DockLiveUndockEffectPumpState<Fact, Effects> {
    owner: Option<WeakEntity<DockSurfaceOwner>>,
    commands: VecDeque<DockLiveUndockPumpCommand<Fact, Effects>>,
    scheduled: bool,
    draining: bool,
}

impl<Fact, Effects> Default for DockLiveUndockEffectPumpState<Fact, Effects> {
    fn default() -> Self {
        Self {
            owner: None,
            commands: VecDeque::new(),
            scheduled: false,
            draining: false,
        }
    }
}

/// One-shot authority to drain queued work while retaining the surface owner.
///
/// The permit deliberately holds a strong [`Entity`] from scheduling through drain execution. A
/// command accepted before the deferred callback therefore cannot lose its only processing owner
/// in the scheduling gap. Dropping an unused permit releases the scheduled reservation without
/// clearing queued commands, allowing a later enqueue with a live owner to reserve another permit.
#[must_use = "dropping the permit releases the drain reservation without consuming queued work"]
pub(crate) struct DockLiveUndockDrainPermit<Fact, Effects> {
    state: Rc<RefCell<DockLiveUndockEffectPumpState<Fact, Effects>>>,
    owner: Option<Entity<DockSurfaceOwner>>,
}

impl<Fact, Effects> fmt::Debug for DockLiveUndockDrainPermit<Fact, Effects> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DockLiveUndockDrainPermit")
            .field("owner_retained", &self.owner.is_some())
            .finish_non_exhaustive()
    }
}

/// Cloneable, owner-bound serialization point for live-undock facts and effects.
///
/// The pump deliberately does not know how to reduce facts or execute effects. Its caller supplies
/// a narrow command handler to [`DockLiveUndockDrainPermit::drain`]. The handler runs without an
/// outstanding borrow of the pump state, so reentrant facts and follow-up effects can only append
/// work for the outer drain.
pub(crate) struct DockLiveUndockEffectPump<Fact, Effects> {
    state: Rc<RefCell<DockLiveUndockEffectPumpState<Fact, Effects>>>,
}

impl<Fact, Effects> fmt::Debug for DockLiveUndockEffectPump<Fact, Effects> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.state.borrow();
        formatter
            .debug_struct("DockLiveUndockEffectPump")
            .field("owner_bound", &state.owner.is_some())
            .field("queued_commands", &state.commands.len())
            .field("scheduled", &state.scheduled)
            .field("draining", &state.draining)
            .finish()
    }
}

impl<Fact, Effects> Clone for DockLiveUndockEffectPump<Fact, Effects> {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
        }
    }
}

impl<Fact, Effects> Default for DockLiveUndockEffectPump<Fact, Effects> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Fact, Effects> DockLiveUndockEffectPump<Fact, Effects> {
    pub(crate) fn new() -> Self {
        Self {
            state: Rc::new(RefCell::new(DockLiveUndockEffectPumpState::default())),
        }
    }

    /// Binds the pump to exactly one surface owner for its entire lifetime.
    pub(crate) fn bind_owner(&self, owner: WeakEntity<DockSurfaceOwner>) {
        let mut state = self.state.borrow_mut();
        assert!(
            state.owner.is_none(),
            "live-undock effect pump owner is already bound"
        );
        state.owner = Some(owner);
    }

    /// Enqueues a fact if the bound owner still exists.
    ///
    /// The first accepted command for an idle pump returns the permit that must be moved into the
    /// deferred drain. Later commands are already covered by that permit or the active drain.
    pub(crate) fn enqueue_fact(&self, fact: Fact) -> DockLiveUndockEnqueueResult<Fact, Effects> {
        self.enqueue(DockLiveUndockPumpCommand::Fact(fact))
    }

    /// Enqueues a batch of reducer effects if the bound owner still exists.
    pub(crate) fn enqueue_effects(
        &self,
        effects: Effects,
    ) -> DockLiveUndockEnqueueResult<Fact, Effects> {
        self.enqueue(DockLiveUndockPumpCommand::Effects(effects))
    }

    fn enqueue(
        &self,
        command: DockLiveUndockPumpCommand<Fact, Effects>,
    ) -> DockLiveUndockEnqueueResult<Fact, Effects> {
        let mut state = self.state.borrow_mut();
        if state.scheduled || state.draining {
            state.commands.push_back(command);
            return DockLiveUndockEnqueueResult::Accepted { drain_permit: None };
        }
        let Some(owner) = state.owner.as_ref().and_then(WeakEntity::upgrade) else {
            return DockLiveUndockEnqueueResult::OwnerUnavailable(command);
        };
        state.commands.push_back(command);
        state.scheduled = true;
        DockLiveUndockEnqueueResult::Accepted {
            drain_permit: Some(DockLiveUndockDrainPermit {
                state: self.state.clone(),
                owner: Some(owner),
            }),
        }
    }

    fn drain_with_owner(
        &self,
        owner: Entity<DockSurfaceOwner>,
        mut handle: impl FnMut(
            Entity<DockSurfaceOwner>,
            DockLiveUndockPumpCommand<Fact, Effects>,
            &Self,
        ),
    ) -> DockLiveUndockPumpDrainOutcome {
        {
            let mut state = self.state.borrow_mut();
            if state.draining {
                return DockLiveUndockPumpDrainOutcome::AlreadyDraining;
            }
            debug_assert!(
                state.scheduled,
                "a live-undock drain requires its scheduled permit"
            );
            state.scheduled = false;
            state.draining = true;
        }

        let _drain_guard = DockLiveUndockDrainGuard {
            state: self.state.clone(),
        };
        let mut command_count = 0;
        loop {
            let next = self.state.borrow_mut().commands.pop_front();
            let Some(command) = next else {
                return DockLiveUndockPumpDrainOutcome::Drained { command_count };
            };

            command_count += 1;
            handle(owner.clone(), command, self);
        }
    }
}

impl<Fact, Effects> DockLiveUndockDrainPermit<Fact, Effects> {
    /// Drains all currently queued and reentrantly appended commands in FIFO order.
    pub(crate) fn drain(
        mut self,
        handle: impl FnMut(
            Entity<DockSurfaceOwner>,
            DockLiveUndockPumpCommand<Fact, Effects>,
            &DockLiveUndockEffectPump<Fact, Effects>,
        ),
    ) -> DockLiveUndockPumpDrainOutcome {
        let owner = self
            .owner
            .take()
            .expect("a live-undock drain permit can only be consumed once");
        let pump = DockLiveUndockEffectPump {
            state: self.state.clone(),
        };
        pump.drain_with_owner(owner, handle)
    }
}

impl<Fact, Effects> Drop for DockLiveUndockDrainPermit<Fact, Effects> {
    fn drop(&mut self) {
        if self.owner.is_some() {
            // Preserve queued commands. A surviving owner can atomically reserve a replacement
            // permit on the next enqueue instead of losing already-accepted work.
            self.state.borrow_mut().scheduled = false;
        }
    }
}

struct DockLiveUndockDrainGuard<Fact, Effects> {
    state: Rc<RefCell<DockLiveUndockEffectPumpState<Fact, Effects>>>,
}

impl<Fact, Effects> Drop for DockLiveUndockDrainGuard<Fact, Effects> {
    fn drop(&mut self) {
        self.state.borrow_mut().draining = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    fn bound_pump<Fact, Effects>(
        cx: &mut open_gpui::App,
    ) -> (DockLiveUndockEffectPump<Fact, Effects>, crate::DockSurface) {
        let surface = crate::DockSurface::builder("main")
            .build(cx)
            .expect("test surface should validate");
        let pump = DockLiveUndockEffectPump::new();
        pump.bind_owner(surface.owner().downgrade());
        (pump, surface)
    }

    #[open_gpui::test]
    fn reentrant_work_never_recurses_the_command_handler(cx: &mut open_gpui::TestAppContext) {
        cx.update(|cx| {
            let (pump, _surface) = bound_pump::<u8, u8>(cx);
            let permit = match pump.enqueue_fact(1) {
                DockLiveUndockEnqueueResult::Accepted {
                    drain_permit: Some(permit),
                } => permit,
                result => panic!("idle enqueue should reserve a drain permit: {result:?}"),
            };

            let depth = Rc::new(Cell::new(0));
            let max_depth = Rc::new(Cell::new(0));
            let handled = Rc::new(Cell::new(0));
            let outcome = permit.drain({
                let depth = depth.clone();
                let max_depth = max_depth.clone();
                let handled = handled.clone();
                move |_, command, pump| {
                    let next_depth = depth.get() + 1;
                    depth.set(next_depth);
                    max_depth.set(max_depth.get().max(next_depth));
                    handled.set(handled.get() + 1);

                    if command == DockLiveUndockPumpCommand::Fact(1) {
                        assert!(matches!(
                            pump.enqueue_effects(2),
                            DockLiveUndockEnqueueResult::Accepted { drain_permit: None }
                        ));
                    }

                    depth.set(depth.get() - 1);
                }
            });

            assert_eq!(
                outcome,
                DockLiveUndockPumpDrainOutcome::Drained { command_count: 2 }
            );
            assert_eq!(handled.get(), 2);
            assert_eq!(max_depth.get(), 1);
        });
    }

    #[open_gpui::test]
    fn commands_are_drained_in_fifo_order(cx: &mut open_gpui::TestAppContext) {
        cx.update(|cx| {
            let (pump, _surface) = bound_pump::<u8, u8>(cx);
            let permit = match pump.enqueue_fact(1) {
                DockLiveUndockEnqueueResult::Accepted {
                    drain_permit: Some(permit),
                } => permit,
                result => panic!("idle enqueue should reserve a drain permit: {result:?}"),
            };
            assert!(matches!(
                pump.enqueue_effects(2),
                DockLiveUndockEnqueueResult::Accepted { drain_permit: None }
            ));
            assert!(matches!(
                pump.enqueue_fact(3),
                DockLiveUndockEnqueueResult::Accepted { drain_permit: None }
            ));

            let mut observed = Vec::new();
            let outcome = permit.drain(|_, command, _| observed.push(command));

            assert_eq!(
                observed,
                [
                    DockLiveUndockPumpCommand::Fact(1),
                    DockLiveUndockPumpCommand::Effects(2),
                    DockLiveUndockPumpCommand::Fact(3),
                ]
            );
            assert_eq!(
                outcome,
                DockLiveUndockPumpDrainOutcome::Drained { command_count: 3 }
            );
        });
    }

    #[open_gpui::test]
    fn scheduled_permit_retains_owner_until_fifo_drain(cx: &mut open_gpui::TestAppContext) {
        let weak_owner = cx.update(|cx| {
            let (pump, surface) = bound_pump::<u8, u8>(cx);
            let weak_owner = surface.owner().downgrade();
            let permit = match pump.enqueue_fact(1) {
                DockLiveUndockEnqueueResult::Accepted {
                    drain_permit: Some(permit),
                } => permit,
                result => panic!("idle enqueue should reserve a drain permit: {result:?}"),
            };

            drop(surface);
            assert!(
                weak_owner.upgrade().is_some(),
                "the scheduled permit must retain the owner before its deferred drain"
            );
            assert!(matches!(
                pump.enqueue_effects(2),
                DockLiveUndockEnqueueResult::Accepted { drain_permit: None }
            ));

            let mut observed = Vec::new();
            assert_eq!(
                permit.drain(|_, command, _| observed.push(command)),
                DockLiveUndockPumpDrainOutcome::Drained { command_count: 2 }
            );
            assert_eq!(
                observed,
                [
                    DockLiveUndockPumpCommand::Fact(1),
                    DockLiveUndockPumpCommand::Effects(2),
                ]
            );
            weak_owner
        });
        assert!(
            weak_owner.upgrade().is_none(),
            "the owner should be released after the permit finishes draining"
        );
    }

    #[open_gpui::test]
    fn unavailable_enqueue_returns_the_original_command(cx: &mut open_gpui::TestAppContext) {
        let pump = cx.update(|cx| {
            let (pump, surface) = bound_pump::<u8, u8>(cx);
            drop(surface);
            pump
        });

        assert!(matches!(
            pump.enqueue_fact(1),
            DockLiveUndockEnqueueResult::OwnerUnavailable(DockLiveUndockPumpCommand::Fact(1))
        ));
    }

    #[open_gpui::test]
    fn dropped_permit_does_not_clear_queued_work(cx: &mut open_gpui::TestAppContext) {
        cx.update(|cx| {
            let (pump, surface) = bound_pump::<u8, u8>(cx);
            let permit = match pump.enqueue_fact(1) {
                DockLiveUndockEnqueueResult::Accepted {
                    drain_permit: Some(permit),
                } => permit,
                result => panic!("idle enqueue should reserve a drain permit: {result:?}"),
            };
            drop(permit);
            let retry_permit = match pump.enqueue_effects(2) {
                DockLiveUndockEnqueueResult::Accepted {
                    drain_permit: Some(permit),
                } => permit,
                result => panic!("a surviving owner should allow a new permit: {result:?}"),
            };

            let mut observed = Vec::new();
            assert_eq!(
                retry_permit.drain(|_, command, _| observed.push(command)),
                DockLiveUndockPumpDrainOutcome::Drained { command_count: 2 }
            );
            assert_eq!(
                observed,
                [
                    DockLiveUndockPumpCommand::Fact(1),
                    DockLiveUndockPumpCommand::Effects(2),
                ]
            );
            drop(surface);
        });
    }
}
