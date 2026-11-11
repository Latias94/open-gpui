use crate::WindowVisibility;
use std::cell::{Cell, RefCell};

type Callback = Box<dyn FnMut(WindowVisibility)>;

#[derive(Default)]
pub(super) struct VisibilityCallback {
    callback: RefCell<Option<Callback>>,
    pending: Cell<Option<WindowVisibility>>,
    delivering: Cell<bool>,
    terminal: Cell<bool>,
    revision: Cell<u64>,
}

impl VisibilityCallback {
    pub(super) fn set(&self, callback: Callback) {
        if !self.terminal.get() {
            self.revision.set(
                self.revision
                    .get()
                    .checked_add(1)
                    .expect("hosted visibility callback revision overflowed"),
            );
            self.callback.replace(Some(callback));
        }
    }

    pub(super) fn call(&self, visibility: WindowVisibility) {
        if self.terminal.get() {
            return;
        }
        self.pending.set(Some(visibility));
        if self.delivering.replace(true) {
            return;
        }
        let mut delivery = Delivery {
            slot: self,
            callback: None,
            revision: self.revision.get(),
        };
        while !self.terminal.get() {
            let Some(value) = self.pending.take() else {
                break;
            };
            delivery.revision = self.revision.get();
            delivery.callback = self.callback.take();
            let Some(callback) = delivery.callback.as_mut() else {
                self.pending.set(Some(value));
                break;
            };
            callback(value);
            delivery.restore();
        }
    }

    pub(super) fn terminate(&self) {
        self.terminal.set(true);
        self.pending.take();
        self.callback.take();
    }
}

struct Delivery<'a> {
    slot: &'a VisibilityCallback,
    callback: Option<Callback>,
    revision: u64,
}

impl Delivery<'_> {
    fn restore(&mut self) {
        let Some(callback) = self.callback.take() else {
            return;
        };
        if !self.slot.terminal.get() && self.slot.revision.get() == self.revision {
            let previous = self.slot.callback.replace(Some(callback));
            drop(previous);
        }
    }
}

impl Drop for Delivery<'_> {
    fn drop(&mut self) {
        self.restore();
        self.slot.delivering.set(false);
    }
}
