//! Runtime-neutral dynamic command providers.

use std::fmt;

use crate::{CommandContribution, CommandScopeId, CommandSourceId};

/// Stable provider identifier for dynamic command sources.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CommandProviderId(String);

impl CommandProviderId {
    /// Creates a provider id.
    pub fn new(provider: impl Into<String>) -> Self {
        Self(provider.into())
    }

    /// Returns the provider id as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns whether the provider id is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for CommandProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<String> for CommandProviderId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for CommandProviderId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

/// Monotonic request id issued by a command center for one provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct CommandProviderRequestId(u64);

impl CommandProviderRequestId {
    /// Creates a request id from a caller-owned monotonic value.
    pub const fn new(request_id: u64) -> Self {
        Self(request_id)
    }

    /// Returns the numeric request id.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for CommandProviderRequestId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<u64> for CommandProviderRequestId {
    fn from(value: u64) -> Self {
        Self(value)
    }
}

/// Request passed to a dynamic command provider.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommandProviderRequest {
    request_id: Option<CommandProviderRequestId>,
    query: String,
    active_scopes: Vec<CommandScopeId>,
}

impl CommandProviderRequest {
    /// Creates a provider request for the current command query.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            request_id: None,
            query: query.into(),
            active_scopes: Vec::new(),
        }
    }

    /// Binds this request to a center-issued lifecycle id.
    pub fn request_id(mut self, request_id: impl Into<CommandProviderRequestId>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// Sets active command scopes visible to the provider.
    pub fn active_scopes(
        mut self,
        scopes: impl IntoIterator<Item = impl Into<CommandScopeId>>,
    ) -> Self {
        self.active_scopes = scopes.into_iter().map(Into::into).collect();
        self
    }

    /// Returns the center-issued request id, when this request participates in lifecycle checks.
    pub const fn request_id_ref(&self) -> Option<CommandProviderRequestId> {
        self.request_id
    }

    /// Returns the current command query.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns active command scopes visible to the provider.
    pub fn active_scopes_ref(&self) -> &[CommandScopeId] {
        &self.active_scopes
    }
}

/// State reported by a dynamic command provider response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandProviderState {
    /// Provider results are complete for the request.
    Ready,
    /// Provider work is still running; sources in the response are the caller-owned interim state.
    Loading,
    /// Provider work failed; sources in the response are the caller-owned fallback state.
    Failed,
}

/// One dynamic source returned by a command provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandProviderSource {
    scope_id: CommandScopeId,
    source_id: CommandSourceId,
    contributions: Vec<CommandContribution>,
}

impl CommandProviderSource {
    /// Creates a dynamic provider source.
    pub fn new(
        scope_id: impl Into<CommandScopeId>,
        source_id: impl Into<CommandSourceId>,
        contributions: impl IntoIterator<Item = CommandContribution>,
    ) -> Self {
        Self {
            scope_id: scope_id.into(),
            source_id: source_id.into(),
            contributions: contributions.into_iter().collect(),
        }
    }

    /// Returns the scope id where this source should be registered.
    pub const fn scope_id(&self) -> &CommandScopeId {
        &self.scope_id
    }

    /// Returns the source id used for replacement and unregistration.
    pub const fn source_id(&self) -> &CommandSourceId {
        &self.source_id
    }

    /// Returns contributed commands.
    pub fn contributions(&self) -> &[CommandContribution] {
        &self.contributions
    }

    /// Returns the number of contributed commands.
    pub const fn len(&self) -> usize {
        self.contributions.len()
    }

    /// Returns whether this source contains no commands.
    pub const fn is_empty(&self) -> bool {
        self.contributions.is_empty()
    }
}

/// Dynamic provider response.
///
/// Responses are intentionally runtime-neutral. Applications may compute them synchronously or
/// asynchronously, then apply the latest response to a [`crate::CommandCenter`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandProviderResponse {
    request_id: Option<CommandProviderRequestId>,
    state: CommandProviderState,
    message: Option<String>,
    sources: Vec<CommandProviderSource>,
}

