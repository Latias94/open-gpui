use open_gpui_ui_core::ThemeTokens;

use crate::avatar::{AvatarColors, AvatarGroupCountColors};
use crate::color::{ColorIntent, ColorState};
use crate::feedback::{FeedbackColors, FeedbackIntent};
use crate::kbd::KbdColors;
use crate::progress::ProgressColors;
use crate::separator::SeparatorColors;
use crate::skeleton::SkeletonColors;
use crate::theme::palette::{
    DEFAULT_ACCENT, DEFAULT_BORDER, DEFAULT_DESTRUCTIVE, DEFAULT_GHOST_SURFACE, DEFAULT_TEXT,
    DEFAULT_TEXT_MUTED,
};
use crate::theme::resolver::ThemeResolver;

impl ThemeResolver {
    pub(crate) const fn avatar_colors(tokens: ThemeTokens) -> AvatarColors {
        AvatarColors {
            background: ColorIntent::new(tokens.surface_muted, DEFAULT_GHOST_SURFACE),
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
        }
    }

    pub(crate) const fn avatar_group_count_colors(tokens: ThemeTokens) -> AvatarGroupCountColors {
        AvatarGroupCountColors {
            background: ColorIntent::new(tokens.surface_muted, DEFAULT_GHOST_SURFACE),
            foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
        }
    }

    pub(crate) const fn separator_colors(tokens: ThemeTokens) -> SeparatorColors {
        SeparatorColors {
            line: ColorIntent::new(tokens.border, DEFAULT_BORDER),
        }
    }

    pub(crate) const fn kbd_colors(tokens: ThemeTokens) -> KbdColors {
        KbdColors {
            background: ColorIntent::new(tokens.surface_muted, DEFAULT_GHOST_SURFACE),
            foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
        }
    }

    pub(crate) const fn progress_colors(tokens: ThemeTokens) -> ProgressColors {
        ProgressColors {
            track: ColorIntent::new(tokens.surface_muted, 0xdfe6dc),
            indicator: ColorIntent::new(tokens.accent, DEFAULT_ACCENT),
        }
    }

    pub(crate) const fn skeleton_colors(tokens: ThemeTokens) -> SkeletonColors {
        SkeletonColors {
            background: ColorIntent::new(tokens.surface_muted, DEFAULT_GHOST_SURFACE),
        }
    }

    pub(crate) const fn feedback_colors(
        tokens: ThemeTokens,
        intent: FeedbackIntent,
    ) -> FeedbackColors {
        let marker = match intent {
            FeedbackIntent::Neutral => ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            FeedbackIntent::Info | FeedbackIntent::Success => {
                ColorIntent::new(tokens.accent, DEFAULT_ACCENT)
            }
            FeedbackIntent::Warning => {
                ColorIntent::with_state(tokens.text_muted, ColorState::Message, 0xbf8700)
            }
            FeedbackIntent::Danger => ColorIntent::with_state(
                tokens.destructive,
                ColorState::Invalid,
                DEFAULT_DESTRUCTIVE,
            ),
        };

        FeedbackColors {
            background: ColorIntent::new(tokens.surface_muted, DEFAULT_GHOST_SURFACE),
            foreground: ColorIntent::new(tokens.text, DEFAULT_TEXT),
            muted_foreground: ColorIntent::new(tokens.text_muted, DEFAULT_TEXT_MUTED),
            border: ColorIntent::new(tokens.border, DEFAULT_BORDER),
            marker,
        }
    }
}
