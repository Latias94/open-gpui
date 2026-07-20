//! Resource-state projection helpers for data-heavy UI components.

use open_gpui_resource::{
    MutationSnapshot, MutationStatus, QueryKey, QueryKeySegment, ResourceSnapshot, ResourceStatus,
};
use open_gpui_ui_core::{Size, TableRowChildrenLoadState, ThemeTokens};
use std::fmt;

use crate::{
    command::{CommandLoadingState, CommandStatusIntent, CommandStatusItem},
    feedback::{EmptyStateState, FeedbackIntent, StatusCueState},
    tree::TreeChildrenLoadState,
    virtualized_list::VirtualizedListItemDescriptor,
};

/// User-displayable copy for resource adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceAdapterLabels {
    resource: String,
    loading: String,
    empty_title: String,
    empty_description: Option<String>,
    stale: String,
    refreshing: String,
    retry_action: String,
    error_title: String,
    mutation_pending: String,
    mutation_success: String,
    mutation_error_title: String,
}

impl ResourceAdapterLabels {
    /// Creates default copy for a named resource.
    pub fn new(resource: impl Into<String>) -> Self {
        let resource = resource.into();
        Self {
            loading: format!("Loading {resource}"),
            empty_title: format!("No {resource}"),
            empty_description: None,
            stale: format!("{resource} is stale"),
            refreshing: format!("Refreshing {resource}"),
            retry_action: "Retry".to_owned(),
            error_title: format!("Failed to load {resource}"),
            mutation_pending: format!("Saving {resource}"),
            mutation_success: format!("Saved {resource}"),
            mutation_error_title: format!("Failed to save {resource}"),
            resource,
        }
    }

    /// Returns the resource label.
    pub fn resource(&self) -> &str {
        &self.resource
    }

    /// Overrides loading copy.
    pub fn loading(mut self, message: impl Into<String>) -> Self {
        self.loading = message.into();
        self
    }

    /// Overrides empty-state title copy.
    pub fn empty_title(mut self, title: impl Into<String>) -> Self {
        self.empty_title = title.into();
        self
    }

    /// Overrides empty-state description copy.
    pub fn empty_description(mut self, description: impl Into<String>) -> Self {
        self.empty_description = Some(description.into());
        self
    }

    /// Overrides stale copy.
    pub fn stale(mut self, message: impl Into<String>) -> Self {
        self.stale = message.into();
        self
    }

    /// Overrides refreshing copy.
    pub fn refreshing(mut self, message: impl Into<String>) -> Self {
        self.refreshing = message.into();
        self
    }

    /// Overrides retry action copy.
    pub fn retry_action(mut self, label: impl Into<String>) -> Self {
        self.retry_action = label.into();
        self
    }

    /// Overrides load-error title copy.
    pub fn error_title(mut self, title: impl Into<String>) -> Self {
        self.error_title = title.into();
        self
    }

    /// Overrides pending mutation copy.
    pub fn mutation_pending(mut self, message: impl Into<String>) -> Self {
        self.mutation_pending = message.into();
        self
    }

    /// Overrides successful mutation copy.
    pub fn mutation_success(mut self, message: impl Into<String>) -> Self {
        self.mutation_success = message.into();
        self
    }

    /// Overrides mutation-error title copy.
    pub fn mutation_error_title(mut self, title: impl Into<String>) -> Self {
        self.mutation_error_title = title.into();
        self
    }
}

impl Default for ResourceAdapterLabels {
    fn default() -> Self {
        Self::new("resource")
    }
}

/// Caller-owned identity namespace for resource-backed command status items.
///
/// The value must be stable for the lifetime of one command surface and must not contain query
/// keys, mutation IDs, user content, or other sensitive runtime data. The adapter cannot infer
/// that contract from a resource snapshot, so the caller supplies it explicitly.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ResourceAdapterNamespace(String);

/// Why a resource adapter status namespace was rejected.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResourceAdapterNamespaceError {
    /// The namespace contained no non-whitespace characters.
    Empty,
    /// The namespace contained a control character.
    ControlCharacter,
}

impl fmt::Display for ResourceAdapterNamespaceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("resource adapter status namespace is empty"),
            Self::ControlCharacter => formatter
                .write_str("resource adapter status namespace contains a control character"),
        }
    }
}

impl std::error::Error for ResourceAdapterNamespaceError {}

