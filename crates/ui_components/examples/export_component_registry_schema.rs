fn main() {
    let schema = open_gpui_ui_components::component_contract::component_registry_manifest_schema();
    if let Err(error) = serde_json::to_writer_pretty(std::io::stdout().lock(), &schema) {
        if error.io_error_kind() == Some(std::io::ErrorKind::BrokenPipe) {
            return;
        }
        panic!("component registry manifest JSON schema should serialize: {error}");
    }
    println!();
}
