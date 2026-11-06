//! Profiling utilities for task, input, frame, and presentation performance tracking.

#[cfg(feature = "profiler")]
use hdrhistogram::Histogram;
#[cfg(feature = "profiler")]
use itertools::Itertools;
use open_gpui_scheduler::{Instant, SpawnTime};
#[cfg(feature = "profiler")]
use smallvec::SmallVec;
#[cfg(feature = "profiler")]
use std::cell::LazyCell;
use std::{
    collections::{HashMap, VecDeque},
    hash::{DefaultHasher, Hash, Hasher},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    thread::ThreadId,
    time::Duration,
};

mod actions;
#[cfg(feature = "profiler")]
pub mod hang;
#[cfg(feature = "profiler")]
pub mod journal;
#[cfg(feature = "profiler")]
pub use actions::ActionHandlerGuard;
pub use actions::{ActionStatistics, ActionTiming, take_action_stats};

use serde::{Deserialize, Serialize};

#[cfg(feature = "profiler")]
use crate::{Action, App, PlatformWindowPresentOutcome, WindowId};
use crate::{SharedString, TasksIncluded};

#[cfg(feature = "profiler")]
#[cold]
#[inline(never)]
fn cold_path() {}

#[doc(hidden)]
#[cfg(feature = "profiler")]
pub fn get_all_timings(included: open_gpui::TasksIncluded) -> Vec<open_gpui::ThreadTaskTimings> {
    ThreadTaskTimings::collect(upgraded_thread_timings(), included)
}

#[doc(hidden)]
#[cfg(not(feature = "profiler"))]
pub fn get_all_timings(_included: TasksIncluded) -> Vec<ThreadTaskTimings> {
    Vec::new()
}

#[doc(hidden)]
pub fn get_current_thread_timings(included: TasksIncluded) -> open_gpui::ThreadTaskTimings {
    open_gpui::profiler::get_current_thread_task_timings(included)
}

#[doc(hidden)]
#[cfg(feature = "profiler")]
pub fn take_all_stats(included: TasksIncluded) -> Vec<open_gpui::ThreadTaskStatistics> {
    ThreadTaskStatistics::collect_and_reset(upgraded_thread_timings(), included)
}

#[doc(hidden)]
#[cfg(not(feature = "profiler"))]
pub fn take_all_stats(_included: TasksIncluded) -> Vec<ThreadTaskStatistics> {
    Vec::new()
}

#[doc(hidden)]
#[derive(Debug, Copy, Clone)]
pub struct YieldTime(pub Instant);

#[doc(hidden)]
#[derive(Copy, Clone)]
pub struct TaskTiming {
    pub location: &'static core::panic::Location<'static>,
    pub spawned: SpawnTime,
    pub start: Instant,
    pub end: YieldTime,
}

impl std::fmt::Debug for TaskTiming {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TaskTiming")
            .field("location", &self.location)
            .field("since_spawned", &self.spawned.0.elapsed())
            .field("last_poll_duration", &self.poll_duration())
            .field("total_runtime", &self.since_spawn())
            .finish()
    }
}

#[doc(hidden)]
#[derive(Debug, Copy, Clone)]
pub struct ActiveTiming {
    pub location: &'static core::panic::Location<'static>,
    pub spawned: SpawnTime,
    pub start: Instant,
}

impl TaskTiming {
    /// A task timing with a duration of zero. Any task will replace this in history.
    pub fn placeholder() -> Self {
        let now = Instant::now();
        Self {
            location: std::panic::Location::caller(),
            spawned: SpawnTime(now),
            start: now,
            end: YieldTime(now),
        }
    }

    #[inline(always)]
    pub fn poll_duration(&self) -> Duration {
        self.end.0 - self.start
    }

    #[inline(always)]
    fn since_spawn(&self) -> Duration {
        self.end.0 - self.spawned.0
    }
}

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct ThreadTaskTimings {
    pub thread_name: Option<String>,
    pub thread_id: ThreadId,
    pub timings: Vec<TaskTiming>,
    pub stats: TaskStatistics,
    pub total_pushed: u64,
}

impl ThreadTaskTimings {
    /// Converts owned, upgraded thread handles after the global registry lock is released.
    #[cfg(feature = "profiler")]
    pub fn collect(
        timings: Vec<(ThreadId, Arc<GuardedTaskTimings>)>,
        included: TasksIncluded,
    ) -> Vec<Self> {
        timings
            .into_iter()
            .map(|(thread_id, timings)| {
                let timings = timings.lock();
                let thread_name = timings.thread_name.clone();
                let total_pushed = timings.total_pushed;
                let completed = &timings.timings;

                let mut vec = Vec::with_capacity(completed.len() + 1); // +1 for running task
                let (s1, s2) = completed.as_slices();
                vec.extend_from_slice(s1);
                vec.extend_from_slice(s2);
                if let TasksIncluded::CompletedAndRunning = included
                    && let Some(running) = timings.running
                {
                    vec.push(TaskTiming {
                        location: running.location,
                        spawned: running.spawned,
                        start: running.start,
                        end: YieldTime(Instant::now()),
                    })
                }

                ThreadTaskTimings {
                    thread_name,
                    thread_id,
                    timings: vec,
                    stats: timings.stats.clone(),
                    total_pushed,
                }
            })
            .collect()
    }

    /// Returns no captured timings when profiling is not compiled in.
    #[cfg(not(feature = "profiler"))]
    pub fn collect(
        _timings: Vec<(ThreadId, Arc<GuardedTaskTimings>)>,
        _included: TasksIncluded,
    ) -> Vec<Self> {
        Vec::new()
    }
}

#[doc(hidden)]
#[derive(Debug)]
pub struct ThreadTaskStatistics {
    pub thread_name: Option<String>,
    pub thread_id: ThreadId,
    pub stats: TaskStatistics,
}

impl ThreadTaskStatistics {
    #[cfg(feature = "profiler")]
    pub fn collect_and_reset(
        timings: Vec<(ThreadId, Arc<GuardedTaskTimings>)>,
        include_running: TasksIncluded,
    ) -> Vec<Self> {
        timings
            .into_iter()
            .map(|(thread_id, timings)| {
                let mut timings = timings.lock();
                let thread_name = timings.thread_name.clone();

                let mut stats = std::mem::take(&mut timings.stats);
                if let TasksIncluded::CompletedAndRunning = include_running
                    && let Some(ActiveTiming {
                        location,
                        spawned,
                        start,
                    }) = timings.running
                {
                    let end = YieldTime(Instant::now());
                    let timing = TaskTiming {
                        location,
                        spawned,
                        start,
                        end,
                    };
                    stats.add_runtime(timing);
                    stats.add_yield_timing(timing);
                }

                Self {
                    thread_name,
                    thread_id,
                    stats,
                }
            })
            .collect()
    }

    #[cfg(not(feature = "profiler"))]
    pub fn collect_and_reset(
        _timings: Vec<(ThreadId, Arc<GuardedTaskTimings>)>,
        _include_running: TasksIncluded,
    ) -> Vec<Self> {
        Vec::new()
    }
}

/// Serializable variant of [`core::panic::Location`]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedLocation {
    /// Name of the source file
    pub file: SharedString,
    /// Line in the source file
    pub line: u32,
    /// Column in the source file
    pub column: u32,
}

