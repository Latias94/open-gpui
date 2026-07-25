use super::DockSurfaceActivationState;
use crate::{DockController, DockSpaceId, DockViewportRuntimeHandle};
use open_gpui::{App, AppContext, Context, Entity, EventEmitter, Subscription};

/// A durable category of committed docking-surface change.
///
/// Categories describe which snapshot domains may have changed. They intentionally exclude
/// transient focus, styling, and viewport-dispatch state.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum DockSurfaceChangeCategory {
    /// The logical docking layout changed.
    Layout,
    /// The selected panel changed.
    Selection,
    /// A panel was opened, closed, attached, detached, or otherwise changed lifecycle state.
    PanelLifecycle,
    /// The set or routing of platform viewports changed.
    ViewportTopology,
    /// Committed platform observation changed a viewport's placement.
    ObservedViewportPlacement,
}

impl DockSurfaceChangeCategory {
    const ALL: [Self; 5] = [
        Self::Layout,
        Self::Selection,
        Self::PanelLifecycle,
        Self::ViewportTopology,
        Self::ObservedViewportPlacement,
    ];

    const fn bit(self) -> u8 {
        match self {
            Self::Layout => 1 << 0,
            Self::Selection => 1 << 1,
            Self::PanelLifecycle => 1 << 2,
            Self::ViewportTopology => 1 << 3,
            Self::ObservedViewportPlacement => 1 << 4,
        }
    }
}

/// Metadata emitted after one docking-surface transaction commits.
///
/// The event does not contain a layout or viewport snapshot. Applications can debounce these
/// lightweight events and explicitly export a revision-consistent snapshot when their persistence
/// policy requires one.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DockSurfaceChangeEvent {
    revision: u64,
    categories: Vec<DockSurfaceChangeCategory>,
}

impl DockSurfaceChangeEvent {
    fn new(revision: u64, categories: Vec<DockSurfaceChangeCategory>) -> Self {
        debug_assert!(!categories.is_empty());
        Self {
            revision,
            categories,
        }
    }

    /// Returns the committed surface revision.
    pub const fn revision(&self) -> u64 {
        self.revision
    }

    /// Returns the deduplicated change categories in stable declaration order.
    pub fn categories(&self) -> &[DockSurfaceChangeCategory] {
        &self.categories
    }

    /// Returns whether this commit contains `category`.
    pub fn contains(&self, category: DockSurfaceChangeCategory) -> bool {
        self.categories.contains(&category)
    }
}

/// Internal identity for one explicit root surface transaction.
///
/// This identity is threaded through controller and viewport-runtime commits, but it is never
/// exposed as part of the application-facing mutation API or change event.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct DockSurfaceTransactionId(u64);

#[derive(Debug)]
struct PendingDockSurfaceTransaction {
    id: DockSurfaceTransactionId,
    category_bits: u8,
}

impl PendingDockSurfaceTransaction {
    fn new(id: DockSurfaceTransactionId) -> Self {
        Self {
            id,
            category_bits: 0,
        }
    }

    fn record(&mut self, category: DockSurfaceChangeCategory) {
        self.category_bits |= category.bit();
    }

    fn categories(&self) -> Vec<DockSurfaceChangeCategory> {
        DockSurfaceChangeCategory::ALL
            .into_iter()
            .filter(|category| self.category_bits & category.bit() != 0)
            .collect()
    }
}

/// Private application-level owner for one dock controller and viewport runtime.
///
/// Every `DockSurface` clone points at the same entity, so revision and transaction state cannot
/// diverge between handles.
#[derive(Debug)]
pub(crate) struct DockSurfaceOwner {
    controller: Entity<DockController>,
    viewport_runtime: DockViewportRuntimeHandle,
    primary_space: DockSpaceId,
    activation: DockSurfaceActivationState,
    revision: u64,
    last_transaction_id: u64,
    pending_transaction: Option<PendingDockSurfaceTransaction>,
}

impl DockSurfaceOwner {
    /// Creates an owner around one controller/runtime pair.
    pub(crate) fn new(
        controller: Entity<DockController>,
        viewport_runtime: DockViewportRuntimeHandle,
        primary_space: DockSpaceId,
    ) -> Self {
        Self {
            controller,
            viewport_runtime,
            primary_space,
            activation: DockSurfaceActivationState::new(),
            revision: 0,
            last_transaction_id: 0,
            pending_transaction: None,
        }
    }

    /// Returns the shared controller entity.
    pub(crate) fn controller(&self) -> Entity<DockController> {
        self.controller.clone()
    }

    /// Returns the shared viewport-runtime handle.
    pub(crate) fn runtime(&self) -> DockViewportRuntimeHandle {
        self.viewport_runtime.clone()
    }

    /// Returns the default logical dock space.
    pub(crate) fn primary_space(&self) -> &DockSpaceId {
        &self.primary_space
    }