impl ResourceAdapterNamespace {
    /// Creates a stable, non-sensitive namespace.
    ///
    /// # Errors
    ///
    /// Returns [`ResourceAdapterNamespaceError::Empty`] when the value contains only whitespace,
    /// or [`ResourceAdapterNamespaceError::ControlCharacter`] when it contains a control character.
    pub fn new(value: impl Into<String>) -> Result<Self, ResourceAdapterNamespaceError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ResourceAdapterNamespaceError::Empty);
        }
        if value.chars().any(char::is_control) {
            return Err(ResourceAdapterNamespaceError::ControlCharacter);
        }
        Ok(Self(value))
    }

    /// Returns the caller-owned namespace value.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Projection from a headless query snapshot into concrete component state inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceCollectionProjection {
    key: QueryKey,
    namespace: ResourceAdapterNamespace,
    status: ResourceStatus,
    visible_item_count: usize,
    has_data: bool,
    error: Option<String>,
    observer_count: usize,
    fetch_attempts: u32,
    labels: ResourceAdapterLabels,
}

impl ResourceCollectionProjection {
    /// Resolves projection state for one resource snapshot.
    pub fn resolve(
        snapshot: &ResourceSnapshot,
        namespace: ResourceAdapterNamespace,
        visible_item_count: usize,
        labels: ResourceAdapterLabels,
    ) -> Self {
        Self {
            key: snapshot.key.clone(),
            namespace,
            status: snapshot.status.clone(),
            visible_item_count,
            has_data: snapshot.data.is_some(),
            error: snapshot.error.clone(),
            observer_count: snapshot.observer_count,
            fetch_attempts: snapshot.fetch_attempts,
            labels,
        }
    }

    /// Returns the resource query key.
    pub const fn key(&self) -> &QueryKey {
        &self.key
    }

    /// Returns the caller-owned status identity namespace.
    pub fn namespace(&self) -> &ResourceAdapterNamespace {
        &self.namespace
    }

    /// Returns the resource lifecycle status.
    pub const fn status(&self) -> &ResourceStatus {
        &self.status
    }

    /// Returns the number of visible rows/items supplied by the caller.
    pub const fn visible_item_count(&self) -> usize {
        self.visible_item_count
    }

    /// Returns whether the snapshot has data, even if it may be redacted.
    pub const fn has_data(&self) -> bool {
        self.has_data
    }

    /// Returns the active observer count.
    pub const fn observer_count(&self) -> usize {
        self.observer_count
    }

    /// Returns how many fetch attempts have run for this entry.
    pub const fn fetch_attempts(&self) -> u32 {
        self.fetch_attempts
    }

    /// Returns whether an initial load is running.
    pub const fn loading(&self) -> bool {
        matches!(self.status, ResourceStatus::Loading)
    }

    /// Returns whether a background refresh is running.
    pub const fn refreshing(&self) -> bool {
        matches!(self.status, ResourceStatus::Refetching)
    }

    /// Returns whether data is present but stale.
    pub const fn stale(&self) -> bool {
        matches!(self.status, ResourceStatus::Stale)
    }

    /// Returns whether the latest fetch failed without usable data.
    pub const fn error(&self) -> bool {
        matches!(self.status, ResourceStatus::Error)
    }

    /// Returns whether the collection loaded successfully but has no visible items.
    pub const fn empty(&self) -> bool {
        matches!(self.status, ResourceStatus::Success | ResourceStatus::Stale)
            && self.visible_item_count == 0
    }

    /// Returns whether the UI can expose a retry action.
    pub const fn retryable(&self) -> bool {
        self.error()
    }

    /// Returns whether user interaction should be suppressed for initial loading.
    pub const fn interaction_disabled(&self) -> bool {
        self.loading()
    }

    /// Returns the primary status message, when the resource needs visible feedback.
    pub fn status_message(&self) -> Option<&str> {
        match self.status {
            ResourceStatus::Idle => None,
            ResourceStatus::Loading => Some(self.labels.loading.as_str()),
            ResourceStatus::Success if self.empty() => Some(self.labels.empty_title.as_str()),
            ResourceStatus::Success => None,
            ResourceStatus::Stale => Some(self.labels.stale.as_str()),
            ResourceStatus::Refetching => Some(self.labels.refreshing.as_str()),
            ResourceStatus::Error => self
                .error
                .as_deref()
                .or(Some(self.labels.error_title.as_str())),
        }
    }

    /// Returns a compact status cue for loading, refresh, stale, and error states.
    pub fn status_cue_state(&self, tokens: ThemeTokens) -> Option<StatusCueState> {
        let intent = match self.status {
            ResourceStatus::Loading | ResourceStatus::Refetching => FeedbackIntent::Info,
            ResourceStatus::Stale => FeedbackIntent::Warning,
            ResourceStatus::Error => FeedbackIntent::Danger,
            ResourceStatus::Idle | ResourceStatus::Success => return None,
        };
        Some(
            StatusCueState::resolve(
                intent,
                self.status_message().unwrap_or_default(),
                Size::Medium,
                tokens,
            )
            .with_busy(matches!(
                self.status,
                ResourceStatus::Loading | ResourceStatus::Refetching
            )),
        )
    }

