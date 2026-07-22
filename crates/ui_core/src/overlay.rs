//! Overlay geometry helpers for the Open GPUI component ecosystem.

use crate::focus::{FocusRestoreIntent, InitialFocusIntent};
use crate::geometry::{UiEdges, UiPoint, UiPx, UiRect, UiSize, ui_point, ui_px, ui_size};

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

    /// Returns whether this overlay kind participates in outside-press arbitration by default.
    pub const fn default_outside_press_participation(self) -> OutsidePressParticipation {
        match self {
            Self::Tooltip => OutsidePressParticipation::Transparent,
            Self::NonModalDismissible | Self::Modal | Self::Menu => {
                OutsidePressParticipation::Participating
            }
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

/// Canonical mount, paint, and interaction state for an overlay layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayPresence {
    /// The layer is absent from paint and interaction.
    Hidden,
    /// The layer is mounted and fully interactive.
    Open,
    /// The layer remains mounted for exit paint but is not interactive.
    Closing,
}

impl OverlayPresence {
    /// Returns a hidden overlay state.
    pub const fn hidden() -> Self {
        Self::Hidden
    }

    /// Returns an open overlay state that is mounted and interactive.
    pub const fn open() -> Self {
        Self::Open
    }

    /// Returns a closing overlay state that remains painted but no longer interactive.
    pub const fn closing() -> Self {
        Self::Closing
    }

    /// Returns an instant open/closed presence state.
    pub const fn from_open(open: bool) -> Self {
        if open { Self::open() } else { Self::hidden() }
    }

    /// Converts canonical lifecycle flags into a presence state.
    pub const fn from_parts(open: bool, present: bool, interactive: bool) -> Option<Self> {
        match (open, present, interactive) {
            (false, false, false) => Some(Self::Hidden),
            (true, true, true) => Some(Self::Open),
            (false, true, false) => Some(Self::Closing),
            _ => None,
        }
    }

    /// Returns whether the overlay is semantically open.
    pub const fn is_open(self) -> bool {
        matches!(self, Self::Open)
    }

    /// Returns whether the overlay should remain mounted or painted.
    pub const fn present(self) -> bool {
        matches!(self, Self::Open | Self::Closing)
    }

    /// Returns whether the overlay should accept interaction and dismissal events.
    pub const fn interactive(self) -> bool {
        matches!(self, Self::Open)
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
    /// The live trigger anchor became absent, hidden, invalid, or otherwise ineligible.
    AnchorUnlinked,
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

/// Whether an interactive overlay participates in outside-press stack arbitration.
///
/// This is independent from [`OutsidePressPolicy`]: a participating layer may explicitly ignore
/// an offered press and thereby stop overlay cascade, while a transparent layer is never offered
/// the press and cannot block a participating layer below it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutsidePressParticipation {
    /// Offer outside presses to this layer when it is interactive.
    Participating,
    /// Skip this layer during outside-press stack arbitration.
    Transparent,
}

impl OutsidePressParticipation {
    /// Returns whether the layer may own outside-press arbitration.
    pub const fn participates(self) -> bool {
        matches!(self, Self::Participating)
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

/// Renderer-neutral behavior policy for one overlay layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayLayerPolicy {
    kind: OverlayLayerKind,
    presence: OverlayPresence,
    outside_press_participation: OutsidePressParticipation,
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
            outside_press_participation: kind.default_outside_press_participation(),
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

    /// Returns whether this layer participates in outside-press stack arbitration.
    pub const fn outside_press_participation(&self) -> OutsidePressParticipation {
        self.outside_press_participation
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

    /// Applies custom outside-press stack participation.
    pub fn with_outside_press_participation(
        mut self,
        participation: OutsidePressParticipation,
    ) -> Self {
        self.outside_press_participation = participation;
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
            && policy.outside_press_participation().participates()
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

/// One overlay layer entry in bottom-to-top stacking order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayLayer {
    id: OverlayLayerId,
    policy: OverlayLayerPolicy,
}

impl OverlayLayer {
    /// Creates a layer entry.
    pub fn new(id: impl Into<String>, policy: OverlayLayerPolicy) -> Self {
        Self {
            id: OverlayLayerId::new(id),
            policy,
        }
    }

    /// Returns the layer identity.
    pub const fn id(&self) -> &OverlayLayerId {
        &self.id
    }

    /// Returns the layer policy.
    pub const fn policy(&self) -> &OverlayLayerPolicy {
        &self.policy
    }

    /// Returns whether this layer is currently eligible for outside-press arbitration.
    pub const fn is_outside_press_eligible(&self) -> bool {
        self.policy.presence().interactive()
            && self.policy.outside_press_participation().participates()
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
    /// The topmost eligible layer resolved the press.
    Handled {
        /// Layer that received the outside press offer.
        layer_id: OverlayLayerId,
        /// Resolved outside-press outcome for the layer.
        outcome: OutsidePressOutcome,
    },
    /// No eligible overlay layer can receive an outside press.
    NoOutsidePressLayer,
}

/// Resolves outside-press handling for a bottom-to-top overlay stack.
pub fn resolve_outside_press(layers: &[OverlayLayer]) -> OutsidePressResolution {
    let Some(layer) = layers
        .iter()
        .rev()
        .find(|layer| layer.is_outside_press_eligible())
    else {
        return OutsidePressResolution::NoOutsidePressLayer;
    };

    OutsidePressResolution::Handled {
        layer_id: layer.id().clone(),
        outcome: layer.policy().outside_press_policy().resolve(),
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

    /// Returns the opposite side used for flip fallback.
    pub const fn opposite(self) -> Self {
        match self {
            Self::Top => Self::Bottom,
            Self::Right => Self::Left,
            Self::Bottom => Self::Top,
            Self::Left => Self::Right,
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

/// Final fit category selected by the renderer-neutral placement solver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayPlacementFit {
    /// Preferred side and alignment fit without shifting.
    Preferred,
    /// Preferred side fit after changing alignment.
    Aligned,
    /// Preferred side fit after shifting within safe bounds.
    Shifted,
    /// Preferred side fit after changing alignment and shifting within safe bounds.
    AlignedAndShifted,
    /// A fallback side fit without shifting.
    Flipped,
    /// A fallback side fit after shifting within safe bounds.
    FlippedAndShifted,
    /// Content was constrained to safe bounds but still cannot fully fit.
    Constrained,
    /// No safe bounds were provided, so preferred placement was used directly.
    Unbounded,
}

impl OverlayPlacementFit {
    /// Returns a stable label for diagnostics and tests.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Preferred => "preferred",
            Self::Aligned => "aligned",
            Self::Shifted => "shifted",
            Self::AlignedAndShifted => "aligned + shifted",
            Self::Flipped => "flipped",
            Self::FlippedAndShifted => "flipped + shifted",
            Self::Constrained => "constrained",
            Self::Unbounded => "unbounded",
        }
    }
}

/// One inspected candidate from overlay placement resolution.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayPlacementTraceStep {
    side: OverlayPlacementSide,
    alignment: OverlayPlacementAlignment,
    raw_bounds: Rect,
    resolved_bounds: Rect,
    main_axis_overflow: UiPx,
    total_overflow: UiPx,
    shifted: bool,
    fits: bool,
}

impl OverlayPlacementTraceStep {
    /// Returns the candidate side.
    pub const fn side(&self) -> OverlayPlacementSide {
        self.side
    }

    /// Returns the candidate alignment.
    pub const fn alignment(&self) -> OverlayPlacementAlignment {
        self.alignment
    }

    /// Returns bounds before safe-bound shifting.
    pub const fn raw_bounds(&self) -> Rect {
        self.raw_bounds
    }

    /// Returns bounds after safe-bound shifting.
    pub const fn resolved_bounds(&self) -> Rect {
        self.resolved_bounds
    }

    /// Returns overflow on the side's main axis before shifting.
    pub const fn main_axis_overflow(&self) -> UiPx {
        self.main_axis_overflow
    }

    /// Returns remaining overflow after shifting.
    pub const fn total_overflow(&self) -> UiPx {
        self.total_overflow
    }

    /// Returns whether this candidate shifted on either axis.
    pub const fn shifted(&self) -> bool {
        self.shifted
    }

    /// Returns whether this candidate fully fits the safe bounds.
    pub const fn fits(&self) -> bool {
        self.fits
    }
}

/// Diagnostic trace emitted by overlay placement resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayPlacementTrace {
    steps: Vec<OverlayPlacementTraceStep>,
    selected_index: usize,
}

impl OverlayPlacementTrace {
    /// Creates a trace from inspected candidates and the selected candidate index.
    pub fn new(steps: Vec<OverlayPlacementTraceStep>, selected_index: usize) -> Self {
        Self {
            steps,
            selected_index,
        }
    }

    /// Returns all inspected candidate steps in priority order.
    pub fn steps(&self) -> &[OverlayPlacementTraceStep] {
        &self.steps
    }

    /// Returns the index of the selected candidate.
    pub const fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Returns the selected trace step.
    pub fn selected(&self) -> &OverlayPlacementTraceStep {
        &self.steps[self.selected_index]
    }
}

/// Renderer-neutral result of overlay placement resolution.
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayPlacementResolution {
    preferred_side: OverlayPlacementSide,
    preferred_alignment: OverlayPlacementAlignment,
    side: OverlayPlacementSide,
    alignment: OverlayPlacementAlignment,
    offset: UiPx,
    anchor_bounds: Rect,
    anchor_point: UiPoint,
    content_bounds: Rect,
    arrow_offset: UiPoint,
    safe_bounds: Option<Rect>,
    fit: OverlayPlacementFit,
    trace: OverlayPlacementTrace,
}

impl OverlayPlacementResolution {
    /// Returns the originally requested side.
    pub const fn preferred_side(&self) -> OverlayPlacementSide {
        self.preferred_side
    }

    /// Returns the originally requested alignment.
    pub const fn preferred_alignment(&self) -> OverlayPlacementAlignment {
        self.preferred_alignment
    }

    /// Returns the selected side after fallback evaluation.
    pub const fn side(&self) -> OverlayPlacementSide {
        self.side
    }

    /// Returns the selected alignment.
    pub const fn alignment(&self) -> OverlayPlacementAlignment {
        self.alignment
    }

    /// Returns the offset applied on the selected side.
    pub const fn offset(&self) -> UiPx {
        self.offset
    }

    /// Returns the anchor bounds used for placement.
    pub const fn anchor_bounds(&self) -> Rect {
        self.anchor_bounds
    }

    /// Returns the anchor point for renderer adapters that position by anchor.
    pub const fn anchor_point(&self) -> UiPoint {
        self.anchor_point
    }

    /// Returns the resolved content bounds.
    pub const fn content_bounds(&self) -> Rect {
        self.content_bounds
    }

    /// Returns the arrow attachment offset inside the content bounds.
    pub const fn arrow_offset(&self) -> UiPoint {
        self.arrow_offset
    }

    /// Returns safe placement bounds, when provided.
    pub const fn safe_bounds(&self) -> Option<Rect> {
        self.safe_bounds
    }

    /// Returns the selected fit category.
    pub const fn fit(&self) -> OverlayPlacementFit {
        self.fit
    }

    /// Returns the diagnostic trace.
    pub const fn trace(&self) -> &OverlayPlacementTrace {
        &self.trace
    }
}

/// Resolves overlay content placement within optional safe bounds.
pub fn resolve_overlay_placement(input: OverlayPlacementInput) -> OverlayPlacementResolution {
    let anchor_bounds = input
        .preferred_anchor_bounds()
        .unwrap_or_else(|| anchor_rect_from_point(ui_point(UiPx::ZERO, UiPx::ZERO)));
    let fallback_candidates = placement_fallback_candidates(input.side(), input.alignment());
    let selected_offset = input.offset();

    let (steps, selected_index) = match input.safe_bounds() {
        Some(safe_bounds) => {
            let mut steps = Vec::with_capacity(fallback_candidates.len());
            let mut selected_index = None;
            let mut best_index = 0;

            for (index, candidate) in fallback_candidates.iter().copied().enumerate() {
                let raw_bounds = content_bounds_for_placement(
                    anchor_bounds,
                    input.content_size(),
                    candidate.side,
                    candidate.alignment,
                    input.offset(),
                );
                let resolved_bounds = shift_rect_into_bounds(raw_bounds, safe_bounds);
                let main_axis_overflow =
                    main_axis_overflow(raw_bounds, safe_bounds, candidate.side);
                let total_overflow = total_rect_overflow(resolved_bounds, safe_bounds);
                let step = OverlayPlacementTraceStep {
                    side: candidate.side,
                    alignment: candidate.alignment,
                    raw_bounds,
                    resolved_bounds,
                    main_axis_overflow,
                    total_overflow,
                    shifted: raw_bounds.origin != resolved_bounds.origin,
                    fits: total_overflow == UiPx::ZERO,
                };

                if step.fits() && !step.shifted() {
                    selected_index = Some(index);
                    steps.push(step);
                    break;
                }

                if steps.is_empty() || placement_step_is_better(step, steps[best_index]) {
                    best_index = index;
                }

                steps.push(step);
            }

            (steps, selected_index.unwrap_or(best_index))
        }
        None => {
            let raw_bounds = content_bounds_for_placement(
                anchor_bounds,
                input.content_size(),
                input.side(),
                input.alignment(),
                input.offset(),
            );
            (
                vec![OverlayPlacementTraceStep {
                    side: input.side(),
                    alignment: input.alignment(),
                    raw_bounds,
                    resolved_bounds: raw_bounds,
                    main_axis_overflow: UiPx::ZERO,
                    total_overflow: UiPx::ZERO,
                    shifted: false,
                    fits: true,
                }],
                0,
            )
        }
    };

    let selected = steps[selected_index];
    let flipped = selected.side() != input.side();
    let realigned = selected.alignment() != input.alignment();
    let fit = match (
        input.safe_bounds().is_some(),
        flipped,
        realigned,
        selected.shifted(),
        selected.fits(),
    ) {
        (false, _, _, _, _) => OverlayPlacementFit::Unbounded,
        (true, false, false, false, true) => OverlayPlacementFit::Preferred,
        (true, false, true, false, true) => OverlayPlacementFit::Aligned,
        (true, false, false, true, true) => OverlayPlacementFit::Shifted,
        (true, false, true, true, true) => OverlayPlacementFit::AlignedAndShifted,
        (true, true, _, false, true) => OverlayPlacementFit::Flipped,
        (true, true, _, true, true) => OverlayPlacementFit::FlippedAndShifted,
        (true, _, _, _, false) => OverlayPlacementFit::Constrained,
    };
    let selected_alignment = selected.alignment();
    let anchor_point = placement_anchor_point(anchor_bounds, selected.side(), selected_alignment);
    let arrow_offset =
        arrow_offset_for_placement(selected.resolved_bounds(), anchor_bounds, selected.side());

    OverlayPlacementResolution {
        preferred_side: input.side(),
        preferred_alignment: input.alignment(),
        side: selected.side(),
        alignment: selected_alignment,
        offset: selected_offset,
        anchor_bounds,
        anchor_point,
        content_bounds: selected.resolved_bounds(),
        arrow_offset,
        safe_bounds: input.safe_bounds(),
        fit,
        trace: OverlayPlacementTrace::new(steps, selected_index),
    }
}

/// Returns the preferred bounds when both visual and layout rects are available.
pub fn prefer_visual_bounds(visual: Option<Rect>, layout: Option<Rect>) -> Option<Rect> {
    visual.or(layout)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlacementCandidate {
    side: OverlayPlacementSide,
    alignment: OverlayPlacementAlignment,
}

fn placement_fallback_candidates(
    side: OverlayPlacementSide,
    alignment: OverlayPlacementAlignment,
) -> Vec<PlacementCandidate> {
    let mut candidates = Vec::with_capacity(16);
    let sides = placement_fallback_sides(side);

    push_unique_placement_candidate(&mut candidates, side, alignment);
    for candidate_alignment in placement_fallback_alignments(alignment) {
        push_unique_placement_candidate(&mut candidates, side, candidate_alignment);
    }

    for candidate_side in sides.into_iter().skip(1) {
        push_unique_placement_candidate(&mut candidates, candidate_side, alignment);
        for candidate_alignment in placement_fallback_alignments(alignment) {
            push_unique_placement_candidate(&mut candidates, candidate_side, candidate_alignment);
        }
    }

    candidates
}

fn placement_fallback_sides(side: OverlayPlacementSide) -> [OverlayPlacementSide; 4] {
    match side {
        OverlayPlacementSide::Top => [
            OverlayPlacementSide::Top,
            OverlayPlacementSide::Bottom,
            OverlayPlacementSide::Right,
            OverlayPlacementSide::Left,
        ],
        OverlayPlacementSide::Right => [
            OverlayPlacementSide::Right,
            OverlayPlacementSide::Left,
            OverlayPlacementSide::Bottom,
            OverlayPlacementSide::Top,
        ],
        OverlayPlacementSide::Bottom => [
            OverlayPlacementSide::Bottom,
            OverlayPlacementSide::Top,
            OverlayPlacementSide::Right,
            OverlayPlacementSide::Left,
        ],
        OverlayPlacementSide::Left => [
            OverlayPlacementSide::Left,
            OverlayPlacementSide::Right,
            OverlayPlacementSide::Bottom,
            OverlayPlacementSide::Top,
        ],
    }
}

fn placement_fallback_alignments(
    alignment: OverlayPlacementAlignment,
) -> [OverlayPlacementAlignment; 3] {
    match alignment {
        OverlayPlacementAlignment::Start => [
            OverlayPlacementAlignment::Center,
            OverlayPlacementAlignment::End,
            OverlayPlacementAlignment::Start,
        ],
        OverlayPlacementAlignment::Center => [
            OverlayPlacementAlignment::Start,
            OverlayPlacementAlignment::End,
            OverlayPlacementAlignment::Center,
        ],
        OverlayPlacementAlignment::End => [
            OverlayPlacementAlignment::Center,
            OverlayPlacementAlignment::Start,
            OverlayPlacementAlignment::End,
        ],
    }
}

fn push_unique_placement_candidate(
    candidates: &mut Vec<PlacementCandidate>,
    side: OverlayPlacementSide,
    alignment: OverlayPlacementAlignment,
) {
    let candidate = PlacementCandidate { side, alignment };
    if !candidates.contains(&candidate) {
        candidates.push(candidate);
    }
}

fn content_bounds_for_placement(
    anchor: Rect,
    content_size: OverlaySize,
    side: OverlayPlacementSide,
    alignment: OverlayPlacementAlignment,
    offset: UiPx,
) -> Rect {
    let anchor_left = anchor.origin.x.as_f32();
    let anchor_top = anchor.origin.y.as_f32();
    let anchor_right = rect_right(anchor);
    let anchor_bottom = rect_bottom(anchor);
    let anchor_width = anchor.size.width.as_f32();
    let anchor_height = anchor.size.height.as_f32();
    let content_width = content_size.width.as_f32();
    let content_height = content_size.height.as_f32();
    let offset = offset.as_f32();

    let aligned_x = match alignment {
        OverlayPlacementAlignment::Start => anchor_left,
        OverlayPlacementAlignment::Center => anchor_left + (anchor_width - content_width) / 2.0,
        OverlayPlacementAlignment::End => anchor_right - content_width,
    };
    let aligned_y = match alignment {
        OverlayPlacementAlignment::Start => anchor_top,
        OverlayPlacementAlignment::Center => anchor_top + (anchor_height - content_height) / 2.0,
        OverlayPlacementAlignment::End => anchor_bottom - content_height,
    };

    let (x, y) = match side {
        OverlayPlacementSide::Top => (aligned_x, anchor_top - offset - content_height),
        OverlayPlacementSide::Right => (anchor_right + offset, aligned_y),
        OverlayPlacementSide::Bottom => (aligned_x, anchor_bottom + offset),
        OverlayPlacementSide::Left => (anchor_left - offset - content_width, aligned_y),
    };

    rect(
        ui_point(ui_px(x), ui_px(y)),
        ui_size(ui_px(content_width), ui_px(content_height)),
    )
}

fn shift_rect_into_bounds(bounds: Rect, safe_bounds: Rect) -> Rect {
    let safe_left = safe_bounds.origin.x.as_f32();
    let safe_top = safe_bounds.origin.y.as_f32();
    let safe_width = safe_bounds.size.width.as_f32();
    let safe_height = safe_bounds.size.height.as_f32();
    let content_width = bounds.size.width.as_f32();
    let content_height = bounds.size.height.as_f32();

    let x = if content_width <= safe_width {
        clamp_f32(
            bounds.origin.x.as_f32(),
            safe_left,
            safe_left + safe_width - content_width,
        )
    } else {
        safe_left
    };
    let y = if content_height <= safe_height {
        clamp_f32(
            bounds.origin.y.as_f32(),
            safe_top,
            safe_top + safe_height - content_height,
        )
    } else {
        safe_top
    };

    rect(ui_point(ui_px(x), ui_px(y)), bounds.size)
}

fn main_axis_overflow(bounds: Rect, safe_bounds: Rect, side: OverlayPlacementSide) -> UiPx {
    let overflow = match side {
        OverlayPlacementSide::Top => safe_bounds.origin.y.as_f32() - bounds.origin.y.as_f32(),
        OverlayPlacementSide::Right => rect_right(bounds) - rect_right(safe_bounds),
        OverlayPlacementSide::Bottom => rect_bottom(bounds) - rect_bottom(safe_bounds),
        OverlayPlacementSide::Left => safe_bounds.origin.x.as_f32() - bounds.origin.x.as_f32(),
    };

    ui_px(overflow.max(0.0))
}

fn total_rect_overflow(bounds: Rect, safe_bounds: Rect) -> UiPx {
    let overflow_left = safe_bounds.origin.x.as_f32() - bounds.origin.x.as_f32();
    let overflow_top = safe_bounds.origin.y.as_f32() - bounds.origin.y.as_f32();
    let overflow_right = rect_right(bounds) - rect_right(safe_bounds);
    let overflow_bottom = rect_bottom(bounds) - rect_bottom(safe_bounds);

    ui_px(
        overflow_left.max(0.0)
            + overflow_top.max(0.0)
            + overflow_right.max(0.0)
            + overflow_bottom.max(0.0),
    )
}

fn placement_step_is_better(
    candidate: OverlayPlacementTraceStep,
    current: OverlayPlacementTraceStep,
) -> bool {
    let candidate_fits_main_axis = candidate.main_axis_overflow() == UiPx::ZERO;
    let current_fits_main_axis = current.main_axis_overflow() == UiPx::ZERO;

    match (candidate_fits_main_axis, current_fits_main_axis) {
        (true, false) => true,
        (false, true) => false,
        _ => match (candidate.shifted(), current.shifted()) {
            (false, true) => true,
            (true, false) => false,
            _ => {
                let candidate_total = candidate.total_overflow().as_f32();
                let current_total = current.total_overflow().as_f32();
                candidate_total < current_total
                    || (candidate_total == current_total
                        && candidate.main_axis_overflow().as_f32()
                            < current.main_axis_overflow().as_f32())
            }
        },
    }
}

fn placement_anchor_point(
    bounds: Rect,
    side: OverlayPlacementSide,
    alignment: OverlayPlacementAlignment,
) -> UiPoint {
    match (side, alignment) {
        (OverlayPlacementSide::Top, OverlayPlacementAlignment::Start) => bounds.top_left(),
        (OverlayPlacementSide::Top, OverlayPlacementAlignment::Center) => bounds.top_center(),
        (OverlayPlacementSide::Top, OverlayPlacementAlignment::End) => bounds.top_right(),
        (OverlayPlacementSide::Right, _) => bounds.right_center(),
        (OverlayPlacementSide::Bottom, OverlayPlacementAlignment::Start) => bounds.bottom_left(),
        (OverlayPlacementSide::Bottom, OverlayPlacementAlignment::Center) => bounds.bottom_center(),
        (OverlayPlacementSide::Bottom, OverlayPlacementAlignment::End) => bounds.bottom_right(),
        (OverlayPlacementSide::Left, _) => bounds.left_center(),
    }
}

fn arrow_offset_for_placement(content: Rect, anchor: Rect, side: OverlayPlacementSide) -> UiPoint {
    let anchor_center_x = anchor.origin.x.as_f32() + anchor.size.width.as_f32() / 2.0;
    let anchor_center_y = anchor.origin.y.as_f32() + anchor.size.height.as_f32() / 2.0;
    let content_width = content.size.width.as_f32();
    let content_height = content.size.height.as_f32();
    let arrow_x = clamp_f32(
        anchor_center_x - content.origin.x.as_f32(),
        0.0,
        content_width,
    );
    let arrow_y = clamp_f32(
        anchor_center_y - content.origin.y.as_f32(),
        0.0,
        content_height,
    );

    match side {
        OverlayPlacementSide::Top => ui_point(ui_px(arrow_x), ui_px(content_height)),
        OverlayPlacementSide::Right => ui_point(UiPx::ZERO, ui_px(arrow_y)),
        OverlayPlacementSide::Bottom => ui_point(ui_px(arrow_x), UiPx::ZERO),
        OverlayPlacementSide::Left => ui_point(ui_px(content_width), ui_px(arrow_y)),
    }
}

fn rect_right(bounds: Rect) -> f32 {
    bounds.origin.x.as_f32() + bounds.size.width.as_f32()
}

fn rect_bottom(bounds: Rect) -> f32 {
    bounds.origin.y.as_f32() + bounds.size.height.as_f32()
}

fn clamp_f32(value: f32, min: f32, max: f32) -> f32 {
    value.max(min).min(max)
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
    use crate::focus::FocusTargetId;
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

        assert_eq!(
            OverlayPresence::from_parts(false, false, false),
            Some(OverlayPresence::hidden())
        );
        assert_eq!(
            OverlayPresence::from_parts(true, true, true),
            Some(OverlayPresence::open())
        );
        assert_eq!(
            OverlayPresence::from_parts(false, true, false),
            Some(OverlayPresence::closing())
        );
        for invalid in [
            (true, false, false),
            (true, false, true),
            (true, true, false),
            (false, false, true),
            (false, true, true),
        ] {
            assert_eq!(
                OverlayPresence::from_parts(invalid.0, invalid.1, invalid.2),
                None
            );
        }
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
        assert_eq!(
            tooltip.outside_press_participation(),
            OutsidePressParticipation::Transparent
        );
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
        assert_eq!(
            popover.outside_press_participation(),
            OutsidePressParticipation::Participating
        );
        assert_eq!(popover.escape_key_policy(), EscapeKeyPolicy::Dismiss);
        assert_eq!(popover.focus_restore_intent(), &FocusRestoreIntent::Trigger);
        assert_eq!(popover.initial_focus_intent(), &InitialFocusIntent::None);

        assert_eq!(dialog.outside_press_policy(), OutsidePressPolicy::Consume);
        assert_eq!(
            dialog.outside_press_participation(),
            OutsidePressParticipation::Participating
        );
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
            menu.outside_press_participation(),
            OutsidePressParticipation::Participating
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
            FocusRestoreIntent::TriggerOrFallback(FocusTargetId::new("fallback")).as_str(),
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
    fn outside_press_resolution_uses_topmost_eligible_layer() {
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
            resolve_outside_press(&[lower.clone(), closing_menu.clone(), upper_dialog]),
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
            resolve_outside_press(&[lower, closing_menu]),
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
    fn transparent_tooltip_does_not_block_participating_layer_below() {
        let lower = OverlayLayer::new(
            "lower-popover",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            ),
        );
        let tooltip = OverlayLayer::new(
            "tooltip",
            OverlayLayerPolicy::new(OverlayLayerKind::Tooltip, OverlayPresence::open()),
        );

        assert!(!tooltip.is_outside_press_eligible());
        assert_eq!(
            resolve_outside_press(&[lower, tooltip]),
            OutsidePressResolution::Handled {
                layer_id: OverlayLayerId::new("lower-popover"),
                outcome: OutsidePressPolicy::DismissAndPassThrough.resolve(),
            }
        );
    }

    #[test]
    fn explicitly_transparent_interactive_layer_does_not_block_participating_layer_below() {
        let lower = OverlayLayer::new(
            "lower-menu",
            OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::open()),
        );
        let passive = OverlayLayer::new(
            "passive-surface",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            )
            .with_outside_press_policy(OutsidePressPolicy::DismissAndConsume)
            .with_outside_press_participation(OutsidePressParticipation::Transparent),
        );

        assert!(!passive.is_outside_press_eligible());
        assert!(!passive.policy().layer_state().wants_outside_press());
        assert_eq!(
            resolve_outside_press(&[lower, passive]),
            OutsidePressResolution::Handled {
                layer_id: OverlayLayerId::new("lower-menu"),
                outcome: OutsidePressPolicy::DismissAndConsume.resolve(),
            }
        );
    }

    #[test]
    fn outside_press_resolution_stops_at_top_ignore() {
        let lower = OverlayLayer::new(
            "lower-menu",
            OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::open())
                .with_outside_press_policy(OutsidePressPolicy::DismissAndConsume),
        );
        let top = OverlayLayer::new(
            "top-ignore",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            )
            .with_outside_press_policy(OutsidePressPolicy::Ignore),
        );

        assert!(top.is_outside_press_eligible());
        assert!(!top.policy().layer_state().wants_outside_press());
        assert_eq!(
            resolve_outside_press(&[lower, top]),
            OutsidePressResolution::Handled {
                layer_id: OverlayLayerId::new("top-ignore"),
                outcome: OutsidePressPolicy::Ignore.resolve(),
            }
        );
    }

    #[test]
    fn outside_press_resolution_preserves_top_consume_and_pass_through_outcomes() {
        let lower = OverlayLayer::new(
            "lower-menu",
            OverlayLayerPolicy::new(OverlayLayerKind::Menu, OverlayPresence::open())
                .with_outside_press_policy(OutsidePressPolicy::DismissAndConsume),
        );
        let consuming_modal = OverlayLayer::new(
            "consuming-modal",
            OverlayLayerPolicy::new(OverlayLayerKind::Modal, OverlayPresence::open())
                .with_outside_press_policy(OutsidePressPolicy::Consume),
        );

        assert_eq!(
            resolve_outside_press(&[lower.clone(), consuming_modal]),
            OutsidePressResolution::Handled {
                layer_id: OverlayLayerId::new("consuming-modal"),
                outcome: OutsidePressPolicy::Consume.resolve(),
            }
        );

        let pass_through = OverlayLayer::new(
            "pass-through",
            OverlayLayerPolicy::new(
                OverlayLayerKind::NonModalDismissible,
                OverlayPresence::open(),
            )
            .with_outside_press_policy(OutsidePressPolicy::DismissAndPassThrough),
        );
        assert_eq!(
            resolve_outside_press(&[lower, pass_through]),
            OutsidePressResolution::Handled {
                layer_id: OverlayLayerId::new("pass-through"),
                outcome: OutsidePressPolicy::DismissAndPassThrough.resolve(),
            }
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

    #[test]
    fn overlay_placement_flips_when_preferred_side_overflows_main_axis() {
        let placement = OverlayPlacementInput::new(
            OverlayAnchorInput::from_layout_bounds(rect(
                ui_point(ui_px(40.0), ui_px(170.0)),
                ui_size(ui_px(40.0), ui_px(20.0)),
            )),
            ui_size(ui_px(80.0), ui_px(50.0)),
        )
        .with_side(OverlayPlacementSide::Bottom)
        .with_alignment(OverlayPlacementAlignment::Start)
        .with_offset(ui_px(4.0))
        .with_safe_bounds(rect(
            ui_point(ui_px(0.0), ui_px(0.0)),
            ui_size(ui_px(200.0), ui_px(200.0)),
        ));

        let resolved = resolve_overlay_placement(placement);

        assert_eq!(resolved.preferred_side(), OverlayPlacementSide::Bottom);
        assert_eq!(resolved.side(), OverlayPlacementSide::Top);
        assert_eq!(resolved.alignment(), OverlayPlacementAlignment::Start);
        assert_eq!(resolved.fit(), OverlayPlacementFit::Flipped);
        assert_eq!(
            resolved.content_bounds(),
            rect(
                ui_point(ui_px(40.0), ui_px(116.0)),
                ui_size(ui_px(80.0), ui_px(50.0))
            )
        );
        assert!(
            resolved.trace().steps().len() > 1,
            "trace should show rejected preferred candidates before the flip"
        );
        assert_eq!(
            resolved.trace().selected().side(),
            OverlayPlacementSide::Top
        );
    }

    #[test]
    fn overlay_placement_realigns_before_shifting_when_cross_axis_overflows() {
        let placement = OverlayPlacementInput::new(
            OverlayAnchorInput::from_layout_bounds(rect(
                ui_point(ui_px(170.0), ui_px(40.0)),
                ui_size(ui_px(20.0), ui_px(20.0)),
            )),
            ui_size(ui_px(80.0), ui_px(50.0)),
        )
        .with_side(OverlayPlacementSide::Bottom)
        .with_alignment(OverlayPlacementAlignment::Start)
        .with_offset(ui_px(4.0))
        .with_safe_bounds(rect(
            ui_point(ui_px(0.0), ui_px(0.0)),
            ui_size(ui_px(240.0), ui_px(200.0)),
        ));

        let resolved = resolve_overlay_placement(placement);

        assert_eq!(resolved.side(), OverlayPlacementSide::Bottom);
        assert_eq!(resolved.alignment(), OverlayPlacementAlignment::Center);
        assert_eq!(resolved.fit(), OverlayPlacementFit::Aligned);
        assert_eq!(
            resolved.content_bounds(),
            rect(
                ui_point(ui_px(140.0), ui_px(64.0)),
                ui_size(ui_px(80.0), ui_px(50.0))
            )
        );
        assert!(
            resolved
                .trace()
                .steps()
                .iter()
                .any(|step| step.shifted() && step.side() == OverlayPlacementSide::Bottom),
            "trace should include the shifted preferred candidate before realignment wins"
        );
    }

    #[test]
    fn overlay_placement_constrains_oversized_content_with_trace() {
        let placement = OverlayPlacementInput::new(
            OverlayAnchorInput::from_layout_bounds(rect(
                ui_point(ui_px(12.0), ui_px(12.0)),
                ui_size(ui_px(20.0), ui_px(20.0)),
            )),
            ui_size(ui_px(300.0), ui_px(240.0)),
        )
        .with_side(OverlayPlacementSide::Bottom)
        .with_alignment(OverlayPlacementAlignment::Start)
        .with_safe_bounds(rect(
            ui_point(ui_px(0.0), ui_px(0.0)),
            ui_size(ui_px(200.0), ui_px(160.0)),
        ));

        let resolved = resolve_overlay_placement(placement);

        assert_eq!(resolved.fit(), OverlayPlacementFit::Constrained);
        assert_eq!(
            resolved.content_bounds(),
            rect(
                ui_point(ui_px(0.0), ui_px(0.0)),
                ui_size(ui_px(300.0), ui_px(240.0))
            )
        );
        assert_eq!(resolved.safe_bounds(), placement.safe_bounds());
        assert!(resolved.trace().selected().total_overflow().as_f32() > 0.0);
    }
}