    pub(crate) fn activation(&self) -> &DockSurfaceActivationState {
        &self.activation
    }

    pub(crate) fn activation_mut(&mut self) -> &mut DockSurfaceActivationState {
        &mut self.activation
    }

    /// Returns the latest committed surface revision.
    pub(crate) const fn revision(&self) -> u64 {
        self.revision
    }

    /// Begins a distinct root surface transaction.
    ///
    /// Nested work must carry the returned identity rather than beginning another root
    /// transaction.
    pub(crate) fn begin_root_transaction(&mut self) -> DockSurfaceTransactionId {
        assert!(
            self.pending_transaction.is_none(),
            "cannot begin a dock surface root transaction while another transaction is active"
        );

        self.last_transaction_id = self
            .last_transaction_id
            .checked_add(1)
            .expect("dock surface transaction identity space exhausted");
        let id = DockSurfaceTransactionId(self.last_transaction_id);
        self.pending_transaction = Some(PendingDockSurfaceTransaction::new(id));
        id
    }

    /// Records one committed change category against the active root transaction.
    pub(crate) fn record_change(
        &mut self,
        transaction: DockSurfaceTransactionId,
        category: DockSurfaceChangeCategory,
    ) {
        let pending = self
            .pending_transaction
            .as_mut()
            .expect("cannot record a dock surface change without an active transaction");
        assert_eq!(
            pending.id, transaction,
            "dock surface change belongs to a different transaction"
        );
        pending.record(category);
    }

    /// Records committed change categories against the active root transaction.
    pub(crate) fn record_changes(
        &mut self,
        transaction: DockSurfaceTransactionId,
        categories: impl IntoIterator<Item = DockSurfaceChangeCategory>,
    ) {
        for category in categories {
            self.record_change(transaction, category);
        }
    }

    /// Finishes a root transaction and emits one metadata event when it recorded durable changes.
    ///
    /// Empty transactions do not advance the revision. The pending transaction is cleared before
    /// event publication so an event subscriber can synchronously issue another root command.
    pub(crate) fn finish_root_transaction(
        &mut self,
        transaction: DockSurfaceTransactionId,
        cx: &mut Context<Self>,
    ) -> Option<DockSurfaceChangeEvent> {
        let pending = self
            .pending_transaction
            .take()
            .expect("cannot finish a dock surface transaction that is not active");
        assert_eq!(
            pending.id, transaction,
            "attempted to finish a different dock surface transaction"
        );

        let categories = pending.categories();
        if categories.is_empty() {
            return None;
        }

        self.revision = self
            .revision
            .checked_add(1)
            .expect("dock surface revision space exhausted");
        let event = DockSurfaceChangeEvent::new(self.revision, categories);
        cx.emit(event.clone());
        Some(event)
    }
}

impl EventEmitter<DockSurfaceChangeEvent> for DockSurfaceOwner {}

/// Runs one explicit root transaction against a surface owner.
///
/// `update` should thread the supplied identity through nested controller/runtime operations and
/// record only categories backed by committed facts.
pub(crate) fn with_root_transaction<C, R>(
    owner: &Entity<DockSurfaceOwner>,
    cx: &mut C,
    update: impl FnOnce(
        &mut DockSurfaceOwner,
        DockSurfaceTransactionId,
        &mut Context<DockSurfaceOwner>,
    ) -> R,
) -> R
where
    C: AppContext,
{
    cx.update_entity(owner, |owner, cx| {
        let transaction = owner.begin_root_transaction();
        let result = update(owner, transaction, cx);
        owner.finish_root_transaction(transaction, cx);
        result
    })
}

/// Runs a root transaction whose work may synchronously re-enter the app.
///
/// The owner borrow is released while `update` runs. Typed nested commit sinks can therefore
/// record against the explicit transaction identity without re-entering an active entity update.
pub(crate) fn with_detached_root_transaction<C, R>(
    owner: &Entity<DockSurfaceOwner>,
    cx: &mut C,
    update: impl FnOnce(DockSurfaceTransactionId, &mut C) -> R,
) -> R
where
    C: AppContext,
{
    let transaction = cx.update_entity(owner, |owner, _| owner.begin_root_transaction());
    let result = update(transaction, cx);
    cx.update_entity(owner, |owner, owner_cx| {
        owner.finish_root_transaction(transaction, owner_cx);
    });
    result
}

/// Subscribes to committed metadata events from a surface owner.
pub(crate) fn subscribe(
    owner: &Entity<DockSurfaceOwner>,
    cx: &mut App,
    mut on_event: impl FnMut(&DockSurfaceChangeEvent, &mut App) + 'static,
) -> Subscription {
    cx.subscribe(owner, move |_owner, event, cx| on_event(event, cx))
}
