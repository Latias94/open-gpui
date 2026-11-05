use crate::SharedString;
use itertools::Itertools as _;
use open_gpui_collections::FxHashSet;
use std::{
    collections::VecDeque,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

const MAX_REPORTED_MISSING_GLYPHS: usize = 1024;

/// The spacing behavior required of a fallback font.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FallbackFontClass {
    /// A proportionally spaced fallback font.
    Proportional,
    /// A fixed-width fallback font.
    Monospace,
}

/// A complete grapheme cluster that exhausted the platform's font fallback.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct MissingGlyph {
    grapheme: SharedString,
    font_class: FallbackFontClass,
}

impl MissingGlyph {
    /// Creates an unresolved grapheme report with its source run's spacing class.
    pub fn new(grapheme: SharedString, font_class: FallbackFontClass) -> Self {
        Self {
            grapheme,
            font_class,
        }
    }

    /// Returns the complete unresolved grapheme cluster.
    pub fn grapheme(&self) -> &str {
        &self.grapheme
    }

    /// Returns the spacing behavior required by the source run.
    pub fn font_class(&self) -> FallbackFontClass {
        self.font_class
    }
}

/// Receives missing graphemes from a platform shaper without application callbacks.
pub trait MissingGlyphSink: Send + Sync {
    /// Enqueues unresolved clusters without blocking shaping on application work.
    fn report(&self, missing_glyphs: Vec<MissingGlyph>);
}

struct QueuedMissingGlyph {
    generation: usize,
    missing_glyph: MissingGlyph,
}

pub(super) struct MissingGlyphReporter {
    generation: Arc<AtomicUsize>,
    sender: async_channel::Sender<QueuedMissingGlyph>,
}

impl MissingGlyphSink for MissingGlyphReporter {
    fn report(&self, missing_glyphs: Vec<MissingGlyph>) {
        if self.sender.is_closed() {
            return;
        }
        let generation = self.generation.load(Ordering::Acquire);
        for missing_glyph in missing_glyphs.into_iter().unique() {
            if self
                .sender
                .try_send(QueuedMissingGlyph {
                    generation,
                    missing_glyph,
                })
                .is_err()
            {
                break;
            }
        }
    }
}

impl MissingGlyphReporter {
    pub(super) fn reset(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    pub(super) fn generation(&self) -> usize {
        self.generation.load(Ordering::Acquire)
    }
}

#[derive(Default)]
struct MissingGlyphState {
    reported: FxHashSet<MissingGlyph>,
    reported_order: VecDeque<MissingGlyph>,
    generation: usize,
}

pub(crate) struct MissingGlyphReceiver {
    state: MissingGlyphState,
    generation: Arc<AtomicUsize>,
    receiver: async_channel::Receiver<QueuedMissingGlyph>,
}

impl MissingGlyphReceiver {
    pub(crate) fn generation(&self) -> usize {
        self.state.generation
    }

