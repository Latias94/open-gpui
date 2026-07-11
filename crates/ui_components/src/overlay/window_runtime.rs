//! Per-window overlay lifecycle, input, and focus authority.

use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use open_gpui::{
    AnyElement, App, Bounds, Element, ElementId, Entity, EntityId, FocusHandle, GlobalElementId,
    InspectorElementId, IntoElement, KeyDownEvent, LayoutId, MouseButton, MouseDownEvent, Pixels,
    Point, PointerCancelReason, PointerCapture, PointerCaptureHandle, Subscription, Window,
    WindowId, WindowMouseEvent,
};
use open_gpui_ui_core::{
    DismissReason, EscapeKeyPolicy, EscapeKeyResolution, FocusRestoreIntent, FocusScopeId,
    FocusScopeMode, FocusScopePolicy, FocusTargetId, InitialFocusIntent, OutsidePressParticipation,
    OutsidePressPolicy, OutsidePressResolution, OverlayLayer, OverlayLayerId, OverlayLayerKind,
    OverlayLayerPolicy, OverlayPresence, resolve_escape_key, resolve_outside_press,
};

use super::focus_scope::{
    FocusScopeRegistration, FocusScopeRuntime, FocusScopeRuntimeError, FocusTargetRegistration,
};

mod api;
mod focus;
mod input;
mod lifecycle;
mod surface;
#[cfg(test)]
mod surface_tests;

pub use surface::OverlaySurface;

type OpenChangeCallback = Rc<dyn Fn(bool, &mut Window, &mut App)>;

/// Stable identity for one registered inside region.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OverlayInsideRegionId(String);

impl OverlayInsideRegionId {
    /// Creates an inside-region identity.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Returns the stable string value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Open-state ownership for a registered overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayOwnership {
    /// The caller commits open state after receiving an intent callback.
    Controlled,
    /// The runtime commits the owner state before notifying observers.
    Uncontrolled,
}

/// Focus-scope behavior contributed by an overlay layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayFocusMode {
    /// The layer has no focus scope.
    None,
    /// The layer records focus and restoration without trapping Tab.
    Passive,
    /// The layer owns the innermost modal Tab loop while active.
    Modal,
}

/// Condition controlling whether a closing layer restores saved focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayFocusRestoreCondition {
    /// Apply the layer's declared restore intent whenever it closes.
    Always,
    /// Restore only after focus entered the layer surface.
    IfFocusEntered,
    /// Never restore focus for this layer.
    Never,
}

/// Tab behavior that is independent from modal focus trapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OverlayTabBehavior {
    /// Preserve normal Tab dispatch when no modal scope handles it.
    Preserve,
    /// Dismiss this layer when Tab is pressed.
    DismissSelf,
    /// Dismiss the highest contiguous menu ancestor, including this layer when it is the root.
    DismissMenuRoot,
}

/// Committed lifecycle phase for one registered layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayLayerPhase {
    /// Semantically open and fully interactive.
    Open,
    /// A controlled close intent is pending while the owner remains open.
    CloseRequested,
    /// Semantically closed but retained for exit paint or a modal pointer barrier.
    Closing,
    /// Not present in the active window stack.
    Hidden,
}

/// Opaque generation for one layer lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OverlayLayerGeneration(u64);

