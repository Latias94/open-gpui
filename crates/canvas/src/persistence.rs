pub const CANVAS_REDB_STORE_FEATURE: &str = "redb-store";
pub const CANVAS_LORO_CRDT_FEATURE: &str = "loro-crdt";
pub const CANVAS_RKYV_SNAPSHOT_FEATURE: &str = "rkyv-snapshot";

mod byte_store;
mod codec;
mod memory;
mod store;
#[cfg(test)]
mod tests;

pub use crate::format::{
    CANVAS_DOCUMENT_FORMAT_VERSION, CANVAS_DOCUMENT_MIN_SUPPORTED_FORMAT_VERSION,
    CANVAS_SNAPSHOT_MIGRATIONS, CanvasSnapshotMigration, default_document_format_version,
    migrate_canvas_snapshot, validate_canvas_document_format_version,
};
pub use byte_store::{
    CanvasEncodedLogEntry, CanvasPersistenceByteStore, CanvasPersistenceByteStoreAdapter,
    CanvasPersistenceByteStoreError,
};
pub use codec::{
    CANVAS_PERSISTENCE_CODEC_VERSION, CanvasJsonPersistenceCodec, CanvasPersistenceCodec,
    CanvasPersistenceCodecError, CanvasPersistenceEnvelope, CanvasPersistenceRecord,
    CanvasPersistenceRecordKind,
};
pub use memory::{MemoryCanvasPersistenceByteStore, MemoryCanvasPersistenceStore};
pub use store::{
    CanvasCheckpoint, CanvasLogEntry, CanvasLogEntryKind, CanvasPersistenceCursor,
    CanvasPersistenceError, CanvasPersistenceStore, CanvasPersistentToolRegistryError,
    CanvasReplayError, apply_persistent_store_transaction, apply_persistent_tool_intent,
    apply_persistent_tool_intents, apply_persistent_transaction, handle_persistent_event,
    handle_persistent_event_with_custom_tool, handle_persistent_event_with_tool_registry,
    load_canvas_document, load_canvas_persistence_cursor, redo_persistent_store_transaction,
    redo_persistent_transaction, replay_canvas_log, save_canvas_checkpoint,
    save_canvas_store_checkpoint, undo_persistent_store_transaction, undo_persistent_transaction,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanvasPersistenceAdapter {
    MemoryStore,
    RedbStore,
    LoroCrdt,
    RkyvSnapshot,
}

impl CanvasPersistenceAdapter {
    pub fn feature_name(self) -> Option<&'static str> {
        match self {
            Self::MemoryStore => None,
            Self::RedbStore => Some(CANVAS_REDB_STORE_FEATURE),
            Self::LoroCrdt => Some(CANVAS_LORO_CRDT_FEATURE),
            Self::RkyvSnapshot => Some(CANVAS_RKYV_SNAPSHOT_FEATURE),
        }
    }

    pub fn feature_enabled(self) -> bool {
        match self {
            Self::MemoryStore => true,
            Self::RedbStore => cfg!(feature = "redb-store"),
            Self::LoroCrdt => cfg!(feature = "loro-crdt"),
            Self::RkyvSnapshot => cfg!(feature = "rkyv-snapshot"),
        }
    }

    pub fn implemented(self) -> bool {
        matches!(self, Self::MemoryStore)
    }

    pub fn status(self) -> CanvasPersistenceAdapterStatus {
        CanvasPersistenceAdapterStatus {
            adapter: self,
            feature_name: self.feature_name(),
            feature_enabled: self.feature_enabled(),
            implemented: self.implemented(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanvasPersistenceAdapterStatus {
    pub adapter: CanvasPersistenceAdapter,
    pub feature_name: Option<&'static str>,
    pub feature_enabled: bool,
    pub implemented: bool,
}

pub const CANVAS_PERSISTENCE_ADAPTERS: &[CanvasPersistenceAdapter] = &[
    CanvasPersistenceAdapter::MemoryStore,
    CanvasPersistenceAdapter::RedbStore,
    CanvasPersistenceAdapter::LoroCrdt,
    CanvasPersistenceAdapter::RkyvSnapshot,
];

pub fn canvas_persistence_adapter_statuses() -> [CanvasPersistenceAdapterStatus; 4] {
    [
        CanvasPersistenceAdapter::MemoryStore.status(),
        CanvasPersistenceAdapter::RedbStore.status(),
        CanvasPersistenceAdapter::LoroCrdt.status(),
        CanvasPersistenceAdapter::RkyvSnapshot.status(),
    ]
}
