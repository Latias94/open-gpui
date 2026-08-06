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
    OwnerUnavailable { command_count: usize },
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

impl<Fact, Effects> DockLiveUndockEffectPumpState<Fact, Effects> {
    fn retire_unavailable_owner(&mut self) {
        self.commands.clear();
        self.scheduled = false;
    }
}

/// Cloneable, owner-bound serialization point for live-undock facts and effects.
///
/// The pump deliberately does not know how to reduce facts or execute effects. Its caller supplies
/// a narrow command handler to [`Self::drain`]. The handler runs without an outstanding borrow of
/// the pump state, so reentrant facts and follow-up effects can only append work for the outer drain.
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
    pub(crate) fn enqueue_fact(&self, fact: Fact) -> bool {
        self.enqueue(DockLiveUndockPumpCommand::Fact(fact))
    }

    /// Enqueues a batch of reducer effects if the bound owner still exists.
    pub(crate) fn enqueue_effects(&self, effects: Effects) -> bool {
        self.enqueue(DockLiveUndockPumpCommand::Effects(effects))
    }

    fn enqueue(&self, command: DockLiveUndockPumpCommand<Fact, Effects>) -> bool {
        let mut state = self.state.borrow_mut();
        if state.owner.as_ref().and_then(WeakEntity::upgrade).is_none() {
            state.retire_unavailable_owner();
            return false;
        }
        state.commands.push_back(command);
        true
    }

    /// Claims the one deferred drain that the caller should schedule.
    ///
    /// `false` means the queue is empty, a drain is already scheduled or active, or the owner is
    /// unavailable. Calls made by a command handler therefore never schedule a recursive drain.
    pub(crate) fn schedule(&self) -> bool {
        let mut state = self.state.borrow_mut();
        if state.owner.as_ref().and_then(WeakEntity::upgrade).is_none() {
            state.retire_unavailable_owner();
            return false;
        }
        if state.commands.is_empty() || state.scheduled || state.draining {
            return false;
        }
        state.scheduled = true;
        true
    }

    /// Drains commands in FIFO order without recursively invoking the handler.
    pub(crate) fn drain(
        &self,
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
            if state.owner.as_ref().and_then(WeakEntity::upgrade).is_none() {
                state.retire_unavailable_owner();
                return DockLiveUndockPumpDrainOutcome::OwnerUnavailable { command_count: 0 };
            }
            state.scheduled = false;
            state.draining = true;
        }

        let _drain_guard = DockLiveUndockDrainGuard {
            state: self.state.clone(),
        };
        let mut command_count = 0;
        loop {
            let next = {
                let mut state = self.state.borrow_mut();
                let Some(owner) = state.owner.as_ref().and_then(WeakEntity::upgrade) else {
                    state.retire_unavailable_owner();
                    return DockLiveUndockPumpDrainOutcome::OwnerUnavailable { command_count };
                };
                state.commands.pop_front().map(|command| (owner, command))
            };
            let Some((owner, command)) = next else {
                return DockLiveUndockPumpDrainOutcome::Drained { command_count };
            };

            command_count += 1;
            handle(owner, command, self);
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
            assert!(pump.enqueue_fact(1));
            assert!(pump.schedule());

            let depth = Rc::new(Cell::new(0));
            let max_depth = Rc::new(Cell::new(0));
            let handled = Rc::new(Cell::new(0));
            let outcome = pump.drain({
                let depth = depth.clone();
                let max_depth = max_depth.clone();
                let handled = handled.clone();
                move |_, command, pump| {
                    let next_depth = depth.get() + 1;
                    depth.set(next_depth);
                    max_depth.set(max_depth.get().max(next_depth));
                    handled.set(handled.get() + 1);

                    if command == DockLiveUndockPumpCommand::Fact(1) {
                        assert!(pump.enqueue_effects(2));
                        assert!(!pump.schedule());
                        assert_eq!(
                            pump.drain(|_, _, _| panic!("reentrant drain must not execute")),
                            DockLiveUndockPumpDrainOutcome::AlreadyDraining
                        );
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
            assert!(pump.enqueue_fact(1));
            assert!(pump.enqueue_effects(2));
            assert!(pump.enqueue_fact(3));
            assert!(pump.schedule());
            assert!(!pump.schedule());

            let mut observed = Vec::new();
            let outcome = pump.drain(|_, command, _| observed.push(command));

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
    fn unavailable_owner_discards_work_without_calling_handler(cx: &mut open_gpui::TestAppContext) {
        let pump = cx.update(|cx| {
            let (pump, surface) = bound_pump::<u8, u8>(cx);
            drop(surface);
            pump
        });

        assert!(!pump.enqueue_fact(1));
        assert!(!pump.schedule());
        assert_eq!(
            pump.drain(|_, _, _| panic!("an unavailable owner must not receive commands")),
            DockLiveUndockPumpDrainOutcome::OwnerUnavailable { command_count: 0 }
        );
    }
}
