fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        // Debug host-frame preparation retains several large transactional values
        // on the stack. The wasm linker's 1 MiB default cannot hold this path.
        println!("cargo:rustc-link-arg-bin=smoke_web=-zstack-size=8388608");
    }
}