impl From<&core::panic::Location<'static>> for SerializedLocation {
    fn from(value: &core::panic::Location<'static>) -> Self {
        SerializedLocation {
            file: value.file().into(),
            line: value.line(),
            column: value.column(),
        }
    }
}

/// Serializable variant of [`TaskTiming`]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedTaskTiming {
    /// Location of the timing
    pub location: SerializedLocation,
    /// Time at which the measurement was reported in nanoseconds
    pub start: u128,
    /// Duration of the measurement in nanoseconds
    pub duration: u128,
}

impl SerializedTaskTiming {
    /// Convert an array of [`TaskTiming`] into their serializable format
    ///
    /// # Params
    ///
    /// `anchor` - [`Instant`] that should be earlier than all timings to use as base anchor
    pub fn convert(anchor: Instant, timings: &[TaskTiming]) -> Vec<SerializedTaskTiming> {
        let serialized = timings
            .iter()
            .map(|timing| {
                let start = timing.start.duration_since(anchor).as_nanos();
                let duration = timing.end.0.duration_since(timing.start).as_nanos();
                SerializedTaskTiming {
                    location: timing.location.into(),
                    start,
                    duration,
                }
            })
            .collect::<Vec<_>>();

        serialized
    }

    /// `anchor` - [`Instant`] that should be earlier than all timings to use as base anchor
    pub fn from(anchor: Instant, timing: TaskTiming) -> SerializedTaskTiming {
        let start = timing.start.duration_since(anchor).as_nanos();
        let duration = timing.end.0.duration_since(timing.start).as_nanos();
        SerializedTaskTiming {
            location: timing.location.into(),
            start,
            duration,
        }
    }
}

/// Serializable variant of [`ThreadTaskTimings`]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedThreadTaskTimings {
    /// Thread name
    pub thread_name: Option<String>,
    /// Hash of the thread id
    pub thread_id: u64,
    /// Timing records for this thread
    pub timings: Vec<SerializedTaskTiming>,
}

impl SerializedThreadTaskTimings {
    /// Convert [`ThreadTaskTimings`] into their serializable format
    ///
    /// # Params
    ///
    /// `anchor` - [`Instant`] that should be earlier than all timings to use as base anchor
    pub fn convert(anchor: Instant, timings: ThreadTaskTimings) -> SerializedThreadTaskTimings {
        let serialized_timings = SerializedTaskTiming::convert(anchor, &timings.timings);

        let mut hasher = DefaultHasher::new();
        timings.thread_id.hash(&mut hasher);
        let thread_id = hasher.finish();

        SerializedThreadTaskTimings {
            thread_name: timings.thread_name,
            thread_id,
            timings: serialized_timings,
        }
    }
}

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct ThreadTimingsDelta {
    /// Hashed thread id
    pub thread_id: u64,
    /// Thread name, if known
    pub thread_name: Option<String>,
    /// New timings since the last call. If the circular buffer wrapped around
    /// since the previous poll, some entries may have been lost.
    pub new_timings: Vec<SerializedTaskTiming>,
}

/// Tracks which timing events have already been seen so that callers can request only unseen events.
#[doc(hidden)]
pub struct ProfilingCollector {
    startup_time: Instant,
    cursors: HashMap<ThreadId, u64>,
}

impl ProfilingCollector {
    pub fn new(startup_time: Instant) -> Self {
        Self {
            startup_time,
            cursors: HashMap::default(),
        }
    }

    pub fn startup_time(&self) -> Instant {
        self.startup_time
    }

    pub fn collect_unseen(
        &mut self,
        all_timings: Vec<ThreadTaskTimings>,
    ) -> Vec<ThreadTimingsDelta> {
        let mut deltas = Vec::with_capacity(all_timings.len());

        for thread in all_timings {
            let mut hasher = DefaultHasher::new();
            thread.thread_id.hash(&mut hasher);
            let hashed_id = hasher.finish();

            let prev_cursor = self.cursors.get(&thread.thread_id).copied().unwrap_or(0);
            let buffer_len = thread.timings.len() as u64;
            let buffer_start = thread.total_pushed.saturating_sub(buffer_len);

            let slice = if prev_cursor < buffer_start {
                // Cursor fell behind the buffer — some entries were evicted.
                // Return everything still in the buffer.
                thread.timings.as_slice()
            } else {
                let skip = (prev_cursor - buffer_start) as usize;
                &thread.timings[skip.min(thread.timings.len())..]
            };

            let cursor_advance = thread.total_pushed;
            self.cursors.insert(thread.thread_id, cursor_advance);

            if slice.is_empty() {
                continue;
            }

            let new_timings = SerializedTaskTiming::convert(self.startup_time, slice);

            deltas.push(ThreadTimingsDelta {
                thread_id: hashed_id,
                thread_name: thread.thread_name,
                new_timings,
            });
        }

        deltas
    }

    pub fn reset(&mut self) {
        self.cursors.clear();
    }
}

// Allow 16MiB of task timing entries.
// VecDeque grows by doubling its capacity when full, so keep this a power of 2 to avoid wasting
// memory.
#[cfg(feature = "profiler")]
const MAX_TASK_TIMINGS: usize = (16 * 1024 * 1024) / core::mem::size_of::<TaskTiming>();

#[doc(hidden)]
pub(crate) type TaskTimings = VecDeque<TaskTiming>;

#[doc(hidden)]
pub type GuardedTaskTimings = spin::Mutex<ThreadTimings>;

#[doc(hidden)]
pub struct GlobalThreadTimings {
    pub thread_id: ThreadId,
    pub timings: std::sync::Weak<GuardedTaskTimings>,
}

#[doc(hidden)]
#[derive(Debug, Clone)]
pub struct TaskStatistics {
    pub poll_time_to_beat: Duration,
    pub runtime_to_beat: Duration,
    pub longest_poll_times: [TaskTiming; 5],
    pub longest_runtimes: [TaskTiming; 5],
}

impl std::fmt::Display for TaskStatistics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Tasks that blocked the longest before yielding\n")?;
        for timing in self.longest_poll_times {
            f.write_fmt(format_args!(
                "{:<20} - {}:{}\n",
                format!("{:?}", timing.poll_duration()),
                timing.location.file(),
                timing.location.column()
            ))?;
        }
        f.write_str("Tasks that ran the longest\n")?;
        for timing in self.longest_runtimes {
            f.write_fmt(format_args!(
                "{:<20} - {}:{}\n",
                format!("{:?}", timing.since_spawn()),
                timing.location.file(),
                timing.location.column()
            ))?;
        }
        Ok(())
    }
}

impl Default for TaskStatistics {
    fn default() -> Self {
        Self {
            // Do not track polls that are not problematic
            // this keeps more calls on the fast path
            poll_time_to_beat: Duration::from_micros(100),
            runtime_to_beat: Duration::from_micros(100),
            longest_poll_times: [TaskTiming::placeholder(); 5],
            longest_runtimes: [TaskTiming::placeholder(); 5],
        }
    }
}

