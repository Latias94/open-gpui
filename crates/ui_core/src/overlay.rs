//! Overlay geometry helpers for the Open GPUI component ecosystem.

use crate::geometry::{UiEdges, UiPoint, UiPx, UiRect, UiSize, ui_px, ui_size};

/// A renderer-neutral overlay rectangle.
pub type Rect = UiRect;

/// A renderer-neutral overlay size.
pub type OverlaySize = UiSize;

/// Stable renderer-neutral identity for an overlay layer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OverlayLayerId(String);

impl OverlayLayerId {
    /// Creates an overlay layer identity from a stable string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the stable string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable renderer-neutral identity for an overlay focus target.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OverlayFocusTarget(String);

impl OverlayFocusTarget {
    /// Creates a focus target identity from a stable string.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the stable string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// High-level behavior family for an overlay layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayLayerKind {
    /// Descriptive, non-interactive content associated with a trigger.
    Tooltip,
    /// Non-modal dismissible content such as a popover.
    NonModalDismissible,
    /// Modal content that keeps the underlay inert while present.
    Modal,
    /// Menu-like content that dismisses and consumes outside presses.
    Menu,
}

impl OverlayLayerKind {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Tooltip => "tooltip",
            Self::NonModalDismissible => "non-modal dismissible",
            Self::Modal => "modal",
            Self::Menu => "menu",
        }
    }

    /// Returns the default outside-press policy for this overlay kind.
    pub const fn default_outside_press_policy(self) -> OutsidePressPolicy {
        match self {
            Self::Tooltip => OutsidePressPolicy::Ignore,
            Self::NonModalDismissible => OutsidePressPolicy::DismissAndPassThrough,
            Self::Modal => OutsidePressPolicy::Consume,
            Self::Menu => OutsidePressPolicy::DismissAndConsume,
        }
    }

    /// Returns the default Escape-key policy for this overlay kind.
    pub const fn default_escape_key_policy(self) -> EscapeKeyPolicy {
        match self {
            Self::Tooltip => EscapeKeyPolicy::Ignore,
            Self::NonModalDismissible | Self::Modal | Self::Menu => EscapeKeyPolicy::Dismiss,
        }
    }

    /// Returns the default focus-restore intent for this overlay kind.
    pub const fn default_focus_restore_intent(self) -> FocusRestoreIntent {
        match self {
            Self::Tooltip => FocusRestoreIntent::None,
            Self::NonModalDismissible | Self::Modal | Self::Menu => FocusRestoreIntent::Trigger,
        }
    }

    /// Returns the default initial-focus intent for this overlay kind.
    pub const fn default_initial_focus_intent(self) -> InitialFocusIntent {
        match self {
            Self::Tooltip | Self::NonModalDismissible => InitialFocusIntent::None,
            Self::Modal | Self::Menu => InitialFocusIntent::FirstFocusable,
        }
    }
}

/// Mount, paint, and interaction state for an overlay layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayPresence {
    open: bool,
    present: bool,
    interactive: bool,
}

impl OverlayPresence {
    /// Returns a hidden overlay state.
    pub const fn hidden() -> Self {
        Self {
            open: false,
            present: false,
            interactive: false,
        }
    }

    /// Returns an open overlay state that is mounted and interactive.
    pub const fn open() -> Self {
        Self {
            open: true,
            present: true,
            interactive: true,
        }
    }

    /// Returns a closing overlay state that remains painted but no longer interactive.
    pub const fn closing() -> Self {
        Self {
            open: false,
            present: true,
            interactive: false,
        }
    }

    /// Returns an instant open/closed presence state.
    pub const fn from_open(open: bool) -> Self {
        if open { Self::open() } else { Self::hidden() }
    }

    /// Returns a custom presence state for transition-aware adapters.
    pub const fn from_parts(open: bool, present: bool, interactive: bool) -> Self {
        Self {
            open,
            present,
            interactive,
        }
    }

    /// Returns whether the overlay is semantically open.
    pub const fn is_open(self) -> bool {
        self.open
    }

    /// Returns whether the overlay should remain mounted or painted.
    pub const fn present(self) -> bool {
        self.present
    }

    /// Returns whether the overlay should accept interaction and dismissal events.
    pub const fn interactive(self) -> bool {
        self.interactive
    }
}

/// Reason an overlay asks to close.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DismissReason {
    /// The Escape key was pressed.
    EscapeKey,
    /// Pointer input occurred outside the overlay layer.
    OutsidePress,
    /// The trigger requested closure.
    Trigger,
    /// A close action inside the overlay requested closure.
    CloseAction,
    /// A menu or command selection requested closure.
    Selection,
    /// Application state requested closure directly.
    Programmatic,
}

/// Policy for pointer input outside an overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutsidePressPolicy {
    /// Ignore the outside press and allow normal underlay dispatch.
    Ignore,
    /// Consume the outside press without dismissing.
    Consume,
    /// Dismiss the overlay and consume the outside press.
    DismissAndConsume,
    /// Dismiss the overlay while allowing underlay dispatch.
    DismissAndPassThrough,
}

