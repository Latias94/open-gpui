use crate::{BackgroundExecutor, Task};
use std::{
    fmt,
    future::Future,
    ops::AddAssign,
    panic::Location,
    pin::Pin,
    sync::atomic::{AtomicUsize, Ordering::SeqCst},
    task,
    time::Duration,
};

pub(crate) fn post_inc<T: From<u8> + AddAssign<T> + Copy>(value: &mut T) -> T {
    let previous = *value;
    *value += T::from(1);
    previous
}

macro_rules! debug_panic {
    ($($message:tt)*) => {
        if cfg!(debug_assertions) {
            panic!($($message)*);
        } else {
            let backtrace = std::backtrace::Backtrace::capture();
            log::error!("{}\n{backtrace:?}", format_args!($($message)*));
        }
    };
}
pub(crate) use debug_panic;

/// Logs errors at their call site and returns successful values.
pub trait ResultExt {
    /// The successful value of the result.
    type Ok;

    /// Logs at error level and converts the result to an option.
    fn log_err(self) -> Option<Self::Ok>;

    /// Logs at the supplied level and converts the result to an option.
    fn log_with_level(self, level: log::Level) -> Option<Self::Ok>;
}

impl<T, E: fmt::Display> ResultExt for Result<T, E> {
    type Ok = T;

    #[track_caller]
    fn log_err(self) -> Option<T> {
        self.log_with_level(log::Level::Error)
    }

    #[track_caller]
    fn log_with_level(self, level: log::Level) -> Option<T> {
        match self {
            Ok(value) => Some(value),
            Err(error) => {
                log_error_with_caller(Location::caller(), format_args!("{error:#}"), level);
                None
            }
        }
    }
}

#[inline(never)]
pub(crate) fn log_error_with_caller(
    caller: &Location<'_>,
    arguments: fmt::Arguments<'_>,
    level: log::Level,
) {
    #[cfg(not(windows))]
    let file = caller.file();
    #[cfg(windows)]
    let file = caller.file().replace('\\', "/");
    let file = file.split_once("crates/");
    let target = file.as_ref().and_then(|(_, path)| path.split_once("/src/"));
    let module_path = target.map(|(package, module)| {
        let module_path = module.trim_end_matches(".rs").replace('/', "::");
        if module.starts_with(package) {
            module_path
        } else {
            format!("{package}::{module_path}")
        }
    });
    let file = file.map(|(_, file)| format!("crates/{file}"));
    log::logger().log(
        &log::Record::builder()
            .target(module_path.as_deref().unwrap_or(""))
            .module_path(file.as_deref())
            .args(arguments)
            .file(Some(caller.file()))
            .line(Some(caller.line()))
            .level(level)
            .build(),
    );
}

/// Runs a callback on drop unless cancelled.
pub struct Deferred<F: FnOnce()>(Option<F>);

impl<F: FnOnce()> Deferred<F> {
    /// Drops the guard without running its callback.
    pub fn abort(mut self) {
        self.0.take();
    }
}

impl<F: FnOnce()> Drop for Deferred<F> {
    fn drop(&mut self) {
        if let Some(callback) = self.0.take() {
            callback();
        }
    }
}

/// Returns a guard that runs `callback` when dropped.
#[must_use]
pub fn defer<F: FnOnce()>(callback: F) -> Deferred<F> {
    Deferred(Some(callback))
}

/// A helper trait for building complex objects with imperative conditionals in a fluent style.
pub trait FluentBuilder {
    /// Imperatively modify self with the given closure.
    fn map<U>(self, f: impl FnOnce(Self) -> U) -> U
    where
        Self: Sized,
    {
        f(self)
    }

    /// Conditionally modify self with the given closure.
    fn when(self, condition: bool, then: impl FnOnce(Self) -> Self) -> Self
    where
        Self: Sized,
    {
        self.map(|this| if condition { then(this) } else { this })
    }

    /// Conditionally modify self with the given closure.
    fn when_else(
        self,
        condition: bool,
        then: impl FnOnce(Self) -> Self,
        else_fn: impl FnOnce(Self) -> Self,
    ) -> Self
    where
        Self: Sized,
    {
        self.map(|this| if condition { then(this) } else { else_fn(this) })
    }

