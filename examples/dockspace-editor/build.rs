fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // The unoptimized dockspace frame/reducer call chain exceeds Windows' 1 MiB
    // main-thread default. Reserve virtual space; keep the default commit size.
    match std::env::var("CARGO_CFG_TARGET_ENV").as_deref() {
        Ok("msvc") => println!("cargo::rustc-link-arg=/STACK:8388608"),
        Ok("gnu") => println!("cargo::rustc-link-arg=-Wl,--stack,8388608"),
        _ => {}
    }
}