impl OutsidePressPolicy {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ignore => "ignore",
            Self::Consume => "consume",
            Self::DismissAndConsume => "dismiss + consume",
            Self::DismissAndPassThrough => "dismiss + pass-through",
        }
    }

    /// Resolves the observable outside-press outcome.
    pub const fn resolve(self) -> OutsidePressOutcome {
        match self {
            Self::Ignore => OutsidePressOutcome {
                dismiss: false,
                consume_event: false,
                allow_underlay: true,
                reason: None,
            },
            Self::Consume => OutsidePressOutcome {
                dismiss: false,
                consume_event: true,
                allow_underlay: false,
                reason: None,
            },
            Self::DismissAndConsume => OutsidePressOutcome {
                dismiss: true,
                consume_event: true,
                allow_underlay: false,
                reason: Some(DismissReason::OutsidePress),
            },
            Self::DismissAndPassThrough => OutsidePressOutcome {
                dismiss: true,
                consume_event: false,
                allow_underlay: true,
                reason: Some(DismissReason::OutsidePress),
            },
        }
    }
}

/// Resolved outside-press behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutsidePressOutcome {
    dismiss: bool,
    consume_event: bool,
    allow_underlay: bool,
    reason: Option<DismissReason>,
}

impl OutsidePressOutcome {
    /// Returns whether the outside press should dismiss the overlay.
    pub const fn dismisses(self) -> bool {
        self.dismiss
    }

    /// Returns whether the outside press should be consumed.
    pub const fn consumes_event(self) -> bool {
        self.consume_event
    }

    /// Returns whether the outside press may continue to underlay controls.
    pub const fn allows_underlay_dispatch(self) -> bool {
        self.allow_underlay
    }

    /// Returns the dismiss reason when this outcome requests closure.
    pub const fn dismiss_reason(self) -> Option<DismissReason> {
        self.reason
    }
}

/// Policy for Escape-key handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeKeyPolicy {
    /// Ignore Escape for this overlay layer.
    Ignore,
    /// Dismiss this overlay layer on Escape.
    Dismiss,
}

impl EscapeKeyPolicy {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ignore => "ignore",
            Self::Dismiss => "dismiss",
        }
    }
}

/// Focus restoration behavior after an overlay closes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusRestoreIntent {
    /// Do not restore focus.
    None,
    /// Restore focus to the trigger when it is still available.
    Trigger,
    /// Restore focus to a fallback target.
    Fallback(OverlayFocusTarget),
    /// Prefer the trigger and fall back to a named target.
    TriggerOrFallback(OverlayFocusTarget),
}

impl FocusRestoreIntent {
    /// Returns a stable label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Trigger => "trigger",
            Self::Fallback(_) => "fallback",
            Self::TriggerOrFallback(_) => "trigger or fallback",
        }
    }

    /// Resolves the preferred focus target from the current live trigger identity.
    pub fn resolve_target(
        &self,
        trigger: Option<&OverlayFocusTarget>,
    ) -> Option<OverlayFocusTarget> {
        match self {
            Self::None => None,
            Self::Trigger => trigger.cloned(),
            Self::Fallback(target) => Some(target.clone()),
            Self::TriggerOrFallback(target) => trigger.cloned().or_else(|| Some(target.clone())),
        }
    }
}

/// Initial focus behavior when an overlay opens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InitialFocusIntent {
    /// Do not move focus automatically.
    None,
    /// Focus the first focusable descendant.
    FirstFocusable,
    /// Focus a specific target.
    Target(OverlayFocusTarget),
    /// Prefer a specific target and fall back to the first focusable descendant.
    TargetOrFirstFocusable(OverlayFocusTarget),
}

impl InitialFocusIntent {
    /// Returns a stable label.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::None => "none",
            Self::FirstFocusable => "first focusable",
            Self::Target(_) => "target",
            Self::TargetOrFirstFocusable(_) => "target or first focusable",
        }
    }
}

/// Renderer-neutral behavior policy for one overlay layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayLayerPolicy {
    kind: OverlayLayerKind,
    presence: OverlayPresence,
    outside_press: OutsidePressPolicy,
    escape_key: EscapeKeyPolicy,
    focus_restore: FocusRestoreIntent,
    initial_focus: InitialFocusIntent,
}

impl OverlayLayerPolicy {
    /// Creates a policy with kind-specific defaults.
    pub fn new(kind: OverlayLayerKind, presence: OverlayPresence) -> Self {
        Self {
            kind,
            presence,
            outside_press: kind.default_outside_press_policy(),
            escape_key: kind.default_escape_key_policy(),
            focus_restore: kind.default_focus_restore_intent(),
            initial_focus: kind.default_initial_focus_intent(),
        }
    }