impl TaskStatistics {
    #[cfg(feature = "profiler")]
    #[inline(always)]
    fn add_yield_timing(&mut self, task: TaskTiming) {
        let yielded_after = task.poll_duration();
        if yielded_after >= self.poll_time_to_beat {
            cold_path(); // most tasks are not the worst, optimize for that
            let to_replace = self
                .longest_poll_times
                .iter()
                .position_min_by_key(|task| task.poll_duration())
                .expect("guarded by the comparison with nth_longest_yield_time");
            self.longest_poll_times[to_replace] = task;

            self.poll_time_to_beat = self
                .longest_poll_times
                .iter()
                .map(|task| task.poll_duration())
                .min()
                .expect("never empty");
        }
    }

    #[cfg(feature = "profiler")]
    #[inline(always)]
    fn add_runtime(&mut self, task: TaskTiming) {
        let runtime = task.since_spawn();
        if runtime >= self.runtime_to_beat {
            cold_path(); // most tasks are not the worst, optimize for that
            let to_replace = self
                .longest_runtimes
                .iter()
                .position_min_by_key(|task| task.since_spawn())
                .expect("guarded by the comparison with nth_longest_yield_time");
            self.longest_runtimes[to_replace] = task;

            self.runtime_to_beat = self
                .longest_runtimes
                .iter()
                .map(|task| task.since_spawn())
                .min()
                .expect("never empty");
        }
    }
}

#[doc(hidden)]
pub static GLOBAL_THREAD_TIMINGS: spin::Mutex<Vec<GlobalThreadTimings>> =
    spin::Mutex::new(Vec::new());

/// Upgrades live thread owners while holding only the global registry lock.
///
/// Returned owners must be read and dropped after releasing that lock: the last
/// owner runs `ThreadTimings::drop`, which acquires the registry lock again.
#[cfg(feature = "profiler")]
fn upgraded_thread_timings() -> Vec<(ThreadId, Arc<GuardedTaskTimings>)> {
    let global_thread_timings = GLOBAL_THREAD_TIMINGS.lock();
    global_thread_timings
        .iter()
        .filter_map(|thread| Some((thread.thread_id, thread.timings.upgrade()?)))
        .collect()
}

#[cfg(feature = "profiler")]
thread_local! {
    #[doc(hidden)]
    pub static THREAD_TIMINGS: LazyCell<Arc<GuardedTaskTimings>> = LazyCell::new(|| {
        let current_thread = std::thread::current();
        let thread_name = current_thread.name();
        let thread_id = current_thread.id();
        let timings = ThreadTimings::new(thread_name.map(|e| e.to_string()), thread_id);
        let timings = Arc::new(spin::Mutex::new(timings));

        {
            let timings = Arc::downgrade(&timings);
            let global_timings = GlobalThreadTimings {
                thread_id: std::thread::current().id(),
                timings,
            };
            GLOBAL_THREAD_TIMINGS.lock().push(global_timings);
        }

        timings
    });
}

#[doc(hidden)]
pub struct ThreadTimings {
    pub thread_name: Option<String>,
    pub thread_id: ThreadId,
    pub timings: TaskTimings,
    pub running: Option<ActiveTiming>,
    pub stats: TaskStatistics,
    pub total_pushed: u64,
}

impl ThreadTimings {
    pub fn new(thread_name: Option<String>, thread_id: ThreadId) -> Self {
        ThreadTimings {
            thread_name,
            thread_id,
            timings: TaskTimings::new(),
            stats: TaskStatistics::default(),
            total_pushed: 0,
            running: None,
        }
    }

    #[cfg(feature = "profiler")]
    pub fn update_running_task(
        &mut self,
        spawned: SpawnTime,
        location: &'static std::panic::Location<'_>,
    ) {
        let start = Instant::now();
        self.running = Some(ActiveTiming {
            spawned,
            location,
            start,
        });
    }

    #[cfg(not(feature = "profiler"))]
    pub fn update_running_task(&mut self, _: SpawnTime, _: &'static std::panic::Location<'_>) {}

    #[cfg(feature = "profiler")]
    pub fn save_task_timing(&mut self, ended: YieldTime) -> TaskTiming {
        let ActiveTiming {
            location,
            start,
            spawned,
        } = self
            .running
            .take()
            .expect("this function is only ever called after register_task_start");

        let timing = TaskTiming {
            location,
            spawned,
            start,
            end: ended,
        };
        self.stats.add_yield_timing(timing);
        self.stats.add_runtime(timing);

        if trace_enabled() {
            cold_path(); // optimize for when the profiling is off
            if self.timings.len() >= MAX_TASK_TIMINGS {
                self.timings.pop_front();
            }
            self.timings.push_back(timing);
            self.total_pushed += 1;
        }
        timing
    }

    #[cfg(not(feature = "profiler"))]
    pub fn save_task_timing(&mut self, _: YieldTime) {}

    // Running tasks are included in the reliability trace, which is written
    // whenever the foreground executor makes no progress for > n seconds
    #[cfg(feature = "profiler")]
    pub fn get_thread_task_timings(&self, includes: TasksIncluded) -> ThreadTaskTimings {
        ThreadTaskTimings {
            thread_name: self.thread_name.clone(),
            thread_id: self.thread_id,
            timings: self
                .timings
                .iter()
                .cloned()
                .chain(
                    self.running
                        .filter(|_| matches!(includes, TasksIncluded::CompletedAndRunning))
                        .map(|running| TaskTiming {
                            spawned: running.spawned,
                            location: running.location,
                            start: running.start,
                            end: YieldTime(Instant::now()),
                        }),
                )
                .collect(),
            stats: self.stats.clone(),
            total_pushed: self.total_pushed,
        }
    }

    #[cfg(not(feature = "profiler"))]
    pub fn get_thread_task_timings(&self, _includes: TasksIncluded) -> ThreadTaskTimings {
        empty_thread_task_timings()
    }
}

impl Drop for ThreadTimings {
    fn drop(&mut self) {
        let mut thread_timings = GLOBAL_THREAD_TIMINGS.lock();

        let Some((index, _)) = thread_timings
            .iter()
            .enumerate()
            .find(|(_, t)| t.thread_id == self.thread_id)
        else {
            return;
        };
        thread_timings.swap_remove(index);
    }
}

/// Completes one runnable's poll timing and foreground turn, including during unwind.
#[must_use]
pub struct TaskTimingGuard {
    #[cfg(feature = "profiler")]
    previous_running: Option<ActiveTiming>,
    #[cfg(feature = "profiler")]
    foreground_queued: bool,
    _not_send: std::marker::PhantomData<std::rc::Rc<()>>,
}

/// Profiles a runnable until the returned guard is dropped.
///
/// Use this instead of a bare [`update_running_task`]/[`save_task_timing`] pair
/// when a runnable may unwind. Nested scopes restore the enclosing running task.
/// Without the `profiler` feature, no timings are registered or retained.
pub fn task_timing(
    spawned: SpawnTime,
    location: &'static std::panic::Location<'_>,
) -> TaskTimingGuard {
    task_timing_scope(spawned, location, true)
}

/// Profiles work that was not counted in the foreground executor's queue.
///
/// Use this for timers and background work dispatched onto the foreground thread.
/// It records the same work and restores nested timing state without consuming
/// another runnable's foreground queue count. Without `profiler` it is a no-op.
pub fn task_timing_unqueued(
    spawned: SpawnTime,
    location: &'static std::panic::Location<'_>,
) -> TaskTimingGuard {
    task_timing_scope(spawned, location, false)
}

