use std::cell::Cell;

#[derive(Default)]
pub(crate) struct WebFrameLoop {
    enabled: Cell<bool>,
    pending: Cell<Option<i32>>,
}

impl WebFrameLoop {
    pub(crate) fn start(&self) {
        self.enabled.set(true);
    }

    pub(crate) fn wake(&self, request: impl FnOnce() -> Option<i32>) {
        if self.enabled.get() && self.pending.get().is_none() {
            self.pending.set(request());
        }
    }

    pub(crate) fn frame_started(&self) -> bool {
        self.pending.set(None);
        self.enabled.get()
    }

    pub(crate) fn stop(&self) -> Option<i32> {
        self.enabled.set(false);
        self.pending.take()
    }
}

#[cfg(test)]
mod tests {
    use super::WebFrameLoop;
    use std::cell::Cell;

    #[test]
    fn frame_loop_waits_for_mapping() {
        let frame_loop = WebFrameLoop::default();

        frame_loop.wake(|| panic!("an unmapped window must not request a frame"));

        assert!(frame_loop.pending.get().is_none());
        assert!(!frame_loop.frame_started());
    }

    #[test]
    fn frame_loop_coalesces_pending_wakeups() {
        let frame_loop = WebFrameLoop::default();
        let requests = Cell::new(0);
        frame_loop.start();

        for _ in 0..3 {
            frame_loop.wake(|| {
                requests.set(requests.get() + 1);
                Some(requests.get())
            });
        }

        assert_eq!(requests.get(), 1);
        assert_eq!(frame_loop.pending.get(), Some(1));
    }

    #[test]
    fn frame_loop_parks_after_serving_demand() {
        let frame_loop = WebFrameLoop::default();
        frame_loop.start();
        frame_loop.wake(|| Some(1));

        assert!(frame_loop.frame_started());
        assert!(frame_loop.pending.get().is_none());
        assert!(frame_loop.enabled.get());

        frame_loop.wake(|| Some(2));

        assert_eq!(frame_loop.pending.get(), Some(2));
    }

    #[test]
    fn frame_loop_accepts_wakeups_during_frame_dispatch() {
        let frame_loop = WebFrameLoop::default();
        frame_loop.start();
        frame_loop.wake(|| Some(1));

        assert!(frame_loop.frame_started());
        frame_loop.wake(|| Some(2));
        frame_loop.wake(|| panic!("demand during dispatch must share the pending frame"));

        assert_eq!(frame_loop.pending.get(), Some(2));
        assert!(frame_loop.frame_started());
        assert!(frame_loop.pending.get().is_none());
    }

    #[test]
    fn frame_loop_retries_failed_requests() {
        let frame_loop = WebFrameLoop::default();
        frame_loop.start();
        frame_loop.wake(|| None);

        assert!(frame_loop.pending.get().is_none());
        frame_loop.wake(|| Some(2));

        assert_eq!(frame_loop.pending.get(), Some(2));
    }

    #[test]
    fn frame_loop_stop_retires_pending_request_once() {
        let frame_loop = WebFrameLoop::default();
        frame_loop.start();
        frame_loop.wake(|| Some(1));

        assert_eq!(frame_loop.stop(), Some(1));
        assert_eq!(frame_loop.stop(), None);
        assert!(!frame_loop.frame_started());
        frame_loop.wake(|| panic!("a stopped window must not request another frame"));
        assert!(frame_loop.pending.get().is_none());
    }

    #[test]
    fn frame_loop_stop_during_dispatch_blocks_successor() {
        let frame_loop = WebFrameLoop::default();
        frame_loop.start();
        frame_loop.wake(|| Some(1));

        assert!(frame_loop.frame_started());
        assert_eq!(frame_loop.stop(), None);
        frame_loop.wake(|| panic!("a frame must not rearm after shutdown"));
        assert!(!frame_loop.frame_started());
        assert!(frame_loop.pending.get().is_none());
    }

    #[test]
    fn frame_loops_keep_window_demand_independent() {
        let first = WebFrameLoop::default();
        let second = WebFrameLoop::default();
        first.start();
        second.start();
        first.wake(|| Some(1));
        second.wake(|| Some(2));

        assert!(first.frame_started());
        assert_eq!(second.pending.get(), Some(2));
        assert_eq!(first.stop(), None);
        assert!(second.frame_started());
        second.wake(|| Some(3));

        assert_eq!(second.pending.get(), Some(3));
        first.wake(|| panic!("a stopped window must not inherit another window's demand"));
    }
}