impl Default for CommandProviderResponse {
    fn default() -> Self {
        Self::ready()
    }
}

impl CommandProviderResponse {
    /// Creates a ready response.
    pub fn ready() -> Self {
        Self {
            request_id: None,
            state: CommandProviderState::Ready,
            message: None,
            sources: Vec::new(),
        }
    }

    /// Creates a loading response with an optional display message.
    pub fn loading(message: impl Into<String>) -> Self {
        Self {
            request_id: None,
            state: CommandProviderState::Loading,
            message: non_empty_message(message),
            sources: Vec::new(),
        }
    }

    /// Creates a failed response with an optional display message.
    pub fn failed(message: impl Into<String>) -> Self {
        Self {
            request_id: None,
            state: CommandProviderState::Failed,
            message: non_empty_message(message),
            sources: Vec::new(),
        }
    }

    /// Binds this response to a center-issued provider request.
    pub fn request_id(mut self, request_id: impl Into<CommandProviderRequestId>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }

    /// Binds this response to the id carried by a provider request.
    pub fn for_request(mut self, request: &CommandProviderRequest) -> Self {
        self.request_id = request.request_id_ref();
        self
    }

    /// Returns the request id this response belongs to, when lifecycle-bound.
    pub const fn request_id_ref(&self) -> Option<CommandProviderRequestId> {
        self.request_id
    }

    /// Adds one dynamic source.
    pub fn source(mut self, source: CommandProviderSource) -> Self {
        self.sources.push(source);
        self
    }

    /// Adds many dynamic sources.
    pub fn sources(mut self, sources: impl IntoIterator<Item = CommandProviderSource>) -> Self {
        self.sources.extend(sources);
        self
    }

    /// Returns the provider response state.
    pub const fn state(&self) -> CommandProviderState {
        self.state
    }

    /// Returns the optional provider message.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Returns dynamic sources in deterministic caller-provided order.
    pub fn sources_ref(&self) -> &[CommandProviderSource] {
        &self.sources
    }
}

/// Applied provider status retained by a command center.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandProviderStatus {
    provider_id: CommandProviderId,
    request_id: Option<CommandProviderRequestId>,
    query: Option<String>,
    state: CommandProviderState,
    message: Option<String>,
    source_count: usize,
    command_count: usize,
}

impl CommandProviderStatus {
    pub(crate) fn new(
        provider_id: CommandProviderId,
        request_id: Option<CommandProviderRequestId>,
        query: Option<String>,
        state: CommandProviderState,
        message: Option<String>,
        source_count: usize,
        command_count: usize,
    ) -> Self {
        Self {
            provider_id,
            request_id,
            query,
            state,
            message,
            source_count,
            command_count,
        }
    }

    /// Returns the provider id.
    pub const fn provider_id(&self) -> &CommandProviderId {
        &self.provider_id
    }

    /// Returns the provider request id that produced this status, when lifecycle-bound.
    pub const fn request_id(&self) -> Option<CommandProviderRequestId> {
        self.request_id
    }

    /// Returns the provider query that produced this status, when known.
    pub fn query(&self) -> Option<&str> {
        self.query.as_deref()
    }

    /// Returns the latest provider state.
    pub const fn state(&self) -> CommandProviderState {
        self.state
    }

    /// Returns the optional latest provider message.
    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    /// Returns the number of dynamic sources from the latest response.
    pub const fn source_count(&self) -> usize {
        self.source_count
    }

    /// Returns the number of commands from the latest response.
    pub const fn command_count(&self) -> usize {
        self.command_count
    }
}

/// Metadata for a provider response that was ignored because it is no longer current.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandProviderStaleResponse {
    provider_id: CommandProviderId,
    response_request_id: CommandProviderRequestId,
    current_request_id: Option<CommandProviderRequestId>,
}

impl CommandProviderStaleResponse {
    pub(crate) fn new(
        provider_id: CommandProviderId,
        response_request_id: CommandProviderRequestId,
        current_request_id: Option<CommandProviderRequestId>,
    ) -> Self {
        Self {
            provider_id,
            response_request_id,
            current_request_id,
        }
    }

