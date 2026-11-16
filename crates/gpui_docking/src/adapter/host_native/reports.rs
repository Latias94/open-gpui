//! Bounded application observations, after their protocol owners have handled them.

use std::collections::VecDeque;

use dockspace::runtime::NativeSurfaceCloseRequest;

use super::NativeReport;

const HISTORY_CAPACITY: usize = 256;

#[derive(Default)]
pub(super) struct Reports {
    entries: VecDeque<Entry>,
    discarded: u64,
}

struct Entry {
    report: NativeReport,
    // A deferred close is an application decision, not disposable history.
    deferred: bool,
}

impl Reports {
    pub(super) fn push_back(&mut self, report: NativeReport) {
        self.push(report, false);
    }

    pub(super) fn push_deferred_close(&mut self, close: NativeSurfaceCloseRequest) {
        if self.entries.iter().any(|entry| entry.deferred
            && matches!(&entry.report, NativeReport::CloseRequested(retained) if *retained == close))
        {
            return;
        }
        self.push(NativeReport::CloseRequested(close), true);
    }

    fn push(&mut self, report: NativeReport, deferred: bool) {
        if let NativeReport::NativeTerminal(binding) = &report {
            for entry in &mut self.entries {
                if matches!(&entry.report, NativeReport::CloseRequested(close)
                    if close.binding().window() == *binding)
                {
                    entry.deferred = false;
                }
            }
        }
        self.entries.push_back(Entry { report, deferred });
        self.trim();
    }

    pub(super) fn resolve_close(&mut self, close: NativeSurfaceCloseRequest) {
        for entry in &mut self.entries {
            if matches!(&entry.report, NativeReport::CloseRequested(retained) if *retained == close)
            {
                entry.deferred = false;
            }
        }
        self.trim();
    }

    fn trim(&mut self) {
        let mut history = self.entries.iter().filter(|entry| !entry.deferred).count();
        while history > HISTORY_CAPACITY {
            let index = self
                .entries
                .iter()
                .position(|entry| !entry.deferred)
                .unwrap();
            self.entries.remove(index);
            self.discarded = self.discarded.saturating_add(1);
            history -= 1;
        }
    }

    pub(super) fn take(&mut self) -> Vec<NativeReport> {
        self.entries.drain(..).map(|entry| entry.report).collect()
    }

    pub(super) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn discarded(&self) -> u64 {
        self.discarded
    }

    #[cfg(test)]
    pub(super) fn iter(&self) -> impl DoubleEndedIterator<Item = &NativeReport> {
        self.entries.iter().map(|entry| &entry.report)
    }

    #[cfg(test)]
    pub(super) fn front(&self) -> Option<&NativeReport> {
        self.get(0)
    }

    #[cfg(test)]
    pub(super) fn back(&self) -> Option<&NativeReport> {
        self.entries.back().map(|entry| &entry.report)
    }

    #[cfg(test)]
    pub(super) fn get(&self, index: usize) -> Option<&NativeReport> {
        self.entries.get(index).map(|entry| &entry.report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignored_observations_retain_a_bounded_ordered_suffix_and_report_loss() {
        let mut reports = Reports::default();
        reports.push_back(NativeReport::GroupShutdownRequested);
        for _ in 0..HISTORY_CAPACITY + 5 {
            reports.push_back(NativeReport::Unsupported {
                operation: "sample",
            });
        }
        assert_eq!(reports.len(), HISTORY_CAPACITY);
        assert_eq!(reports.discarded(), 6);
        assert!(reports.iter().all(|report| matches!(
            report,
            NativeReport::Unsupported {
                operation: "sample"
            }
        )));
        assert_eq!(reports.take().len(), HISTORY_CAPACITY);
        assert_eq!(reports.len(), 0);
        assert_eq!(reports.discarded(), 6);
    }
}
