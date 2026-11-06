use super::*;
use crate::{Position, Style, div};
use open_gpui_refineable::Refineable;
use serde_json::json;

#[test]
fn grid_axes_keep_independent_content_sizing() {
    let mut element = div()
        .grid()
        .grid_cols_min_content(3)
        .grid_rows_max_content(5)
        .items_start()
        .justify_start();
    let declaration = element.style();
    assert_eq!(declaration.display, Some(Display::Grid));
    assert_eq!(
        declaration.grid_cols,
        Some(GridTemplate {
            repeat: 3,
            min_size: GridTemplateMinSize::MinContent,
        })
    );
    assert_eq!(
        declaration.grid_rows,
        Some(GridTemplate {
            repeat: 5,
            min_size: GridTemplateMinSize::MaxContent,
        })
    );
    assert_eq!(declaration.align_items, Some(AlignItems::FlexStart));
    assert_eq!(declaration.justify_content, Some(JustifyContent::Start));

    let columns = declaration.grid_cols;
    let mut element = element.grid_rows(7);
    assert_eq!(element.style().grid_cols, columns);
    assert_eq!(
        element.style().grid_rows,
        Some(GridTemplate {
            repeat: 7,
            min_size: GridTemplateMinSize::Zero,
        })
    );
}

#[test]
fn grid_content_sizing_survives_serialization_and_refinement() {
    let declaration = StyleRefinement::default()
        .grid_cols_max_content(4)
        .grid_rows_min_content(2);
    let value = serde_json::to_value(&declaration).unwrap();
    assert_eq!(
        value["grid_cols"],
        json!({ "repeat": 4, "min_size": "MaxContent" })
    );
    assert_eq!(
        value["grid_rows"],
        json!({ "repeat": 2, "min_size": "MinContent" })
    );
    let restored: StyleRefinement = serde_json::from_value(value).unwrap();
    assert_eq!(restored, declaration);

    let mut style = Style::default();
    style.refine(&StyleRefinement::default().grid().grid_cols(1).grid_rows(1));
    style.refine(&restored);
    style.refine(&StyleRefinement::default().grid_cols(6));
    assert_eq!(style.display, Display::Grid);
    assert_eq!(style.position, Position::Relative);
    assert_eq!(
        style.grid_cols,
        Some(GridTemplate {
            repeat: 6,
            min_size: GridTemplateMinSize::Zero,
        })
    );
    assert_eq!(
        style.grid_rows,
        Some(GridTemplate {
            repeat: 2,
            min_size: GridTemplateMinSize::MinContent,
        })
    );
}