/// Settles one already admitted foreground runnable rejected by a later platform stage.
///
/// Call only on the originating foreground thread, after asynchronous admission
/// prevents the scheduler from receiving a synchronous rejection. Synchronous
/// dispatch rejection is settled by the scheduler's captured queue counter.
#[doc(hidden)]
pub fn foreground_task_rejected() {
    #[cfg(feature = "profiler")]
    journal::foreground_runnable_counter().finished();
}

fn task_timing_scope(
    spawned: SpawnTime,
    location: &'static std::panic::Location<'_>,
    foreground_queued: bool,
) -> TaskTimingGuard {
    #[cfg(feature = "profiler")]
    let previous_running = THREAD_TIMINGS.with(|timings| timings.lock().running);
    #[cfg(not(feature = "profiler"))]
    let _ = foreground_queued;
    update_running_task(spawned, location);
    TaskTimingGuard {
        #[cfg(feature = "profiler")]
        previous_running,
        #[cfg(feature = "profiler")]
        foreground_queued,
        _not_send: std::marker::PhantomData,
    }
}

impl Drop for TaskTimingGuard {
    fn drop(&mut self) {
        #[cfg(feature = "profiler")]
        {
            let yielded_at = YieldTime(Instant::now());
            let timing = THREAD_TIMINGS.with(|timings| {
                let mut timings = timings.lock();
                let timing = timings.save_task_timing(yielded_at);
                timings.running = self.previous_running;
                timing
            });
            if self.foreground_queued {
                journal::record_task_poll(timing);
            } else {
                journal::record_unqueued_task_poll(timing);
            }
        }
    }
}

#[doc(hidden)]
pub fn update_running_task(spawned: SpawnTime, location: &'static std::panic::Location<'_>) {
    #[cfg(feature = "profiler")]
    {
        journal::begin_foreground_turn();
        THREAD_TIMINGS.with(|timings| {
            timings.lock().update_running_task(spawned, location);
        });
    }
    #[cfg(not(feature = "profiler"))]
    let _ = (spawned, location);
}

#[doc(hidden)]
pub fn save_task_timing() {
    #[cfg(feature = "profiler")]
    {
        let yielded_at = YieldTime(Instant::now());
        let timing = THREAD_TIMINGS.with(|timings| timings.lock().save_task_timing(yielded_at));
        journal::record_task_poll(timing);
    }
}

#[doc(hidden)]
pub fn get_current_thread_task_timings(include_running: TasksIncluded) -> ThreadTaskTimings {
    #[cfg(feature = "profiler")]
    {
        THREAD_TIMINGS.with(|timings| timings.lock().get_thread_task_timings(include_running))
    }
    #[cfg(not(feature = "profiler"))]
    {
        let _ = include_running;
        empty_thread_task_timings()
    }
}

#[cfg(not(feature = "profiler"))]
fn empty_thread_task_timings() -> ThreadTaskTimings {
    ThreadTaskTimings {
        thread_name: None,
        thread_id: std::thread::current().id(),
        timings: Vec::new(),
        stats: TaskStatistics::default(),
        total_pushed: 0,
    }
}

const TRACE_SETTING_ENABLED: u64 = 1 << 63;
const TRACE_SCOPE_COUNT_MASK: u64 = TRACE_SETTING_ENABLED - 1;
static TRACE_STATE: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "profiler")]
static TRACE_LIFECYCLE_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

/// Enables or disables profiler trace collection at runtime.
///
/// Returns whether the manual setting changed. Active trace-scope guards
/// keep tracing enabled independently of this setting. When the final scope
/// and manual setting are both disabled, task and frame trace buffers are
/// cleared. Aggregate statistics remain available with the `profiler` feature.
/// Without that feature this is a no-op returning `false`.
pub fn set_trace_enabled(enabled: bool) -> bool {
    if !cfg!(feature = "profiler") {
        return false;
    }
    #[cfg(feature = "profiler")]
    let _lifecycle = TRACE_LIFECYCLE_LOCK.lock();
    let mut state = TRACE_STATE.load(Ordering::Acquire);
    loop {
        if (state & TRACE_SETTING_ENABLED != 0) == enabled {
            return false;
        }
        let next_state = if enabled {
            state | TRACE_SETTING_ENABLED
        } else {
            state & TRACE_SCOPE_COUNT_MASK
        };
        match TRACE_STATE.compare_exchange_weak(
            state,
            next_state,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                if next_state == 0 {
                    clear_trace_buffers();
                }
                return true;
            }
            Err(updated_state) => state = updated_state,
        }
    }
}

/// Keeps trace collection enabled until this guard is dropped.
#[cfg(all(feature = "profiler", any(test, feature = "test-support")))]
#[must_use]
pub(crate) struct TraceGuard {
    _private: (),
}

/// Starts a reference-counted trace scope, independent of the manual setting.
#[cfg(all(feature = "profiler", any(test, feature = "test-support")))]
pub(crate) fn trace_scope() -> TraceGuard {
    let _lifecycle = TRACE_LIFECYCLE_LOCK.lock();
    let incremented = TRACE_STATE.fetch_update(Ordering::AcqRel, Ordering::Acquire, |state| {
        (state & TRACE_SCOPE_COUNT_MASK < TRACE_SCOPE_COUNT_MASK).then(|| state + 1)
    });
    assert!(incremented.is_ok(), "too many active profiler trace scopes");
    TraceGuard { _private: () }
}

#[cfg(all(feature = "profiler", any(test, feature = "test-support")))]
impl Drop for TraceGuard {
    fn drop(&mut self) {
        let _lifecycle = TRACE_LIFECYCLE_LOCK.lock();
        let previous_state =
            TRACE_STATE.fetch_update(Ordering::AcqRel, Ordering::Acquire, |state| {
                (state & TRACE_SCOPE_COUNT_MASK > 0).then(|| state - 1)
            });
        match previous_state {
            Ok(1) => clear_trace_buffers(),
            Ok(_) => {}
            Err(_) => debug_assert!(false, "profiler trace scope count underflowed"),
        }
    }
}

/// Returns whether profiler trace collection is enabled.
pub fn trace_enabled() -> bool {
    cfg!(feature = "profiler") && TRACE_STATE.load(Ordering::Relaxed) != 0
}

fn clear_trace_buffers() {
    #[cfg(feature = "profiler")]
    {
        for (_, timings) in upgraded_thread_timings() {
            let mut timings = timings.lock();
            timings.timings.clear();
            timings.timings.shrink_to_fit();
        }
        let mut frames = FRAME_TIMINGS.lock();
        frames.timings.clear();
        frames.timings.shrink_to_fit();
    }
}

/// Timing for a single accepted window draw.
#[cfg(feature = "profiler")]
#[derive(Debug, Copy, Clone)]
pub struct FrameTiming {
    /// The window that was drawn.
    pub window_id: WindowId,
    /// The frame's first observed invalidation, when known.
    pub dirty_at: Option<Instant>,
    /// Number of invalidations coalesced into this frame.
    pub invalidations: u64,
    /// When the draw started.
    pub draw_start: Instant,
    /// When the draw finished.
    pub draw_end: Instant,
}