    pub(crate) async fn recv(&mut self) -> Result<Vec<MissingGlyph>, async_channel::RecvError> {
        loop {
            let first = self.receiver.recv().await?;
            let mut batch = Vec::new();
            for queued in std::iter::once(first)
                .chain(std::iter::from_fn(|| self.receiver.try_recv().ok()))
                .take(MAX_REPORTED_MISSING_GLYPHS)
            {
                let generation = self.generation.load(Ordering::Acquire);
                if self.state.generation != generation {
                    self.state.reported.clear();
                    self.state.reported_order.clear();
                    self.state.generation = generation;
                    batch.clear();
                }
                if queued.generation != generation
                    || !self.state.reported.insert(queued.missing_glyph.clone())
                {
                    continue;
                }
                self.state
                    .reported_order
                    .push_back(queued.missing_glyph.clone());
                batch.push(queued.missing_glyph);
                if self.state.reported.len() > MAX_REPORTED_MISSING_GLYPHS
                    && let Some(expired) = self.state.reported_order.pop_front()
                {
                    self.state.reported.remove(&expired);
                }
            }
            if !batch.is_empty() {
                return Ok(batch);
            }
            let mut yielded = false;
            std::future::poll_fn(|cx| {
                if std::mem::replace(&mut yielded, true) {
                    std::task::Poll::Ready(())
                } else {
                    cx.waker().wake_by_ref();
                    std::task::Poll::Pending
                }
            })
            .await;
        }
    }
}

impl Drop for MissingGlyphReceiver {
    fn drop(&mut self) {
        self.receiver.close();
        while self.receiver.try_recv().is_ok() {}
    }
}

pub(super) fn missing_glyph_channel() -> (Arc<MissingGlyphReporter>, MissingGlyphReceiver) {
    let (sender, receiver) = async_channel::bounded(MAX_REPORTED_MISSING_GLYPHS);
    let generation = Arc::default();
    (
        Arc::new(MissingGlyphReporter {
            generation: Arc::clone(&generation),
            sender,
        }),
        MissingGlyphReceiver {
            state: MissingGlyphState::default(),
            generation,
            receiver,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::FutureExt as _;

    fn glyph(value: impl Into<SharedString>) -> MissingGlyph {
        MissingGlyph::new(value.into(), FallbackFontClass::Proportional)
    }

    #[test]
    fn keeps_whole_clusters_and_separate_spacing_classes() {
        let (reporter, mut receiver) = missing_glyph_channel();
        let cluster = "👩🏽\u{200d}💻";
        let proportional = glyph(cluster);
        let monospace = MissingGlyph::new(cluster.into(), FallbackFontClass::Monospace);
        reporter.report(vec![
            proportional.clone(),
            proportional.clone(),
            monospace.clone(),
        ]);
        assert_eq!(
            receiver.recv().now_or_never().unwrap().unwrap(),
            [proportional, monospace]
        );
        reporter.report(vec![glyph(cluster)]);
        assert!(receiver.recv().now_or_never().is_none());
    }

    #[test]
    fn discards_queued_old_generations_and_resets_deduplication() {
        let (reporter, mut receiver) = missing_glyph_channel();
        reporter.report(vec![glyph("before")]);
        assert_eq!(
            receiver.recv().now_or_never().unwrap().unwrap(),
            [glyph("before")]
        );
        reporter.report(vec![glyph("obsolete")]);
        reporter.reset();
        reporter.report(vec![glyph("before"), glyph("after")]);
        assert_eq!(
            receiver.recv().now_or_never().unwrap().unwrap(),
            [glyph("before"), glyph("after")]
        );
    }

    #[test]
    fn bounds_queue_and_dedup_retention_without_losing_overflow_retry() {
        let (reporter, mut receiver) = missing_glyph_channel();
        reporter.report(
            (0..MAX_REPORTED_MISSING_GLYPHS + 1)
                .map(|i| glyph(i.to_string()))
                .collect(),
        );
        assert_eq!(reporter.sender.len(), MAX_REPORTED_MISSING_GLYPHS);
        let batch = receiver.recv().now_or_never().unwrap().unwrap();
        assert_eq!(batch.len(), MAX_REPORTED_MISSING_GLYPHS);
        let retry = glyph(MAX_REPORTED_MISSING_GLYPHS.to_string());
        reporter.report(vec![retry.clone()]);
        assert_eq!(
            receiver.recv().now_or_never().unwrap().unwrap(),
            [retry.clone()]
        );
        assert_eq!(receiver.state.reported.len(), MAX_REPORTED_MISSING_GLYPHS);
        assert_eq!(
            receiver.state.reported_order.len(),
            MAX_REPORTED_MISSING_GLYPHS
        );
        assert!(receiver.state.reported.contains(&retry));
        assert!(!receiver.state.reported.contains(&glyph("0")));
    }

    #[test]
    fn duplicate_only_batches_yield_after_bounded_work() {
        let (reporter, mut receiver) = missing_glyph_channel();
        reporter.report(vec![glyph("duplicate")]);
        receiver.recv().now_or_never().unwrap().unwrap();
        for _ in 0..MAX_REPORTED_MISSING_GLYPHS {
            reporter.report(vec![glyph("duplicate")]);
        }
        reporter.report(vec![glyph("retry")]);
        assert!(receiver.recv().now_or_never().is_none());
        assert!(reporter.sender.is_empty());
        reporter.report(vec![glyph("retry")]);
        assert_eq!(
            receiver.recv().now_or_never().unwrap().unwrap(),
            [glyph("retry")]
        );
    }

    #[test]
    fn receiver_drop_closes_and_drains_the_queue() {
        let (reporter, receiver) = missing_glyph_channel();
        reporter.report(vec![glyph("queued")]);
        drop(receiver);
        assert!(reporter.sender.is_closed());
        assert!(reporter.sender.is_empty());
        reporter.report(vec![glyph("ignored")]);
        assert!(reporter.sender.is_empty());
    }
}
