use crate::{
    CanvasClipboardPayload, CanvasDocument, CanvasPasteTransaction, CanvasSelection,
    CanvasTransaction,
};
use open_gpui::{Pixels, Point};

pub(crate) fn copy_selection(
    document: &CanvasDocument,
    selection: &CanvasSelection,
) -> Option<CanvasClipboardPayload> {
    let payload = CanvasClipboardPayload::from_document_selection(document, selection);
    (!payload.is_empty()).then_some(payload)
}

pub(crate) fn paste_clipboard(
    document: &CanvasDocument,
    payload: &CanvasClipboardPayload,
    offset: Point<Pixels>,
) -> CanvasPasteTransaction {
    payload.paste_transaction(document, offset)
}

pub(crate) fn duplicate_selection(
    document: &CanvasDocument,
    selection: &CanvasSelection,
    offset: Point<Pixels>,
) -> Option<CanvasPasteTransaction> {
    copy_selection(document, selection).map(|payload| paste_clipboard(document, &payload, offset))
}

pub(crate) fn paste_transaction_parts(
    pasted: CanvasPasteTransaction,
) -> Option<(CanvasTransaction, CanvasSelection)> {
    (!pasted.transaction.is_empty()).then_some((pasted.transaction, pasted.selection))
}