#[cfg(feature = "profiler")]
impl FrameTiming {
    /// Time spent drawing the frame.
    pub fn draw_duration(&self) -> Duration {
        self.draw_end.duration_since(self.draw_start)
    }

    /// Time from the first observed invalidation to the end of the draw.
    pub fn dirty_to_draw_duration(&self) -> Option<Duration> {
        self.dirty_at
            .map(|dirty_at| self.draw_end.duration_since(dirty_at))
    }
}

/// Work spent attempting to submit a window frame to the platform.
///
/// A journal event can describe an unsuccessful attempt or an unchanged scene;
/// successful newly drawn frames additionally produce a presentation boundary.
#[cfg(feature = "profiler")]
#[derive(Debug, Copy, Clone)]
pub struct PresentTiming {
    /// The window whose frame was submitted.
    pub window_id: WindowId,
    /// When the platform submission began.
    pub present_start: Instant,
    /// When the platform submission completed.
    pub present_end: Instant,
    /// Interval since the preceding new frame's submission in an active animation.
    pub animation_interval: Option<Duration>,
}

#[cfg(feature = "profiler")]
impl PresentTiming {
    /// Time spent submitting the frame to the platform.
    pub fn present_duration(&self) -> Duration {
        self.present_end.duration_since(self.present_start)
    }
}

/// A frame event retained while profiler tracing is enabled.
#[cfg(feature = "profiler")]
#[derive(Debug, Copy, Clone)]
pub enum FrameEvent {
    /// An accepted window frame was drawn.
    Draw(FrameTiming),
    /// A newly drawn window frame was submitted.
    Present(PresentTiming),
}

/// A point-in-time snapshot of a window's frame-duration histograms.
#[cfg(feature = "profiler")]
#[derive(Clone)]
pub struct FrameDurationSnapshot {
    /// Time from the first observed invalidation to submission completion, in nanoseconds.
    pub dirty_to_present_histogram: Histogram<u64>,
    /// Durations of accepted window draws, in nanoseconds.
    pub draw_duration_histogram: Histogram<u64>,
    /// Intervals between newly submitted animation frames, in nanoseconds.
    pub present_interval_histogram: Histogram<u64>,
}

/// A point-in-time snapshot of input latency and actual window presentation diagnostics.
#[cfg(feature = "profiler")]
#[derive(Clone)]
pub struct InputLatencySnapshot {
    /// Input-to-submission-completion latency samples, in nanoseconds.
    pub latency_histogram: Histogram<u64>,
    /// Input events coalesced per submitted frame.
    pub events_per_frame_histogram: Histogram<u64>,
    /// Invalidating input events received during a draw and excluded from latency recording.
    pub mid_draw_events_dropped: u64,
    /// Recorded input events still waiting for a submitted frame.
    pub pending_input_count: u64,
    /// Accepted framework frames built during this measurement window.
    pub drawn_frame_count: u64,
    /// Frames handed to the platform presentation path.
    pub present_attempt_count: u64,
    /// Presentation attempts submitted to the platform renderer.
    pub submitted_present_count: u64,
    /// Presentation attempts deferred by the native surface.
    pub deferred_present_count: u64,
    /// Presentation attempts requiring a newer rendered frame.
    pub repaint_required_present_count: u64,
    /// Presentation attempts rejected by the renderer or native surface.
    pub rejected_present_count: u64,
    /// Redraw requests admitted by the window invalidator.
    pub refresh_request_count: u64,
    /// Admitted redraw requests that found the window already dirty.
    pub coalesced_refresh_request_count: u64,
    /// Redraw requests grouped by their direct caller's file and line.
    pub refresh_callsites: Vec<InputLatencyRefreshCallsite>,
}

/// Input-latency diagnostics for one direct [`crate::Window::refresh`] caller.
#[cfg(feature = "profiler")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InputLatencyRefreshCallsite {
    /// Source file containing the redraw request.
    pub file: &'static str,
    /// Source line containing the redraw request.
    pub line: u32,
    /// Admitted redraw requests from this caller.
    pub request_count: u64,
    /// Requests from this caller that found the window already dirty.
    pub coalesced_request_count: u64,
}

#[cfg(feature = "profiler")]
enum WindowActivity {
    Input {
        started_at: Instant,
        kind: &'static str,
        covering_draw: Option<(Instant, usize)>,
        measurement_generation: u64,
        _turn: journal::ForegroundTurnGuard,
    },
    Draw {
        started_at: Instant,
        _turn: journal::ForegroundTurnGuard,
    },
}

#[cfg(feature = "profiler")]
#[derive(Clone, Copy)]
struct PendingFrame {
    timing: FrameTiming,
    first_input_at: Option<Instant>,
    input_count: u64,
}

/// Captures the enclosing activity depth and accepted pending frame for one draw attempt.
#[cfg(feature = "profiler")]
#[must_use]
pub(crate) struct DrawCheckpoint {
    activity_depth: usize,
    pending_frame: Option<PendingFrame>,
    input_measurement_generation: u64,
}

/// Collects profiling information for one window.
///
/// Aggregate histograms and diagnostics are collected whenever the `profiler`
/// feature is compiled in. Individual task, draw, and presentation records are
/// retained in the trace buffers only while tracing is enabled; the foreground
/// journal records completed work independently. Draw scopes must end after
/// candidate acceptance or be canceled with [`Self::abort_draw`] on refusal or unwind.
#[cfg(feature = "profiler")]
pub struct WindowProfiler {
    window_id: WindowId,
    active_activities: SmallVec<[WindowActivity; 4]>,
    dirty_to_present_histogram: Histogram<u64>,
    draw_duration_histogram: Histogram<u64>,
    present_interval_histogram: Histogram<u64>,
    first_input_at: Option<Instant>,
    input_after_draw_at: Option<Instant>,
    pending_input_count: u64,
    input_measurement_generation: u64,
    input_latency_histogram: Histogram<u64>,
    events_per_frame_histogram: Histogram<u64>,
    mid_draw_events_dropped: u64,
    last_present_at: Option<Instant>,
    animating_at_last_present: bool,
    pending_frame: Option<PendingFrame>,
    drawn_frame_count: u64,
    present_attempt_count: u64,
    submitted_present_count: u64,
    deferred_present_count: u64,
    repaint_required_present_count: u64,
    rejected_present_count: u64,
    refresh_request_count: u64,
    coalesced_refresh_request_count: u64,
    refresh_callsites: Vec<InputLatencyRefreshCallsite>,
    _not_send: std::marker::PhantomData<std::rc::Rc<()>>,
}