impl OverlayLayerGeneration {
    /// Returns the opaque numeric value for diagnostics and tests.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Opaque revision for one unresolved controlled open-state intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OverlayOpenIntentRevision(u64);

impl OverlayOpenIntentRevision {
    /// Returns the opaque numeric value for diagnostics and tests.
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Failure returned while configuring or driving a window overlay runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowOverlayRuntimeError {
    /// The runtime or binding belongs to another window.
    WrongWindow,
    /// A layer with the same stable identity is already registered.
    DuplicateLayer(OverlayLayerId),
    /// The declared parent layer is not registered.
    MissingParent(OverlayLayerId),
    /// The declared parent relationship would create a cycle.
    CyclicParent(OverlayLayerId),
    /// A layer lease attempted to change its stable parent identity.
    ParentChanged(OverlayLayerId),
    /// An interactive or present layer has an inactive ancestor.
    InactiveAncestor(OverlayLayerId),
    /// A parent lifecycle transition still has present descendants.
    PresentDescendants(OverlayLayerId),
    /// The requested layer is not registered.
    UnknownLayer(OverlayLayerId),
    /// The binding lease does not own the requested layer registration.
    ForeignLease(OverlayLayerId),
    /// A rebind attempted to change open-state ownership for an existing lease.
    OwnershipChanged(OverlayLayerId),
    /// A controlled-intent operation targeted an uncontrolled layer.
    NotControlled(OverlayLayerId),
    /// A controlled-intent resolution no longer matches the pending request.
    StaleIntent(OverlayLayerId),
    /// An uncontrolled registration omitted its commit callback.
    MissingUncontrolledCommit(OverlayLayerId),
    /// The declared overlay kind and focus-scope behavior are incompatible.
    IncompatibleFocusMode {
        /// Stable layer identity.
        layer: OverlayLayerId,
        /// Renderer-neutral overlay kind.
        kind: OverlayLayerKind,
        /// GPUI focus behavior requested by the registration.
        focus_mode: OverlayFocusMode,
    },
    /// The declared overlay kind and Tab behavior are incompatible.
    IncompatibleTabBehavior {
        /// Stable layer identity.
        layer: OverlayLayerId,
        /// Renderer-neutral overlay kind.
        kind: OverlayLayerKind,
        /// Tab behavior rejected for the layer.
        behavior: OverlayTabBehavior,
    },
    /// A layer cannot be unregistered while child layers remain registered.
    HasChildren(OverlayLayerId),
    /// A layer that is already unregistering cannot accept new owned resources.
    LayerUnregistering(OverlayLayerId),
    /// A rebind attempted to change whether the layer owns a focus scope.
    FocusModeChanged(OverlayLayerId),
    /// The layer does not own a focus scope for additional targets.
    MissingFocusScope(OverlayLayerId),
    /// The focus-target lease does not belong to this layer incarnation.
    ForeignFocusTargetLease(FocusTargetId),
    /// A focus-target rebind attempted to change its stable identity.
    FocusTargetIdChanged {
        /// Stable identity owned by the lease.
        expected: FocusTargetId,
        /// Identity supplied by the new registration.
        actual: FocusTargetId,
    },
    /// This window already owns an application fallback target.
    DuplicateWindowFallback(FocusTargetId),
    /// The fallback lease does not own the current window fallback registration.
    ForeignWindowFallbackLease(FocusTargetId),
    /// An exit completion used a stale lifecycle generation.
    StaleGeneration(OverlayLayerId),
    /// Focus-scope registration or arbitration failed.
    Focus(FocusScopeRuntimeError),
}

impl fmt::Display for WindowOverlayRuntimeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongWindow => formatter.write_str("overlay runtime belongs to another window"),
            Self::DuplicateLayer(layer) => {
                write!(formatter, "duplicate overlay layer `{}`", layer.as_str())
            }
            Self::MissingParent(layer) => write!(
                formatter,
                "overlay parent `{}` is not registered",
                layer.as_str()
            ),
            Self::CyclicParent(layer) => write!(
                formatter,
                "overlay parent chain for `{}` is cyclic",
                layer.as_str()
            ),
            Self::ParentChanged(layer) => write!(
                formatter,
                "overlay layer `{}` changed its stable parent",
                layer.as_str()
            ),
            Self::InactiveAncestor(layer) => write!(
                formatter,
                "overlay ancestor `{}` is not active enough for its child",
                layer.as_str()
            ),
            Self::PresentDescendants(layer) => write!(
                formatter,
                "overlay layer `{}` still has present descendants",
                layer.as_str()
            ),
            Self::UnknownLayer(layer) => {
                write!(formatter, "unknown overlay layer `{}`", layer.as_str())
            }
            Self::ForeignLease(layer) => write!(
                formatter,
                "foreign lease for overlay layer `{}`",
                layer.as_str()
            ),
            Self::OwnershipChanged(layer) => write!(
                formatter,
                "overlay layer `{}` changed open-state ownership",
                layer.as_str()
            ),
            Self::NotControlled(layer) => write!(
                formatter,
                "overlay layer `{}` does not use controlled open state",
                layer.as_str()
            ),
            Self::StaleIntent(layer) => write!(
                formatter,
                "controlled intent for overlay layer `{}` is stale",
                layer.as_str()
            ),
            Self::MissingUncontrolledCommit(layer) => write!(
                formatter,
                "uncontrolled overlay layer `{}` has no commit callback",
                layer.as_str()
            ),
            Self::IncompatibleFocusMode {
                layer,
                kind,
                focus_mode,
            } => write!(
                formatter,
                "overlay layer `{}` cannot combine {} policy with {focus_mode:?} focus",
                layer.as_str(),
                kind.as_str()
            ),
            Self::IncompatibleTabBehavior {
                layer,
                kind,
                behavior,
            } => write!(
                formatter,
                "overlay layer `{}` cannot combine {} policy with {behavior:?} Tab behavior",
                layer.as_str(),
                kind.as_str()
            ),
            Self::HasChildren(layer) => write!(
                formatter,
                "overlay layer `{}` still owns child layers",
                layer.as_str()
            ),
            Self::LayerUnregistering(layer) => write!(
                formatter,
                "overlay layer `{}` is unregistering",
                layer.as_str()
            ),
            Self::FocusModeChanged(layer) => write!(
                formatter,
                "overlay layer `{}` changed focus-scope ownership",
                layer.as_str()
            ),
            Self::MissingFocusScope(layer) => write!(
                formatter,
                "overlay layer `{}` has no focus scope",
                layer.as_str()
            ),
            Self::ForeignFocusTargetLease(target) => write!(
                formatter,
                "foreign lease for overlay focus target `{target}`"
            ),
            Self::FocusTargetIdChanged { expected, actual } => write!(
                formatter,
                "overlay focus target `{expected}` cannot be rebound as `{actual}`"
            ),
            Self::DuplicateWindowFallback(target) => write!(
                formatter,
                "window fallback `{target}` is already registered"
            ),
            Self::ForeignWindowFallbackLease(target) => {
                write!(formatter, "foreign lease for window fallback `{target}`")
            }
            Self::StaleGeneration(layer) => write!(
                formatter,
                "stale exit generation for overlay layer `{}`",
                layer.as_str()
            ),
            Self::Focus(error) => write!(formatter, "overlay focus runtime failed: {error}"),
        }
    }
}

