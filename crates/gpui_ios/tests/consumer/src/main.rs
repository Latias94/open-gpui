#[cfg(target_os = "ios")]
fn main() {
    use open_gpui::{AppContext, Empty, WindowOptions};
    use open_gpui_ios::StatusBarContentStyle;

    open_gpui_platform::application().run(|cx| {
        open_gpui_platform::set_status_bar_style(StatusBarContentStyle::Dark);
        cx.on_app_lifecycle(|phase, _| {
            println!("Application lifecycle: {phase:?}");
        })
        .detach();
        cx.on_memory_warning(|_| {
            println!("Application memory warning");
        })
        .detach();
        cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| Empty))
            .expect("open a native iOS window through the ordinary public platform facade");
    });
}

#[cfg(not(target_os = "ios"))]
fn main() {
    eprintln!("Check this consumer with an iOS target");
}
