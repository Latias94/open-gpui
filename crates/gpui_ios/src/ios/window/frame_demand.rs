use open_gpui::RequestFrameOptions;
use std::cell::Cell;

pub(super) struct FrameDemand {
    pending: Cell<Option<RequestFrameOptions>>,
    force_next: Cell<bool>,
}

impl Default for FrameDemand {
    fn default() -> Self {
        Self {
            pending: Cell::new(None),
            force_next: Cell::new(true),
        }
    }
}

impl FrameDemand {
    pub(super) fn queue(&self, mut options: RequestFrameOptions) {
        if let Some(mut pending) = self.pending.take() {
            pending.force_render |= options.force_render;
            pending.require_presentation |= options.require_presentation;
            if options.signal_at.is_some()
                && (pending.signal_at.is_none() || options.signal_at < pending.signal_at)
            {
                pending.signal_at = options.signal_at;
                pending.signal_source = options.signal_source;
            }
            options = pending;
        }
        self.pending.set(Some(options));
    }

    pub(super) fn is_ready(&self, surface_ready: bool) -> bool {
        surface_ready && self.pending.get().is_some()
    }

    pub(super) fn take(&self, surface_ready: bool) -> Option<RequestFrameOptions> {
        if !surface_ready {
            return None;
        }
        self.pending.take().map(|mut options| {
            options.force_render |= self.force_next.replace(false);
            options
        })
    }

    pub(super) fn clear(&self) {
        self.pending.take();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use open_gpui::FrameRequestSource;
    use std::time::{Duration, Instant};

    #[test]
    fn parked_surface_retains_demand_until_it_becomes_ready() {
        let demand = FrameDemand::default();
        demand.queue(RequestFrameOptions {
            require_presentation: true,
            ..Default::default()
        });
        for _ in 0..3 {
            assert!(!demand.is_ready(false));
            assert!(demand.take(false).is_none());
        }
        assert!(demand.is_ready(true));
        let options = demand.take(true).unwrap();
        assert!(options.force_render);
        assert!(options.require_presentation);
        assert!(!demand.is_ready(true));

        demand.queue(RequestFrameOptions::default());
        assert!(!demand.take(true).unwrap().force_render);
    }

    #[test]
    fn parked_requests_merge_flags_and_preserve_the_earliest_signal_pair() {
        let demand = FrameDemand::default();
        let earlier = Instant::now();
        let later = earlier + Duration::from_secs(1);
        demand.queue(RequestFrameOptions {
            force_render: true,
            signal_at: Some(earlier),
            signal_source: FrameRequestSource::LocalSchedule,
            ..Default::default()
        });
        assert!(demand.take(false).is_none());
        demand.queue(RequestFrameOptions {
            require_presentation: true,
            signal_at: Some(later),
            signal_source: FrameRequestSource::NativeCallback,
            ..Default::default()
        });
        let options = demand.take(true).unwrap();
        assert!(options.force_render);
        assert!(options.require_presentation);
        assert_eq!(options.signal_at, Some(earlier));
        assert_eq!(options.signal_source, FrameRequestSource::LocalSchedule);
    }

    #[test]
    fn shutdown_discards_parked_demand() {
        let demand = FrameDemand::default();
        demand.queue(RequestFrameOptions::default());
        demand.clear();
        assert!(demand.take(true).is_none());
    }
}
