use open_gpui_util::paths::{PathExt as _, home_dir};

fn main() {
    let home = home_dir();
    assert!(home.is_absolute());
    let directory = std::fs::read_dir(home)
        .expect("the public home directory must be usable by an ordinary downstream application");
    drop(directory);
    let config = home.join("open-gpui-consumer-settings.json");
    #[cfg(unix)]
    assert_eq!(
        config.compact(),
        std::path::Path::new("~").join("open-gpui-consumer-settings.json")
    );
    #[cfg(windows)]
    assert_eq!(config.compact(), config);
}
