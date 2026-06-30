//! Pure foundation gallery for dogfooding `open-gpui-ui-core`.

pub mod pages;
pub mod shell;
pub mod story;

pub use pages::{GALLERY_SECTIONS, GalleryPage, GalleryPageSpec};
pub use shell::{
    DEFAULT_GALLERY_HEIGHT, DEFAULT_GALLERY_WIDTH, GalleryShell, GalleryShellSnapshot,
    foundation_snapshot, open_gallery, open_gallery_page,
};
pub use story::{
    STORY_PROBE_OPERATIONS, StoryContract, StoryContractKind, StoryProbeContract,
    StoryProbeOperation, StorySelectorContract,
};
