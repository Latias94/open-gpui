//! Layout-stable focus ring primitive.

use open_gpui::{BoxShadow, Pixels, point, px};

use crate::color::ColorIntent;
use crate::theme::ThemeResolver;

/// Default outer focus ring width.
pub const DEFAULT_FOCUS_RING_WIDTH: Pixels = px(2.0);

/// Resolved focus ring metadata for interactive components.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FocusRing {
    color: ColorIntent,
    width: Pixels,
}

impl FocusRing {
    /// Creates a focus ring from a color intent and paint width.
    pub const fn new(color: ColorIntent, width: Pixels) -> Self {
        Self { color, width }
    }

    /// Creates a focus ring using the default component width.
    pub const fn from_color(color: ColorIntent) -> Self {
        Self::new(color, DEFAULT_FOCUS_RING_WIDTH)
    }

    /// Returns the focus ring color intent.
    pub const fn color(self) -> ColorIntent {
        self.color
    }

    /// Returns the outer focus ring width.
    pub const fn width(self) -> Pixels {
        self.width
    }

    /// Returns whether painting this focus ring changes component layout.
    pub const fn changes_layout(self) -> bool {
        false
    }
}

/// Converts a focus ring into a GPUI box shadow for render adapters.
pub fn focus_ring_shadow(ring: FocusRing) -> Vec<BoxShadow> {
    vec![BoxShadow {
        color: ThemeResolver::resolve(ring.color).into(),
        offset: point(px(0.0), px(0.0)),
        blur_radius: px(0.0),
        spread_radius: ring.width,
        inset: false,
    }]
}

#[cfg(test)]
mod tests {
    use open_gpui_ui_core::semantic;

    use super::*;

    #[test]
    fn gpui_adapter_paints_focus_ring_as_outer_shadow() {
        let ring = FocusRing::from_color(ColorIntent::new(semantic::FOCUS_RING, 0x2f80ed));

        let shadow = focus_ring_shadow(ring);

        assert_eq!(shadow.len(), 1);
        assert_eq!(shadow[0].spread_radius, DEFAULT_FOCUS_RING_WIDTH);
        assert_eq!(shadow[0].blur_radius, px(0.0));
        assert!(!shadow[0].inset);
    }
}