    /// Conditionally unwrap and modify self with the given closure, if the given option is Some.
    fn when_some<T>(self, option: Option<T>, then: impl FnOnce(Self, T) -> Self) -> Self
    where
        Self: Sized,
    {
        self.map(|this| {
            if let Some(value) = option {
                then(this, value)
            } else {
                this
            }
        })
    }
    /// Conditionally unwrap and modify self with the given closure, if the given option is None.
    fn when_none<T>(self, option: &Option<T>, then: impl FnOnce(Self) -> Self) -> Self
    where
        Self: Sized,
    {
        self.map(|this| if option.is_some() { this } else { then(this) })
    }
}

/// Extensions for Future types that provide additional combinators and utilities.
pub trait FutureExt {
    /// Requires a Future to complete before the specified duration has elapsed.
    /// Similar to tokio::timeout.
    fn with_timeout(self, timeout: Duration, executor: &BackgroundExecutor) -> WithTimeout<Self>
    where
        Self: Sized;
}

impl<T: Future> FutureExt for T {
    fn with_timeout(self, timeout: Duration, executor: &BackgroundExecutor) -> WithTimeout<Self>
    where
        Self: Sized,
    {
        WithTimeout {
            future: self,
            timer: executor.timer(timeout),
        }
    }
}

#[pin_project::pin_project]
pub struct WithTimeout<T> {
    #[pin]
    future: T,
    #[pin]
    timer: Task<()>,
}

#[derive(Debug, thiserror::Error)]
#[error("Timed out before future resolved")]
/// Error returned by with_timeout when the timeout duration elapsed before the future resolved
pub struct Timeout;

impl<T: Future> Future for WithTimeout<T> {
    type Output = Result<T::Output, Timeout>;

    fn poll(self: Pin<&mut Self>, cx: &mut task::Context) -> task::Poll<Self::Output> {
        let this = self.project();

        if let task::Poll::Ready(output) = this.future.poll(cx) {
            task::Poll::Ready(Ok(output))
        } else if this.timer.poll(cx).is_ready() {
            task::Poll::Ready(Err(Timeout))
        } else {
            task::Poll::Pending
        }
    }
}

/// Increment the given atomic counter if it is not zero.
/// Return the new value of the counter.
pub(crate) fn atomic_incr_if_not_zero(counter: &AtomicUsize) -> usize {
    let mut loaded = counter.load(SeqCst);
    loop {
        if loaded == 0 {
            return 0;
        }
        match counter.compare_exchange_weak(loaded, loaded + 1, SeqCst, SeqCst) {
            Ok(x) => return x + 1,
            Err(actual) => loaded = actual,
        }
    }
}

/// Rounds to the nearest integer with 0.5 ties toward zero.
#[inline]
pub(crate) fn round_half_toward_zero(value: f32) -> f32 {
    (value.abs() - 0.5).ceil().copysign(value)
}

#[inline]
pub(crate) fn round_half_toward_zero_f64(value: f64) -> f64 {
    (value.abs() - 0.5).ceil().copysign(value)
}

#[inline]
pub(crate) fn round_to_device_pixel(logical: f32, scale_factor: f32) -> f32 {
    round_half_toward_zero(logical * scale_factor)
}

#[inline]
pub(crate) fn round_stroke_to_device_pixel(logical: f32, scale_factor: f32) -> f32 {
    if logical == 0.0 {
        0.0
    } else {
        round_to_device_pixel(logical.max(0.0), scale_factor).max(1.0)
    }
}

#[inline]
pub(crate) fn floor_to_device_pixel(logical: f32, scale_factor: f32) -> f32 {
    (logical * scale_factor).floor()
}

#[inline]
pub(crate) fn ceil_to_device_pixel(logical: f32, scale_factor: f32) -> f32 {
    (logical * scale_factor).ceil()
}

#[cfg(test)]
mod tests {
    use crate::TestAppContext;

    use super::*;

