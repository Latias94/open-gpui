use crate::CanvasTransaction;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct CanvasHistory {
    undo_stack: Vec<CanvasTransaction>,
    redo_stack: Vec<CanvasTransaction>,
}

impl CanvasHistory {
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    pub fn redo_depth(&self) -> usize {
        self.redo_stack.len()
    }

    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn next_undo_transaction(&self) -> Option<&CanvasTransaction> {
        self.undo_stack.last()
    }

    pub fn next_redo_transaction(&self) -> Option<&CanvasTransaction> {
        self.redo_stack.last()
    }

    pub(crate) fn push_undo(&mut self, transaction: CanvasTransaction) {
        if !transaction.is_empty() {
            self.undo_stack.push(transaction);
            self.redo_stack.clear();
        }
    }

    pub(crate) fn pop_undo(&mut self) -> Option<CanvasTransaction> {
        self.undo_stack.pop()
    }

    pub(crate) fn push_redo(&mut self, transaction: CanvasTransaction) {
        if !transaction.is_empty() {
            self.redo_stack.push(transaction);
        }
    }

    pub(crate) fn pop_redo(&mut self) -> Option<CanvasTransaction> {
        self.redo_stack.pop()
    }
}
