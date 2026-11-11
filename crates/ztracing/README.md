# Open GPUI Tracing

By default, `instrument` and the span and event macros do not record anything. An application
can enable `tracy` for native instrumentation or `web` for the browser performance layer.
Select the dependency from the same Open GPUI source as the instrumented packages so Cargo
unifies its features.

The application calls `init` only when it owns the process's global tracing subscriber. A
browser application can instead compose `performance_layer()` with its existing subscriber
and drive the returned reporter on the browser thread. On native `tracy`, an application may
select `ProfiledAllocator` in its own binary. Building with `tracy` does not start the native
client; installing the Tracy layer or using the opted-in allocator starts it explicitly. The
library never sets a global allocator.

These features do not enable GPUI's separate foreground frame and task profiler.
