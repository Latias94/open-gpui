//! Provider refresh controller for command palette query pipelines.

use crate::{
    CommandCenter, CommandProviderApplyOutcome, CommandProviderId, CommandProviderRequest,
    CommandProviderResponse, CommandProviderStatus, CommandRegistryError, CommandRegistrySnapshot,
};

/// Reusable runtime-neutral controller for one provider-backed command palette query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandProviderRefreshController {
    provider_id: CommandProviderId,
    query: String,
    current_request: Option<CommandProviderRequest>,
    loading_message: Option<String>,
}

impl CommandProviderRefreshController {
    /// Creates a refresh controller for one provider id.
    pub fn new(provider_id: impl Into<CommandProviderId>) -> Self {
        Self {
            provider_id: provider_id.into(),
            query: String::new(),
            current_request: None,
            loading_message: None,
        }
    }

    /// Applies a loading response with this message whenever a new query starts.
    pub fn with_loading_message(mut self, message: impl Into<String>) -> Self {
        let message = message.into();
        self.loading_message = (!message.is_empty()).then_some(message);
        self
    }

    /// Returns the provider id this controller refreshes.
    pub const fn provider_id(&self) -> &CommandProviderId {
        &self.provider_id
    }

    /// Returns the latest query owned by this controller.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns the latest provider request, when a query has started.
    pub const fn current_request(&self) -> Option<&CommandProviderRequest> {
        self.current_request.as_ref()
    }

    /// Starts a new request when the query changes and returns a registry snapshot projection.
    pub fn set_query(
        &mut self,
        center: &mut CommandCenter,
        query: impl Into<String>,
    ) -> Result<CommandProviderRefreshProjection, CommandRegistryError> {
        let query = query.into();
        let query_changed = self.current_request.is_none() || self.query != query;
        if !query_changed {
            return Ok(self.projection(center, false, None));
        }

        self.query = query;
        let request = center.begin_provider_request(self.provider_id.clone(), self.query.as_str());
        self.current_request = Some(request.clone());

        let outcome = match self.loading_message.as_ref() {
            Some(message) => Some(center.apply_provider_response_for_request(
                self.provider_id.clone(),
                &request,
                CommandProviderResponse::loading(message.clone()),
            )?),
            None => None,
        };

        Ok(self.projection(center, true, outcome))
    }

    /// Refreshes a registered synchronous provider only when the query changes.
    pub fn refresh_provider(
        &mut self,
        center: &mut CommandCenter,
        query: impl Into<String>,
    ) -> Option<Result<CommandProviderRefreshProjection, CommandRegistryError>> {
        let projection = match self.set_query(center, query) {
            Ok(projection) => projection,
            Err(error) => return Some(Err(error)),
        };
        if !projection.query_changed() {
            return Some(Ok(projection));
        }

        let request = self.current_request.clone()?;
        let response = center.provider_response_for_request(self.provider_id.clone(), &request)?;
        let outcome = match center.apply_provider_response_for_request(
            self.provider_id.clone(),
            &request,
            response,
        ) {
            Ok(outcome) => outcome,
            Err(error) => return Some(Err(error)),
        };
        Some(Ok(self.projection(center, true, Some(outcome))))
    }

    /// Applies an externally produced provider response for a captured request.
    pub fn apply_response(
        &mut self,
        center: &mut CommandCenter,
        request: &CommandProviderRequest,
        response: CommandProviderResponse,
    ) -> Result<CommandProviderRefreshProjection, CommandRegistryError> {
        let outcome = center.apply_provider_response_for_request(
            self.provider_id.clone(),
            request,
            response,
        )?;
        Ok(self.projection(center, false, Some(outcome)))
    }

    /// Projects the current center state for this controller's query.
    pub fn snapshot(&self, center: &CommandCenter) -> CommandProviderRefreshProjection {
        self.projection(center, false, None)
    }

    fn projection(
        &self,
        center: &CommandCenter,
        query_changed: bool,
        outcome: Option<CommandProviderApplyOutcome>,
    ) -> CommandProviderRefreshProjection {
        CommandProviderRefreshProjection {
            provider_id: self.provider_id.clone(),
            query: self.query.clone(),
            request: self.current_request.clone(),
            query_changed,
            outcome,
            provider_status: center.provider_status(self.provider_id.clone()).cloned(),
            snapshot: center.search_snapshot(self.query.as_str()),
        }
    }
}

/// Snapshot of a provider refresh controller after a query or response step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandProviderRefreshProjection {
    provider_id: CommandProviderId,
    query: String,
    request: Option<CommandProviderRequest>,
    query_changed: bool,
    outcome: Option<CommandProviderApplyOutcome>,
    provider_status: Option<CommandProviderStatus>,
    snapshot: CommandRegistrySnapshot,
}

