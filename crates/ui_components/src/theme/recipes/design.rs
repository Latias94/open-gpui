use open_gpui_motion::MotionPreference;
use open_gpui_ui_core::{Size, ThemeElevationLayer};

use crate::button::ButtonMetrics;
use crate::text_input::TextInputMetrics;
use crate::theme::resolver::ThemeResolver;
use crate::theme::runtime::ThemeContext;

impl ThemeResolver {
    pub(crate) fn button_metrics(
        theme: &ThemeContext,
        explicit_size: Option<Size>,
    ) -> ButtonMetrics {
        let design = theme.design_scales();
        let size = design.resolve_size(explicit_size);
        ButtonMetrics::from_theme_values(
            size,
            design.spacing().control_inline().resolve(size),
            design.spacing().control_block().resolve(size),
            design.radius().control().resolve(size),
            design.typography().control_text().resolve(size),
            design.typography().control_line_height().resolve(size),
        )
    }

    pub(crate) fn text_input_metrics(
        theme: &ThemeContext,
        explicit_size: Option<Size>,
    ) -> TextInputMetrics {
        let design = theme.design_scales();
        let size = design.resolve_size(explicit_size);
        TextInputMetrics::from_theme_values(
            size,
            design.spacing().control_inline().resolve(size),
            design.spacing().control_block().resolve(size),
            design.radius().control().resolve(size),
            design.typography().control_text().resolve(size),
            design.typography().control_line_height().resolve(size),
        )
    }

    pub(crate) fn overlay_surface_elevation(theme: &ThemeContext) -> [ThemeElevationLayer; 2] {
        theme.design_scales().elevation().overlay()
    }

    pub(crate) fn tooltip_elevation(theme: &ThemeContext) -> [ThemeElevationLayer; 2] {
        theme.design_scales().elevation().overlay()
    }

    pub(crate) fn splitter_motion_preference(
        theme: &ThemeContext,
        explicit: Option<MotionPreference>,
    ) -> MotionPreference {
        theme.design_scales().resolve_motion(explicit)
    }

    pub(crate) fn virtualized_list_motion_preference(
        theme: &ThemeContext,
        explicit: Option<MotionPreference>,
    ) -> MotionPreference {
        theme.design_scales().resolve_motion(explicit)
    }
}

#[cfg(test)]
mod tests {
    use open_gpui_motion::MotionPreference;
    use open_gpui_ui_core::{
        Density, Size, SizeScale, ThemeDesignScales, ThemeElevationLayer, ThemeElevationScale,
        ThemeRadiusScale, ThemeSpacingScale, ThemeTypographyScale, ui_px,
    };

    use super::*;
    use crate::theme::{ThemeContext, ThemeDefinition, ThemeRegistry, ThemeSnapshot};

    fn custom_design(density: Density, motion: MotionPreference) -> ThemeDesignScales {
        ThemeDesignScales::new(
            ThemeTypographyScale::new(
                SizeScale::new(11, 13, 17, 19),
                SizeScale::new(15, 18, 22, 25),
            ),
            ThemeSpacingScale::new(SizeScale::new(7, 9, 13, 17), SizeScale::new(3, 5, 7, 9)),
            ThemeRadiusScale::new(SizeScale::new(2, 4, 6, 10)),
            ThemeElevationScale::new([
                ThemeElevationLayer::new(1, 2, 3, 4, 35),
                ThemeElevationLayer::new(-1, 5, 8, -2, 45),
            ]),
            density,
            motion,
        )
    }

    fn context(design: ThemeDesignScales) -> ThemeContext {
        let source = ThemeSnapshot::light();
        let mut registry = ThemeRegistry::new();
        let snapshot = registry
            .register(
                ThemeDefinition::from_snapshot("design-recipe", "Design recipe", &source)
                    .design_scales(design),
            )
            .expect("complete design recipe theme should register")
            .snapshot()
            .clone();
        ThemeContext::new(snapshot)
    }

    #[test]
    fn button_and_text_input_recipes_share_typed_scales_and_explicit_size_precedence() {
        let compact = context(custom_design(Density::Compact, MotionPreference::Animated));

        let button = ThemeResolver::button_metrics(&compact, None);
        let input = ThemeResolver::text_input_metrics(&compact, None);
        assert_eq!(button.size(), Size::Small);
        assert_eq!(input.size(), Size::Small);
        assert_eq!(button.padding_x(), ui_px(9.0));
        assert_eq!(button.padding_y(), ui_px(5.0));
        assert_eq!(button.radius(), ui_px(4.0));
        assert_eq!(button.text_size(), ui_px(13.0));
        assert_eq!(button.line_height(), ui_px(18.0));
        assert_eq!(input.padding_x(), ui_px(9.0));
        assert_eq!(input.padding_y(), ui_px(5.0));
        assert_eq!(input.radius(), ui_px(4.0));
        assert_eq!(input.text_size(), ui_px(13.0));
        assert_eq!(input.line_height(), ui_px(18.0));

        let large = ThemeResolver::button_metrics(&compact, Some(Size::Large));
        assert_eq!(large.size(), Size::Large);
        assert_eq!(large.padding_x(), ui_px(17.0));
        assert_eq!(large.padding_y(), ui_px(9.0));
        assert_eq!(large.radius(), ui_px(10.0));
        assert_eq!(large.text_size(), ui_px(19.0));
        assert_eq!(large.line_height(), ui_px(25.0));

        let xsmall = ThemeResolver::text_input_metrics(&compact, Some(Size::XSmall));
        assert_eq!(xsmall.size(), Size::XSmall);
        assert_eq!(xsmall.padding_x(), ui_px(7.0));
        assert_eq!(xsmall.padding_y(), ui_px(3.0));
        assert_eq!(xsmall.radius(), ui_px(2.0));
        assert_eq!(xsmall.text_size(), ui_px(11.0));
        assert_eq!(xsmall.line_height(), ui_px(15.0));
    }

    #[test]
    fn reduced_motion_is_the_strict_floor_for_both_production_recipes() {
        let animated = context(custom_design(
            Density::Comfortable,
            MotionPreference::Animated,
        ));
        let reduced = context(custom_design(
            Density::Comfortable,
            MotionPreference::Reduced,
        ));

        assert_eq!(
            ThemeResolver::splitter_motion_preference(&animated, None),
            MotionPreference::Animated
        );
        assert_eq!(
            ThemeResolver::splitter_motion_preference(&reduced, Some(MotionPreference::Animated)),
            MotionPreference::Reduced
        );
        assert_eq!(
            ThemeResolver::virtualized_list_motion_preference(
                &animated,
                Some(MotionPreference::Reduced)
            ),
            MotionPreference::Reduced
        );
        assert_eq!(
            ThemeResolver::virtualized_list_motion_preference(
                &reduced,
                Some(MotionPreference::Animated)
            ),
            MotionPreference::Reduced
        );
    }

    #[test]
    fn overlay_and_tooltip_recipes_consume_the_same_typed_elevation_value() {
        let design = custom_design(Density::Comfortable, MotionPreference::Animated);
        let theme = context(design);

        assert_eq!(
            ThemeResolver::overlay_surface_elevation(&theme),
            design.elevation().overlay()
        );
        assert_eq!(
            ThemeResolver::tooltip_elevation(&theme),
            design.elevation().overlay()
        );
    }
}
