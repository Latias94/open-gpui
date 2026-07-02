fn main() {
    let manifest = open_gpui_ui_components::component_contract::component_registry_manifest();
    if let Err(error) = serde_json::to_writer_pretty(std::io::stdout().lock(), &manifest) {
        if error.io_error_kind() == Some(std::io::ErrorKind::BrokenPipe) {
            return;
        }
        panic!("component registry manifest should serialize: {error}");
    }
    println!();
}