#[cfg(feature = "profiler")]
impl WindowProfiler {
    /// Creates a profiler for a window.
    ///
    /// Returns an error if any aggregate histogram cannot be created.
    pub fn new(window_id: WindowId) -> anyhow::Result<Self> {
        let profiler = Self {
            window_id,
            active_activities: SmallVec::new(),
            dirty_to_present_histogram: Histogram::new(3).map_err(|error| {
                anyhow::anyhow!("Failed to create dirty-to-present histogram: {error}")
            })?,
            draw_duration_histogram: Histogram::new(3).map_err(|error| {
                anyhow::anyhow!("Failed to create draw duration histogram: {error}")
            })?,
            present_interval_histogram: Histogram::new(3).map_err(|error| {
                anyhow::anyhow!("Failed to create present interval histogram: {error}")
            })?,
            first_input_at: None,
            input_after_draw_at: None,
            pending_input_count: 0,
            input_measurement_generation: 0,
            input_latency_histogram: Histogram::new(3).map_err(|error| {
                anyhow::anyhow!("Failed to create input latency histogram: {error}")
            })?,
            events_per_frame_histogram: Histogram::new(3).map_err(|error| {
                anyhow::anyhow!("Failed to create events per frame histogram: {error}")
            })?,
            mid_draw_events_dropped: 0,
            last_present_at: None,
            animating_at_last_present: false,
            pending_frame: None,
            drawn_frame_count: 0,
            present_attempt_count: 0,
            submitted_present_count: 0,
            deferred_present_count: 0,
            repaint_required_present_count: 0,
            rejected_present_count: 0,
            refresh_request_count: 0,
            coalesced_refresh_request_count: 0,
            refresh_callsites: Vec::new(),
            _not_send: std::marker::PhantomData,
        };
        journal::record_frame_pending(window_id, Instant::now());
        Ok(profiler)
    }

    /// Records the beginning of an input dispatch.
    ///
    /// `kind` names the platform input variant. Aggregate measurements do not
    /// retain individual input event names.
    pub fn begin_input(&mut self, kind: &'static str) {
        let turn = journal::foreground_turn();
        self.active_activities.push(WindowActivity::Input {
            started_at: Instant::now(),
            kind,
            covering_draw: None,
            measurement_generation: self.input_measurement_generation,
            _turn: turn,
        });
    }

    /// Ends the current input dispatch and journals its work.
    ///
    /// Only invalidating input contributes to latency aggregates. Input received
    /// inside a draw is excluded because it cannot affect that draw. Nested input
    /// dispatches retain the earliest invalidating input.
    pub fn end_input(&mut self, caused_invalidation: bool) {
        self.finish_input(caused_invalidation, None);
    }

    pub(crate) fn end_input_dispatch(&mut self, before: usize, after: usize, completed: bool) {
        if !completed
            && let Some(index) = self
                .active_activities
                .iter()
                .rposition(|activity| matches!(activity, WindowActivity::Input { .. }))
        {
            self.truncate_activities(index + 1);
        }
        self.finish_input(completed && after > before, Some((before, after)));
    }

    pub(crate) fn note_accepted_draw_covering_input(&mut self, invalidations: usize) {
        let Some(frame) = self.pending_frame else {
            return;
        };
        for activity in &mut self.active_activities {
            if let WindowActivity::Input { covering_draw, .. } = activity {
                *covering_draw = Some((frame.timing.draw_start, invalidations));
            }
        }
    }

    fn finish_input(&mut self, caused_invalidation: bool, dispatch: Option<(usize, usize)>) {
        if !matches!(
            self.active_activities.last(),
            Some(WindowActivity::Input { .. })
        ) {
            debug_assert!(false, "input activity must be the current window activity");
            return;
        }
        let Some(WindowActivity::Input {
            started_at,
            kind,
            covering_draw,
            measurement_generation,
            _turn,
        }) = self.active_activities.pop()
        else {
            unreachable!("checked the current input activity");
        };
        if !journal::power_interrupted_since(started_at) {
            journal::record_input(journal::InputTiming {
                kind,
                start: started_at,
                end: Instant::now(),
                caused_invalidation,
            });
        }
        self.discard_stale_input_samples();
        if !caused_invalidation
            || measurement_generation != self.input_measurement_generation
            || !journal::frame_sample_is_valid(self.window_id, started_at)
        {
            return;
        }
        if self
            .active_activities
            .iter()
            .any(|activity| matches!(activity, WindowActivity::Draw { .. }))
        {
            self.mid_draw_events_dropped += 1;
        } else {
            self.first_input_at = Some(
                self.first_input_at
                    .map_or(started_at, |first| first.min(started_at)),
            );
            if let Some(frame) = self.pending_frame.as_mut() {
                let covered = match (covering_draw, dispatch) {
                    (Some((draw_start, covered_at)), Some((before, after))) => {
                        draw_start == frame.timing.draw_start
                            && covered_at > before
                            && after == covered_at
                    }
                    _ => false,
                };
                if covered {
                    frame.first_input_at = Some(
                        frame
                            .first_input_at
                            .map_or(started_at, |first| first.min(started_at)),
                    );
                    frame.input_count += 1;
                } else {
                    self.input_after_draw_at = Some(
                        self.input_after_draw_at
                            .map_or(started_at, |first| first.min(started_at)),
                    );
                }
            }
            self.pending_input_count += 1;
        }
    }

