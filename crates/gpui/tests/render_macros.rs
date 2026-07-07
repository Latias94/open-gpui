#[test]
fn derive_render_macro_compiles_against_open_gpui_facade() {
    use open_gpui_macros::Render;

    #[derive(Render)]
    struct _Element;
}
