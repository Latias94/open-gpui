# Open GPUI Log

An application can install this logger with `try_init` and select a standard output, error
output or rotating file sink. GPUI does not install it on the application's behalf. It works
with ordinary `log` records and also provides scoped logging macros.

Set `OPEN_GPUI_LOG=info,my_app=debug` for module-level configuration. When that variable is
absent the logger reads `RUST_LOG`, then the CI default or the supplied default filter. Levels
are `off`, `error`, `warn`, `info`, `debug` and `trace`; `none` is also accepted for `off`.
Applications can update scoped settings through the logger's filter API.