    /// Begins an action handler whose returned guard also completes on unwind.
    pub fn begin_action_handler(
        &mut self,
        action: &(dyn Action + 'static),
        cx: &mut App,
    ) -> ActionHandlerGuard {
        actions::update_running_action(action, cx)
    }

    /// Completes the action handler owned by this guard.
    pub fn end_action_handler(&mut self, handler: ActionHandlerGuard) {
        drop(handler);
    }

    /// Begins a candidate draw without replacing the previous accepted pending frame.
    pub fn begin_draw(&mut self) {
        let turn = journal::foreground_turn();
        let started_at = Instant::now();
        journal::record_frame_pending(self.window_id, started_at);
        self.active_activities.push(WindowActivity::Draw {
            started_at,
            _turn: turn,
        });
    }

    /// Captures state before a draw attempt that may refuse or unwind before beginning a draw.
    ///
    /// The token must be consumed by this profiler's finish or abort method.
    pub(crate) fn checkpoint_draw(&self) -> DrawCheckpoint {
        DrawCheckpoint {
            activity_depth: self.active_activities.len(),
            pending_frame: self.pending_frame,
            input_measurement_generation: self.input_measurement_generation,
        }
    }

    /// Ends a draw begun after the checkpoint only when its candidate was accepted.
    ///
    /// Refusal or an attempt that never began a draw restores the checkpoint and
    /// returns zero. An accepted attempt must first finish its nested activities.
    pub(crate) fn finish_draw_checkpoint(
        &mut self,
        checkpoint: DrawCheckpoint,
        dirty_at: Option<Instant>,
        invalidations: u64,
        accepted: bool,
    ) -> Duration {
        if !accepted || self.active_activities.len() <= checkpoint.activity_depth {
            self.abort_draw_checkpoint(checkpoint);
            return Duration::ZERO;
        }
        if self.active_activities.len() != checkpoint.activity_depth + 1
            || !matches!(
                self.active_activities.last(),
                Some(WindowActivity::Draw { .. })
            )
        {
            self.abort_draw_checkpoint(checkpoint);
            debug_assert!(false, "accepted draw must finish its nested activities");
            return Duration::ZERO;
        }
        let duration = self.end_draw(dirty_at, invalidations);
        self.truncate_activities(checkpoint.activity_depth);
        duration
    }

    /// Restores the accepted pending frame and enclosing activity depth after refusal or unwind.
    /// Input measurements reset during the attempt remain cleared after rollback.
    pub(crate) fn abort_draw_checkpoint(&mut self, checkpoint: DrawCheckpoint) {
        self.truncate_activities(checkpoint.activity_depth);
        self.pending_frame = checkpoint.pending_frame;
        if checkpoint.input_measurement_generation != self.input_measurement_generation
            && let Some(pending) = self.pending_frame.as_mut()
        {
            pending.first_input_at = None;
            pending.input_count = 0;
        }
    }

    /// Discards pending frame timing after a scene was accepted but its draw unwound.
    ///
    /// Input remains pending for a subsequent completed and accepted draw.
    pub(crate) fn discard_pending_frame(&mut self) {
        self.pending_frame = None;
    }

    /// Accepts the current draw and returns its duration.
    ///
    /// Call this only after the framework accepts the candidate frame. A refused
    /// or unwinding candidate must instead call [`Self::abort_draw`]. An unmatched
    /// end asserts in debug builds and leaves activities unchanged otherwise.
    pub fn end_draw(&mut self, dirty_at: Option<Instant>, invalidations: u64) -> Duration {
        if !matches!(
            self.active_activities.last(),
            Some(WindowActivity::Draw { .. })
        ) {
            debug_assert!(false, "draw activity must be the current window activity");
            return Duration::ZERO;
        }
        let Some(WindowActivity::Draw {
            started_at: draw_start,
            _turn,
        }) = self.active_activities.pop()
        else {
            unreachable!("checked the current draw activity");
        };
        let timing = FrameTiming {
            window_id: self.window_id,
            dirty_at: dirty_at.filter(|at| journal::frame_sample_is_valid(self.window_id, *at)),
            invalidations,
            draw_start,
            draw_end: Instant::now(),
        };
        let duration = timing.draw_duration();
        if !journal::power_interrupted_since(draw_start) {
            self.record_draw_timing(timing);
        } else {
            self.discard_pending_frame();
            self.discard_stale_input_samples();
        }
        duration
    }

    /// Cancels the innermost draw and any unfinished activities nested inside it.
    ///
    /// This is safe to call from refusal or unwind cleanup, and is a no-op
    /// without an active draw. It does not publish a draw event or change the
    /// previous accepted pending frame, pending input, or aggregate histograms.
    pub fn abort_draw(&mut self) {
        if let Some(index) = self
            .active_activities
            .iter()
            .rposition(|activity| matches!(activity, WindowActivity::Draw { .. }))
        {
            self.truncate_activities(index);
        }
    }

    /// Records successful platform submission using the caller's actual timestamps.
    ///
    /// `present_end` is submission completion, not a display scanout timestamp.
    /// Input latency ends at `present_end` and includes only the inputs covered
    /// by the accepted draw. Later input remains pending for a subsequent draw.
    /// All aggregate samples require a newly accepted pending draw; presenting
    /// an unchanged scene journals submission work without completing input or
    /// adding frame samples.
    /// `next_frame_scheduled` sets animation state for the next new frame.
    ///
    /// This method does not increment presentation outcome counters. Window
    /// integrations should call [`Self::record_present_outcome`] once per actual
    /// attempt. The caller must supply ordered timestamps from the same clock.
    /// Histogram recording failures are logged and do not change submission.
    pub fn record_present(
        &mut self,
        present_start: Instant,
        present_end: Instant,
        window_active: bool,
        next_frame_scheduled: bool,
    ) {
        self.discard_stale_input_samples();
        let pending = self
            .pending_frame
            .take()
            .filter(|pending| {
                journal::frame_sample_is_valid(self.window_id, pending.timing.draw_start)
            })
            .map(|mut pending| {
                pending.timing.dirty_at = pending
                    .timing
                    .dirty_at
                    .filter(|at| journal::frame_sample_is_valid(self.window_id, *at));
                pending
            });
        let interrupted = journal::power_interrupted_since(present_start);
        let pending = pending.filter(|_| !interrupted);
        let animation_interval =
            if pending.is_some() && self.animating_at_last_present && window_active {
                self.last_present_at
                    .filter(|at| journal::frame_sample_is_valid(self.window_id, *at))
                    .map(|last_present| present_end.duration_since(last_present))
            } else {
                None
            };
        let present_timing = PresentTiming {
            window_id: self.window_id,
            present_start,
            present_end,
            animation_interval,
        };
        journal::record_present(present_timing, pending.map(|pending| pending.timing));
        let Some(pending) = pending else {
            self.input_after_draw_at = None;
            return;
        };
        if let Some(first_input_at) = pending.first_input_at {
            record_duration(
                &mut self.input_latency_histogram,
                present_end.duration_since(first_input_at),
            );
        }
        if pending.input_count > 0 {
            if let Err(error) = self.events_per_frame_histogram.record(pending.input_count) {
                log::error!("failed to record input events per frame: {error}");
            }
        }
        self.pending_input_count = self.pending_input_count.saturating_sub(pending.input_count);
        self.first_input_at = self.input_after_draw_at.take();
        let frame = pending.timing;
        if let Some(dirty_at) = frame.dirty_at {
            record_duration(
                &mut self.dirty_to_present_histogram,
                present_end.duration_since(dirty_at),
            );
        }
        if let Some(interval) = animation_interval {
            record_duration(&mut self.present_interval_histogram, interval);
        }
        record_frame_event(FrameEvent::Present(present_timing));
        self.last_present_at = Some(present_end);
        self.animating_at_last_present = next_frame_scheduled && window_active;
    }

    /// Records exactly one actual platform presentation outcome.
    ///
    /// Every call increments the attempt counter and its outcome counter. Only
    /// [`PlatformWindowPresentOutcome::Submitted`] records successful timing
    /// through [`Self::record_present`]. Deferred, repaint-required, and rejected
    /// attempts retain pending input and the previous accepted pending frame.
    /// The framework remains responsible for choosing which scene may be retried.
    ///
    /// Pass the actual start and end of the platform call, including refusal
    /// paths, from the same monotonic clock used by the profiler. This method
    /// returns no error and never changes the platform's outcome; failures to
    /// record histogram samples are logged. Submission is not proof of scanout.
    pub fn record_present_outcome(
        &mut self,
        outcome: PlatformWindowPresentOutcome,
        present_start: Instant,
        present_end: Instant,
        window_active: bool,
        next_frame_scheduled: bool,
    ) {
        self.present_attempt_count += 1;
        if outcome != PlatformWindowPresentOutcome::Submitted {
            journal::record_unsubmitted_present(PresentTiming {
                window_id: self.window_id,
                present_start,
                present_end,
                animation_interval: None,
            });
        }
        match outcome {
            PlatformWindowPresentOutcome::Submitted => {
                self.submitted_present_count += 1;
                self.record_present(
                    present_start,
                    present_end,
                    window_active,
                    next_frame_scheduled,
                );
            }
            PlatformWindowPresentOutcome::Deferred => self.deferred_present_count += 1,
            PlatformWindowPresentOutcome::RepaintRequired => {
                self.repaint_required_present_count += 1;
            }
            PlatformWindowPresentOutcome::Rejected => self.rejected_present_count += 1,
        }
    }

    /// Records an admitted redraw request with its exact direct caller's file and line.
    pub fn record_refresh_request(
        &mut self,
        already_dirty: bool,
        caller: &'static std::panic::Location<'static>,
    ) {
        self.refresh_request_count += 1;
        self.coalesced_refresh_request_count += u64::from(already_dirty);
        if let Some(callsite) = self
            .refresh_callsites
            .iter_mut()
            .find(|callsite| callsite.file == caller.file() && callsite.line == caller.line())
        {
            callsite.request_count += 1;
            callsite.coalesced_request_count += u64::from(already_dirty);
        } else {
            self.refresh_callsites.push(InputLatencyRefreshCallsite {
                file: caller.file(),
                line: caller.line(),
                request_count: 1,
                coalesced_request_count: u64::from(already_dirty),
            });
        }
    }

    /// Returns the current input histograms, pending inputs, and actual consumer diagnostics.
    pub fn input_latency_snapshot(&self) -> InputLatencySnapshot {
        InputLatencySnapshot {
            latency_histogram: self.input_latency_histogram.clone(),
            events_per_frame_histogram: self.events_per_frame_histogram.clone(),
            mid_draw_events_dropped: self.mid_draw_events_dropped,
            pending_input_count: self.pending_input_count,
            drawn_frame_count: self.drawn_frame_count,
            present_attempt_count: self.present_attempt_count,
            submitted_present_count: self.submitted_present_count,
            deferred_present_count: self.deferred_present_count,
            repaint_required_present_count: self.repaint_required_present_count,
            rejected_present_count: self.rejected_present_count,
            refresh_request_count: self.refresh_request_count,
            coalesced_refresh_request_count: self.coalesced_refresh_request_count,
            refresh_callsites: self.refresh_callsites.clone(),
        }
    }

    /// Returns the current accepted draw and successful presentation histograms.
    pub fn frame_duration_snapshot(&self) -> FrameDurationSnapshot {
        FrameDurationSnapshot {
            dirty_to_present_histogram: self.dirty_to_present_histogram.clone(),
            draw_duration_histogram: self.draw_duration_histogram.clone(),
            present_interval_histogram: self.present_interval_histogram.clone(),
        }
    }

    /// Clears recorded and pending input latency and all input snapshot diagnostics.
    ///
    /// Accepted pending frames, active scopes, frame histograms, and animation
    /// continuity are preserved. Refresh callsite rows are cleared exactly.
    pub fn reset_input_latency_measurements(&mut self) {
        self.input_measurement_generation = self
            .input_measurement_generation
            .checked_add(1)
            .expect("input measurement generation exhausted");
        self.first_input_at = None;
        self.input_after_draw_at = None;
        self.pending_input_count = 0;
        if let Some(pending) = self.pending_frame.as_mut() {
            pending.first_input_at = None;
            pending.input_count = 0;
        }
        self.input_latency_histogram.reset();
        self.events_per_frame_histogram.reset();
        self.mid_draw_events_dropped = 0;
        self.drawn_frame_count = 0;
        self.present_attempt_count = 0;
        self.submitted_present_count = 0;
        self.deferred_present_count = 0;
        self.repaint_required_present_count = 0;
        self.rejected_present_count = 0;
        self.refresh_request_count = 0;
        self.coalesced_refresh_request_count = 0;
        self.refresh_callsites.clear();
    }

    fn record_draw_timing(&mut self, timing: FrameTiming) {
        self.discard_stale_input_samples();
        record_duration(&mut self.draw_duration_histogram, timing.draw_duration());
        self.drawn_frame_count += 1;
        self.pending_frame = Some(PendingFrame {
            timing,
            first_input_at: self.first_input_at,
            input_count: self.pending_input_count,
        });
        self.input_after_draw_at = None;
        record_frame_event(FrameEvent::Draw(timing));
        journal::record_draw(timing);
    }

    fn discard_stale_input_samples(&mut self) {
        if self
            .first_input_at
            .is_some_and(|at| !journal::frame_sample_is_valid(self.window_id, at))
        {
            self.first_input_at = None;
            self.input_after_draw_at = None;
            self.pending_input_count = 0;
        }
        if let Some(pending) = self.pending_frame.as_mut()
            && pending
                .first_input_at
                .is_some_and(|at| !journal::frame_sample_is_valid(self.window_id, at))
        {
            pending.first_input_at = None;
            pending.input_count = 0;
        }
    }

    fn truncate_activities(&mut self, depth: usize) {
        while self.active_activities.len() > depth {
            self.active_activities.pop();
        }
    }
}

#[cfg(feature = "profiler")]
impl Drop for WindowProfiler {
    fn drop(&mut self) {
        self.truncate_activities(0);
        journal::record_window_closed(self.window_id);
    }
}

#[cfg(feature = "profiler")]
fn record_duration(histogram: &mut Histogram<u64>, duration: Duration) {
    let nanos = u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX);
    if let Err(error) = histogram.record(nanos) {
        log::error!("failed to record profiler duration: {error}");
    }
}

