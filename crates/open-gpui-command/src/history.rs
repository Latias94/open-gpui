//! In-memory command usage and query history.

use std::collections::{BTreeMap, VecDeque};

use crate::CommandRegistrySnapshot;

/// One command history entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandHistoryEntry {
    command_id: String,
    query: String,
    sequence: u64,
}

impl CommandHistoryEntry {
    /// Returns the command id.
    pub fn command_id(&self) -> &str {
        &self.command_id
    }

    /// Returns the query text active when the command was used.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Returns the monotonic in-memory sequence.
    pub const fn sequence(&self) -> u64 {
        self.sequence
    }
}

/// Command usage history API used by dispatch adapters and ranking projections.
pub trait CommandUsageHistory {
    /// Records one command usage.
    fn record_usage(&mut self, command_id: &str, query: &str);

    /// Returns how many times a command id has been recorded.
    fn usage_count(&self, command_id: &str) -> usize;

    /// Returns the most recent sequence for a command id.
    fn last_used_sequence(&self, command_id: &str) -> Option<u64>;
}

/// Memory-backed command usage and query history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryCommandHistory {
    capacity: usize,
    next_sequence: u64,
    entries: VecDeque<CommandHistoryEntry>,
    queries: VecDeque<String>,
    query_cursor: Option<usize>,
}

impl Default for MemoryCommandHistory {
    fn default() -> Self {
        Self::new(128)
    }
}

impl MemoryCommandHistory {
    /// Creates an in-memory history with a bounded entry capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            next_sequence: 1,
            entries: VecDeque::new(),
            queries: VecDeque::new(),
            query_cursor: None,
        }
    }

    /// Returns recorded entries from oldest to newest.
    pub fn entries(&self) -> &VecDeque<CommandHistoryEntry> {
        &self.entries
    }

    /// Returns recent unique command ids from newest to oldest.
    pub fn recent_command_ids(&self) -> Vec<String> {
        let mut seen = BTreeMap::<String, ()>::new();
        let mut recent = Vec::new();
        for entry in self.entries.iter().rev() {
            if seen.insert(entry.command_id.clone(), ()).is_none() {
                recent.push(entry.command_id.clone());
            }
        }
        recent
    }

    /// Returns the newest query, when one has been recorded.
    pub fn last_query(&self) -> Option<&str> {
        self.queries.back().map(String::as_str).or_else(|| {
            self.entries
                .iter()
                .rev()
                .find(|entry| !entry.query.is_empty())
                .map(CommandHistoryEntry::query)
        })
    }

    /// Records one command query without requiring command dispatch.
    pub fn record_query(&mut self, query: &str) {
        if query.is_empty() {
            return;
        }
        if let Some(position) = self.queries.iter().position(|candidate| candidate == query) {
            self.queries.remove(position);
        }
        if self.queries.len() == self.capacity {
            self.queries.pop_front();
        }
        self.queries.push_back(query.to_owned());
        self.query_cursor = None;
    }

    /// Returns recent unique queries from newest to oldest.
    pub fn recent_queries(&self) -> Vec<String> {
        let mut seen = BTreeMap::<String, ()>::new();
        let mut recent = Vec::new();
        for query in self.queries.iter().rev() {
            if seen.insert(query.clone(), ()).is_none() {
                recent.push(query.clone());
            }
        }
        recent
    }

    /// Moves to the previous query matching `prefix`.
    ///
    /// The returned value is owned so UI runtimes can update controlled query state without
    /// borrowing this history store.
    pub fn previous_query(&mut self, prefix: &str) -> Option<String> {
        self.navigate_query(prefix, QueryNavigation::Previous)
    }

    /// Moves to the next query matching `prefix`.
    ///
    /// Returns `None` at the newest matching query boundary.
    pub fn next_query(&mut self, prefix: &str) -> Option<String> {
        self.navigate_query(prefix, QueryNavigation::Next)
    }

    /// Resets query-history navigation.
    pub fn reset_query_navigation(&mut self) {
        self.query_cursor = None;
    }

    /// Ranks a registry snapshot using in-memory usage and recency hints.
    pub fn rank_registry_snapshot(
        &self,
        snapshot: &CommandRegistrySnapshot,
    ) -> CommandRegistrySnapshot {
        let mut contributions = snapshot
            .contributions()
            .iter()
            .cloned()
            .enumerate()
            .collect::<Vec<_>>();
        contributions.sort_by(|(left_index, left), (right_index, right)| {
            let left_score = self.score(left.descriptor().id());
            let right_score = self.score(right.descriptor().id());
            right_score
                .cmp(&left_score)
                .then_with(|| left_index.cmp(right_index))
        });
        CommandRegistrySnapshot::new(
            snapshot.revision(),
            contributions
                .into_iter()
                .map(|(_, contribution)| contribution)
                .collect::<Vec<_>>(),
        )
    }

    fn navigate_query(&mut self, prefix: &str, direction: QueryNavigation) -> Option<String> {
        let matching_indices = self
            .queries
            .iter()
            .enumerate()
            .filter_map(|(index, query)| {
                (prefix.is_empty() || query.starts_with(prefix)).then_some(index)
            })
            .collect::<Vec<_>>();
        if matching_indices.is_empty() {
            self.query_cursor = None;
            return None;
        }

        let next_match = match (self.query_cursor, direction) {
            (None, QueryNavigation::Previous) => matching_indices.last().copied(),
            (None, QueryNavigation::Next) => None,
            (Some(cursor), QueryNavigation::Previous) => {
                let position = matching_indices
                    .iter()
                    .position(|index| *index == cursor)
                    .unwrap_or(matching_indices.len());
                Some(matching_indices[position.saturating_sub(1)])
            }
            (Some(cursor), QueryNavigation::Next) => {
                let position = matching_indices.iter().position(|index| *index == cursor)?;
                matching_indices.get(position + 1).copied()
            }
        }?;

        self.query_cursor = Some(next_match);
        self.queries.get(next_match).cloned()
    }

    fn score(&self, command_id: &str) -> u64 {
        let usage = self.usage_count(command_id) as u64;
        let recency = self.last_used_sequence(command_id).unwrap_or_default();
        usage.saturating_mul(1_000_000).saturating_add(recency)
    }
}

