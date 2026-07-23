use crate::a11y::UiA11yElementExt;
use crate::collection_typeahead::{CollectionTypeaheadInput, CollectionTypeaheadSession};
use crate::focus::focus_ring_shadow_with_theme;
use crate::geometry::gpui_px_from_ui;
use crate::scroll_area::ScrollArea;
use crate::scroll_surface::{
    ScrollSurfaceRuntime, scroll_surface_handle, set_vertical_scroll_offset_with_source,
    vertical_scroll_offset, vertical_viewport_extent,
};
use crate::theme::ThemeResolver;
use open_gpui::prelude::FluentBuilder;
use open_gpui::{
    AnyElement, App, BringIntoViewCancelReason, BringIntoViewChainGeneration, BringIntoViewOptions,
    BringIntoViewOutcome, Context, DeferredBringIntoViewGuard, Entity, FocusHandle,
    InteractiveElement, IntoElement, KeyDownEvent, ParentElement, RenderOnce, RevealTargetHandle,
    ScrollChainFence, ScrollHandle, ScrollViewportChangeSource, ScrollViewportProgrammaticSource,
    SharedString, StatefulInteractiveElement, Styled, Window, div, px,
};
use open_gpui_motion::{MotionFrameDriver, MotionPreference, advanced::MotionPreset};
use open_gpui_ui_core::virtualizer::VirtualizerGeometryCache;
use open_gpui_ui_core::{
    AccessibleAction, SemanticDescriptor, Sizable, Size, ThemeTokens, UiPx, VirtualizerSnapshot,
};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use super::data::VirtualizedListDataSource;
use super::descriptor::VirtualizedListItemDescriptor;
use super::model::{
    VirtualizedListActivation, VirtualizedListMaterializationResult,
    VirtualizedListMaterializationTarget, VirtualizedListSelectionChange,
    VirtualizedListSelectionMode, VirtualizedListState, virtualized_list_state_items,
};
use super::motion::VirtualizedListActiveIndicatorRuntime;
use super::render::{render_virtualized_list_body, render_virtualized_list_sticky_overlay};
use super::render_plan::{
    VirtualizedListBehaviorSnapshot, VirtualizedListRenderPlan, VirtualizedListRowMeasureMode,
    VirtualizedListRowRenderContext,
};
use super::style::{
    DEFAULT_VIRTUALIZED_LIST_VIEWPORT_ITEM_COUNT, VirtualizedListMetrics, nonnegative_px,
};

pub(super) type VirtualizedListActivationHandler =
    Rc<dyn Fn(VirtualizedListActivation, &mut Window, &mut App)>;
pub(super) type VirtualizedListSelectionChangeHandler =
    Rc<dyn Fn(VirtualizedListSelectionChange, &mut Window, &mut App)>;
pub(super) type VirtualizedListRowRenderer =
    Rc<dyn Fn(VirtualizedListRowRenderContext, &mut Window, &mut App) -> AnyElement>;

#[derive(Debug, Clone)]
struct VirtualizedListGeometryAuthority {
    items: Arc<[VirtualizedListItemDescriptor]>,
    snapshot: Option<VirtualizerSnapshot>,
    cache: VirtualizerGeometryCache,
    revision: u64,
}

impl VirtualizedListGeometryAuthority {
    fn new(
        items: Arc<[VirtualizedListItemDescriptor]>,
        snapshot: Option<&VirtualizerSnapshot>,
    ) -> Self {
        Self {
            items,
            snapshot: snapshot.cloned(),
            cache: VirtualizerGeometryCache::default(),
            revision: 0,
        }
    }

    fn sync(
        &mut self,
        items: &Arc<[VirtualizedListItemDescriptor]>,
        snapshot: Option<&VirtualizerSnapshot>,
    ) -> u64 {
        let items_changed = !same_geometry_items(&self.items, items);
        let snapshot_changed = !same_snapshot_measurements(self.snapshot.as_ref(), snapshot);

        self.items = items.clone();
        self.snapshot = snapshot.cloned();
        if items_changed || snapshot_changed {
            self.invalidate();
        }
        self.revision
    }

    fn invalidate(&mut self) {
        let Some(next) = self.revision.checked_add(1) else {
            self.revision = 0;
            self.cache.clear();
            return;
        };
        self.revision = next;
    }
}

fn same_geometry_items(
    left: &Arc<[VirtualizedListItemDescriptor]>,
    right: &Arc<[VirtualizedListItemDescriptor]>,
) -> bool {
    Arc::ptr_eq(left, right)
        || (left.len() == right.len()
            && left
                .iter()
                .zip(right.iter())
                .all(|(left, right)| left.key() == right.key()))
}

