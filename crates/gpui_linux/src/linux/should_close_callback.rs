use std::{cell::RefCell, rc::Rc};

type ShouldCloseCallback = Box<dyn FnMut() -> bool>;

#[derive(Default)]
struct ShouldCloseCallbackSlotState {
    generation: u64,
    checked_out_generation: Option<u64>,
    callback: Option<ShouldCloseCallback>,
    terminal: bool,
}

/// A generation-aware callback slot for synchronous native close queries.
#[derive(Clone, Default)]
pub(crate) struct ShouldCloseCallbackSlot {
    state: Rc<RefCell<ShouldCloseCallbackSlotState>>,
}

impl ShouldCloseCallbackSlot {
    /// Installs a callback and retires any callback currently checked out by a native stack.
    pub(crate) fn set(&self, callback: Box<dyn FnMut() -> bool>) {
        let previous = {
            let mut state = self.state.borrow_mut();
            if state.terminal {
                return;
            }
            state.generation = state
                .generation
                .checked_add(1)
                .expect("should-close callback generation overflowed");
            state.callback.replace(callback)
        };
        drop(previous);
    }

    /// Evaluates the current callback without holding the slot borrow across user code.
    pub(crate) fn invoke(&self) -> bool {
        let mut checkout = {
            let mut state = self.state.borrow_mut();
            if state.terminal {
                return false;
            }
            if state.checked_out_generation == Some(state.generation) {
                return false;
            }
            let Some(callback) = state.callback.take() else {
                return true;
            };
            let generation = state.generation;
            state.checked_out_generation = Some(generation);
            ShouldCloseCallbackCheckout {
                slot: self.clone(),
                generation,
                callback: Some(callback),
            }
        };

        (checkout.callback_mut())()
    }

    /// Permanently retires this window's callback slot.
    pub(crate) fn terminate(&self) {
        let callback = {
            let mut state = self.state.borrow_mut();
            if state.terminal {
                return;
            }
            state.terminal = true;
            state.generation = state
                .generation
                .checked_add(1)
                .expect("should-close callback generation overflowed");
            state.callback.take()
        };
        drop(callback);
    }
}

struct ShouldCloseCallbackCheckout {
    slot: ShouldCloseCallbackSlot,
    generation: u64,
    callback: Option<ShouldCloseCallback>,
}

impl ShouldCloseCallbackCheckout {
    fn callback_mut(&mut self) -> &mut ShouldCloseCallback {
        self.callback
            .as_mut()
            .expect("checked-out should-close callback must remain available")
    }
}

impl Drop for ShouldCloseCallbackCheckout {
    fn drop(&mut self) {
        let retired_callback = {
            let mut state = self.slot.state.borrow_mut();
            if state.checked_out_generation == Some(self.generation) {
                state.checked_out_generation = None;
                if !state.terminal
                    && state.generation == self.generation
                    && state.callback.is_none()
                {
                    state.callback = self.callback.take();
                }
            }
            self.callback.take()
        };
        drop(retired_callback);
    }
}
