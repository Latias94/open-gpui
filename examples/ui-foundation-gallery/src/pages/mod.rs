//! Foundation gallery page registry.

pub mod adaptive;
pub mod components;
pub mod focus_a11y;
pub mod overlay;
pub mod sizing;
pub mod tokens;

/// A gallery page that represents one foundation slice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GalleryPage {
    /// Semantic token vocabulary.
    Tokens,
    /// Size and density vocabulary.
    SizingDensity,
    /// Adaptive shell and panel vocabulary.
    Adaptive,
    /// Focus and accessibility vocabulary.
    FocusAccessibility,
    /// Overlay geometry vocabulary.
    Overlay,
    /// First concrete component consumers.
    Components,
}

impl GalleryPage {
    /// Parses a stable page id.
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "tokens" => Some(Self::Tokens),
            "sizing-density" => Some(Self::SizingDensity),
            "adaptive" => Some(Self::Adaptive),
            "focus-a11y" => Some(Self::FocusAccessibility),
            "overlay" => Some(Self::Overlay),
            "components" => Some(Self::Components),
            _ => None,
        }
    }

    /// Returns the stable page id.
    pub const fn id(self) -> &'static str {
        match self {
            Self::Tokens => "tokens",
            Self::SizingDensity => "sizing-density",
            Self::Adaptive => "adaptive",
            Self::FocusAccessibility => "focus-a11y",
            Self::Overlay => "overlay",
            Self::Components => "components",
        }
    }

    /// Returns the user-facing page title.
    pub const fn title(self) -> &'static str {
        match self {
            Self::Tokens => tokens::TITLE,
            Self::SizingDensity => sizing::TITLE,
            Self::Adaptive => adaptive::TITLE,
            Self::FocusAccessibility => focus_a11y::TITLE,
            Self::Overlay => overlay::TITLE,
            Self::Components => components::TITLE,
        }
    }

    /// Returns a short page summary.
    pub const fn summary(self) -> &'static str {
        match self {
            Self::Tokens => tokens::SUMMARY,
            Self::SizingDensity => sizing::SUMMARY,
            Self::Adaptive => adaptive::SUMMARY,
            Self::FocusAccessibility => focus_a11y::SUMMARY,
            Self::Overlay => overlay::SUMMARY,
            Self::Components => components::SUMMARY,
        }
    }

    /// Returns the foundation signals this page should exercise.
    pub const fn signals(self) -> &'static [&'static str] {
        match self {
            Self::Tokens => tokens::SIGNALS,
            Self::SizingDensity => sizing::SIGNALS,
            Self::Adaptive => adaptive::SIGNALS,
            Self::FocusAccessibility => focus_a11y::SIGNALS,
            Self::Overlay => overlay::SIGNALS,
            Self::Components => components::SIGNALS,
        }
    }
}

/// Static metadata for one gallery section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GalleryPageSpec {
    /// The foundation page represented by this section.
    pub page: GalleryPage,
    /// Stable section id.
    pub id: &'static str,
    /// User-facing section title.
    pub title: &'static str,
    /// Short section summary.
    pub summary: &'static str,
}

impl GalleryPageSpec {
    /// Creates page metadata from a page enum.
    pub const fn new(page: GalleryPage) -> Self {
        Self {
            page,
            id: page.id(),
            title: page.title(),
            summary: page.summary(),
        }
    }
}

/// The canonical foundation section order.
pub const GALLERY_SECTIONS: [GalleryPageSpec; 6] = [
    GalleryPageSpec::new(GalleryPage::Tokens),
    GalleryPageSpec::new(GalleryPage::SizingDensity),
    GalleryPageSpec::new(GalleryPage::Adaptive),
    GalleryPageSpec::new(GalleryPage::FocusAccessibility),
    GalleryPageSpec::new(GalleryPage::Overlay),
    GalleryPageSpec::new(GalleryPage::Components),
];
