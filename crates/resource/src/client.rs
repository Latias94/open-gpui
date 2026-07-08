use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::{
    FetchTicket, MutationSnapshot, MutationStatus, MutationTicket, QueryKey, ResourceError,
    ResourceRedactionPolicy, ResourceSnapshot, ResourceStatus,
};

/// Handle returned by an active resource observer subscription.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ObserverHandle {
    key: QueryKey,
    id: u64,
}

/// Result of invalidating a query.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InvalidationOutcome {
    /// True when active observers should trigger a refetch.
    pub refetch_requested: bool,
}

/// Renderer-neutral query and mutation state owner.
#[derive(Clone, Debug, Default)]
pub struct ResourceClient {
    queries: BTreeMap<QueryKey, QueryEntry>,
    mutations: BTreeMap<String, MutationEntry>,
}

impl ResourceClient {
    /// Ensures a query entry exists.
    pub fn ensure_query(&mut self, key: QueryKey) {
        self.queries
            .entry(key.clone())
            .or_insert_with(|| QueryEntry::new(key));
    }

    /// Subscribes one observer to a query.
    pub fn subscribe(&mut self, key: QueryKey) -> Result<ObserverHandle, ResourceError> {
        self.ensure_query(key.clone());
        let entry = self.query_mut(&key)?;
        entry.next_observer_id += 1;
        let id = entry.next_observer_id;
        entry.observers.insert(id);
        Ok(ObserverHandle { key, id })
    }

    /// Unsubscribes one observer.
    pub fn unsubscribe(&mut self, handle: ObserverHandle) -> Result<(), ResourceError> {
        let entry = self.query_mut(&handle.key)?;
        if !entry.observers.remove(&handle.id) {
            return Err(ResourceError::UnknownObserver);
        }
        Ok(())
    }

    /// Begins a fetch generation.
    pub fn begin_fetch(&mut self, key: &QueryKey) -> Result<FetchTicket, ResourceError> {
        self.ensure_query(key.clone());
        let entry = self.query_mut(key)?;
        entry.fetch_generation += 1;
        entry.fetch_attempts += 1;
        entry.status = if entry.data.is_some() {
            ResourceStatus::Refetching
        } else {
            ResourceStatus::Loading
        };
        entry.error = None;
        Ok(FetchTicket {
            key: key.clone(),
            generation: entry.fetch_generation,
        })
    }

    /// Completes a fetch generation with data.
    pub fn complete_fetch_success(&mut self, ticket: FetchTicket, data: Value) -> bool {
        self.queries
            .get_mut(&ticket.key)
            .is_some_and(|entry| entry.complete_fetch_success(ticket.generation, data))
    }

    /// Completes a fetch generation with an error.
    pub fn complete_fetch_error(&mut self, ticket: FetchTicket, error: impl Into<String>) -> bool {
        self.queries
            .get_mut(&ticket.key)
            .is_some_and(|entry| entry.complete_fetch_error(ticket.generation, error.into()))
    }

    /// Invalidates a query.
    pub fn invalidate(&mut self, key: &QueryKey) -> Result<InvalidationOutcome, ResourceError> {
        let entry = self.query_mut(key)?;
        let refetch_requested = !entry.observers.is_empty();
        entry.status = if refetch_requested {
            ResourceStatus::Refetching
        } else {
            ResourceStatus::Stale
        };
        Ok(InvalidationOutcome { refetch_requested })
    }

    /// Returns a redaction-aware query snapshot.
    pub fn snapshot(
        &self,
        key: &QueryKey,
        redaction: ResourceRedactionPolicy,
    ) -> Result<ResourceSnapshot, ResourceError> {
        let entry = self.query(key)?;
        Ok(ResourceSnapshot {
            key: key.clone(),
            status: entry.status.clone(),
            data: entry.data.clone().map(|data| redaction.apply(data)),
            error: entry.error.clone(),
            observer_count: entry.observers.len(),
            fetch_attempts: entry.fetch_attempts,
        })
    }

