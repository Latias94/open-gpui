use crate::session::{CanvasToolSessionEffect, ToolState};
use crate::{
    CanvasConnectionRelease, CanvasSelection, CanvasTool, CanvasTransaction, CanvasViewport,
    HitTarget,
};
use open_gpui::{Pixels, Point};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) enum CanvasToolEffect {
    ApplyTransaction(CanvasTransaction),
    BeginGesture,
    UpdateGesture(CanvasTransaction),
    CommitGesture,
    CancelGesture,
    SetTool(CanvasTool),
    SetSelection(CanvasSelection),
    ReplaceSelection(HitTarget),
    AddSelection(HitTarget),
    RemoveSelection(HitTarget),
    ToggleSelection(HitTarget),
    ClearSelection,
    SetConnectionRelease(Option<CanvasConnectionRelease>),
    SetState(ToolState),
    PanViewport(Point<Pixels>),
    SetViewport(CanvasViewport),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum CanvasToolIntent {
    ApplyTransaction(CanvasTransaction),
    CommitTransaction,
    CancelTransaction,
    SetTool(CanvasTool),
    SetSelection(CanvasSelection),
    ReplaceSelection(HitTarget),
    AddSelection(HitTarget),
    RemoveSelection(HitTarget),
    ToggleSelection(HitTarget),
    ClearSelection,
    PanViewport(Point<Pixels>),
    SetViewport(CanvasViewport),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum CanvasEditorAction {
    ApplyTransaction(CanvasTransaction),
    BeginGesture,
    UpdateGesture(CanvasTransaction),
    CommitGesture,
    CancelGesture,
    SetTool(CanvasTool),
    SetConnectionRelease(Option<CanvasConnectionRelease>),
    Session(CanvasToolSessionEffect),
}

impl From<CanvasToolEffect> for CanvasEditorAction {
    fn from(effect: CanvasToolEffect) -> Self {
        match effect {
            CanvasToolEffect::ApplyTransaction(transaction) => Self::ApplyTransaction(transaction),
            CanvasToolEffect::BeginGesture => Self::BeginGesture,
            CanvasToolEffect::UpdateGesture(transaction) => Self::UpdateGesture(transaction),
            CanvasToolEffect::CommitGesture => Self::CommitGesture,
            CanvasToolEffect::CancelGesture => Self::CancelGesture,
            CanvasToolEffect::SetTool(tool) => Self::SetTool(tool),
            CanvasToolEffect::SetSelection(selection) => {
                Self::Session(CanvasToolSessionEffect::SetSelection(selection))
            }
            CanvasToolEffect::ReplaceSelection(target) => {
                Self::Session(CanvasToolSessionEffect::ReplaceSelection(target))
            }
            CanvasToolEffect::AddSelection(target) => {
                Self::Session(CanvasToolSessionEffect::AddSelection(target))
            }
            CanvasToolEffect::RemoveSelection(target) => {
                Self::Session(CanvasToolSessionEffect::RemoveSelection(target))
            }
            CanvasToolEffect::ToggleSelection(target) => {
                Self::Session(CanvasToolSessionEffect::ToggleSelection(target))
            }
            CanvasToolEffect::ClearSelection => {
                Self::Session(CanvasToolSessionEffect::ClearSelection)
            }
            CanvasToolEffect::SetConnectionRelease(release) => Self::SetConnectionRelease(release),
            CanvasToolEffect::SetState(state) => {
                Self::Session(CanvasToolSessionEffect::SetState(state))
            }
            CanvasToolEffect::PanViewport(delta) => {
                Self::Session(CanvasToolSessionEffect::PanViewport(delta))
            }
            CanvasToolEffect::SetViewport(viewport) => {
                Self::Session(CanvasToolSessionEffect::SetViewport(viewport))
            }
        }
    }
}

impl From<CanvasToolIntent> for CanvasEditorAction {
    fn from(intent: CanvasToolIntent) -> Self {
        match intent {
            CanvasToolIntent::ApplyTransaction(transaction) => Self::ApplyTransaction(transaction),
            CanvasToolIntent::CommitTransaction => Self::CommitGesture,
            CanvasToolIntent::CancelTransaction => Self::CancelGesture,
            CanvasToolIntent::SetTool(tool) => Self::SetTool(tool),
            CanvasToolIntent::SetSelection(selection) => {
                Self::Session(CanvasToolSessionEffect::SetSelection(selection))
            }
            CanvasToolIntent::ReplaceSelection(target) => {
                Self::Session(CanvasToolSessionEffect::ReplaceSelection(target))
            }
            CanvasToolIntent::AddSelection(target) => {
                Self::Session(CanvasToolSessionEffect::AddSelection(target))
            }
            CanvasToolIntent::RemoveSelection(target) => {
                Self::Session(CanvasToolSessionEffect::RemoveSelection(target))
            }
            CanvasToolIntent::ToggleSelection(target) => {
                Self::Session(CanvasToolSessionEffect::ToggleSelection(target))
            }
            CanvasToolIntent::ClearSelection => {
                Self::Session(CanvasToolSessionEffect::ClearSelection)
            }
            CanvasToolIntent::PanViewport(delta) => {
                Self::Session(CanvasToolSessionEffect::PanViewport(delta))
            }
            CanvasToolIntent::SetViewport(viewport) => {
                Self::Session(CanvasToolSessionEffect::SetViewport(viewport))
            }
        }
    }
}