    /// Returns the layer kind.
    pub const fn kind(&self) -> OverlayLayerKind {
        self.kind
    }

    /// Returns the layer presence state.
    pub const fn presence(&self) -> OverlayPresence {
        self.presence
    }

    /// Returns the outside-press policy.
    pub const fn outside_press_policy(&self) -> OutsidePressPolicy {
        self.outside_press
    }

    /// Returns the Escape-key policy.
    pub const fn escape_key_policy(&self) -> EscapeKeyPolicy {
        self.escape_key
    }

    /// Returns the focus-restore intent.
    pub const fn focus_restore_intent(&self) -> &FocusRestoreIntent {
        &self.focus_restore
    }

    /// Returns the initial-focus intent.
    pub const fn initial_focus_intent(&self) -> &InitialFocusIntent {
        &self.initial_focus
    }

    /// Applies a custom presence state.
    pub fn with_presence(mut self, presence: OverlayPresence) -> Self {
        self.presence = presence;
        self
    }

    /// Applies a custom outside-press policy.
    pub fn with_outside_press_policy(mut self, policy: OutsidePressPolicy) -> Self {
        self.outside_press = policy;
        self
    }

    /// Applies a custom Escape-key policy.
    pub fn with_escape_key_policy(mut self, policy: EscapeKeyPolicy) -> Self {
        self.escape_key = policy;
        self
    }

    /// Applies a custom focus-restore intent.
    pub fn with_focus_restore_intent(mut self, intent: FocusRestoreIntent) -> Self {
        self.focus_restore = intent;
        self
    }

    /// Applies a custom initial-focus intent.
    pub fn with_initial_focus_intent(mut self, intent: InitialFocusIntent) -> Self {
        self.initial_focus = intent;
        self
    }

    /// Resolves layer rendering and input behavior from the policy.
    pub const fn layer_state(&self) -> OverlayLayerState {
        OverlayLayerState::from_policy(self)
    }
}

/// Renderer-neutral resolved state for one overlay layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayResolvedState {
    policy: OverlayLayerPolicy,
    layer_state: OverlayLayerState,
}

impl OverlayResolvedState {
    /// Resolves neutral layer state from an overlay policy.
    pub fn resolve(policy: OverlayLayerPolicy) -> Self {
        let layer_state = policy.layer_state();

        Self {
            policy,
            layer_state,
        }
    }

    /// Returns the shared overlay policy.
    pub const fn policy(&self) -> &OverlayLayerPolicy {
        &self.policy
    }

    /// Returns resolved layer state.
    pub const fn layer_state(&self) -> OverlayLayerState {
        self.layer_state
    }

    /// Returns whether an adapter should render the overlay layer.
    pub const fn should_render_deferred_layer(&self) -> bool {
        self.layer_state.visible()
    }

    /// Returns whether an adapter should attach outside-press handling.
    pub const fn wants_outside_press_handler(&self) -> bool {
        self.layer_state.wants_outside_press()
    }
}

/// Resolved layer state that a renderer adapter can map to its own layer system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OverlayLayerState {
    visible: bool,
    hit_testable: bool,
    blocks_underlay_input: bool,
    wants_outside_press: bool,
}

impl OverlayLayerState {
    /// Resolves layer state from renderer-neutral policy.
    pub const fn from_policy(policy: &OverlayLayerPolicy) -> Self {
        let presence = policy.presence();
        let visible = presence.present();
        let hit_testable = match policy.kind() {
            OverlayLayerKind::Tooltip => false,
            OverlayLayerKind::NonModalDismissible | OverlayLayerKind::Menu => {
                presence.interactive()
            }
            OverlayLayerKind::Modal => presence.present(),
        };
        let blocks_underlay_input =
            matches!(policy.kind(), OverlayLayerKind::Modal) && presence.present();
        let wants_outside_press = presence.interactive()
            && !matches!(policy.outside_press_policy(), OutsidePressPolicy::Ignore);

        Self {
            visible,
            hit_testable,
            blocks_underlay_input,
            wants_outside_press,
        }
    }

    /// Returns whether the layer should be visible.
    pub const fn visible(self) -> bool {
        self.visible
    }

    /// Returns whether the layer should be included in hit testing.
    pub const fn hit_testable(self) -> bool {
        self.hit_testable
    }

    /// Returns whether the layer should keep underlay controls inert.
    pub const fn blocks_underlay_input(self) -> bool {
        self.blocks_underlay_input
    }

    /// Returns whether the layer wants outside-press notification.
    pub const fn wants_outside_press(self) -> bool {
        self.wants_outside_press
    }
}

/// One overlay layer entry in top-to-bottom stacking order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayLayer {
    id: OverlayLayerId,
    policy: OverlayLayerPolicy,
    trigger_focus_target: Option<OverlayFocusTarget>,
}

impl OverlayLayer {
    /// Creates a layer entry.
    pub fn new(id: impl Into<String>, policy: OverlayLayerPolicy) -> Self {
        Self {
            id: OverlayLayerId::new(id),
            policy,
            trigger_focus_target: None,
        }
    }

