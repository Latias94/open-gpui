//! Pure foundation gallery for dogfooding `open-gpui-ui-core`.

pub mod pages;
pub mod shell;

pub use pages::{GALLERY_SECTIONS, GalleryPage, GalleryPageSpec};
pub use shell::{
    DEFAULT_GALLERY_HEIGHT, DEFAULT_GALLERY_WIDTH, GalleryShell, GalleryShellSnapshot,
    density_label, device_class_label, foundation_snapshot, open_gallery, open_gallery_page,
    panel_class_label, shell_mode_label, size_label,
};