impl CommandUsageHistory for MemoryCommandHistory {
    fn record_usage(&mut self, command_id: &str, query: &str) {
        if command_id.is_empty() {
            return;
        }
        if self.entries.len() == self.capacity {
            self.entries.pop_front();
        }
        self.record_query(query);
        self.entries.push_back(CommandHistoryEntry {
            command_id: command_id.to_owned(),
            query: query.to_owned(),
            sequence: self.next_sequence,
        });
        self.next_sequence = self.next_sequence.saturating_add(1);
    }

    fn usage_count(&self, command_id: &str) -> usize {
        self.entries
            .iter()
            .filter(|entry| entry.command_id == command_id)
            .count()
    }

    fn last_used_sequence(&self, command_id: &str) -> Option<u64> {
        self.entries
            .iter()
            .rev()
            .find(|entry| entry.command_id == command_id)
            .map(CommandHistoryEntry::sequence)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QueryNavigation {
    Previous,
    Next,
}

impl<T> CommandUsageHistory for &mut T
where
    T: CommandUsageHistory + ?Sized,
{
    fn record_usage(&mut self, command_id: &str, query: &str) {
        (**self).record_usage(command_id, query);
    }

    fn usage_count(&self, command_id: &str) -> usize {
        (**self).usage_count(command_id)
    }

    fn last_used_sequence(&self, command_id: &str) -> Option<u64> {
        (**self).last_used_sequence(command_id)
    }
}

#[cfg(test)]
mod tests {
    use crate::{CommandDescriptor, CommandRegistry, CommandUsageHistory, MemoryCommandHistory};

    #[test]
    fn memory_history_records_recent_usage_and_caps_entries() {
        let mut history = MemoryCommandHistory::new(2);
        history.record_usage("file.open", "open");
        history.record_usage("file.save", "save");
        history.record_usage("file.open", "again");

        assert_eq!(history.entries().len(), 2);
        assert_eq!(history.usage_count("file.open"), 1);
        assert_eq!(history.recent_command_ids(), ["file.open", "file.save"]);
        assert_eq!(history.last_query(), Some("again"));
        assert_eq!(history.recent_queries(), ["again", "save"]);
    }

    #[test]
    fn memory_history_can_rank_registry_snapshots() {
        let mut registry = CommandRegistry::new("commands:history");
        registry
            .register(CommandDescriptor::new("file.open", "Open File"))
            .unwrap();
        registry
            .register(CommandDescriptor::new("file.save", "Save File"))
            .unwrap();
        let mut history = MemoryCommandHistory::default();
        history.record_usage("file.save", "save");
        history.record_usage("file.save", "save");

        let ranked = history.rank_registry_snapshot(&registry.snapshot());
        let ids = ranked
            .descriptors()
            .map(|descriptor| descriptor.id().to_owned())
            .collect::<Vec<_>>();

        assert_eq!(ids, ["file.save", "file.open"]);
    }

    #[test]
    fn memory_history_navigates_recent_queries_with_prefix() {
        let mut history = MemoryCommandHistory::new(4);
        history.record_query("open file");
        history.record_query("save file");
        history.record_query("open settings");

        assert_eq!(history.previous_query("open"), Some("open settings".into()));
        assert_eq!(history.previous_query("open"), Some("open file".into()));
        assert_eq!(history.previous_query("open"), Some("open file".into()));
        assert_eq!(history.next_query("open"), Some("open settings".into()));
        assert_eq!(history.next_query("open"), None);

        history.reset_query_navigation();
        assert_eq!(history.previous_query("save"), Some("save file".into()));
    }

    #[test]
    fn memory_history_promotes_duplicate_queries() {
        let mut history = MemoryCommandHistory::new(4);
        history.record_query("open file");
        history.record_query("save file");
        history.record_query("open settings");
        history.record_query("open file");

        assert_eq!(
            history.recent_queries(),
            ["open file", "open settings", "save file"]
        );
        assert_eq!(history.previous_query("open"), Some("open file".into()));
        assert_eq!(history.previous_query("open"), Some("open settings".into()));
    }
}