    /// Applies the focus target associated with this layer's trigger.
    pub fn with_trigger_focus_target(mut self, target: OverlayFocusTarget) -> Self {
        self.trigger_focus_target = Some(target);
        self
    }

    /// Returns the layer identity.
    pub const fn id(&self) -> &OverlayLayerId {
        &self.id
    }

    /// Returns the layer policy.
    pub const fn policy(&self) -> &OverlayLayerPolicy {
        &self.policy
    }

    /// Returns the focus target associated with this layer's trigger.
    pub fn trigger_focus_target(&self) -> Option<&OverlayFocusTarget> {
        self.trigger_focus_target.as_ref()
    }
}

/// Resolved Escape-key handling for an overlay stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EscapeKeyResolution {
    /// The topmost interactive layer requested dismissal.
    Dismiss {
        /// Layer that should be dismissed.
        layer_id: OverlayLayerId,
        /// Reason to report to the adapter.
        reason: DismissReason,
    },
    /// The topmost interactive layer ignored Escape, so lower layers should not handle it.
    IgnoredByTopLayer {
        /// Layer that ignored Escape.
        layer_id: OverlayLayerId,
    },
    /// No interactive overlay layer can handle Escape.
    NoInteractiveLayer,
}

/// Resolves Escape-key handling for a bottom-to-top overlay stack.
pub fn resolve_escape_key(layers: &[OverlayLayer]) -> EscapeKeyResolution {
    let Some(layer) = layers
        .iter()
        .rev()
        .find(|layer| layer.policy().presence().interactive())
    else {
        return EscapeKeyResolution::NoInteractiveLayer;
    };

    match layer.policy().escape_key_policy() {
        EscapeKeyPolicy::Dismiss => EscapeKeyResolution::Dismiss {
            layer_id: layer.id().clone(),
            reason: DismissReason::EscapeKey,
        },
        EscapeKeyPolicy::Ignore => EscapeKeyResolution::IgnoredByTopLayer {
            layer_id: layer.id().clone(),
        },
    }
}

/// Resolved outside-press handling for an overlay stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutsidePressResolution {
    /// The topmost outside-press-aware layer handled the press.
    Handled {
        /// Layer that handled the outside press.
        layer_id: OverlayLayerId,
        /// Resolved outside-press outcome for the layer.
        outcome: OutsidePressOutcome,
    },
    /// No visible or interactive layer wants outside-press notification.
    NoOutsidePressLayer,
}

/// Resolves outside-press handling for a bottom-to-top overlay stack.
pub fn resolve_outside_press(layers: &[OverlayLayer]) -> OutsidePressResolution {
    let Some(layer) = layers
        .iter()
        .rev()
        .find(|layer| layer.policy().layer_state().wants_outside_press())
    else {
        return OutsidePressResolution::NoOutsidePressLayer;
    };

    OutsidePressResolution::Handled {
        layer_id: layer.id().clone(),
        outcome: layer.policy().outside_press_policy().resolve(),
    }
}

/// Resolved focus restoration for an overlay stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusRestoreResolution {
    /// The topmost restorable layer resolved a concrete focus target.
    Restore {
        /// Layer that owns the focus restoration request.
        layer_id: OverlayLayerId,
        /// Target that should receive focus.
        target: OverlayFocusTarget,
    },
    /// The topmost restorable layer requested restoration but has no live target.
    NoTarget {
        /// Layer that owns the unresolved focus restoration request.
        layer_id: OverlayLayerId,
    },
    /// No present layer requested focus restoration.
    NoRestorableLayer,
}

/// Resolves focus restoration for the topmost present overlay layer that requested it.
pub fn resolve_focus_restore(layers: &[OverlayLayer]) -> FocusRestoreResolution {
    let Some(layer) = layers.iter().rev().find(|layer| {
        layer.policy().presence().present()
            && focus_restore_requested(layer.policy().focus_restore_intent())
    }) else {
        return FocusRestoreResolution::NoRestorableLayer;
    };

    match layer
        .policy()
        .focus_restore_intent()
        .resolve_target(layer.trigger_focus_target())
    {
        Some(target) => FocusRestoreResolution::Restore {
            layer_id: layer.id().clone(),
            target,
        },
        None => FocusRestoreResolution::NoTarget {
            layer_id: layer.id().clone(),
        },
    }
}

fn focus_restore_requested(intent: &FocusRestoreIntent) -> bool {
    match intent {
        FocusRestoreIntent::None => false,
        FocusRestoreIntent::Trigger
        | FocusRestoreIntent::Fallback(_)
        | FocusRestoreIntent::TriggerOrFallback(_) => true,
    }
}

/// Anchor information used by overlay placement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayAnchorInput {
    point: Option<UiPoint>,
    visual_bounds: Option<Rect>,
    layout_bounds: Option<Rect>,
}

