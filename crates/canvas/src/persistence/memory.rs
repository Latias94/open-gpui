use super::{
    CanvasCheckpoint, CanvasEncodedLogEntry, CanvasLogEntry, CanvasPersistenceByteStore,
    CanvasPersistenceStore,
};
use std::convert::Infallible;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MemoryCanvasPersistenceStore {
    checkpoint: Option<CanvasCheckpoint>,
    log_entries: Vec<CanvasLogEntry>,
}

impl MemoryCanvasPersistenceStore {
    pub fn checkpoint(&self) -> Option<&CanvasCheckpoint> {
        self.checkpoint.as_ref()
    }

    pub fn log_entries(&self) -> &[CanvasLogEntry] {
        &self.log_entries
    }
}

impl CanvasPersistenceStore for MemoryCanvasPersistenceStore {
    type Error = Infallible;

    fn load_checkpoint(&self) -> Result<Option<CanvasCheckpoint>, Self::Error> {
        Ok(self.checkpoint.clone())
    }

    fn save_checkpoint(&mut self, checkpoint: CanvasCheckpoint) -> Result<(), Self::Error> {
        self.checkpoint = Some(checkpoint);
        Ok(())
    }

    fn append_log_entry(&mut self, entry: CanvasLogEntry) -> Result<(), Self::Error> {
        self.log_entries.push(entry);
        Ok(())
    }

    fn load_log_entries(&self, after_sequence: u64) -> Result<Vec<CanvasLogEntry>, Self::Error> {
        Ok(self
            .log_entries
            .iter()
            .filter(|entry| entry.sequence > after_sequence)
            .cloned()
            .collect())
    }

    fn compact_log_entries(&mut self, through_sequence: u64) -> Result<(), Self::Error> {
        self.log_entries
            .retain(|entry| entry.sequence > through_sequence);
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MemoryCanvasPersistenceByteStore {
    checkpoint: Option<Vec<u8>>,
    log_entries: Vec<CanvasEncodedLogEntry>,
}

impl MemoryCanvasPersistenceByteStore {
    pub fn checkpoint_bytes(&self) -> Option<&[u8]> {
        self.checkpoint.as_deref()
    }

    pub fn encoded_log_entries(&self) -> &[CanvasEncodedLogEntry] {
        &self.log_entries
    }
}

impl CanvasPersistenceByteStore for MemoryCanvasPersistenceByteStore {
    type Error = Infallible;

    fn load_checkpoint_bytes(&self) -> Result<Option<Vec<u8>>, Self::Error> {
        Ok(self.checkpoint.clone())
    }

    fn save_checkpoint_bytes(&mut self, bytes: Vec<u8>) -> Result<(), Self::Error> {
        self.checkpoint = Some(bytes);
        Ok(())
    }

    fn append_log_entry_bytes(&mut self, sequence: u64, bytes: Vec<u8>) -> Result<(), Self::Error> {
        self.log_entries
            .push(CanvasEncodedLogEntry::new(sequence, bytes));
        Ok(())
    }

    fn load_log_entry_bytes(
        &self,
        after_sequence: u64,
    ) -> Result<Vec<CanvasEncodedLogEntry>, Self::Error> {
        Ok(self
            .log_entries
            .iter()
            .filter(|entry| entry.sequence > after_sequence)
            .cloned()
            .collect())
    }

    fn compact_log_entry_bytes(&mut self, through_sequence: u64) -> Result<(), Self::Error> {
        self.log_entries
            .retain(|entry| entry.sequence > through_sequence);
        Ok(())
    }
}