impl CommandProviderRefreshProjection {
    /// Returns the provider id projected by this controller.
    pub const fn provider_id(&self) -> &CommandProviderId {
        &self.provider_id
    }

    /// Returns the query used for this projection.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns the request active for this projection.
    pub const fn request(&self) -> Option<&CommandProviderRequest> {
        self.request.as_ref()
    }

    /// Returns whether this projection started a new provider request.
    pub const fn query_changed(&self) -> bool {
        self.query_changed
    }

    /// Returns the provider application outcome from this step, if one ran.
    pub const fn outcome(&self) -> Option<&CommandProviderApplyOutcome> {
        self.outcome.as_ref()
    }

    /// Returns the latest provider status retained by the command center.
    pub const fn provider_status(&self) -> Option<&CommandProviderStatus> {
        self.provider_status.as_ref()
    }

    /// Returns the center search snapshot for this projection's query.
    pub const fn snapshot(&self) -> &CommandRegistrySnapshot {
        &self.snapshot
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        CommandContribution, CommandDescriptor, CommandProviderApplyOutcome,
        CommandProviderRefreshController, CommandProviderResponse, CommandProviderSource,
        CommandProviderState,
    };

    use super::*;

    #[test]
    fn refresh_controller_runs_registered_provider_for_changed_query() {
        let mut center = CommandCenter::new("center-v1");
        center.register_provider("recent", |request: &crate::CommandProviderRequest| {
            CommandProviderResponse::ready().source(CommandProviderSource::new(
                "global",
                "recent-source",
                [CommandContribution::new(CommandDescriptor::new(
                    format!("recent.{}", request.query()),
                    format!("Recent {}", request.query()),
                ))],
            ))
        });
        let mut controller =
            CommandProviderRefreshController::new("recent").with_loading_message("Searching");

        let projection = controller
            .refresh_provider(&mut center, "alpha")
            .expect("provider should be registered")
            .unwrap();

        assert!(projection.query_changed());
        assert_eq!(projection.query(), "alpha");
        assert_eq!(
            projection
                .provider_status()
                .and_then(CommandProviderStatus::request_id)
                .map(|request_id| request_id.get()),
            Some(1)
        );
        assert_eq!(
            projection
                .provider_status()
                .map(CommandProviderStatus::state),
            Some(CommandProviderState::Ready)
        );
        assert!(projection.snapshot().descriptor("recent.alpha").is_some());
    }

    #[test]
    fn refresh_controller_does_not_restart_unchanged_query() {
        let mut center = CommandCenter::new("center-v1");
        let mut controller = CommandProviderRefreshController::new("recent");

        let first = controller.set_query(&mut center, "alpha").unwrap();
        let second = controller.set_query(&mut center, "alpha").unwrap();

        assert!(first.query_changed());
        assert!(!second.query_changed());
        assert_eq!(
            first
                .request()
                .and_then(CommandProviderRequest::request_id_ref),
            second
                .request()
                .and_then(CommandProviderRequest::request_id_ref)
        );
    }

    #[test]
    fn refresh_controller_ignores_stale_async_response_and_projects_current_snapshot() {
        let mut center = CommandCenter::new("center-v1");
        let mut controller =
            CommandProviderRefreshController::new("search").with_loading_message("Searching");

        let alpha = controller.set_query(&mut center, "alpha").unwrap();
        let alpha_request = alpha.request().expect("alpha request").clone();
        let beta = controller.set_query(&mut center, "beta").unwrap();
        let beta_request = beta.request().expect("beta request").clone();

        let stale = controller
            .apply_response(
                &mut center,
                &alpha_request,
                CommandProviderResponse::ready().source(CommandProviderSource::new(
                    "global",
                    "search-source",
                    [CommandContribution::new(CommandDescriptor::new(
                        "search.alpha",
                        "Search Alpha",
                    ))],
                )),
            )
            .unwrap();

        assert!(matches!(
            stale.outcome(),
            Some(CommandProviderApplyOutcome::Stale(_))
        ));
        assert_eq!(stale.query(), "beta");
        assert!(stale.snapshot().descriptor("search.alpha").is_none());

        let ready = controller
            .apply_response(
                &mut center,
                &beta_request,
                CommandProviderResponse::ready().source(CommandProviderSource::new(
                    "global",
                    "search-source",
                    [CommandContribution::new(CommandDescriptor::new(
                        "search.beta",
                        "Search Beta",
                    ))],
                )),
            )
            .unwrap();

        assert!(
            ready
                .outcome()
                .is_some_and(CommandProviderApplyOutcome::applied)
        );
        assert_eq!(
            ready
                .provider_status()
                .and_then(CommandProviderStatus::request_id),
            beta_request.request_id_ref()
        );
        assert!(ready.snapshot().descriptor("search.beta").is_some());
    }
}