impl OverlayAnchorInput {
    /// Creates an anchor input from a point.
    pub const fn from_point(point: UiPoint) -> Self {
        Self {
            point: Some(point),
            visual_bounds: None,
            layout_bounds: None,
        }
    }

    /// Creates an anchor input from layout bounds.
    pub const fn from_layout_bounds(layout_bounds: Rect) -> Self {
        Self {
            point: None,
            visual_bounds: None,
            layout_bounds: Some(layout_bounds),
        }
    }

    /// Creates an anchor input from visual and layout bounds.
    pub const fn from_visual_and_layout_bounds(
        visual_bounds: Option<Rect>,
        layout_bounds: Option<Rect>,
    ) -> Self {
        Self {
            point: None,
            visual_bounds,
            layout_bounds,
        }
    }

    /// Returns the preferred anchor bounds.
    pub fn preferred_bounds(self) -> Option<Rect> {
        prefer_visual_bounds(self.visual_bounds, self.layout_bounds)
            .or_else(|| self.point.map(anchor_rect_from_point))
    }
}

/// Preferred side for overlay content relative to its anchor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayPlacementSide {
    /// Place content above the anchor.
    Top,
    /// Place content to the right of the anchor.
    Right,
    /// Place content below the anchor.
    Bottom,
    /// Place content to the left of the anchor.
    Left,
}

impl OverlayPlacementSide {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Right => "right",
            Self::Bottom => "bottom",
            Self::Left => "left",
        }
    }
}

/// Preferred alignment for overlay content along the anchor edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayPlacementAlignment {
    /// Align to the start edge.
    Start,
    /// Align to the center.
    Center,
    /// Align to the end edge.
    End,
}

impl OverlayPlacementAlignment {
    /// Returns a stable label.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Center => "center",
            Self::End => "end",
        }
    }
}

/// Renderer-neutral placement input for anchored overlay content.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayPlacementInput {
    anchor: OverlayAnchorInput,
    content_size: OverlaySize,
    side: OverlayPlacementSide,
    alignment: OverlayPlacementAlignment,
    offset: UiPx,
    safe_bounds: Option<Rect>,
}

impl OverlayPlacementInput {
    /// Creates placement input with bottom-start defaults.
    pub fn new(anchor: OverlayAnchorInput, content_size: OverlaySize) -> Self {
        Self {
            anchor,
            content_size,
            side: OverlayPlacementSide::Bottom,
            alignment: OverlayPlacementAlignment::Start,
            offset: ui_px(0.0),
            safe_bounds: None,
        }
    }

    /// Returns the anchor input.
    pub const fn anchor(self) -> OverlayAnchorInput {
        self.anchor
    }

    /// Returns the overlay content size.
    pub const fn content_size(self) -> OverlaySize {
        self.content_size
    }

    /// Returns the preferred placement side.
    pub const fn side(self) -> OverlayPlacementSide {
        self.side
    }

    /// Returns the preferred alignment.
    pub const fn alignment(self) -> OverlayPlacementAlignment {
        self.alignment
    }

    /// Returns the placement offset.
    pub const fn offset(self) -> UiPx {
        self.offset
    }

    /// Returns the safe placement bounds, when provided.
    pub const fn safe_bounds(self) -> Option<Rect> {
        self.safe_bounds
    }

    /// Applies a preferred placement side.
    pub const fn with_side(mut self, side: OverlayPlacementSide) -> Self {
        self.side = side;
        self
    }

    /// Applies a preferred alignment.
    pub const fn with_alignment(mut self, alignment: OverlayPlacementAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Applies a placement offset.
    pub const fn with_offset(mut self, offset: UiPx) -> Self {
        self.offset = offset;
        self
    }

    /// Applies safe placement bounds.
    pub const fn with_safe_bounds(mut self, safe_bounds: Rect) -> Self {
        self.safe_bounds = Some(safe_bounds);
        self
    }

    /// Returns the preferred anchor bounds for this placement input.
    pub fn preferred_anchor_bounds(self) -> Option<Rect> {
        self.anchor.preferred_bounds()
    }
}

/// Returns the preferred bounds when both visual and layout rects are available.
pub fn prefer_visual_bounds(visual: Option<Rect>, layout: Option<Rect>) -> Option<Rect> {
    visual.or(layout)
}

/// Returns a 1x1 rectangle anchor derived from a point.
pub fn anchor_rect_from_point(point: UiPoint) -> Rect {
    rect(point, ui_size(UiPx::ONE, UiPx::ONE))
}

/// Returns a rectangle inset by a uniform window margin.
pub fn outer_bounds_with_window_margin(bounds: Rect, window_margin: UiPx) -> Rect {
    bounds.inset(window_margin)
}

/// Returns a rectangle from the given origin and size.
pub const fn rect(origin: UiPoint, size: OverlaySize) -> Rect {
    UiRect::new(origin, size)
}

/// Returns a rectangle inset by a uniform margin.
pub fn inset_rect(bounds: Rect, margin: UiPx) -> Rect {
    outer_bounds_with_window_margin(bounds, margin)
}

/// Renderer-neutral overlay edge insets.
pub type OverlayEdges = UiEdges;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::ui_point;