    /// Returns the provider id whose response was ignored.
    pub const fn provider_id(&self) -> &CommandProviderId {
        &self.provider_id
    }

    /// Returns the stale response request id.
    pub const fn response_request_id(&self) -> CommandProviderRequestId {
        self.response_request_id
    }

    /// Returns the current request id for that provider, when the center has one.
    pub const fn current_request_id(&self) -> Option<CommandProviderRequestId> {
        self.current_request_id
    }
}

/// Result of applying a provider response to a command center.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandProviderApplyOutcome {
    /// The response was current and updated the command center.
    Applied(CommandProviderStatus),
    /// The response was stale and was ignored.
    Stale(CommandProviderStaleResponse),
}

impl CommandProviderApplyOutcome {
    /// Returns whether this response updated the command center.
    pub const fn applied(&self) -> bool {
        matches!(self, Self::Applied(_))
    }

    /// Returns whether this response was ignored as stale.
    pub const fn stale(&self) -> bool {
        matches!(self, Self::Stale(_))
    }

    /// Returns the applied status, when this response was accepted.
    pub const fn status(&self) -> Option<&CommandProviderStatus> {
        match self {
            Self::Applied(status) => Some(status),
            Self::Stale(_) => None,
        }
    }

    /// Consumes the outcome and returns the applied status, when accepted.
    pub fn into_status(self) -> Option<CommandProviderStatus> {
        match self {
            Self::Applied(status) => Some(status),
            Self::Stale(_) => None,
        }
    }

    /// Returns stale-response metadata, when this response was ignored.
    pub const fn stale_response(&self) -> Option<&CommandProviderStaleResponse> {
        match self {
            Self::Applied(_) => None,
            Self::Stale(stale) => Some(stale),
        }
    }
}

/// Dynamic command provider callback.
pub trait CommandProvider: 'static {
    /// Produces dynamic commands for the given request.
    fn provide_commands(&self, request: &CommandProviderRequest) -> CommandProviderResponse;
}

impl<F> CommandProvider for F
where
    F: Fn(&CommandProviderRequest) -> CommandProviderResponse + 'static,
{
    fn provide_commands(&self, request: &CommandProviderRequest) -> CommandProviderResponse {
        self(request)
    }
}

fn non_empty_message(message: impl Into<String>) -> Option<String> {
    let message = message.into();
    (!message.is_empty()).then_some(message)
}

#[cfg(test)]
mod tests {
    use crate::{
        CommandContribution, CommandDescriptor, CommandProviderRequest, CommandProviderRequestId,
        CommandProviderResponse, CommandProviderSource, CommandProviderState,
    };

    #[test]
    fn provider_request_records_query_and_active_scopes() {
        let request = CommandProviderRequest::new("open")
            .request_id(CommandProviderRequestId::new(7))
            .active_scopes(["global", "workspace"]);

        assert_eq!(
            request.request_id_ref(),
            Some(CommandProviderRequestId::new(7))
        );
        assert_eq!(request.query(), "open");
        assert_eq!(
            request
                .active_scopes_ref()
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["global", "workspace"]
        );
    }

    #[test]
    fn provider_response_records_status_and_sources() {
        let request = CommandProviderRequest::new("open").request_id(3);
        let response = CommandProviderResponse::loading("Indexing")
            .source(CommandProviderSource::new(
                "global",
                "recent-files",
                [CommandContribution::new(CommandDescriptor::new(
                    "file.open_recent",
                    "Open Recent File",
                ))],
            ))
            .for_request(&request);

        assert_eq!(
            response.request_id_ref(),
            Some(CommandProviderRequestId::new(3))
        );
        assert_eq!(response.state(), CommandProviderState::Loading);
        assert_eq!(response.message(), Some("Indexing"));
        assert_eq!(response.sources_ref().len(), 1);
        assert_eq!(response.sources_ref()[0].len(), 1);
        assert_eq!(response.sources_ref()[0].scope_id().as_str(), "global");
        assert_eq!(
            response.sources_ref()[0].source_id().as_str(),
            "recent-files"
        );
    }
}
