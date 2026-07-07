#[test]
fn derive_context_macros_compile_against_open_gpui_facade() {
    use open_gpui::{App, Window};
    use open_gpui_macros::{AppContext, VisualContext};

    #[derive(AppContext, VisualContext)]
    struct _MyCustomContext<'a, 'b> {
        #[app]
        app: &'a mut App,
        #[window]
        window: &'b mut Window,
    }
}