    #[test]
    fn prefer_visual_bounds_prefers_visual() {
        let visual = rect(
            ui_point(ui_px(10.0), ui_px(20.0)),
            ui_size(ui_px(100.0), ui_px(60.0)),
        );
        let layout = rect(
            ui_point(ui_px(30.0), ui_px(40.0)),
            ui_size(ui_px(120.0), ui_px(80.0)),
        );

        assert_eq!(
            prefer_visual_bounds(Some(visual), Some(layout)),
            Some(visual)
        );
    }

    #[test]
    fn prefer_visual_bounds_falls_back_to_layout() {
        let layout = rect(
            ui_point(ui_px(30.0), ui_px(40.0)),
            ui_size(ui_px(120.0), ui_px(80.0)),
        );

        assert_eq!(prefer_visual_bounds(None, Some(layout)), Some(layout));
    }

    #[test]
    fn anchor_rect_from_point_creates_one_pixel_anchor() {
        let anchor = anchor_rect_from_point(ui_point(ui_px(12.0), ui_px(34.0)));

        assert_eq!(anchor.origin.x, ui_px(12.0));
        assert_eq!(anchor.origin.y, ui_px(34.0));
        assert_eq!(anchor.size.width, ui_px(1.0));
        assert_eq!(anchor.size.height, ui_px(1.0));
    }

    #[test]
    fn outer_bounds_with_window_margin_insets_uniformly() {
        let input = rect(
            ui_point(ui_px(240.0), ui_px(64.0)),
            ui_size(ui_px(220.0), ui_px(190.0)),
        );

        assert_eq!(
            outer_bounds_with_window_margin(input, ui_px(10.0)),
            rect(
                ui_point(ui_px(250.0), ui_px(74.0)),
                ui_size(ui_px(200.0), ui_px(170.0))
            )
        );
    }

    #[test]
    fn overlay_presence_keeps_open_present_and_interactive_distinct() {
        assert_eq!(OverlayPresence::hidden(), OverlayPresence::from_open(false));
        assert_eq!(OverlayPresence::open(), OverlayPresence::from_open(true));

        let closing = OverlayPresence::closing();

        assert!(!closing.is_open());
        assert!(closing.present());
        assert!(!closing.interactive());
    }

