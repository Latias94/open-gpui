fn main() {
    let schema = open_gpui_ui_components::theme_json_schema();
    serde_json::to_writer_pretty(std::io::stdout().lock(), &schema)
        .expect("theme JSON schema should serialize");
    println!();
}