impl std::error::Error for WindowOverlayRuntimeError {}

impl From<FocusScopeRuntimeError> for WindowOverlayRuntimeError {
    fn from(error: FocusScopeRuntimeError) -> Self {
        Self::Focus(error)
    }
}

/// Builder describing one layer registration or rebind.
#[derive(Clone)]
pub struct OverlayLayerRegistration {
    id: OverlayLayerId,
    parent: Option<OverlayLayerId>,
    policy: OverlayLayerPolicy,
    ownership: OverlayOwnership,
    focus_mode: OverlayFocusMode,
    focus_restore_condition: OverlayFocusRestoreCondition,
    tab_behavior: OverlayTabBehavior,
    on_open_change: Option<OpenChangeCallback>,
    uncontrolled_commit: Option<OpenChangeCallback>,
}

impl OverlayLayerRegistration {
    /// Creates a registration from a stable identity, resolved policy, and owner mode.
    pub fn new(
        id: impl Into<String>,
        policy: OverlayLayerPolicy,
        ownership: OverlayOwnership,
    ) -> Self {
        let kind = policy.kind();
        let focus_mode = match kind {
            OverlayLayerKind::Tooltip => OverlayFocusMode::None,
            OverlayLayerKind::NonModalDismissible | OverlayLayerKind::Menu => {
                OverlayFocusMode::Passive
            }
            OverlayLayerKind::Modal => OverlayFocusMode::Modal,
        };
        let focus_restore_condition = match kind {
            OverlayLayerKind::Modal => OverlayFocusRestoreCondition::Always,
            OverlayLayerKind::NonModalDismissible | OverlayLayerKind::Menu => {
                OverlayFocusRestoreCondition::IfFocusEntered
            }
            OverlayLayerKind::Tooltip => OverlayFocusRestoreCondition::Never,
        };
        let tab_behavior = match kind {
            OverlayLayerKind::Menu => OverlayTabBehavior::DismissMenuRoot,
            OverlayLayerKind::Tooltip
            | OverlayLayerKind::NonModalDismissible
            | OverlayLayerKind::Modal => OverlayTabBehavior::Preserve,
        };
        Self {
            id: OverlayLayerId::new(id),
            parent: None,
            policy,
            ownership,
            focus_mode,
            focus_restore_condition,
            tab_behavior,
            on_open_change: None,
            uncontrolled_commit: None,
        }
    }

    /// Associates this layer with a registered parent.
    pub fn parent(mut self, parent: impl Into<String>) -> Self {
        self.parent = Some(OverlayLayerId::new(parent));
        self
    }

    /// Associates this layer with a typed parent identity.
    pub fn parent_id(mut self, parent: OverlayLayerId) -> Self {
        self.parent = Some(parent);
        self
    }

    /// Applies passive or modal focus-scope behavior.
    pub const fn focus_mode(mut self, mode: OverlayFocusMode) -> Self {
        self.focus_mode = mode;
        self
    }

    /// Applies a conditional focus-restore policy.
    pub const fn focus_restore_condition(
        mut self,
        condition: OverlayFocusRestoreCondition,
    ) -> Self {
        self.focus_restore_condition = condition;
        self
    }

    /// Applies non-modal Tab behavior.
    pub fn tab_behavior(mut self, behavior: OverlayTabBehavior) -> Self {
        self.tab_behavior = behavior;
        self
    }

    /// Registers the observable open-change callback.
    pub fn on_open_change(
        mut self,
        callback: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_open_change = Some(Rc::new(callback));
        self
    }