    #[test]
    fn layer_kinds_expose_distinct_default_behavior() {
        let tooltip = OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open());
        let popover = OverlayLayerPolicy::new(
            OverlayLayerKind::NonModalDismissible,
            OverlayPresence::open(),
        );
        let dialog = OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::closing());
        let menu = OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::open());

        assert_eq!(tooltip.outside_press_policy(), OutsidePressPolicy::Ignore);
        assert_eq!(tooltip.escape_key_policy(), EscapeKeyPolicy::Ignore);
        assert_eq!(tooltip.focus_restore_intent(), &FocusRestoreIntent::None);
        assert_eq!(tooltip.initial_focus_intent(), &InitialFocusIntent::None);
        assert_eq!(
            tooltip.layer_state(),
            OverlayLayerState {
                visible: true,
                hit_testable: false,
                blocks_underlay_input: false,
                wants_outside_press: false,
            }
        );

        assert_eq!(
            popover.outside_press_policy(),
            OutsidePressPolicy::DismissAndPassThrough
        );
        assert_eq!(popover.escape_key_policy(), EscapeKeyPolicy::Dismiss);
        assert_eq!(popover.focus_restore_intent(), &FocusRestoreIntent::Trigger);
        assert_eq!(popover.initial_focus_intent(), &InitialFocusIntent::None);

        assert_eq!(dialog.outside_press_policy(), OutsidePressPolicy::Consume);
        assert_eq!(dialog.escape_key_policy(), EscapeKeyPolicy::Dismiss);
        assert_eq!(
            dialog.initial_focus_intent(),
            &InitialFocusIntent::FirstFocusable
        );
        assert_eq!(
            dialog.layer_state(),
            OverlayLayerState {
                visible: true,
                hit_testable: true,
                blocks_underlay_input: true,
                wants_outside_press: false,
            }
        );

        assert_eq!(
            menu.outside_press_policy(),
            OutsidePressPolicy::DismissAndConsume
        );
        assert_eq!(
            menu.initial_focus_intent(),
            &InitialFocusIntent::FirstFocusable
        );
    }

    #[test]
    fn overlay_labels_are_stable() {
        assert_eq!(OverlayLayerKind::Tooltip.as_str(), "tooltip");
        assert_eq!(
            OverlayLayerKind::NonModalDismissible.as_str(),
            "non-modal dismissible"
        );
        assert_eq!(OverlayLayerKind::Modal.as_str(), "modal");
        assert_eq!(OverlayLayerKind::Menu.as_str(), "menu");
        assert_eq!(OutsidePressPolicy::Ignore.as_str(), "ignore");
        assert_eq!(
            OutsidePressPolicy::DismissAndConsume.as_str(),
            "dismiss + consume"
        );
        assert_eq!(EscapeKeyPolicy::Dismiss.as_str(), "dismiss");
        assert_eq!(FocusRestoreIntent::Trigger.as_str(), "trigger");
        assert_eq!(
            FocusRestoreIntent::TriggerOrFallback(OverlayFocusTarget::new("fallback")).as_str(),
            "trigger or fallback"
        );
        assert_eq!(
            InitialFocusIntent::FirstFocusable.as_str(),
            "first focusable"
        );
        assert_eq!(OverlayPlacementSide::Left.as_str(), "left");
        assert_eq!(OverlayPlacementAlignment::Center.as_str(), "center");
    }

    #[test]
    fn outside_press_policy_represents_dismiss_consume_ignore_and_pass_through() {
        let ignored = OutsidePressPolicy::Ignore.resolve();
        assert!(!ignored.dismisses());
        assert!(!ignored.consumes_event());
        assert!(ignored.allows_underlay_dispatch());
        assert_eq!(ignored.dismiss_reason(), None);

        let consumed = OutsidePressPolicy::Consume.resolve();
        assert!(!consumed.dismisses());
        assert!(consumed.consumes_event());
        assert!(!consumed.allows_underlay_dispatch());
        assert_eq!(consumed.dismiss_reason(), None);

        let dismiss_consumed = OutsidePressPolicy::DismissAndConsume.resolve();
        assert!(dismiss_consumed.dismisses());
        assert!(dismiss_consumed.consumes_event());
        assert!(!dismiss_consumed.allows_underlay_dispatch());
        assert_eq!(
            dismiss_consumed.dismiss_reason(),
            Some(DismissReason::OutsidePress)
        );

        let dismiss_passthrough = OutsidePressPolicy::DismissAndPassThrough.resolve();
        assert!(dismiss_passthrough.dismisses());
        assert!(!dismiss_passthrough.consumes_event());
        assert!(dismiss_passthrough.allows_underlay_dispatch());
        assert_eq!(
            dismiss_passthrough.dismiss_reason(),
            Some(DismissReason::OutsidePress)
        );
    }

    #[test]
    fn escape_resolution_uses_topmost_interactive_layer() {
        let lower = OverlayLayer::new(
            "lower-popover",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            ),
        );
        let upper_tooltip = OverlayLayer::new(
            "upper-tooltip",
            OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open()),
        );

        assert_eq!(
            resolve_escape_key(&[lower.clone(), upper_tooltip]),
            EscapeKeyResolution::IgnoredByTopLayer {
                layer_id: OverlayLayerId::new("upper-tooltip")
            }
        );

        let closing_upper = OverlayLayer::new(
            "closing-dialog",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::closing()),
        );

        assert_eq!(
            resolve_escape_key(&[lower, closing_upper]),
            EscapeKeyResolution::Dismiss {
                layer_id: OverlayLayerId::new("lower-popover"),
                reason: DismissReason::EscapeKey,
            }
        );
        assert_eq!(
            resolve_escape_key(&[]),
            EscapeKeyResolution::NoInteractiveLayer
        );
    }

    #[test]
    fn outside_press_resolution_uses_topmost_interactive_dismissible_layer() {
        let lower = OverlayLayer::new(
            "lower-popover",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            )
            .with_outside_press_policy(OutsidePressPolicy::DismissAndPassThrough),
        );
        let closing_menu = OverlayLayer::new(
            "closing-menu",
            OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::closing()),
        );
        let upper_dialog = OverlayLayer::new(
            "upper-dialog",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open())
                .with_outside_press_policy(OutsidePressPolicy::DismissAndConsume),
        );

        assert_eq!(
            resolve_outside_press(&[lower.clone(), closing_menu, upper_dialog]),
            OutsidePressResolution::Handled {
                layer_id: OverlayLayerId::new("upper-dialog"),
                outcome: OutsidePressOutcome {
                    dismiss: true,
                    consume_event: true,
                    allow_underlay: false,
                    reason: Some(DismissReason::OutsidePress),
                },
            }
        );

        assert_eq!(
            resolve_outside_press(&[lower]),
            OutsidePressResolution::Handled {
                layer_id: OverlayLayerId::new("lower-popover"),
                outcome: OutsidePressOutcome {
                    dismiss: true,
                    consume_event: false,
                    allow_underlay: true,
                    reason: Some(DismissReason::OutsidePress),
                },
            }
        );

        let tooltip = OverlayLayer::new(
            "tooltip",
            OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open()),
        );
        assert_eq!(
            resolve_outside_press(&[tooltip]),
            OutsidePressResolution::NoOutsidePressLayer
        );
    }

    #[test]
    fn focus_restore_prefers_trigger_but_can_fallback_or_skip() {
        let trigger = OverlayFocusTarget::new("trigger");
        let fallback = OverlayFocusTarget::new("fallback");

        assert_eq!(
            FocusRestoreIntent::None.resolve_target(Some(&trigger)),
            None
        );
        assert_eq!(
            FocusRestoreIntent::Trigger.resolve_target(Some(&trigger)),
            Some(trigger.clone())
        );
        assert_eq!(FocusRestoreIntent::Trigger.resolve_target(None), None);
        assert_eq!(
            FocusRestoreIntent::Fallback(fallback.clone()).resolve_target(Some(&trigger)),
            Some(fallback.clone())
        );
        assert_eq!(
            FocusRestoreIntent::TriggerOrFallback(fallback.clone()).resolve_target(Some(&trigger)),
            Some(trigger)
        );
        assert_eq!(
            FocusRestoreIntent::TriggerOrFallback(fallback.clone()).resolve_target(None),
            Some(fallback)
        );
    }

    #[test]
    fn focus_restore_resolution_uses_topmost_present_restorable_layer() {
        let lower_trigger = OverlayFocusTarget::new("lower-trigger");
        let upper_trigger = OverlayFocusTarget::new("upper-trigger");
        let fallback = OverlayFocusTarget::new("fallback-target");
        let lower = OverlayLayer::new(
            "lower-popover",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            )
            .with_focus_restore_intent(FocusRestoreIntent::Trigger),
        )
        .with_trigger_focus_target(lower_trigger);
        let upper = OverlayLayer::new(
            "upper-menu",
            OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::closing())
                .with_focus_restore_intent(FocusRestoreIntent::TriggerOrFallback(fallback.clone())),
        )
        .with_trigger_focus_target(upper_trigger.clone());

        assert_eq!(
            resolve_focus_restore(&[lower.clone(), upper]),
            FocusRestoreResolution::Restore {
                layer_id: OverlayLayerId::new("upper-menu"),
                target: upper_trigger,
            }
        );

        let fallback_layer = OverlayLayer::new(
            "fallback-menu",
            OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::closing())
                .with_focus_restore_intent(FocusRestoreIntent::TriggerOrFallback(fallback.clone())),
        );
        assert_eq!(
            resolve_focus_restore(&[lower.clone(), fallback_layer]),
            FocusRestoreResolution::Restore {
                layer_id: OverlayLayerId::new("fallback-menu"),
                target: fallback,
            }
        );

        let missing_trigger = OverlayLayer::new(
            "missing-trigger",
            OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::closing())
                .with_focus_restore_intent(FocusRestoreIntent::Trigger),
        );
        assert_eq!(
            resolve_focus_restore(&[lower, missing_trigger]),
            FocusRestoreResolution::NoTarget {
                layer_id: OverlayLayerId::new("missing-trigger"),
            }
        );
        assert_eq!(
            resolve_focus_restore(&[]),
            FocusRestoreResolution::NoRestorableLayer
        );
    }

    #[test]
    fn placement_input_prefers_visual_layout_then_point_anchor() {
        let visual = rect(
            ui_point(ui_px(10.0), ui_px(20.0)),
            ui_size(ui_px(100.0), ui_px(40.0)),
        );
        let layout = rect(
            ui_point(ui_px(30.0), ui_px(40.0)),
            ui_size(ui_px(120.0), ui_px(60.0)),
        );
        let point_anchor = ui_point(ui_px(7.0), ui_px(9.0));

        let visual_input =
            OverlayAnchorInput::from_visual_and_layout_bounds(Some(visual), Some(layout));
        assert_eq!(visual_input.preferred_bounds(), Some(visual));

        let layout_input = OverlayAnchorInput::from_visual_and_layout_bounds(None, Some(layout));
        assert_eq!(layout_input.preferred_bounds(), Some(layout));

        let point_input = OverlayAnchorInput::from_point(point_anchor);
        assert_eq!(
            point_input.preferred_bounds(),
            Some(anchor_rect_from_point(point_anchor))
        );

        let placement =
            OverlayPlacementInput::new(point_input, ui_size(ui_px(180.0), ui_px(120.0)))
                .with_side(OverlayPlacementSide::Right)
                .with_alignment(OverlayPlacementAlignment::End)
                .with_offset(ui_px(6.0))
                .with_safe_bounds(rect(
                    ui_point(ui_px(0.0), ui_px(0.0)),
                    ui_size(ui_px(300.0), ui_px(220.0)),
                ));

        assert_eq!(placement.side(), OverlayPlacementSide::Right);
        assert_eq!(placement.alignment(), OverlayPlacementAlignment::End);
        assert_eq!(placement.offset(), ui_px(6.0));
        assert!(placement.safe_bounds().is_some());
        assert_eq!(
            placement.preferred_anchor_bounds(),
            Some(anchor_rect_from_point(point_anchor))
        );
    }
}
