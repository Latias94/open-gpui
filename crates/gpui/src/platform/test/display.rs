use crate::{Bounds, DisplayId, Pixels, PlatformDisplay, Point, px};
use anyhow::{Ok, Result};
use std::{
    cell::{Cell, RefCell},
    time::Duration,
};

#[derive(Debug, Clone)]
pub(crate) struct TestDisplay {
    id: DisplayId,
    uuid: uuid::Uuid,
    pub(crate) bounds: Bounds<Pixels>,
    pub(crate) refresh_interval: Option<Duration>,
}

impl TestDisplay {
    pub fn new() -> Self {
        Self::with_id(DisplayId(1))
    }

    pub fn with_id(id: DisplayId) -> Self {
        TestDisplay {
            id,
            uuid: uuid::Uuid::new_v4(),
            bounds: Bounds::from_corners(Point::default(), Point::new(px(1920.), px(1080.))),
            refresh_interval: Some(Duration::from_secs(1) / 60),
        }
    }
}

impl PlatformDisplay for TestDisplay {
    fn id(&self) -> crate::DisplayId {
        self.id
    }

    fn uuid(&self) -> Result<uuid::Uuid> {
        Ok(self.uuid)
    }

    fn bounds(&self) -> crate::Bounds<crate::Pixels> {
        self.bounds
    }

    fn refresh_interval(&self) -> Option<Duration> {
        self.refresh_interval
    }
}

#[derive(Default)]
pub(super) struct TestDisplayChangeCallback {
    callback: RefCell<Option<Box<dyn FnMut()>>>,
    pending: Cell<bool>,
}

impl TestDisplayChangeCallback {
    pub(super) fn set(&self, callback: Box<dyn FnMut()>) {
        self.callback.replace(Some(callback));
    }

    pub(super) fn call(&self) {
        self.pending.set(true);
        let Some(callback) = self.callback.take() else {
            return;
        };
        let mut lease = TestDisplayChangeCallbackLease {
            slot: self,
            callback: Some(callback),
        };
        while self.pending.replace(false) {
            lease.callback.as_mut().unwrap()();
        }
    }
}

struct TestDisplayChangeCallbackLease<'a> {
    slot: &'a TestDisplayChangeCallback,
    callback: Option<Box<dyn FnMut()>>,
}

impl Drop for TestDisplayChangeCallbackLease<'_> {
    fn drop(&mut self) {
        if let Some(callback) = self.callback.take() {
            self.slot.callback.borrow_mut().get_or_insert(callback);
        }
    }
}