fn same_snapshot_measurements(
    left: Option<&VirtualizerSnapshot>,
    right: Option<&VirtualizerSnapshot>,
) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.shares_measurement_authority_with(right)
                || left.measurements() == right.measurements()
        }
        (None, Some(snapshot)) | (Some(snapshot), None) => snapshot.measurements().is_empty(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VirtualizedListBringIntoViewSource {
    Active,
    Builder,
}

#[derive(Clone, Debug, PartialEq)]
struct VirtualizedListBringIntoViewIntent {
    key: String,
    options: BringIntoViewOptions,
}

impl VirtualizedListBringIntoViewIntent {
    fn new(key: impl Into<String>, options: BringIntoViewOptions) -> Self {
        Self {
            key: key.into(),
            options,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct VirtualizedListBringIntoViewRequest {
    sequence: u64,
    source: VirtualizedListBringIntoViewSource,
    intent: VirtualizedListBringIntoViewIntent,
}

impl VirtualizedListBringIntoViewRequest {
    fn active(sequence: u64, key: impl Into<String>) -> Self {
        Self {
            sequence,
            source: VirtualizedListBringIntoViewSource::Active,
            intent: VirtualizedListBringIntoViewIntent::new(
                key,
                BringIntoViewOptions::vertical(open_gpui::BringIntoViewAlignment::Nearest),
            ),
        }
    }

    fn builder(sequence: u64, intent: VirtualizedListBringIntoViewIntent) -> Self {
        Self {
            sequence,
            source: VirtualizedListBringIntoViewSource::Builder,
            intent,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum VirtualizedListBringIntoViewStage {
    Materializing,
    Ready,
    Queued,
    InFlight,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VirtualizedListBringIntoViewRetry {
    GeometryChanged,
    BuilderTargetUnlinked,
}

impl VirtualizedListBringIntoViewRetry {
    fn accepts(self, outcome: BringIntoViewOutcome) -> bool {
        match self {
            Self::GeometryChanged => retry_after_geometry_change(outcome),
            Self::BuilderTargetUnlinked => {
                retry_after_geometry_change(outcome)
                    || outcome
                        == BringIntoViewOutcome::Cancelled(
                            BringIntoViewCancelReason::TargetUnlinked,
                        )
            }
        }
    }
}

#[derive(Debug)]
struct VirtualizedListBringIntoViewOperation {
    request: VirtualizedListBringIntoViewRequest,
    stage: VirtualizedListBringIntoViewStage,
    materialization_revision: Option<u64>,
    deferred_guard: Option<DeferredBringIntoViewGuard>,
    materialization_fence: Option<ScrollChainFence>,
    requires_input_fence: bool,
    retry_after_completion: Option<VirtualizedListBringIntoViewRetry>,
    submitted_authority_generation: Option<BringIntoViewChainGeneration>,
    retry_authority_generation: Option<BringIntoViewChainGeneration>,
}

impl VirtualizedListBringIntoViewOperation {
    fn scroll_was_overridden(&self, window: &Window) -> bool {
        self.materialization_fence
            .as_ref()
            .is_some_and(|fence| window.scroll_chain_fence_was_interrupted(fence))
    }

    fn input_fence_is_missing(&self) -> bool {
        self.requires_input_fence && self.materialization_fence.is_none()
    }

    fn retry_authority_was_replaced(&self, window: &Window) -> bool {
        self.retry_authority_generation
            .is_some_and(|generation| window.bring_into_view_authority_generation() != generation)
    }

    fn has_deferred_guard(&self) -> bool {
        self.deferred_guard.is_some()
    }

    fn render_snapshot(&self, window: &Window) -> VirtualizedListBringIntoViewRenderOperation {
        VirtualizedListBringIntoViewRenderOperation {
            request: self.request.clone(),
            stage: self.stage,
            materialization_revision: self.materialization_revision,
            has_deferred_guard: self.has_deferred_guard(),
            input_fence_is_missing: self.input_fence_is_missing(),
            direct_scroll_was_overridden: self.scroll_was_overridden(window),
            retry_authority_was_replaced: self.retry_authority_was_replaced(window),
        }
    }
}

#[derive(Clone, Debug)]
struct VirtualizedListBringIntoViewRenderOperation {
    request: VirtualizedListBringIntoViewRequest,
    stage: VirtualizedListBringIntoViewStage,
    materialization_revision: Option<u64>,
    has_deferred_guard: bool,
    input_fence_is_missing: bool,
    direct_scroll_was_overridden: bool,
    retry_authority_was_replaced: bool,
}

#[derive(Clone)]
pub(super) struct VirtualizedListDeferredMaterialization {
    pub(super) sequence: u64,
    pub(super) options: BringIntoViewOptions,
    pub(super) geometry_revision: u64,
    pub(super) target: VirtualizedListMaterializationTarget,
    pub(super) state: VirtualizedListState,
    pub(super) row_measure_mode: VirtualizedListRowMeasureMode,
    pub(super) virtualizer_snapshot: VirtualizerSnapshot,
    pub(super) target_is_rendered: bool,
}

fn retry_after_geometry_change(outcome: BringIntoViewOutcome) -> bool {
    matches!(outcome, BringIntoViewOutcome::Completed(_))
}

#[cfg(test)]
fn finish_in_flight_bring_into_view(
    operation: &mut Option<VirtualizedListBringIntoViewOperation>,
    sequence: u64,
) -> bool {
    if !operation.as_ref().is_some_and(|operation| {
        operation.stage == VirtualizedListBringIntoViewStage::InFlight
            && operation.request.sequence == sequence
    }) {
        return false;
    }
    *operation = None;
    true
}

#[derive(Debug)]
pub(super) struct VirtualizedListRuntime {
    pub(super) scroll_surface: ScrollSurfaceRuntime,
    pub(super) focus_handle: FocusHandle,
    pub(super) active_key: Option<String>,
    pub(super) selected_keys: BTreeSet<String>,
    pub(super) selection_anchor_key: Option<String>,
    pub(super) row_measurements: BTreeMap<String, UiPx>,
    geometry: VirtualizedListGeometryAuthority,
    bring_into_view: Option<VirtualizedListBringIntoViewOperation>,
    scroll_chain_anchor: Option<RevealTargetHandle>,
    last_builder_bring_into_view: Option<VirtualizedListBringIntoViewIntent>,
    next_bring_into_view_sequence: u64,
    pub(super) typeahead: CollectionTypeaheadSession,
    pub(super) active_indicator: VirtualizedListActiveIndicatorRuntime,
    pub(super) active_indicator_frame_host: MotionFrameDriver,
}

#[derive(Debug)]
struct VirtualizedListRuntimeRenderSnapshot {
    scroll_surface: ScrollSurfaceRuntime,
    focus_handle: FocusHandle,
    active_key: Option<String>,
    selected_keys: BTreeSet<String>,
    bring_into_view: Option<VirtualizedListBringIntoViewRenderOperation>,
}

impl VirtualizedListRuntime {
    fn next_bring_into_view_sequence(&mut self) -> u64 {
        self.next_bring_into_view_sequence = self
            .next_bring_into_view_sequence
            .checked_add(1)
            .expect("virtualized-list bring-into-view sequence exhausted");
        self.next_bring_into_view_sequence
    }

    fn request_for(
        &mut self,
        source: VirtualizedListBringIntoViewSource,
        intent: VirtualizedListBringIntoViewIntent,
    ) -> VirtualizedListBringIntoViewRequest {
        let sequence = self.next_bring_into_view_sequence();
        match source {
            VirtualizedListBringIntoViewSource::Active => {
                VirtualizedListBringIntoViewRequest::active(sequence, intent.key)
            }
            VirtualizedListBringIntoViewSource::Builder => {
                VirtualizedListBringIntoViewRequest::builder(sequence, intent)
            }
        }
    }

    fn replace_with_pending_bring_into_view(
        &mut self,
        source: VirtualizedListBringIntoViewSource,
        intent: VirtualizedListBringIntoViewIntent,
        materialization_fence: Option<ScrollChainFence>,
        requires_input_fence: bool,
    ) {
        let request = self.request_for(source, intent);
        self.bring_into_view = Some(VirtualizedListBringIntoViewOperation {
            request,
            stage: VirtualizedListBringIntoViewStage::Materializing,
            materialization_revision: None,
            deferred_guard: None,
            materialization_fence,
            requires_input_fence,
            retry_after_completion: None,
            submitted_authority_generation: None,
            retry_authority_generation: None,
        });
    }

    fn retry_bring_into_view(
        &mut self,
        request: &VirtualizedListBringIntoViewRequest,
        expected_stage: VirtualizedListBringIntoViewStage,
    ) -> bool {
        if !self.bring_into_view.as_ref().is_some_and(|operation| {
            operation.request == *request && operation.stage == expected_stage
        }) {
            return false;
        }
        let operation = self
            .bring_into_view
            .as_mut()
            .expect("matching virtualized-list reveal operation should remain present");
        operation.stage = VirtualizedListBringIntoViewStage::Materializing;
        operation.materialization_revision = None;
        operation.deferred_guard = None;
        operation.retry_after_completion = None;
        operation.submitted_authority_generation = None;
        true
    }

    fn transition_bring_into_view(
        &mut self,
        request: &VirtualizedListBringIntoViewRequest,
        from: VirtualizedListBringIntoViewStage,
        to: VirtualizedListBringIntoViewStage,
    ) -> bool {
        let Some(operation) = self.bring_into_view.as_mut() else {
            return false;
        };
        if operation.request != *request || operation.stage != from {
            return false;
        }
        operation.stage = to;
        true
    }

    pub(super) fn publish_deferred_bring_into_view_guard(
        &mut self,
        sequence: u64,
        materialization_revision: u64,
        guard: DeferredBringIntoViewGuard,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(operation) = self.bring_into_view.as_mut() else {
            return false;
        };
        if operation.request.sequence != sequence
            || operation.stage != VirtualizedListBringIntoViewStage::Materializing
        {
            return false;
        }
        operation.stage = VirtualizedListBringIntoViewStage::Ready;
        operation.materialization_revision = Some(materialization_revision);
        operation.materialization_fence = Some(guard.scroll_chain_fence());
        operation.deferred_guard = Some(guard);
        cx.notify();
        true
    }

    pub(super) fn abandon_materializing_bring_into_view(
        &mut self,
        sequence: u64,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.bring_into_view.as_ref().is_some_and(|operation| {
            operation.request.sequence == sequence
                && operation.stage == VirtualizedListBringIntoViewStage::Materializing
        }) {
            return false;
        }
        self.bring_into_view = None;
        cx.notify();
        true
    }

    fn take_window_bring_into_view_guard(
        &mut self,
        sequence: u64,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<DeferredBringIntoViewGuard> {
        let Some(operation) = self.bring_into_view.as_ref() else {
            return None;
        };
        if operation.request.sequence != sequence
            || operation.stage != VirtualizedListBringIntoViewStage::Queued
        {
            return None;
        }
        if !operation.has_deferred_guard()
            || operation.input_fence_is_missing()
            || operation.scroll_was_overridden(window)
            || operation.retry_authority_was_replaced(window)
        {
            self.bring_into_view = None;
            cx.notify();
            return None;
        }
        let operation = self
            .bring_into_view
            .as_mut()
            .expect("matching virtualized-list reveal operation should remain present");
        operation.stage = VirtualizedListBringIntoViewStage::InFlight;
        Some(
            operation
                .deferred_guard
                .take()
                .expect("queued virtualized-list reveal should retain its deferred guard"),
        )
    }

    fn record_submitted_bring_into_view_authority_generation(
        &mut self,
        sequence: u64,
        generation: BringIntoViewChainGeneration,
    ) -> bool {
        let Some(operation) = self.bring_into_view.as_mut() else {
            return false;
        };
        if operation.request.sequence != sequence
            || operation.stage != VirtualizedListBringIntoViewStage::InFlight
        {
            return false;
        }
        operation.submitted_authority_generation = Some(generation);
        operation.retry_authority_generation = None;
        true
    }

    pub(super) fn prepare_deferred_materialization(
        &mut self,
        sequence: u64,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(operation) = self.bring_into_view.as_ref() else {
            return false;
        };
        let is_current = operation.request.sequence == sequence
            && operation.stage == VirtualizedListBringIntoViewStage::Materializing;
        if !is_current {
            return false;
        }
        let fence_is_valid = operation
            .materialization_fence
            .as_ref()
            .is_none_or(|fence| {
                !window.scroll_chain_fence_was_interrupted(fence)
                    && window.scroll_chain_fence_matches_current_ancestry(fence)
            });
        if operation.input_fence_is_missing()
            || !fence_is_valid
            || operation.retry_authority_was_replaced(window)
        {
            self.bring_into_view = None;
            cx.notify();
            return false;
        }
        true
    }

    fn retry_after_window_completion(
        &mut self,
        request: &VirtualizedListBringIntoViewRequest,
        window: &Window,
        retry: VirtualizedListBringIntoViewRetry,
    ) -> bool {
        let Some(operation) = self.bring_into_view.as_mut() else {
            return false;
        };
        if operation.request != *request
            || operation.stage != VirtualizedListBringIntoViewStage::InFlight
        {
            return false;
        }
        if operation.scroll_was_overridden(window) || operation.retry_authority_was_replaced(window)
        {
            return false;
        }
        operation.retry_after_completion = Some(retry);
        true
    }

    fn cancel_bring_into_view(
        &mut self,
        request: &VirtualizedListBringIntoViewRequest,
        expected_stage: VirtualizedListBringIntoViewStage,
    ) -> bool {
        if !self.bring_into_view.as_ref().is_some_and(|operation| {
            operation.request == *request && operation.stage == expected_stage
        }) {
            return false;
        }
        self.bring_into_view = None;
        true
    }

    fn finish_bring_into_view(
        &mut self,
        sequence: u64,
        outcome: BringIntoViewOutcome,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        let Some(operation) = self.bring_into_view.as_ref() else {
            return;
        };
        if operation.stage != VirtualizedListBringIntoViewStage::InFlight
            || operation.request.sequence != sequence
        {
            return;
        }
        let retry_generation = operation
            .submitted_authority_generation
            .filter(|generation| {
                operation
                    .retry_after_completion
                    .is_some_and(|retry| retry.accepts(outcome))
                    && !operation.scroll_was_overridden(window)
                    && window.bring_into_view_authority_generation() == *generation
            });
        if let Some(retry_generation) = retry_generation {
            let operation = self
                .bring_into_view
                .as_mut()
                .expect("matching virtualized-list reveal operation should remain present");
            operation.stage = VirtualizedListBringIntoViewStage::Materializing;
            operation.materialization_revision = None;
            operation.deferred_guard = None;
            operation.retry_after_completion = None;
            operation.submitted_authority_generation = None;
            operation.retry_authority_generation = Some(retry_generation);
        } else {
            self.bring_into_view = None;
        }
        cx.notify();
    }

    fn render_snapshot(&self, window: &Window) -> VirtualizedListRuntimeRenderSnapshot {
        VirtualizedListRuntimeRenderSnapshot {
            scroll_surface: self.scroll_surface.clone(),
            focus_handle: self.focus_handle.clone(),
            active_key: self.active_key.clone(),
            selected_keys: self.selected_keys.clone(),
            bring_into_view: self
                .bring_into_view
                .as_ref()
                .map(|operation| operation.render_snapshot(window)),
        }
    }

    fn sync_builder_bring_into_view(
        &mut self,
        requested: Option<VirtualizedListBringIntoViewIntent>,
    ) {
        if self.last_builder_bring_into_view == requested {
            return;
        }

        self.last_builder_bring_into_view = requested.clone();
        match requested {
            Some(intent) => {
                self.replace_with_pending_bring_into_view(
                    VirtualizedListBringIntoViewSource::Builder,
                    intent,
                    None,
                    false,
                );
            }
            None => {
                if self.bring_into_view.as_ref().is_some_and(|operation| {
                    operation.request.source == VirtualizedListBringIntoViewSource::Builder
                }) {
                    self.bring_into_view = None;
                }
            }
        }
    }

    fn queue_active_bring_into_view(&mut self, key: impl Into<String>) {
        self.replace_with_pending_bring_into_view(
            VirtualizedListBringIntoViewSource::Active,
            VirtualizedListBringIntoViewIntent::new(
                key,
                BringIntoViewOptions::vertical(open_gpui::BringIntoViewAlignment::Nearest),
            ),
            None,
            false,
        );
    }

    fn queue_active_bring_into_view_from_input(
        &mut self,
        key: impl Into<String>,
        window: &mut Window,
    ) -> bool {
        let anchor = self.scroll_chain_anchor(window);
        let Some(fence) = window
            .capture_committed_scroll_chain_fence(
                &anchor,
                BringIntoViewOptions::vertical(open_gpui::BringIntoViewAlignment::Nearest),
            )
            .ok()
            .flatten()
        else {
            return false;
        };
        self.replace_with_pending_bring_into_view(
            VirtualizedListBringIntoViewSource::Active,
            VirtualizedListBringIntoViewIntent::new(
                key,
                BringIntoViewOptions::vertical(open_gpui::BringIntoViewAlignment::Nearest),
            ),
            Some(fence),
            true,
        );
        true
    }

    pub(super) fn scroll_chain_anchor(&mut self, window: &mut Window) -> RevealTargetHandle {
        let window_id = window.window_handle().window_id();
        if !self
            .scroll_chain_anchor
            .is_some_and(|anchor| anchor.window_id() == window_id)
        {
            self.scroll_chain_anchor = Some(window.new_reveal_target());
        }
        self.scroll_chain_anchor
            .expect("virtualized-list scroll-chain anchor should be initialized for this window")
    }

    pub(super) fn clear_active_bring_into_view(&mut self) {
        if self.bring_into_view.as_ref().is_some_and(|operation| {
            operation.request.source == VirtualizedListBringIntoViewSource::Active
        }) {
            self.bring_into_view = None;
        }
    }

    pub(super) fn set_row_measurement(
        &mut self,
        render_key: String,
        height: UiPx,
        cx: &mut Context<Self>,
    ) {
        let height = nonnegative_px(height);
        if self.row_measurements.get(&render_key).copied() != Some(height) {
            self.row_measurements.insert(render_key, height);
            self.geometry.invalidate();
            cx.notify();
        }
    }
}

#[cfg(test)]
mod geometry_authority_tests {
    use super::*;
    use open_gpui::BringIntoViewCancelReason;
    use open_gpui_ui_core::{VirtualizerSnapshotItem, ui_px};

    fn items(keys: &[&str]) -> Arc<[VirtualizedListItemDescriptor]> {
        keys.iter()
            .map(|key| VirtualizedListItemDescriptor::new(*key, key.to_uppercase()))
            .collect::<Vec<_>>()
            .into()
    }

    #[test]
    fn stale_bring_into_view_completion_cannot_finish_a_newer_equal_intent() {
        let intent = VirtualizedListBringIntoViewIntent::new(
            "alpha",
            BringIntoViewOptions::vertical(open_gpui::BringIntoViewAlignment::Nearest),
        );
        let mut operation = Some(VirtualizedListBringIntoViewOperation {
            request: VirtualizedListBringIntoViewRequest::builder(2, intent),
            stage: VirtualizedListBringIntoViewStage::InFlight,
            materialization_revision: Some(0),
            deferred_guard: None,
            materialization_fence: None,
            requires_input_fence: false,
            retry_after_completion: None,
            submitted_authority_generation: None,
            retry_authority_generation: None,
        });

        assert!(!finish_in_flight_bring_into_view(&mut operation, 1));
        assert_eq!(
            operation
                .as_ref()
                .map(|operation| operation.request.sequence),
            Some(2)
        );
        assert!(finish_in_flight_bring_into_view(&mut operation, 2));
        assert!(operation.is_none());
    }

    #[test]
    fn geometry_retry_reopens_only_a_completed_request() {
        assert!(retry_after_geometry_change(
            BringIntoViewOutcome::Completed(open_gpui::BringIntoViewCompletion::Revealed,)
        ));
        for outcome in [
            BringIntoViewOutcome::Cancelled(BringIntoViewCancelReason::Superseded),
            BringIntoViewOutcome::Cancelled(BringIntoViewCancelReason::ScrollOverridden),
            BringIntoViewOutcome::Cancelled(BringIntoViewCancelReason::TargetUnlinked),
            BringIntoViewOutcome::Cancelled(BringIntoViewCancelReason::AncestryChanged),
            BringIntoViewOutcome::Cancelled(BringIntoViewCancelReason::TargetSuppressed),
            BringIntoViewOutcome::Cancelled(BringIntoViewCancelReason::NoProgress),
            BringIntoViewOutcome::Cancelled(BringIntoViewCancelReason::WindowClosed),
        ] {
            assert!(
                !retry_after_geometry_change(outcome),
                "{outcome:?} must terminate an interrupted virtual reveal"
            );
        }
    }

    #[test]
    fn fresh_equivalent_item_storage_preserves_geometry_revision() {
        let first = items(&["alpha", "beta", "gamma"]);
        let equivalent = items(&["alpha", "beta", "gamma"]);
        let reordered = items(&["beta", "alpha", "gamma"]);
        let mut authority = VirtualizedListGeometryAuthority::new(first.clone(), None);

        assert_eq!(authority.sync(&first, None), 0);
        assert_eq!(authority.sync(&equivalent, None), 0);
        assert_eq!(authority.sync(&reordered, None), 1);
    }

    #[test]
    fn snapshot_measurement_authority_and_wrap_invalidate_exactly() {
        let list_items = items(&["alpha", "beta"]);
        let snapshot = VirtualizerSnapshot::new(
            UiPx::ZERO,
            [VirtualizerSnapshotItem::new("beta".into(), ui_px(44.0))],
        );
        let same_content = VirtualizerSnapshot::new(
            ui_px(20.0),
            [VirtualizerSnapshotItem::new("beta".into(), ui_px(44.0))],
        );
        let changed = VirtualizerSnapshot::new(
            ui_px(20.0),
            [VirtualizerSnapshotItem::new("beta".into(), ui_px(72.0))],
        );
        let mut authority =
            VirtualizedListGeometryAuthority::new(list_items.clone(), Some(&snapshot));

        assert_eq!(authority.sync(&list_items, Some(&snapshot)), 0);
        assert_eq!(authority.sync(&list_items, Some(&same_content)), 0);
        assert_eq!(authority.sync(&list_items, Some(&changed)), 1);

        authority.revision = u64::MAX;
        authority.invalidate();
        assert_eq!(authority.revision, 0);
    }
}

/// A concrete GPUI virtualized list renderer.
#[derive(IntoElement)]
pub struct VirtualizedList {
    id: String,
    label: SharedString,
    items: Arc<[VirtualizedListItemDescriptor]>,
    size: Size,
    disabled: bool,
    active_key: Option<String>,
    selected_keys: BTreeSet<String>,
    selection_mode: VirtualizedListSelectionMode,
    tokens: ThemeTokens,
    viewport_item_count: usize,
    metrics: VirtualizedListMetrics,
    row_measure_mode: VirtualizedListRowMeasureMode,
    motion_preference: Option<MotionPreference>,
    snapshot: Option<VirtualizerSnapshot>,
    scroll_handle: Option<ScrollHandle>,
    bring_into_view_key: Option<String>,
    bring_into_view_options: BringIntoViewOptions,
    row_renderer: Option<VirtualizedListRowRenderer>,
    on_activate: Option<VirtualizedListActivationHandler>,
    on_selection_change: Option<VirtualizedListSelectionChangeHandler>,
}

impl VirtualizedList {
    /// Creates a new virtualized list renderer.
    pub fn new(
        id: impl Into<String>,
        label: impl Into<SharedString>,
        items: impl IntoIterator<Item = VirtualizedListItemDescriptor>,
    ) -> Self {
        Self::from_shared_items(
            id,
            label,
            Arc::from(items.into_iter().collect::<Vec<_>>().into_boxed_slice()),
        )
    }

    /// Creates a new virtualized list renderer from shared item storage.
    pub fn from_shared_items(
        id: impl Into<String>,
        label: impl Into<SharedString>,
        items: Arc<[VirtualizedListItemDescriptor]>,
    ) -> Self {
        let size = Size::Medium;

        Self {
            id: id.into(),
            label: label.into(),
            items,
            size,
            disabled: false,
            active_key: None,
            selected_keys: BTreeSet::new(),
            selection_mode: VirtualizedListSelectionMode::Single,
            tokens: ThemeTokens::default(),
            viewport_item_count: DEFAULT_VIRTUALIZED_LIST_VIEWPORT_ITEM_COUNT,
            metrics: VirtualizedListMetrics::from_size(size),
            row_measure_mode: VirtualizedListRowMeasureMode::default(),
            motion_preference: None,
            snapshot: None,
            scroll_handle: None,
            bring_into_view_key: None,
            bring_into_view_options: BringIntoViewOptions::vertical(
                open_gpui::BringIntoViewAlignment::Nearest,
            ),
            row_renderer: None,
            on_activate: None,
            on_selection_change: None,
        }
    }

    /// Creates a new virtualized list renderer from an application-level data source.
    pub fn from_data_source(
        id: impl Into<String>,
        label: impl Into<SharedString>,
        data_source: impl Into<VirtualizedListDataSource>,
    ) -> Self {
        Self::from_shared_items(id, label, data_source.into().into_shared_items())
    }

    /// Marks the list as disabled.
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Applies the default active item key for adapter-owned runtime state.
    pub fn default_active_key(mut self, key: impl Into<String>) -> Self {
        self.active_key = Some(key.into());
        self
    }

    /// Applies the default selected item key for adapter-owned runtime state.
    pub fn default_selected_key(mut self, key: impl Into<String>) -> Self {
        self.selected_keys.clear();
        self.selected_keys.insert(key.into());
        self
    }

    /// Applies the default selected item keys for adapter-owned runtime state.
    pub fn default_selected_keys<I, K>(mut self, keys: I) -> Self
    where
        I: IntoIterator<Item = K>,
        K: Into<String>,
    {
        self.selected_keys = keys.into_iter().map(Into::into).collect();
        self
    }

    /// Applies the list selection behavior.
    pub fn selection_mode(mut self, selection_mode: VirtualizedListSelectionMode) -> Self {
        self.selection_mode = selection_mode;
        self
    }

    /// Applies the theme token bundle used by virtualized-list color recipes.
    pub fn tokens(mut self, tokens: ThemeTokens) -> Self {
        self.tokens = tokens;
        self
    }

    /// Applies the estimated viewport item count used for keyboard page navigation.
    pub fn viewport_item_count(mut self, count: usize) -> Self {
        self.viewport_item_count = count.max(1);
        self
    }

    /// Applies a fixed row height.
    pub fn row_height(mut self, row_height: UiPx) -> Self {
        self.metrics = self.metrics.with_row_height(row_height);
        self
    }

    /// Applies the body row measurement mode.
    pub fn row_measure_mode(mut self, row_measure_mode: VirtualizedListRowMeasureMode) -> Self {
        self.row_measure_mode = row_measure_mode;
        self
    }

    /// Requests a motion preference for active-descendant chrome.
    ///
    /// Reduced motion from either this request or the active theme remains authoritative.
    pub fn motion_preference(mut self, motion_preference: MotionPreference) -> Self {
        self.motion_preference = Some(motion_preference);
        self
    }

    /// Seeds measured-row virtualizer measurements from a snapshot.
    pub fn virtualizer_snapshot(mut self, snapshot: VirtualizerSnapshot) -> Self {
        self.snapshot = Some(snapshot);
        self
    }

    /// Materializes a keyed row, then requests its final physical reveal from GPUI.
    pub fn bring_key_into_view(
        mut self,
        key: impl Into<String>,
        options: BringIntoViewOptions,
    ) -> Self {
        self.bring_into_view_key = Some(key.into());
        self.bring_into_view_options = options;
        self
    }

    /// Applies the overscan row budget.
    pub fn overscan(mut self, overscan: usize) -> Self {
        self.metrics = self.metrics.with_overscan_count(overscan);
        self
    }

    /// Registers an activation handler for clicked or keyboard-activated rows.
    pub fn on_activate(
        mut self,
        handler: impl Fn(VirtualizedListActivation, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_activate = Some(Rc::new(handler));
        self
    }

    /// Registers a selection-change handler for controlled selected keys.
    pub fn on_selection_change(
        mut self,
        handler: impl Fn(VirtualizedListSelectionChange, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_selection_change = Some(Rc::new(handler));
        self
    }

    /// Returns resolved renderer-neutral list state from the builder seed.
    pub fn state(&self) -> VirtualizedListState {
        self.resolved_state(
            self.active_key.as_deref(),
            self.selected_keys.iter().map(String::as_str),
            self.viewport_item_count,
        )
    }

    /// Returns the public behavior snapshot at the default viewport origin.
    pub fn behavior_snapshot(&self) -> VirtualizedListBehaviorSnapshot {
        self.behavior_snapshot_with_viewport(
            UiPx::ZERO,
            self.metrics.row_height() * self.viewport_item_count as f32,
        )
    }

    /// Resolves the public behavior snapshot for a viewport.
    pub fn behavior_snapshot_with_viewport(
        &self,
        scroll_offset: UiPx,
        viewport_extent: UiPx,
    ) -> VirtualizedListBehaviorSnapshot {
        let plan = self.render_plan(scroll_offset, viewport_extent);
        VirtualizedListBehaviorSnapshot::from_render_plan(&plan)
    }

    /// Resolves the renderer-neutral state and virtual window for the current list.
    fn render_plan(&self, scroll_offset: UiPx, viewport_extent: UiPx) -> VirtualizedListRenderPlan {
        let state = self.resolved_state(
            self.active_key.as_deref(),
            self.selected_keys.iter().map(String::as_str),
            self.viewport_item_count,
        );
        VirtualizedListRenderPlan::resolve(
            self.id.clone(),
            self.label.to_string(),
            state,
            self.items.as_ref(),
            self.row_measure_mode,
            &BTreeMap::new(),
            self.snapshot.as_ref(),
            scroll_offset,
            viewport_extent,
        )
    }

    fn resolved_state<'a, I>(
        &self,
        active_key: Option<&str>,
        selected_keys: I,
        viewport_item_count: usize,
    ) -> VirtualizedListState
    where
        I: IntoIterator<Item = &'a str>,
    {
        VirtualizedListState::resolve(
            self.size,
            self.disabled,
            virtualized_list_state_items(self.items.as_ref()),
            active_key,
            selected_keys,
            self.selection_mode,
            Some(viewport_item_count.max(1)),
        )
        .with_metrics(self.metrics)
    }
}

/// GPUI-specific adapter extension methods for `VirtualizedList`.
///
/// The core `VirtualizedList` builder keeps renderer-neutral state and semantics. Import this
/// trait when a concrete GPUI surface needs a host-owned `ScrollHandle` or custom row renderer.
pub trait VirtualizedListGpuiExt: Sized {
    /// Uses an externally owned GPUI scroll handle for the list viewport.
    fn scroll_handle(self, scroll_handle: &ScrollHandle) -> Self;

    /// Registers a custom row-content renderer.
    ///
    /// The outer row keeps ownership of virtual layout, accessibility, focus, hit testing, and
    /// selection behavior. The renderer replaces only the row content.
    fn render_row<E>(
        self,
        renderer: impl Fn(VirtualizedListRowRenderContext, &mut Window, &mut App) -> E + 'static,
    ) -> Self
    where
        E: IntoElement + 'static;
}

impl VirtualizedListGpuiExt for VirtualizedList {
    fn scroll_handle(mut self, scroll_handle: &ScrollHandle) -> Self {
        self.scroll_handle = Some(scroll_handle.clone());
        self
    }

    fn render_row<E>(
        mut self,
        renderer: impl Fn(VirtualizedListRowRenderContext, &mut Window, &mut App) -> E + 'static,
    ) -> Self
    where
        E: IntoElement + 'static,
    {
        self.row_renderer = Some(Rc::new(move |context, window, cx| {
            renderer(context, window, cx).into_any_element()
        }));
        self
    }
}

impl Sizable for VirtualizedList {
    fn with_size(mut self, size: Size) -> Self {
        self.size = size;
        self.metrics = VirtualizedListMetrics::from_size(size);
        self
    }
}

impl RenderOnce for VirtualizedList {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let theme = ThemeResolver::current(window, cx);
        let colors = ThemeResolver::virtualized_list_colors(self.tokens);
        let focus_shadow = focus_ring_shadow_with_theme(colors.focus_ring_shape(), &theme);
        let runtime_id = format!("virtualized-list:{}:runtime", self.id);
        let debug_id = self.id.to_string();
        let now = Instant::now();
        let motion_preference =
            ThemeResolver::virtualized_list_motion_preference(&theme, self.motion_preference);
        let active_indicator_model = MotionPreset::affordance(motion_preference).resolve_model();
        let runtime = window.use_keyed_state(runtime_id, cx, |_, cx| VirtualizedListRuntime {
            scroll_surface: ScrollSurfaceRuntime::new(None),
            focus_handle: cx.focus_handle(),
            active_key: self.active_key.clone(),
            selected_keys: self.selected_keys.clone(),
            selection_anchor_key: self.active_key.clone(),
            row_measurements: BTreeMap::new(),
            geometry: VirtualizedListGeometryAuthority::new(
                self.items.clone(),
                self.snapshot.as_ref(),
            ),
            bring_into_view: None,
            scroll_chain_anchor: None,
            last_builder_bring_into_view: None,
            next_bring_into_view_sequence: 0,
            typeahead: CollectionTypeaheadSession::default(),
            active_indicator: VirtualizedListActiveIndicatorRuntime::default(),
            active_indicator_frame_host: MotionFrameDriver::new(),
        });
        let requested_bring_into_view = self
            .bring_into_view_key
            .as_deref()
            .map(|key| VirtualizedListBringIntoViewIntent::new(key, self.bring_into_view_options));
        let (runtime_state, scroll_chain_anchor) = runtime.update(cx, |runtime, _| {
            runtime.sync_builder_bring_into_view(requested_bring_into_view);
            let scroll_chain_anchor = runtime.scroll_chain_anchor(window);
            (runtime.render_snapshot(window), scroll_chain_anchor)
        });
        let scroll_handle =
            scroll_surface_handle(&runtime_state.scroll_surface, self.scroll_handle.as_ref());
        let focus_handle = runtime_state.focus_handle.clone();
        let viewport_extent = vertical_viewport_extent(&scroll_handle);
        let viewport_item_count = resolve_viewport_item_count(
            self.metrics.row_height(),
            viewport_extent,
            self.viewport_item_count,
        );
        let state = self.resolved_state(
            runtime_state.active_key.as_deref(),
            runtime_state.selected_keys.iter().map(String::as_str),
            viewport_item_count,
        );
        let scroll_offset = vertical_scroll_offset(&scroll_handle);
        let (plan, geometry_revision) = runtime.update(cx, |runtime, _| {
            let VirtualizedListRuntime {
                row_measurements,
                geometry,
                ..
            } = runtime;
            let geometry_revision = geometry.sync(&self.items, self.snapshot.as_ref());
            let plan = VirtualizedListRenderPlan::resolve_cached(
                self.id.clone(),
                self.label.to_string(),
                state.clone(),
                self.items.as_ref(),
                self.row_measure_mode,
                row_measurements,
                self.snapshot.as_ref(),
                scroll_offset,
                viewport_extent,
                &mut geometry.cache,
                geometry_revision,
            );
            (plan, geometry_revision)
        });
        let mut tracked_reveal_key = state.active_key().map(str::to_owned);
        let mut ready_bring_into_view = None;
        let mut deferred_materialization = None;
        if let Some(operation) = runtime_state.bring_into_view {
            let request = operation.request.clone();
            if operation.input_fence_is_missing
                || operation.direct_scroll_was_overridden
                || operation.retry_authority_was_replaced
            {
                runtime.update(cx, |runtime, _| {
                    runtime.cancel_bring_into_view(&request, operation.stage);
                });
            } else {
                let resolution = resolve_virtualized_list_materialization_target(
                    &state,
                    &request.intent.key,
                    plan.row_measure_mode(),
                    plan.virtualizer().snapshot(),
                );
                match resolution {
                    VirtualizedListMaterializationResult::Target(target) => {
                        let target_is_rendered = plan
                            .rows()
                            .iter()
                            .any(|row| row.index() == target.index() && row.key() == target.key());
                        match operation.stage {
                            VirtualizedListBringIntoViewStage::Materializing => {
                                tracked_reveal_key = Some(request.intent.key.clone());
                                deferred_materialization =
                                    Some(VirtualizedListDeferredMaterialization {
                                        sequence: request.sequence,
                                        options: request.intent.options,
                                        geometry_revision,
                                        target,
                                        state: state.clone(),
                                        row_measure_mode: plan.row_measure_mode(),
                                        virtualizer_snapshot: plan.virtualizer().snapshot().clone(),
                                        target_is_rendered,
                                    });
                            }
                            VirtualizedListBringIntoViewStage::Ready if target_is_rendered => {
                                if !operation.has_deferred_guard {
                                    runtime.update(cx, |runtime, _| {
                                        runtime.cancel_bring_into_view(
                                            &request,
                                            VirtualizedListBringIntoViewStage::Ready,
                                        );
                                    });
                                } else {
                                    tracked_reveal_key = Some(request.intent.key.clone());
                                    ready_bring_into_view = Some(request);
                                }
                            }
                            VirtualizedListBringIntoViewStage::Queued if target_is_rendered => {
                                if !operation.has_deferred_guard {
                                    runtime.update(cx, |runtime, _| {
                                        runtime.cancel_bring_into_view(
                                            &request,
                                            VirtualizedListBringIntoViewStage::Queued,
                                        );
                                    });
                                } else {
                                    tracked_reveal_key = Some(request.intent.key.clone());
                                }
                            }
                            VirtualizedListBringIntoViewStage::InFlight if target_is_rendered => {
                                tracked_reveal_key = Some(request.intent.key.clone());
                            }
                            stage @ (VirtualizedListBringIntoViewStage::Ready
                            | VirtualizedListBringIntoViewStage::Queued)
                                if operation.materialization_revision
                                    != Some(geometry_revision) =>
                            {
                                let retried = runtime.update(cx, |runtime, _| {
                                    runtime.retry_bring_into_view(&request, stage)
                                });
                                if retried {
                                    window.refresh();
                                }
                            }
                            stage @ (VirtualizedListBringIntoViewStage::Ready
                            | VirtualizedListBringIntoViewStage::Queued) => {
                                runtime.update(cx, |runtime, _| {
                                    runtime.cancel_bring_into_view(&request, stage);
                                });
                            }
                            VirtualizedListBringIntoViewStage::InFlight
                                if operation.materialization_revision
                                    != Some(geometry_revision) =>
                            {
                                runtime.update(cx, |runtime, _| {
                                    runtime.retry_after_window_completion(
                                        &request,
                                        window,
                                        VirtualizedListBringIntoViewRetry::GeometryChanged,
                                    );
                                });
                            }
                            VirtualizedListBringIntoViewStage::InFlight => {
                                // The core request owns physical reveal until this exact terminal
                                // outcome returns to the retained operation.
                            }
                        }
                    }
                    VirtualizedListMaterializationResult::NotFound(_)
                        if request.source == VirtualizedListBringIntoViewSource::Builder
                            && operation.stage
                                == VirtualizedListBringIntoViewStage::Materializing => {}
                    VirtualizedListMaterializationResult::NotFound(_)
                        if request.source == VirtualizedListBringIntoViewSource::Builder
                            && matches!(
                                operation.stage,
                                VirtualizedListBringIntoViewStage::Ready
                                    | VirtualizedListBringIntoViewStage::Queued
                            ) =>
                    {
                        let retried = runtime.update(cx, |runtime, _| {
                            runtime.retry_bring_into_view(&request, operation.stage)
                        });
                        if retried {
                            window.refresh();
                        }
                    }
                    VirtualizedListMaterializationResult::NotFound(_)
                        if request.source == VirtualizedListBringIntoViewSource::Builder
                            && operation.stage == VirtualizedListBringIntoViewStage::InFlight =>
                    {
                        runtime.update(cx, |runtime, _| {
                            runtime.retry_after_window_completion(
                                &request,
                                window,
                                VirtualizedListBringIntoViewRetry::BuilderTargetUnlinked,
                            );
                        });
                    }
                    _ => {
                        runtime.update(cx, |runtime, _| {
                            runtime.cancel_bring_into_view(&request, operation.stage);
                        });
                    }
                }
            }
        }
        let reveal_target_identity = tracked_reveal_key
            .as_deref()
            .unwrap_or("virtualized-list:no-tracked-row");
        let reveal_target = runtime.update(cx, |runtime, _| {
            runtime
                .scroll_surface
                .reveal_target_for(reveal_target_identity, window)
        });
        if let Some(request) = ready_bring_into_view {
            let sequence = request.sequence;
            let transitioned = runtime.update(cx, |runtime, _| {
                runtime.transition_bring_into_view(
                    &request,
                    VirtualizedListBringIntoViewStage::Ready,
                    VirtualizedListBringIntoViewStage::Queued,
                )
            });
            if transitioned {
                let runtime_for_frame = runtime.clone();
                window.on_next_frame(move |window, cx| {
                    let guard = runtime_for_frame.update(cx, |runtime, cx| {
                        runtime.take_window_bring_into_view_guard(sequence, window, cx)
                    });
                    let Some(guard) = guard else {
                        return;
                    };

                    let runtime_for_completion = runtime_for_frame.clone();
                    match window.try_bring_into_view_with_guard_and_completion(
                        guard,
                        cx,
                        move |outcome, window, cx| {
                            runtime_for_completion.update(cx, |runtime, cx| {
                                runtime.finish_bring_into_view(sequence, outcome, window, cx);
                            });
                        },
                    ) {
                        Ok(Some((request_id, subscription))) => {
                            runtime_for_frame.update(cx, |runtime, _| {
                                runtime.record_submitted_bring_into_view_authority_generation(
                                    sequence,
                                    request_id.chain_generation(),
                                );
                            });
                            subscription.detach();
                        }
                        Ok(None) | Err(_) => {
                            runtime_for_frame.update(cx, |runtime, cx| {
                                runtime.cancel_bring_into_view(
                                    &request,
                                    VirtualizedListBringIntoViewStage::InFlight,
                                );
                                cx.notify();
                            });
                        }
                    }
                });
            }
        }
        let on_activate = self.on_activate.clone();
        let on_selection_change = self.on_selection_change.clone();
        let list_state = plan.state().clone();
        let rows = plan.rows().to_vec();
        let sticky_overlay = plan.sticky_overlay().cloned();
        let row_measure_mode = plan.row_measure_mode();
        let estimated_row_height = plan.metrics().row_height();
        let row_renderer = self.row_renderer.clone();
        let list_id = plan.list_id().to_owned();
        let scroll_viewport_id = format!("virtualized-list:{}:viewport", plan.list_id());
        let root_click_state = list_state.clone();

        let active_indicator_frame = runtime.update(cx, |runtime, _| {
            if runtime.active_key.as_deref() != list_state.active_key() {
                runtime.active_key = list_state.active_key().map(str::to_owned);
                if runtime.last_builder_bring_into_view.is_none()
                    && let Some(active_key) = list_state.active_key()
                {
                    runtime.queue_active_bring_into_view(active_key);
                }
            }
            if &runtime.selected_keys != list_state.selected_key_set() {
                runtime.selected_keys = list_state.selected_key_set().clone();
            }
            let anchor_is_valid = runtime
                .selection_anchor_key
                .as_deref()
                .is_some_and(|key| list_state.selectable_index_for_key(key).is_some());
            if !anchor_is_valid {
                runtime.selection_anchor_key = list_state.active_key().map(str::to_owned);
            }
            let active_indicator_demand =
                runtime
                    .active_indicator
                    .sync(&plan, now, active_indicator_model);
            if let Some(reset_reason) = active_indicator_demand.reset_reason() {
                runtime.active_indicator_frame_host.reset(reset_reason);
            }
            runtime
                .active_indicator_frame_host
                .observe(active_indicator_demand.frame_demand())
        });
        if active_indicator_frame.should_request_frame() {
            window.request_animation_frame();
        }
        let active_indicator = runtime.read(cx).active_indicator.snapshot();
        let root_label = plan.label().to_owned();
        let root_actions: &[AccessibleAction] = if list_state.visible_empty() {
            &[]
        } else {
            &[AccessibleAction::Focus]
        };
        let root_semantics = SemanticDescriptor::new(plan.role())
            .with_label(&root_label)
            .with_disabled(list_state.disabled())
            .with_actions(root_actions);
        let list_body = render_virtualized_list_body(
            &list_id,
            &rows,
            plan.virtualizer().total_size(),
            active_indicator,
            colors,
            row_measure_mode,
            estimated_row_height,
            row_renderer,
            list_state.clone(),
            runtime.clone(),
            focus_handle.clone(),
            reveal_target,
            scroll_chain_anchor,
            scroll_handle.clone(),
            tracked_reveal_key,
            deferred_materialization,
            on_activate.clone(),
            on_selection_change.clone(),
            window,
            cx,
        );

        div()
            .id(self.id)
            .debug_selector({
                let debug_id = debug_id.clone();
                move || format!("virtualized-list:{debug_id}:root")
            })
            .size_full()
            .min_w(px(0.0))
            .min_h(px(0.0))
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded(px(6.0))
            .border_1()
            .border_color(theme.resolve(colors.border()))
            .bg(theme.resolve(colors.surface()))
            .text_size(gpui_px_from_ui(self.size.control_text_px()))
            .text_color(theme.resolve(colors.foreground()))
            .focusable()
            .tab_group()
            .tab_stop(!list_state.disabled() && !list_state.visible_empty())
            .track_focus(&focus_handle)
            .focus_visible(move |style| style.shadow(focus_shadow.clone()))
            .ui_semantics(&root_semantics)
            .on_click({
                let focus_handle = focus_handle.clone();
                move |_, window, cx| {
                    if !root_click_state.disabled() && !root_click_state.visible_empty() {
                        focus_handle.focus(window, cx);
                    }
                }
            })
            .on_scroll_wheel(|_, _, _| open_gpui::ScrollWheelIntent::handled().stop_propagation())
            .on_key_down({
                let runtime = runtime.clone();
                let on_activate = on_activate.clone();
                let on_selection_change = on_selection_change.clone();
                let plan_state = list_state.clone();
                move |event: &KeyDownEvent, window, cx| {
                    handle_virtualized_list_key_down(
                        &plan_state,
                        runtime.clone(),
                        on_activate.clone(),
                        on_selection_change.clone(),
                        event,
                        window,
                        cx,
                    );
                }
            })
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h(px(0.0))
                    .overflow_hidden()
                    .child(
                        ScrollArea::new(scroll_viewport_id, list_body)
                            .vertical()
                            .scroll_handle(&scroll_handle)
                            .with_size(self.size),
                    )
                    .when_some(sticky_overlay, |this, overlay| {
                        this.child(render_virtualized_list_sticky_overlay(
                            list_id,
                            overlay,
                            colors,
                            estimated_row_height,
                            window,
                            cx,
                        ))
                    }),
            )
    }
}

fn handle_virtualized_list_key_down(
    state: &VirtualizedListState,
    runtime: Entity<VirtualizedListRuntime>,
    on_activate: Option<VirtualizedListActivationHandler>,
    on_selection_change: Option<VirtualizedListSelectionChangeHandler>,
    event: &KeyDownEvent,
    window: &mut Window,
    cx: &mut App,
) {
    if state.disabled() || state.visible_empty() || window.default_prevented() {
        return;
    }

    let key = event.keystroke.key.as_str();
    let character_input = event.prefer_character_input;
    let unmodified = !event.keystroke.modifiers.modified() && !character_input;
    let shift_only = virtualized_list_shift_only(event);
    let (current_key, current_selected_keys, runtime_anchor_key) = {
        let runtime = runtime.read(cx);
        (
            runtime.active_key.clone(),
            runtime.selected_keys.clone(),
            runtime.selection_anchor_key.clone(),
        )
    };

    if !character_input
        && (unmodified || shift_only)
        && let Some(target) = state.navigation_target_from_key(key, current_key.as_deref())
    {
        let Some(target) = state.target_at_index(target) else {
            return;
        };
        cx.stop_propagation();
        window.prevent_default();
        let anchor_key = shift_only
            .then(|| {
                state
                    .range_anchor_key(
                        runtime_anchor_key.as_deref().or(current_key.as_deref()),
                        target.key(),
                    )
                    .map(str::to_owned)
            })
            .flatten();
        let selection_change = if shift_only {
            state.range_selection_change_from_selected(
                anchor_key.as_deref(),
                target.key(),
                &current_selected_keys,
            )
        } else {
            None
        };
        runtime.update(cx, |runtime, cx| {
            runtime.active_key = Some(target.key().to_owned());
            runtime.selection_anchor_key = if shift_only {
                anchor_key.clone().or_else(|| Some(target.key().to_owned()))
            } else {
                Some(target.key().to_owned())
            };
            if let Some(selection_change) = selection_change.as_ref() {
                runtime.selected_keys = selection_change.selected_key_set();
            }
            runtime.queue_active_bring_into_view_from_input(target.key(), window);
            cx.notify();
        });
        if let (Some(on_selection_change), Some(selection_change)) =
            (on_selection_change.as_ref(), selection_change)
        {
            on_selection_change(selection_change, window, cx);
        }
        return;
    }

    if !character_input
        && shift_only
        && key == "space"
        && state.selection_mode() == VirtualizedListSelectionMode::Multiple
    {
        let Some(active_key) = current_key.as_deref() else {
            return;
        };
        cx.stop_propagation();
        window.prevent_default();
        let anchor_key = state
            .range_anchor_key(
                runtime_anchor_key.as_deref().or(Some(active_key)),
                active_key,
            )
            .map(str::to_owned);
        let selection_change = state.range_selection_change_from_selected(
            anchor_key.as_deref(),
            active_key,
            &current_selected_keys,
        );
        runtime.update(cx, |runtime, cx| {
            runtime.selection_anchor_key =
                anchor_key.clone().or_else(|| Some(active_key.to_owned()));
            if let Some(selection_change) = selection_change.as_ref() {
                runtime.selected_keys = selection_change.selected_key_set();
            }
            cx.notify();
        });
        if let (Some(on_selection_change), Some(selection_change)) =
            (on_selection_change.as_ref(), selection_change)
        {
            on_selection_change(selection_change, window, cx);
        }
        return;
    }

    if unmodified
        && let Some(activation) =
            state.activation_for_key_from_state(key, current_key.as_deref(), &current_selected_keys)
    {
        cx.stop_propagation();
        window.prevent_default();
        let selection_change = if state.selection_mode() == VirtualizedListSelectionMode::Single {
            state
                .target_at_index(activation.index())
                .and_then(|target| {
                    state.selection_change_for_target_from_selected(&target, &current_selected_keys)
                })
        } else {
            None
        };
        runtime.update(cx, |runtime, cx| {
            runtime.active_key = Some(activation.key().to_owned());
            runtime.selection_anchor_key = Some(activation.key().to_owned());
            if let Some(selection_change) = selection_change.as_ref() {
                runtime.selected_keys = selection_change.selected_key_set();
            }
            runtime.queue_active_bring_into_view_from_input(activation.key(), window);
            cx.notify();
        });
        if let (Some(on_selection_change), Some(selection_change)) =
            (on_selection_change.as_ref(), selection_change)
        {
            on_selection_change(selection_change, window, cx);
        }
        if let Some(on_activate) = on_activate.as_ref() {
            on_activate(activation, window, cx);
        }
        return;
    }

    if unmodified
        && let Some(selection_change) = state.selection_change_for_key_from_state(
            key,
            current_key.as_deref(),
            &current_selected_keys,
        )
    {
        cx.stop_propagation();
        window.prevent_default();
        runtime.update(cx, |runtime, cx| {
            runtime.selected_keys = selection_change.selected_key_set();
            runtime.selection_anchor_key = Some(selection_change.changed_key().to_owned());
            cx.notify();
        });
        if let Some(on_selection_change) = on_selection_change.as_ref() {
            on_selection_change(selection_change, window, cx);
        }
        return;
    }

    let now = cx.background_executor().now();
    let update = runtime.update(cx, |runtime, _| {
        runtime
            .typeahead
            .push(CollectionTypeaheadInput::from_key_down(event), now)
    });
    let Some(update) = update else {
        return;
    };

    cx.stop_propagation();
    window.prevent_default();
    if let Some(target) = state.typeahead_target_from_key(
        update.match_query(),
        current_key.as_deref(),
        update.searches_after_current(),
    ) {
        let target_key = target.key().to_owned();
        runtime.update(cx, |runtime, cx| {
            runtime.active_key = Some(target_key.clone());
            runtime.selection_anchor_key = Some(target_key.clone());
            runtime.queue_active_bring_into_view_from_input(target_key.clone(), window);
            cx.notify();
        });
    }
}

fn virtualized_list_shift_only(event: &KeyDownEvent) -> bool {
    let modifiers = event.keystroke.modifiers;
    modifiers.shift
        && !modifiers.control
        && !modifiers.alt
        && !modifiers.platform
        && !modifiers.function
}

fn resolve_virtualized_list_materialization_target(
    state: &VirtualizedListState,
    key: &str,
    row_measure_mode: VirtualizedListRowMeasureMode,
    virtualizer_snapshot: &VirtualizerSnapshot,
) -> VirtualizedListMaterializationResult {
    match row_measure_mode.measured().then_some(virtualizer_snapshot) {
        Some(snapshot) => state.materialization_target_for_key_with_snapshot(key, snapshot),
        None => state.materialization_target_for_key(key),
    }
}

pub(super) fn materialize_virtualized_list_target(
    scroll_handle: &ScrollHandle,
    state: &VirtualizedListState,
    target: &VirtualizedListMaterializationTarget,
    row_measure_mode: VirtualizedListRowMeasureMode,
    virtualizer_snapshot: &VirtualizerSnapshot,
) {
    let viewport_extent = state.viewport_extent();
    let current_scroll_offset = vertical_scroll_offset(scroll_handle);
    let snapshot = row_measure_mode.measured().then_some(virtualizer_snapshot);
    let materialization_offset = state.materialization_scroll_offset(
        target,
        viewport_extent,
        current_scroll_offset,
        snapshot,
    );

    if materialization_offset != current_scroll_offset {
        set_vertical_scroll_offset_with_source(
            scroll_handle,
            materialization_offset,
            ScrollViewportChangeSource::Programmatic(ScrollViewportProgrammaticSource::Offset),
        );
    }
}

fn resolve_viewport_item_count(row_height: UiPx, viewport_extent: UiPx, fallback: usize) -> usize {
    let row_height = nonnegative_px(row_height);
    let viewport_extent = nonnegative_px(viewport_extent);
    if viewport_extent.as_f32() > 0.0 && row_height.as_f32() > 0.0 {
        (viewport_extent.as_f32() / row_height.as_f32())
            .ceil()
            .max(1.0) as usize
    } else {
        fallback.max(1)
    }
}
