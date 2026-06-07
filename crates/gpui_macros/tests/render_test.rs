#[test]
fn test_derive_render() {
    use open_gpui_macros::Render;

    #[derive(Render)]
    struct _Element;
}