    #[test]
    fn test_round_half_toward_zero() {
        // Midpoint ties go toward zero
        assert_eq!(round_half_toward_zero(0.5), 0.0);
        assert_eq!(round_half_toward_zero(1.5), 1.0);
        assert_eq!(round_half_toward_zero(2.5), 2.0);
        assert_eq!(round_half_toward_zero(-0.5), 0.0);
        assert_eq!(round_half_toward_zero(-1.5), -1.0);
        assert_eq!(round_half_toward_zero(-2.5), -2.0);

        // Non-midpoint values round to nearest
        assert_eq!(round_half_toward_zero(1.5001), 2.0);
        assert_eq!(round_half_toward_zero(1.4999), 1.0);
        assert_eq!(round_half_toward_zero(-1.5001), -2.0);
        assert_eq!(round_half_toward_zero(-1.4999), -1.0);

        // Integers are unchanged
        assert_eq!(round_half_toward_zero(0.0), 0.0);
        assert_eq!(round_half_toward_zero(3.0), 3.0);
        assert_eq!(round_half_toward_zero(-3.0), -3.0);
    }

    #[test]
    fn test_device_pixel_helpers() {
        // Snap uses half-toward-zero: 1.0 * 1.5 = 1.5 ties toward 1.0.
        assert_eq!(round_to_device_pixel(1.0, 1.5), 1.0);
        // Below the tie rounds down, above rounds up.
        assert_eq!(round_to_device_pixel(0.3, 2.0), 1.0);
        assert_eq!(round_to_device_pixel(1.4, 1.0), 1.0);
        assert_eq!(round_to_device_pixel(1.6, 1.0), 2.0);

        // Stroke uses snap, but clamps non-zero input up to at least 1dp.
        assert_eq!(round_stroke_to_device_pixel(0.0, 1.0), 0.0);
        assert_eq!(round_stroke_to_device_pixel(0.4, 1.0), 1.0);
        assert_eq!(round_stroke_to_device_pixel(0.5, 1.0), 1.0);
        assert_eq!(round_stroke_to_device_pixel(1.0, 1.5), 1.0);
        assert_eq!(round_stroke_to_device_pixel(1.6, 1.0), 2.0);

        // Cover's near edge floors, far edge ceils. Together they form a strict superset.
        assert_eq!(floor_to_device_pixel(0.3, 2.0), 0.0);
        assert_eq!(ceil_to_device_pixel(0.3, 2.0), 1.0);
        assert_eq!(floor_to_device_pixel(2.1, 1.0), 2.0);
        assert_eq!(ceil_to_device_pixel(2.1, 1.0), 3.0);

        // Integer device-pixel inputs are stable under all three.
        assert_eq!(round_to_device_pixel(2.0, 2.0), 4.0);
        assert_eq!(floor_to_device_pixel(2.0, 2.0), 4.0);
        assert_eq!(ceil_to_device_pixel(2.0, 2.0), 4.0);
    }

    #[test]
    fn test_round_half_toward_zero_f64() {
        assert_eq!(round_half_toward_zero_f64(0.5), 0.0);
        assert_eq!(round_half_toward_zero_f64(-0.5), 0.0);
        assert_eq!(round_half_toward_zero_f64(1.5), 1.0);
        assert_eq!(round_half_toward_zero_f64(-1.5), -1.0);
        assert_eq!(round_half_toward_zero_f64(2.5001), 3.0);
    }

    #[open_gpui::test]
    async fn test_with_timeout(cx: &mut TestAppContext) {
        Task::ready(())
            .with_timeout(Duration::from_secs(1), &cx.executor())
            .await
            .expect("Timeout should be noop");

        let long_duration = Duration::from_secs(6000);
        let short_duration = Duration::from_secs(1);
        cx.executor()
            .timer(long_duration)
            .with_timeout(short_duration, &cx.executor())
            .await
            .expect_err("timeout should have triggered");

        let fut = cx
            .executor()
            .timer(long_duration)
            .with_timeout(short_duration, &cx.executor());
        cx.executor().advance_clock(short_duration * 2);
        futures::FutureExt::now_or_never(fut)
            .unwrap_or_else(|| panic!("timeout should have triggered"))
            .expect_err("timeout");
    }
}
