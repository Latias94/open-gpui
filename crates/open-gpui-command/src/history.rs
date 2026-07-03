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
        self.entries
            .iter()
            .rev()
            .find(|entry| !entry.query.is_empty())
            .map(CommandHistoryEntry::query)
    }

    /// Ranks a registry snapshot using in-memory usage and recency hints.
    pub fn rank_registry_snapshot(
        &self,
        snapshot: &CommandRegistrySnapshot,
    ) -> CommandRegistrySnapshot {
        let mut contributions = snapshot.contributions().to_vec();
        contributions.sort_by(|left, right| {
            let left_score = self.score(left.descriptor().id());
            let right_score = self.score(right.descriptor().id());
            right_score
                .cmp(&left_score)
                .then_with(|| left.descriptor().label().cmp(right.descriptor().label()))
        });
        CommandRegistrySnapshot::new(snapshot.revision(), contributions)
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
}