    /// Returns an empty/error state suitable for full-surface fallback content.
    pub fn empty_state(&self, tokens: ThemeTokens) -> Option<EmptyStateState> {
        match self.status {
            ResourceStatus::Error => Some(EmptyStateState::resolve(
                FeedbackIntent::Danger,
                self.labels.error_title.as_str(),
                self.error.as_deref(),
                Size::Medium,
                tokens,
            )),
            ResourceStatus::Success | ResourceStatus::Stale if self.empty() => {
                Some(EmptyStateState::resolve(
                    FeedbackIntent::Neutral,
                    self.labels.empty_title.as_str(),
                    self.labels.empty_description.as_deref(),
                    Size::Medium,
                    tokens,
                ))
            }
            ResourceStatus::Idle
            | ResourceStatus::Loading
            | ResourceStatus::Success
            | ResourceStatus::Stale
            | ResourceStatus::Refetching => None,
        }
    }

    /// Returns a virtualized-list status row for initial loading, empty, and retry states.
    pub fn virtualized_status_row(
        &self,
        key: impl Into<String>,
    ) -> Option<VirtualizedListItemDescriptor> {
        let key = key.into();
        match self.status {
            ResourceStatus::Loading => Some(VirtualizedListItemDescriptor::loading(
                key,
                self.labels.loading.clone(),
            )),
            ResourceStatus::Refetching if self.visible_item_count == 0 => Some(
                VirtualizedListItemDescriptor::loading(key, self.labels.refreshing.clone()),
            ),
            ResourceStatus::Success | ResourceStatus::Stale if self.empty() => Some(
                VirtualizedListItemDescriptor::empty(key, self.labels.empty_title.clone()),
            ),
            ResourceStatus::Error => Some(VirtualizedListItemDescriptor::retry(
                key,
                self.status_message()
                    .unwrap_or(self.labels.error_title.as_str())
                    .to_owned(),
                self.labels.retry_action.clone(),
            )),
            ResourceStatus::Idle
            | ResourceStatus::Success
            | ResourceStatus::Stale
            | ResourceStatus::Refetching => None,
        }
    }

    /// Returns command loading metadata for resource-backed command providers.
    pub fn command_loading_state(&self) -> Option<CommandLoadingState> {
        match self.status {
            ResourceStatus::Loading | ResourceStatus::Refetching => Some(CommandLoadingState::new(
                self.status_message().unwrap_or_default(),
                None,
            )),
            ResourceStatus::Idle
            | ResourceStatus::Success
            | ResourceStatus::Stale
            | ResourceStatus::Error => None,
        }
    }

    /// Returns command status metadata for degraded resource-backed command providers.
    ///
    /// The stored namespace supplies a stable identity that is not derived from the resource query
    /// key and remains unchanged across stale/error transitions.
    pub fn command_status_item(&self) -> Option<CommandStatusItem> {
        let status_id = format!("resource:{}", self.namespace.as_str());
        match self.status {
            ResourceStatus::Stale => Some(CommandStatusItem::new(
                status_id,
                CommandStatusIntent::Warning,
                self.labels.stale.clone(),
            )),
            ResourceStatus::Error => Some(CommandStatusItem::new(
                status_id,
                CommandStatusIntent::Error,
                self.status_message()
                    .unwrap_or(self.labels.error_title.as_str())
                    .to_owned(),
            )),
            ResourceStatus::Idle
            | ResourceStatus::Loading
            | ResourceStatus::Success
            | ResourceStatus::Refetching => None,
        }
    }

    /// Returns Table row children loading metadata for lazy tree-table branches.
    pub fn table_children_load_state(&self) -> TableRowChildrenLoadState {
        match self.status {
            ResourceStatus::Loading | ResourceStatus::Refetching => {
                TableRowChildrenLoadState::loading(
                    self.status_message()
                        .unwrap_or(self.labels.loading.as_str())
                        .to_owned(),
                )
            }
            ResourceStatus::Error => TableRowChildrenLoadState::failed(
                self.status_message()
                    .unwrap_or(self.labels.error_title.as_str())
                    .to_owned(),
            ),
            ResourceStatus::Idle | ResourceStatus::Success | ResourceStatus::Stale => {
                TableRowChildrenLoadState::Idle
            }
        }
    }

