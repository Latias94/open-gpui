use std::collections::BTreeMap;

use open_gpui::{Font, Pixels, TextRun, Window, rems};
use open_gpui_ui_core::{
    TableCellValue, TableColumnId, TableColumnSizing, TableColumnWidthPolicy, TableResolvedRow,
    TableSortDirection, UiPx, VirtualizerResolvedState, table::TableStateCacheKey, ui_px,
};

use super::{TableColumnRenderPlan, TableMetrics, resolve_table_column_offsets};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct TableContentFitMeasureKey {
    state_key: TableStateCacheKey,
    font: Font,
    font_size: Pixels,
    cell_padding_x: UiPx,
    sample_set: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub(super) struct TableContentFitMeasureCache {
    key: Option<TableContentFitMeasureKey>,
    widths: BTreeMap<TableColumnId, UiPx>,
}

impl TableContentFitMeasureCache {
    pub(super) fn widths_for(
        &mut self,
        key: TableContentFitMeasureKey,
        columns: &[TableColumnRenderPlan],
        rendered_rows: &[&TableResolvedRow],
        metrics: TableMetrics,
        window: &Window,
    ) -> &BTreeMap<TableColumnId, UiPx> {
        let needs_refresh = self.key.as_ref() != Some(&key);
        if needs_refresh {
            let measured =
                measure_table_content_fit_widths(columns, rendered_rows, metrics, window);
            for (column_id, width) in measured {
                self.widths
                    .entry(column_id)
                    .and_modify(|existing| *existing = (*existing).max(width))
                    .or_insert(width);
            }
            self.key = Some(key);
        }

        &self.widths
    }
}

pub(super) fn apply_table_content_fit_widths(
    columns: Vec<TableColumnRenderPlan>,
    measured_widths: &BTreeMap<TableColumnId, UiPx>,
    committed_sizing: &TableColumnSizing,
) -> Vec<TableColumnRenderPlan> {
    let columns = columns
        .into_iter()
        .map(|column| {
            if column.width_policy() != TableColumnWidthPolicy::ContentFit
                || committed_sizing.width(column.id()).is_some()
            {
                return column;
            }

            match measured_widths.get(column.id()).copied() {
                Some(width) => column.with_width(width),
                None => column,
            }
        })
        .collect::<Vec<_>>();

    resolve_table_column_offsets(columns)
}

pub(super) fn content_fit_measure_key(
    state_key: TableStateCacheKey,
    metrics: TableMetrics,
    columns: &[TableColumnRenderPlan],
    rendered_rows: &[&TableResolvedRow],
    window: &Window,
) -> TableContentFitMeasureKey {
    let mut sample_set = Vec::new();
    let font = window.text_style().font();
    let font_size = table_content_fit_text_size(window);
    sample_set.push(format!("size:{}", metrics.size().as_str()));
    sample_set.extend(
        columns
            .iter()
            .filter(|column| column.width_policy() == TableColumnWidthPolicy::ContentFit)
            .map(|column| format!("column:{}", column.id().as_str())),
    );
    sample_set.extend(rendered_rows.iter().flat_map(|row| {
        let row_key = table_content_fit_row_sample_key(row);
        let row_depth = row.depth();
        let row_has_tree = row.tree().is_some();
        columns
            .iter()
            .filter(|column| column.width_policy() == TableColumnWidthPolicy::ContentFit)
            .map(move |column| {
                let value = row
                    .cell(column.id())
                    .map(TableCellValue::filter_text)
                    .unwrap_or_default();
                format!(
                    "row:{row_key}|depth:{row_depth}|tree:{row_has_tree}|column:{}|value:{}",
                    column.id().as_str(),
                    value
                )
            })
    }));

    TableContentFitMeasureKey {
        state_key,
        font,
        font_size,
        cell_padding_x: metrics.cell_padding_x(),
        sample_set,
    }
}

pub(super) fn table_content_fit_rendered_rows<'a>(
    table: &'a open_gpui_ui_core::TableResolvedState,
    virtualizer: &'a VirtualizerResolvedState,
) -> Vec<&'a TableResolvedRow> {
    let mut rows = Vec::with_capacity(
        table.top_rows().len() + virtualizer.items().len() + table.bottom_rows().len(),
    );
    rows.extend(table.top_rows());
    rows.extend(
        virtualizer
            .items()
            .iter()
            .filter_map(|measurement| table.center_rows().get(measurement.index())),
    );
    rows.extend(table.bottom_rows());
    rows
}

fn measure_table_content_fit_widths(
    columns: &[TableColumnRenderPlan],
    rendered_rows: &[&TableResolvedRow],
    metrics: TableMetrics,
    window: &Window,
) -> BTreeMap<TableColumnId, UiPx> {
    let mut widths = BTreeMap::new();
    let font = window.text_style().font();
    let font_size = table_content_fit_text_size(window);
    let padding_x = metrics.cell_padding_x();
    let tree_affordance_column_id = columns.first().map(|column| column.id().clone());

    for column in columns
        .iter()
        .filter(|column| column.width_policy() == TableColumnWidthPolicy::ContentFit)
    {
        let mut measured = measure_table_header_text_width(
            window,
            column.label(),
            column.sort_direction(),
            font.clone(),
            font_size,
        );
        for row in rendered_rows {
            if let Some(value) = row.cell(column.id()) {
                let value_text = value.filter_text();
                let mut value_width =
                    measure_table_text_width(window, &value_text, font.clone(), font_size);
                if tree_affordance_column_id
                    .as_ref()
                    .is_some_and(|tree_column_id| {
                        tree_column_id == column.id() && row.tree().is_some()
                    })
                {
                    value_width = value_width + ui_px(18.0) + ui_px(16.0) * row.depth() as f32;
                }
                measured = measured.max(value_width);
            }
        }

        let measured = measured + padding_x * 2.0;
        widths.insert(column.id().clone(), measured);
    }

    widths
}

fn measure_table_header_text_width(
    window: &Window,
    label: &str,
    sort_direction: Option<TableSortDirection>,
    font: Font,
    font_size: Pixels,
) -> UiPx {
    let mut text = label.to_owned();
    if let Some(direction) = sort_direction {
        text.push_str(match direction {
            TableSortDirection::Ascending => " ↑",
            TableSortDirection::Descending => " ↓",
        });
    }

    measure_table_text_width(window, &text, font.bold(), font_size)
}

fn measure_table_text_width(window: &Window, text: &str, font: Font, font_size: Pixels) -> UiPx {
    if text.is_empty() {
        return UiPx::ZERO;
    }

    let shaped = window.text_system().shape_line(
        text.to_owned().into(),
        font_size,
        &[TextRun {
            len: text.len(),
            font,
            color: window.text_style().color,
            background_color: None,
            underline: None,
            strikethrough: None,
        }],
        None,
    );
    ui_px(shaped.width().as_f32())
}

fn table_content_fit_text_size(window: &Window) -> Pixels {
    rems(0.75).to_pixels(window.rem_size())
}

fn table_content_fit_row_sample_key(row: &TableResolvedRow) -> String {
    row.identity_key().as_str().to_owned()
}