    /// Registers the owner commit used by uncontrolled layers.
    pub fn uncontrolled_commit(
        mut self,
        callback: impl Fn(bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.uncontrolled_commit = Some(Rc::new(callback));
        self
    }

    /// Returns the stable layer identity.
    pub const fn id(&self) -> &OverlayLayerId {
        &self.id
    }

    /// Returns the resolved layer policy.
    pub const fn policy(&self) -> &OverlayLayerPolicy {
        &self.policy
    }
}

/// Stable capability proving ownership of one layer registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayLayerLease {
    layer_id: OverlayLayerId,
    token: u64,
    window_id: WindowId,
}

impl OverlayLayerLease {
    /// Returns the registered layer identity.
    pub const fn layer_id(&self) -> &OverlayLayerId {
        &self.layer_id
    }
}

/// Stable capability proving ownership of one additional focus target.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayFocusTargetLease {
    window_id: WindowId,
    layer_id: OverlayLayerId,
    layer_token: u64,
    scope_id: FocusScopeId,
    target_id: FocusTargetId,
    target_token: u64,
}

impl OverlayFocusTargetLease {
    /// Returns the layer that owns this target.
    pub const fn layer_id(&self) -> &OverlayLayerId {
        &self.layer_id
    }

    /// Returns the canonical target identity.
    pub const fn target_id(&self) -> &FocusTargetId {
        &self.target_id
    }
}

/// Stable capability proving ownership of the window application fallback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowFocusFallbackLease {
    window_id: WindowId,
    target_id: FocusTargetId,
    target_token: u64,
}

impl WindowFocusFallbackLease {
    /// Returns the canonical fallback target identity.
    pub const fn target_id(&self) -> &FocusTargetId {
        &self.target_id
    }
}

/// GPUI binding returned for a registered layer.
#[derive(Clone)]
pub struct OverlayLayerBinding {
    lease: OverlayLayerLease,
    trigger_focus: FocusHandle,
    surface_focus: FocusHandle,
}

impl OverlayLayerBinding {
    /// Returns the stable registration lease.
    pub const fn lease(&self) -> &OverlayLayerLease {
        &self.lease
    }

    /// Returns the runtime-owned trigger focus handle.
    pub const fn trigger_focus(&self) -> &FocusHandle {
        &self.trigger_focus
    }

    /// Returns the runtime-owned surface focus handle.
    pub const fn surface_focus(&self) -> &FocusHandle {
        &self.surface_focus
    }
}

/// Immutable window overlay runtime projection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowOverlaySnapshot {
    window_id: WindowId,
    layers: Vec<OverlayLayerSnapshot>,
}

impl WindowOverlaySnapshot {
    /// Returns the window that owns this projection.
    pub const fn window_id(&self) -> WindowId {
        self.window_id
    }

    /// Returns registered layers in active bottom-to-top order followed by hidden registrations.
    pub fn layers(&self) -> &[OverlayLayerSnapshot] {
        &self.layers
    }
}

/// Immutable projection for one registered overlay layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayLayerSnapshot {
    id: OverlayLayerId,
    parent: Option<OverlayLayerId>,
    kind: OverlayLayerKind,
    phase: OverlayLayerPhase,
    presence: OverlayPresence,
    pending_open: Option<bool>,
    pending_intent: Option<DismissReason>,
    pending_intent_revision: Option<OverlayOpenIntentRevision>,
    keyboard_eligible: bool,
    modal_pointer_barrier: bool,
    focus_active: bool,
    generation: OverlayLayerGeneration,
}

impl OverlayLayerSnapshot {
    /// Returns the stable layer identity.
    pub const fn id(&self) -> &OverlayLayerId {
        &self.id
    }
    /// Returns the registered parent identity.
    pub const fn parent(&self) -> Option<&OverlayLayerId> {
        self.parent.as_ref()
    }
    /// Returns the high-level layer kind.
    pub const fn kind(&self) -> OverlayLayerKind {
        self.kind
    }
    /// Returns the lifecycle phase.
    pub const fn phase(&self) -> OverlayLayerPhase {
        self.phase
    }
    /// Returns the current presence projection.
    pub const fn presence(&self) -> OverlayPresence {
        self.presence
    }
    /// Returns the pending desired open value.
    pub const fn pending_open(&self) -> Option<bool> {
        self.pending_open
    }
    /// Returns the pending intent reason.
    pub const fn pending_intent(&self) -> Option<DismissReason> {
        self.pending_intent
    }
    /// Returns the revision required to resolve the current controlled intent.
    pub const fn pending_intent_revision(&self) -> Option<OverlayOpenIntentRevision> {
        self.pending_intent_revision
    }
    /// Returns whether the layer currently owns keyboard behavior.
    pub const fn keyboard_eligible(&self) -> bool {
        self.keyboard_eligible
    }
    /// Returns whether modal presence currently blocks underlay pointer input.
    pub const fn modal_pointer_barrier(&self) -> bool {
        self.modal_pointer_barrier
    }
    /// Returns whether the layer focus scope is active.
    pub const fn focus_active(&self) -> bool {
        self.focus_active
    }
    /// Returns the current lifecycle generation.
    pub const fn generation(&self) -> OverlayLayerGeneration {
        self.generation
    }
}