    /// Begins a mutation generation.
    pub fn begin_mutation(
        &mut self,
        id: impl Into<String>,
    ) -> Result<MutationTicket, ResourceError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(ResourceError::EmptyMutationId);
        }
        let entry = self
            .mutations
            .entry(id.clone())
            .or_insert_with(|| MutationEntry::new(id.clone()));
        entry.generation += 1;
        entry.status = MutationStatus::Pending;
        entry.error = None;
        Ok(MutationTicket {
            id,
            generation: entry.generation,
        })
    }

    /// Completes a mutation generation with success and invalidates configured query keys.
    pub fn complete_mutation_success(
        &mut self,
        ticket: MutationTicket,
        data: Option<Value>,
        invalidate_keys: impl IntoIterator<Item = QueryKey>,
    ) -> bool {
        let Some(entry) = self.mutations.get_mut(&ticket.id) else {
            return false;
        };
        if entry.generation != ticket.generation {
            return false;
        }
        entry.status = MutationStatus::Success;
        entry.data = data;
        entry.error = None;
        for key in invalidate_keys {
            let _ = self.invalidate(&key);
        }
        true
    }

    /// Completes a mutation generation with an error.
    pub fn complete_mutation_error(
        &mut self,
        ticket: MutationTicket,
        error: impl Into<String>,
    ) -> bool {
        self.mutations
            .get_mut(&ticket.id)
            .is_some_and(|entry| entry.complete_error(ticket.generation, error.into()))
    }

    /// Returns a redaction-aware mutation snapshot.
    pub fn mutation_snapshot(
        &self,
        id: &str,
        redaction: ResourceRedactionPolicy,
    ) -> Result<MutationSnapshot, ResourceError> {
        let entry = self
            .mutations
            .get(id)
            .ok_or_else(|| ResourceError::UnknownMutation(id.to_owned()))?;
        Ok(MutationSnapshot {
            id: id.to_owned(),
            status: entry.status.clone(),
            data: entry.data.clone().map(|data| redaction.apply(data)),
            error: entry.error.clone(),
        })
    }

    fn query(&self, key: &QueryKey) -> Result<&QueryEntry, ResourceError> {
        self.queries
            .get(key)
            .ok_or_else(|| ResourceError::UnknownQuery(key.clone()))
    }

    fn query_mut(&mut self, key: &QueryKey) -> Result<&mut QueryEntry, ResourceError> {
        self.queries
            .get_mut(key)
            .ok_or_else(|| ResourceError::UnknownQuery(key.clone()))
    }
}

#[derive(Clone, Debug)]
struct QueryEntry {
    status: ResourceStatus,
    data: Option<Value>,
    error: Option<String>,
    observers: BTreeSet<u64>,
    next_observer_id: u64,
    fetch_generation: u64,
    fetch_attempts: u32,
}

impl QueryEntry {
    fn new(_key: QueryKey) -> Self {
        Self {
            status: ResourceStatus::Idle,
            data: None,
            error: None,
            observers: BTreeSet::new(),
            next_observer_id: 0,
            fetch_generation: 0,
            fetch_attempts: 0,
        }
    }

    fn complete_fetch_success(&mut self, generation: u64, data: Value) -> bool {
        if self.fetch_generation != generation {
            return false;
        }
        self.status = ResourceStatus::Success;
        self.data = Some(data);
        self.error = None;
        true
    }

    fn complete_fetch_error(&mut self, generation: u64, error: String) -> bool {
        if self.fetch_generation != generation {
            return false;
        }
        self.status = if self.data.is_some() {
            ResourceStatus::Stale
        } else {
            ResourceStatus::Error
        };
        self.error = Some(error);
        true
    }
}

#[derive(Clone, Debug)]
struct MutationEntry {
    status: MutationStatus,
    data: Option<Value>,
    error: Option<String>,
    generation: u64,
}

impl MutationEntry {
    fn new(_id: String) -> Self {
        Self {
            status: MutationStatus::Idle,
            data: None,
            error: None,
            generation: 0,
        }
    }

    fn complete_error(&mut self, generation: u64, error: String) -> bool {
        if self.generation != generation {
            return false;
        }
        self.status = MutationStatus::Error;
        self.error = Some(error);
        true
    }
}