#[cfg(feature = "profiler")]
const MAX_FRAME_TIMINGS: usize = (16 * 1024 * 1024) / core::mem::size_of::<FrameEvent>();

#[cfg(feature = "profiler")]
struct FrameTimings {
    timings: VecDeque<FrameEvent>,
    total_pushed: u64,
}

#[cfg(feature = "profiler")]
static FRAME_TIMINGS: spin::Mutex<FrameTimings> = spin::Mutex::new(FrameTimings {
    timings: VecDeque::new(),
    total_pushed: 0,
});

/// Records a frame event only while profiler tracing is enabled.
#[cfg(feature = "profiler")]
pub fn record_frame_event(event: FrameEvent) {
    if !trace_enabled() {
        return;
    }
    cold_path();
    let mut frames = FRAME_TIMINGS.lock();
    if !trace_enabled() {
        return;
    }
    if frames.timings.len() >= MAX_FRAME_TIMINGS {
        frames.timings.pop_front();
    }
    frames.timings.push_back(event);
    frames.total_pushed += 1;
}

/// Tracks unseen frame events in the bounded global trace buffer.
#[cfg(feature = "profiler")]
pub struct FrameTimingCollector {
    cursor: u64,
}

#[cfg(feature = "profiler")]
impl Default for FrameTimingCollector {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "profiler")]
impl FrameTimingCollector {
    /// Creates a collector that sees only events recorded from this point onward.
    pub fn new() -> Self {
        Self {
            cursor: FRAME_TIMINGS.lock().total_pushed,
        }
    }

    /// Returns each retained event at most once, without consuming other collectors' events.
    ///
    /// Events evicted from the bounded buffer before collection are lost.
    pub fn collect_unseen(&mut self) -> Vec<FrameEvent> {
        let frames = FRAME_TIMINGS.lock();
        let buffer_start = frames
            .total_pushed
            .saturating_sub(frames.timings.len() as u64);
        let skip = self.cursor.saturating_sub(buffer_start) as usize;
        let unseen = frames
            .timings
            .iter()
            .skip(skip.min(frames.timings.len()))
            .copied()
            .collect();
        self.cursor = frames.total_pushed;
        unseen
    }
}

#[cfg(test)]
mod foundation_tests;

#[cfg(test)]
mod foreground_tests;
