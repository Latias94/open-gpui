use super::GraphicalEnvironment;

#[test]
fn desktop_environment_is_available_on_every_platform() {
    let _: fn(&GraphicalEnvironment) -> Option<String> = GraphicalEnvironment::desktop_environment;
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    assert_eq!(GraphicalEnvironment::default().desktop_environment(), None);

    #[cfg(not(any(target_os = "linux", target_os = "freebsd")))]
    assert_eq!(GraphicalEnvironment::detect().desktop_environment(), None);
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
#[test]
fn desktop_environment_preserves_utf8_and_rejects_empty_or_non_utf8_values() {
    use std::{ffi::OsString, os::unix::ffi::OsStringExt as _};

    for (current_desktop, expected) in [
        (None, None),
        (Some(OsString::new()), None),
        (Some(OsString::from_vec(vec![0xff])), None),
        (Some("ubuntu:GNOME".into()), Some("ubuntu:GNOME")),
        (Some("桌面:KDE".into()), Some("桌面:KDE")),
        (Some(" GNOME ".into()), Some(" GNOME ")),
    ] {
        let environment = GraphicalEnvironment {
            current_desktop,
            ..GraphicalEnvironment::default()
        };
        assert_eq!(environment.desktop_environment().as_deref(), expected);
    }
}

#[cfg(any(target_os = "linux", target_os = "freebsd"))]
#[test]
fn compositor_guess_uses_enabled_display_backends_and_official_headless_spelling() {
    let mut environment = GraphicalEnvironment {
        current_desktop: Some("ubuntu:GNOME".into()),
        ..GraphicalEnvironment::default()
    };
    assert_eq!(environment.guess_compositor(), "headless");
    environment.wayland_display = Some("".into());
    environment.x11_display = Some("".into());
    assert_eq!(environment.guess_compositor(), "headless");

    environment.x11_display = Some(":99".into());
    assert_eq!(
        environment.guess_compositor(),
        if cfg!(feature = "x11") {
            "X11"
        } else {
            "headless"
        }
    );
    environment.wayland_display = Some("wayland-test".into());
    assert_eq!(
        environment.guess_compositor(),
        if cfg!(feature = "wayland") {
            "Wayland"
        } else if cfg!(feature = "x11") {
            "X11"
        } else {
            "headless"
        }
    );
}
