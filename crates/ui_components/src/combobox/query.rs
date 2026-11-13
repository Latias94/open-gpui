use std::sync::Arc;

use crate::text_editing::TextEditingPolicy;

use super::{ComboboxGroupDescriptor, ComboboxOptionDescriptor};

/// Ownership of result filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComboboxFilterMode {
    /// Match option values, labels, and keywords against the query locally.
    #[default]
    Local,
    /// Display the supplied result page without applying another filter.
    External,
}

/// Application-provided result status, independent of selection and open state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ComboboxResultsStatus {
    /// No request has been started, or its owner has retired it.
    Idle,
    /// A request is pending. Supplied options may remain navigable.
    Loading,
    /// The current result page is available, including an empty page.
    #[default]
    Ready,
    /// The request failed with application-provided display text.
    Error(String),
}

impl ComboboxResultsStatus {
    /// Returns default non-selectable status text, if any.
    pub fn message(&self) -> Option<&str> {
        match self {
            Self::Loading => Some("Loading"),
            Self::Error(message) => Some(message),
            Self::Idle | Self::Ready => None,
        }
    }
}

/// One externally filtered result page, with no provider or task ownership.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ComboboxResults {
    status: ComboboxResultsStatus,
    options: Vec<ComboboxOptionDescriptor>,
    groups: Vec<ComboboxGroupDescriptor>,
}

impl ComboboxResults {
    /// Creates an available, empty result page.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an idle result page.
    pub fn idle() -> Self {
        Self {
            status: ComboboxResultsStatus::Idle,
            ..Self::default()
        }
    }

    /// Creates a pending result page.
    pub fn loading() -> Self {
        Self {
            status: ComboboxResultsStatus::Loading,
            ..Self::default()
        }
    }

    /// Creates a failed result page with application-provided display text.
    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            status: ComboboxResultsStatus::Error(message.into()),
            ..Self::default()
        }
    }

    /// Adds one standalone result.
    pub fn option(mut self, option: ComboboxOptionDescriptor) -> Self {
        self.options.push(option);
        self
    }

    /// Adds standalone results in provider order.
    pub fn options(mut self, options: impl IntoIterator<Item = ComboboxOptionDescriptor>) -> Self {
        self.options.extend(options);
        self
    }

    /// Adds one result group.
    pub fn group(mut self, group: ComboboxGroupDescriptor) -> Self {
        self.groups.push(group);
        self
    }

    /// Adds result groups in provider order.
    pub fn groups(mut self, groups: impl IntoIterator<Item = ComboboxGroupDescriptor>) -> Self {
        self.groups.extend(groups);
        self
    }

    /// Returns the application's result status.
    pub const fn status(&self) -> &ComboboxResultsStatus {
        &self.status
    }

    /// Returns standalone result descriptors.
    pub fn options_ref(&self) -> &[ComboboxOptionDescriptor] {
        &self.options
    }

    /// Returns grouped result descriptors.
    pub fn groups_ref(&self) -> &[ComboboxGroupDescriptor] {
        &self.groups
    }

    pub(super) fn into_parts(
        self,
    ) -> (
        ComboboxResultsStatus,
        Vec<ComboboxOptionDescriptor>,
        Vec<ComboboxGroupDescriptor>,
    ) {
        (self.status, self.options, self.groups)
    }
}

/// Opaque identity for one request in an application-owned query session.
///
/// Clones identify the same request. Starting another request, even with the same
/// text, creates a distinct identity. Identities from different sessions never match.
#[derive(Debug, Clone)]
pub struct ComboboxQueryRequest {
    identity: Arc<()>,
    query: String,
}

impl ComboboxQueryRequest {
    /// Returns the normalized single-line query sent to the provider.
    pub fn query(&self) -> &str {
        &self.query
    }
}

impl PartialEq for ComboboxQueryRequest {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
    }
}

impl Eq for ComboboxQueryRequest {}

/// Bounded request acceptance for an application-owned asynchronous provider.
///
/// Store this value in the application entity. Pass a cloned request to the provider,
/// then call [`Self::complete`] before publishing its result page. Superseding or
/// cancelling the pending request rejects every later reply carrying its identity.
/// The application owns spawning, cancellation, notifications, and host lifetime;
/// call [`Self::cancel`] when closing or relocating a host retires its request scope.
#[derive(Debug)]
pub struct ComboboxQuerySession {
    query: String,
    pending_request: Option<ComboboxQueryRequest>,
    results: ComboboxResults,
}

impl Default for ComboboxQuerySession {
    fn default() -> Self {
        Self {
            query: String::new(),
            pending_request: None,
            results: ComboboxResults::idle(),
        }
    }
}

impl ComboboxQuerySession {
    /// Creates an idle query session without starting work.
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts a request and retires the previous request and result page.
    pub fn begin(&mut self, query: impl Into<String>) -> ComboboxQueryRequest {
        self.query = TextEditingPolicy::single_line().normalize_text(&query.into());
        let request = ComboboxQueryRequest {
            identity: Arc::new(()),
            query: self.query.clone(),
        };
        self.pending_request = Some(request.clone());
        self.results = ComboboxResults::loading();
        request
    }

    /// Publishes a ready or failed reply only if it still owns the pending request.
    ///
    /// Returns `false` without changing state for stale, duplicate, cancelled, or
    /// foreign replies, or nonterminal status. A successful completion consumes
    /// the request identity.
    pub fn complete(&mut self, request: &ComboboxQueryRequest, results: ComboboxResults) -> bool {
        if self.pending_request.as_ref() != Some(request)
            || !matches!(
                results.status(),
                ComboboxResultsStatus::Ready | ComboboxResultsStatus::Error(_)
            )
        {
            return false;
        }
        self.pending_request = None;
        self.results = results;
        true
    }

    /// Retires the pending identity and clears its page without cancelling any task.
    pub fn cancel(&mut self) {
        self.pending_request = None;
        self.results = ComboboxResults::idle();
    }

    /// Returns the current query independently of request status.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns the identity that may still publish a reply.
    pub const fn pending_request(&self) -> Option<&ComboboxQueryRequest> {
        self.pending_request.as_ref()
    }

    /// Returns the accepted page for [`super::Combobox::results`].
    pub const fn results(&self) -> &ComboboxResults {
        &self.results
    }
}