/// The sole live overlay authority for one GPUI window.
#[derive(Clone)]
pub struct WindowOverlayRuntime {
    state: Entity<WindowOverlayRuntimeState>,
    window_id: WindowId,
    ambient_parent_layers: Rc<RefCell<Vec<OverlayLayerId>>>,
}

struct WindowOverlayRuntimeState {
    focus_runtime: FocusScopeRuntime,
    ambient_parent_layers: Rc<RefCell<Vec<OverlayLayerId>>>,
    entries: HashMap<OverlayLayerId, LayerEntry>,
    registration_order: Vec<OverlayLayerId>,
    stack: Vec<OverlayLayerId>,
    next_lease_token: u64,
    next_focus_target_token: u64,
    window_fallback: Option<WindowFallbackEntry>,
    key_subscription: Option<Subscription>,
    mouse_subscription: Option<Subscription>,
    activation_subscription: Option<Subscription>,
    mouse_routes: HashMap<MouseButton, MouseGestureRoute>,
    mouse_authority_revision: u64,
}

#[derive(Clone)]
struct LayerPolicy {
    kind: OverlayLayerKind,
    outside_press_participation: OutsidePressParticipation,
    outside_press: OutsidePressPolicy,
    escape_key: EscapeKeyPolicy,
    focus_restore: FocusRestoreIntent,
    initial_focus: InitialFocusIntent,
}

impl LayerPolicy {
    fn from_overlay_policy(policy: &OverlayLayerPolicy) -> Self {
        Self {
            kind: policy.kind(),
            outside_press_participation: policy.outside_press_participation(),
            outside_press: policy.outside_press_policy(),
            escape_key: policy.escape_key_policy(),
            focus_restore: policy.focus_restore_intent().clone(),
            initial_focus: policy.initial_focus_intent().clone(),
        }
    }

