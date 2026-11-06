use super::*;
use serde_json::json;
use taffy::style::{AlignContentKeyword, AlignItemsKeyword, AlignmentSafety};

#[test]
fn item_alignment_preserves_flex_relative_edges_and_baseline() {
    for (alignment, keyword) in [
        (AlignItems::Start, AlignItemsKeyword::Start),
        (AlignItems::End, AlignItemsKeyword::End),
        (AlignItems::FlexStart, AlignItemsKeyword::FlexStart),
        (AlignItems::FlexEnd, AlignItemsKeyword::FlexEnd),
        (AlignItems::Center, AlignItemsKeyword::Center),
        (AlignItems::Baseline, AlignItemsKeyword::Baseline),
        (AlignItems::Stretch, AlignItemsKeyword::Stretch),
    ] {
        let converted = taffy::style::AlignItems::from(alignment);
        assert_eq!(converted.keyword, keyword);
        assert_eq!(converted.safety, AlignmentSafety::Default);
    }
}

#[test]
fn content_alignment_preserves_flex_relative_edges_and_distribution() {
    for (alignment, keyword) in [
        (AlignContent::Start, AlignContentKeyword::Start),
        (AlignContent::End, AlignContentKeyword::End),
        (AlignContent::FlexStart, AlignContentKeyword::FlexStart),
        (AlignContent::FlexEnd, AlignContentKeyword::FlexEnd),
        (AlignContent::Center, AlignContentKeyword::Center),
        (AlignContent::Stretch, AlignContentKeyword::Stretch),
        (
            AlignContent::SpaceBetween,
            AlignContentKeyword::SpaceBetween,
        ),
        (AlignContent::SpaceEvenly, AlignContentKeyword::SpaceEvenly),
        (AlignContent::SpaceAround, AlignContentKeyword::SpaceAround),
    ] {
        let converted = taffy::style::AlignContent::from(alignment);
        assert_eq!(converted.keyword, keyword);
        assert_eq!(converted.safety, AlignmentSafety::Default);
    }
}

#[test]
fn implicit_alignment_safety_remains_distinct_from_safe_and_unsafe() {
    let items = taffy::style::AlignItems::from(AlignItems::Center);
    assert_ne!(items, taffy::style::AlignItems::SAFE_CENTER);
    assert_ne!(items, items.with_safety(AlignmentSafety::Unsafe));

    let content = taffy::style::AlignContent::from(AlignContent::Center);
    assert_ne!(content, taffy::style::AlignContent::SAFE_CENTER);
    assert_ne!(content, content.with_safety(AlignmentSafety::Unsafe));
}

#[test]
fn layout_defaults_keep_alignment_and_grid_unset() {
    let style = Style::default();
    assert_eq!(style.display, Display::Block);
    assert_eq!(style.position, Position::Relative);
    assert_eq!(style.align_items, None);
    assert_eq!(style.align_self, None);
    assert_eq!(style.align_content, None);
    assert_eq!(style.justify_content, None);
    assert_eq!(style.grid_cols, None);
    assert_eq!(style.grid_rows, None);
    assert_eq!(GridTemplateMinSize::default(), GridTemplateMinSize::Zero);
    assert_eq!(
        GridTemplate::default(),
        GridTemplate {
            repeat: 0,
            min_size: GridTemplateMinSize::Zero,
        }
    );
}

#[test]
fn grid_template_preserves_its_serialized_representation() {
    for (min_size, serialized) in [
        (GridTemplateMinSize::Zero, "Zero"),
        (GridTemplateMinSize::MinContent, "MinContent"),
        (GridTemplateMinSize::MaxContent, "MaxContent"),
    ] {
        let template = GridTemplate {
            repeat: 3,
            min_size,
        };
        let value = json!({ "repeat": 3, "min_size": serialized });
        assert_eq!(serde_json::to_value(template).unwrap(), value);
        assert_eq!(
            serde_json::from_value::<GridTemplate>(value).unwrap(),
            template
        );
    }
}

#[test]
fn grid_template_schema_uses_the_shared_minimum_size_type() {
    assert_eq!(GridTemplateMinSize::schema_name(), "GridTemplateMinSize");

    let schema = schemars::schema_for!(GridTemplate);
    let schema = schema.as_value();
    assert_eq!(
        schema["properties"]["min_size"]["$ref"],
        "#/$defs/GridTemplateMinSize"
    );
    assert!(schema["$defs"]["GridTemplateMinSize"].is_object());
}