    /// Returns Tree item children loading metadata for lazy branches.
    pub fn tree_children_load_state(&self) -> TreeChildrenLoadState {
        match self.status {
            ResourceStatus::Idle => TreeChildrenLoadState::unloaded(),
            ResourceStatus::Loading | ResourceStatus::Refetching => TreeChildrenLoadState::loading(
                self.status_message()
                    .unwrap_or(self.labels.loading.as_str())
                    .to_owned(),
            ),
            ResourceStatus::Error => TreeChildrenLoadState::failed(
                self.status_message()
                    .unwrap_or(self.labels.error_title.as_str())
                    .to_owned(),
            ),
            ResourceStatus::Success | ResourceStatus::Stale => TreeChildrenLoadState::loaded(),
        }
    }
}

/// Projection from a headless mutation snapshot into concrete component state inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceMutationProjection {
    id: String,
    namespace: ResourceAdapterNamespace,
    status: MutationStatus,
    error: Option<String>,
    labels: ResourceAdapterLabels,
}

impl ResourceMutationProjection {
    /// Resolves projection state for one mutation snapshot.
    pub fn resolve(
        snapshot: &MutationSnapshot,
        namespace: ResourceAdapterNamespace,
        labels: ResourceAdapterLabels,
    ) -> Self {
        Self {
            id: snapshot.id.clone(),
            namespace,
            status: snapshot.status.clone(),
            error: snapshot.error.clone(),
            labels,
        }
    }

    /// Returns the mutation id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the caller-owned status identity namespace.
    pub fn namespace(&self) -> &ResourceAdapterNamespace {
        &self.namespace
    }

    /// Returns the mutation lifecycle status.
    pub const fn status(&self) -> &MutationStatus {
        &self.status
    }

    /// Returns whether the mutation is pending.
    pub const fn pending(&self) -> bool {
        matches!(self.status, MutationStatus::Pending)
    }

    /// Returns whether the mutation failed.
    pub const fn error(&self) -> bool {
        matches!(self.status, MutationStatus::Error)
    }

    /// Returns whether actions that would conflict with the mutation should be disabled.
    pub const fn disables_actions(&self) -> bool {
        self.pending()
    }

    /// Returns the primary mutation status message, when visible feedback is useful.
    pub fn status_message(&self) -> Option<&str> {
        match self.status {
            MutationStatus::Idle => None,
            MutationStatus::Pending => Some(self.labels.mutation_pending.as_str()),
            MutationStatus::Success => Some(self.labels.mutation_success.as_str()),
            MutationStatus::Error => self
                .error
                .as_deref()
                .or(Some(self.labels.mutation_error_title.as_str())),
        }
    }

    /// Returns a compact status cue for mutation feedback.
    pub fn status_cue_state(&self, tokens: ThemeTokens) -> Option<StatusCueState> {
        let intent = match self.status {
            MutationStatus::Idle => return None,
            MutationStatus::Pending => FeedbackIntent::Info,
            MutationStatus::Success => FeedbackIntent::Success,
            MutationStatus::Error => FeedbackIntent::Danger,
        };
        Some(
            StatusCueState::resolve(
                intent,
                self.status_message().unwrap_or_default(),
                Size::Medium,
                tokens,
            )
            .with_busy(matches!(self.status, MutationStatus::Pending)),
        )
    }

    /// Returns command status metadata for mutation feedback.
    ///
    /// The stored namespace supplies a stable identity that is not derived from the mutation ID
    /// and remains unchanged across pending/error transitions.
    pub fn command_status_item(&self) -> Option<CommandStatusItem> {
        let status_id = format!("mutation:{}", self.namespace.as_str());
        match self.status {
            MutationStatus::Idle | MutationStatus::Success => None,
            MutationStatus::Pending => Some(CommandStatusItem::new(
                status_id,
                CommandStatusIntent::Info,
                self.labels.mutation_pending.clone(),
            )),
            MutationStatus::Error => Some(CommandStatusItem::new(
                status_id,
                CommandStatusIntent::Error,
                self.status_message()
                    .unwrap_or(self.labels.mutation_error_title.as_str())
                    .to_owned(),
            )),
        }
    }
}

/// Returns a stable slash-separated label for a query key.
pub fn resource_query_key_label(key: &QueryKey) -> String {
    key.segments()
        .iter()
        .map(query_key_segment_label)
        .collect::<Vec<_>>()
        .join("/")
}

fn query_key_segment_label(segment: &QueryKeySegment) -> String {
    match segment {
        QueryKeySegment::Text(value) => value.clone(),
        QueryKeySegment::Integer(value) => value.to_string(),
        QueryKeySegment::Bool(value) => value.to_string(),
    }
}