    fn project(&self, presence: OverlayPresence) -> OverlayLayerPolicy {
        OverlayLayerPolicy::new(self.kind, presence)
            .with_outside_press_participation(self.outside_press_participation)
            .with_outside_press_policy(self.outside_press)
            .with_escape_key_policy(self.escape_key)
            .with_focus_restore_intent(self.focus_restore.clone())
            .with_initial_focus_intent(self.initial_focus.clone())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PendingOpenIntent {
    open: bool,
    reason: DismissReason,
    revision: OverlayOpenIntentRevision,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LayerLifecycleState {
    Hidden { pending: Option<PendingOpenIntent> },
    Open,
    CloseRequested { pending: PendingOpenIntent },
    Closing { pending: Option<PendingOpenIntent> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LayerLifecycle {
    state: LayerLifecycleState,
    next_intent_revision: u64,
}

impl LayerLifecycle {
    fn from_presence(presence: OverlayPresence) -> Self {
        let state = match presence {
            OverlayPresence::Hidden => LayerLifecycleState::Hidden { pending: None },
            OverlayPresence::Open => LayerLifecycleState::Open,
            OverlayPresence::Closing => LayerLifecycleState::Closing { pending: None },
        };
        Self {
            state,
            next_intent_revision: 0,
        }
    }

    fn phase(&self) -> OverlayLayerPhase {
        match &self.state {
            LayerLifecycleState::Hidden { .. } => OverlayLayerPhase::Hidden,
            LayerLifecycleState::Open => OverlayLayerPhase::Open,
            LayerLifecycleState::CloseRequested { .. } => OverlayLayerPhase::CloseRequested,
            LayerLifecycleState::Closing { .. } => OverlayLayerPhase::Closing,
        }
    }

    fn presence(&self) -> OverlayPresence {
        match &self.state {
            LayerLifecycleState::Hidden { .. } => OverlayPresence::hidden(),
            LayerLifecycleState::Open | LayerLifecycleState::CloseRequested { .. } => {
                OverlayPresence::open()
            }
            LayerLifecycleState::Closing { .. } => OverlayPresence::closing(),
        }
    }

    fn pending(&self) -> Option<PendingOpenIntent> {
        match &self.state {
            LayerLifecycleState::Hidden { pending } | LayerLifecycleState::Closing { pending } => {
                *pending
            }
            LayerLifecycleState::Open => None,
            LayerLifecycleState::CloseRequested { pending } => Some(*pending),
        }
    }

    fn pending_open(&self) -> Option<bool> {
        self.pending().map(|pending| pending.open)
    }

    fn committed_open(&self) -> bool {
        matches!(
            &self.state,
            LayerLifecycleState::Open | LayerLifecycleState::CloseRequested { .. }
        )
    }

    fn rebind_presence(&mut self, presence: OverlayPresence) {
        if presence == OverlayPresence::open()
            && matches!(&self.state, LayerLifecycleState::CloseRequested { .. })
        {
            return;
        }
        self.state = match presence {
            OverlayPresence::Hidden => LayerLifecycleState::Hidden { pending: None },
            OverlayPresence::Open => LayerLifecycleState::Open,
            OverlayPresence::Closing => LayerLifecycleState::Closing { pending: None },
        };
    }

    fn request_controlled(
        &mut self,
        open: bool,
        reason: DismissReason,
    ) -> Option<PendingOpenIntent> {
        if open == self.committed_open() || self.pending_open() == Some(open) {
            return None;
        }
        let pending = self.allocate_intent(open, reason);
        self.state = match self.phase() {
            OverlayLayerPhase::Open | OverlayLayerPhase::CloseRequested => {
                debug_assert!(!open);
                LayerLifecycleState::CloseRequested { pending }
            }
            OverlayLayerPhase::Hidden => LayerLifecycleState::Hidden {
                pending: Some(pending),
            },
            OverlayLayerPhase::Closing => LayerLifecycleState::Closing {
                pending: Some(pending),
            },
        };
        Some(pending)
    }

    fn transition_to_noninteractive(
        &mut self,
        phase: OverlayLayerPhase,
        pending: Option<PendingOpenIntent>,
    ) {
        self.state = match phase {
            OverlayLayerPhase::Hidden => LayerLifecycleState::Hidden { pending },
            OverlayLayerPhase::Closing => LayerLifecycleState::Closing { pending },
            OverlayLayerPhase::Open | OverlayLayerPhase::CloseRequested => {
                unreachable!("non-interactive lifecycle transition requires Hidden or Closing")
            }
        };
    }

    fn force_controlled_close(
        &mut self,
        phase: OverlayLayerPhase,
        reason: DismissReason,
    ) -> PendingOpenIntent {
        let pending = self
            .pending()
            .filter(|pending| !pending.open)
            .unwrap_or_else(|| self.allocate_intent(false, reason));
        self.transition_to_noninteractive(phase, Some(pending));
        pending
    }

    fn finish_exit(&mut self) {
        let pending = self.pending();
        self.state = LayerLifecycleState::Hidden { pending };
    }

    fn reject_close_intent(&mut self, revision: OverlayOpenIntentRevision) -> Result<(), ()> {
        let pending = match &self.state {
            LayerLifecycleState::CloseRequested { pending } => *pending,
            _ => return Err(()),
        };
        if pending.open || pending.revision != revision {
            return Err(());
        }
        self.state = LayerLifecycleState::Open;
        Ok(())
    }

    fn allocate_intent(&mut self, open: bool, reason: DismissReason) -> PendingOpenIntent {
        self.next_intent_revision = self.next_intent_revision.wrapping_add(1);
        PendingOpenIntent {
            open,
            reason,
            revision: OverlayOpenIntentRevision(self.next_intent_revision),
        }
    }
}

struct LayerEntry {
    id: OverlayLayerId,
    parent: Option<OverlayLayerId>,
    policy: LayerPolicy,
    ownership: OverlayOwnership,
    focus_mode: OverlayFocusMode,
    focus_restore_condition: OverlayFocusRestoreCondition,
    tab_behavior: OverlayTabBehavior,
    on_open_change: Option<OpenChangeCallback>,
    uncontrolled_commit: Option<OpenChangeCallback>,
    lease_token: u64,
    lifecycle: LayerLifecycle,
    generation: OverlayLayerGeneration,
    registration_revision: u64,
    focus_active: bool,
    focus_entered: bool,
    scope_id: Option<FocusScopeId>,
    trigger_id: FocusTargetId,
    surface_id: FocusTargetId,
    focus_targets: HashMap<FocusTargetId, u64>,
    inside_regions: HashMap<OverlayInsideRegionId, LiveInsideRegion>,
    focus_subscription: Option<Subscription>,
    release_subscription: Option<Subscription>,
    pending_unregister: bool,
    forced_by_ancestor: bool,
}

impl LayerEntry {
    fn keyboard_eligible(&self) -> bool {
        matches!(
            self.lifecycle.phase(),
            OverlayLayerPhase::Open | OverlayLayerPhase::CloseRequested
        ) && self.lifecycle.presence().interactive()
    }

    fn projected_policy(&self) -> OverlayLayerPolicy {
        self.policy.project(self.lifecycle.presence())
    }

    fn should_restore_focus(&self) -> bool {
        match self.focus_restore_condition {
            OverlayFocusRestoreCondition::Always => true,
            OverlayFocusRestoreCondition::IfFocusEntered => self.focus_entered,
            OverlayFocusRestoreCondition::Never => false,
        }
    }

    fn snapshot(&self) -> OverlayLayerSnapshot {
        let pending = self.lifecycle.pending();
        OverlayLayerSnapshot {
            id: self.id.clone(),
            parent: self.parent.clone(),
            kind: self.policy.kind,
            phase: self.lifecycle.phase(),
            presence: self.lifecycle.presence(),
            pending_open: pending.map(|pending| pending.open),
            pending_intent: pending.map(|pending| pending.reason),
            pending_intent_revision: pending.map(|pending| pending.revision),
            keyboard_eligible: self.keyboard_eligible(),
            modal_pointer_barrier: self.policy.kind == OverlayLayerKind::Modal
                && self.lifecycle.presence().present(),
            focus_active: self.focus_active,
            generation: self.generation,
        }
    }
}

struct LiveInsideRegion {
    bounds: Bounds<Pixels>,
    valid_through: u64,
}

struct WindowFallbackEntry {
    target_id: FocusTargetId,
    target_token: u64,
}

struct LayerFocusConfig {
    mode: OverlayFocusMode,
    policy: OverlayLayerPolicy,
    scope_id: Option<FocusScopeId>,
    parent_scope: Option<FocusScopeId>,
    trigger_id: FocusTargetId,
    surface_id: FocusTargetId,
    trigger_focus: FocusHandle,
    surface_focus: FocusHandle,
}

struct RebindTransition {
    focus_transition: FocusTransition,
    generation: OverlayLayerGeneration,
}

struct RebindPlan {
    generation: OverlayLayerGeneration,
    cancel_focus_claims: Vec<FocusScopeId>,
    root_transition: FocusTransition,
    descendant_dispatches: Vec<OpenChangeDispatch>,
}

#[derive(Clone)]
enum FocusTransition {
    None,
    Activate(FocusScopeId),
    Deactivate { scope: FocusScopeId, restore: bool },
}

struct OpenChangePlan {
    generation: OverlayLayerGeneration,
    cancel_focus_claims: Vec<FocusScopeId>,
    dispatches: Vec<OpenChangeDispatch>,
}

struct SubtreeUnregisterPlan {
    cancel_focus_claims: Vec<FocusScopeId>,
    removals: Vec<(OverlayLayerLease, FocusTransition)>,
}

struct OpenChangeDispatch {
    layer_id: OverlayLayerId,
    lease_token: u64,
    generation: OverlayLayerGeneration,
    registration_revision: u64,
    focus_transition: FocusTransition,
    uncontrolled_commit: Option<OpenChangeCallback>,
    on_open_change: Option<OpenChangeCallback>,
    changed: bool,
}

impl OpenChangeDispatch {
    fn noop(
        layer_id: OverlayLayerId,
        lease_token: u64,
        generation: OverlayLayerGeneration,
        registration_revision: u64,
    ) -> Self {
        Self {
            layer_id,
            lease_token,
            generation,
            registration_revision,
            focus_transition: FocusTransition::None,
            uncontrolled_commit: None,
            on_open_change: None,
            changed: false,
        }
    }
}

struct FocusCleanup {
    scope_id: Option<FocusScopeId>,
    trigger_id: FocusTargetId,
}

enum MouseDecision {
    None,
    Consume,
    Dismiss {
        layer_id: OverlayLayerId,
        reason: DismissReason,
        consume: bool,
    },
}

impl MouseDecision {
    const fn consumes(&self) -> bool {
        match self {
            Self::None => false,
            Self::Consume => true,
            Self::Dismiss { consume, .. } => *consume,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum MouseGestureRoute {
    Allowed {
        authority: MouseAuthorityStamp,
        owner: Option<MouseGestureOwner>,
        capture: Option<MouseCaptureStamp>,
    },
    Blocked,
}

impl MouseGestureRoute {
    fn resolve(
        &mut self,
        state: &WindowOverlayRuntimeState,
        current_capture: Option<PointerCapture>,
    ) -> MouseRouteOutcome {
        let Self::Allowed {
            authority,
            owner,
            capture,
        } = self
        else {
            return MouseRouteOutcome::Block;
        };

        if owner
            .as_ref()
            .is_some_and(|owner| !state.mouse_gesture_owner_is_current(owner))
        {
            *self = Self::Blocked;
            return MouseRouteOutcome::Block;
        }

        let current_capture = current_capture.map(MouseCaptureStamp::from);
        match (*capture, current_capture) {
            (Some(expected), Some(current)) if expected == current => {
                if *authority == state.mouse_authority_stamp() {
                    MouseRouteOutcome::Allow
                } else {
                    *self = Self::Blocked;
                    MouseRouteOutcome::Block
                }
            }
            (Some(_), _) => {
                *self = Self::Blocked;
                MouseRouteOutcome::Block
            }
            (None, Some(current)) => {
                if *authority == state.mouse_authority_stamp() {
                    *capture = Some(current);
                    MouseRouteOutcome::Allow
                } else {
                    *self = Self::Blocked;
                    MouseRouteOutcome::Block
                }
            }
            (None, None) if *authority == state.mouse_authority_stamp() => {
                MouseRouteOutcome::Reevaluate
            }
            (None, None) => {
                *self = Self::Blocked;
                MouseRouteOutcome::Block
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MouseAuthorityStamp(u64);

#[derive(Clone, Debug, PartialEq, Eq)]
struct MouseGestureOwner {
    id: OverlayLayerId,
    lease_token: u64,
    generation: OverlayLayerGeneration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct MouseCaptureStamp {
    handle: PointerCaptureHandle,
    button: MouseButton,
}

impl From<PointerCapture> for MouseCaptureStamp {
    fn from(capture: PointerCapture) -> Self {
        Self {
            handle: capture.handle(),
            button: capture.button(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MouseRouteOutcome {
    Allow,
    Reevaluate,
    Block,
}

impl MouseRouteOutcome {
    fn apply(self, window: &mut Window, cx: &mut App) -> bool {
        match self {
            Self::Allow => true,
            Self::Reevaluate => false,
            Self::Block => {
                if window.has_active_pointer_session(cx) {
                    let window_handle = window.window_handle();
                    cx.defer(move |cx| {
                        let _ = window_handle.update(cx, |_, window, cx| {
                            window.cancel_pointer_session(PointerCancelReason::CaptureRevoked, cx);
                        });
                    });
                }
                cx.stop_propagation();
                window.prevent_default();
                true
            }
        }
    }
}

impl OverlayFocusMode {
    fn into_scope_mode(self) -> FocusScopeMode {
        match self {
            Self::None | Self::Passive => FocusScopeMode::Passive,
            Self::Modal => FocusScopeMode::ModalLoop,
        }
    }
}

fn validate_registration(
    registration: &OverlayLayerRegistration,
) -> Result<(), WindowOverlayRuntimeError> {
    if registration.ownership == OverlayOwnership::Uncontrolled
        && registration.uncontrolled_commit.is_none()
    {
        return Err(WindowOverlayRuntimeError::MissingUncontrolledCommit(
            registration.id.clone(),
        ));
    }
    let compatible_focus_mode = matches!(
        (registration.policy.kind(), registration.focus_mode),
        (OverlayLayerKind::Tooltip, OverlayFocusMode::None)
            | (
                OverlayLayerKind::NonModalDismissible,
                OverlayFocusMode::None | OverlayFocusMode::Passive
            )
            | (OverlayLayerKind::Menu, OverlayFocusMode::Passive)
            | (OverlayLayerKind::Modal, OverlayFocusMode::Modal)
    );
    if !compatible_focus_mode {
        return Err(WindowOverlayRuntimeError::IncompatibleFocusMode {
            layer: registration.id.clone(),
            kind: registration.policy.kind(),
            focus_mode: registration.focus_mode,
        });
    }
    let compatible_tab_behavior = match registration.tab_behavior {
        OverlayTabBehavior::DismissMenuRoot => registration.policy.kind() == OverlayLayerKind::Menu,
        OverlayTabBehavior::DismissSelf => matches!(
            registration.policy.kind(),
            OverlayLayerKind::NonModalDismissible | OverlayLayerKind::Menu
        ),
        OverlayTabBehavior::Preserve => true,
    };
    if !compatible_tab_behavior {
        return Err(WindowOverlayRuntimeError::IncompatibleTabBehavior {
            layer: registration.id.clone(),
            kind: registration.policy.kind(),
            behavior: registration.tab_behavior.clone(),
        });
    }
    Ok(())
}

fn scope_id_for(layer: &OverlayLayerId) -> FocusScopeId {
    FocusScopeId::new(format!("overlay:{}:scope", layer.as_str()))
}

fn trigger_id_for(layer: &OverlayLayerId) -> FocusTargetId {
    FocusTargetId::new(format!("overlay:{}:trigger", layer.as_str()))
}

fn surface_id_for(layer: &OverlayLayerId) -> FocusTargetId {
    FocusTargetId::new(format!("overlay:{}:surface", layer.as_str()))
}

fn target_registration(
    id: FocusTargetId,
    handle: &FocusHandle,
    scope: Option<&FocusScopeId>,
) -> FocusTargetRegistration {
    let registration = FocusTargetRegistration::new(id, handle);
    if let Some(scope) = scope {
        registration.within_scope(scope.clone())
    } else {
        registration
    }
}
