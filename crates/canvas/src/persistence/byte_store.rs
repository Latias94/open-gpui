use super::{
    CanvasCheckpoint, CanvasJsonPersistenceCodec, CanvasLogEntry, CanvasPersistenceCodec,
    CanvasPersistenceStore,
};
use std::{error::Error, fmt};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CanvasEncodedLogEntry {
    pub sequence: u64,
    pub bytes: Vec<u8>,
}

impl CanvasEncodedLogEntry {
    pub fn new(sequence: u64, bytes: impl Into<Vec<u8>>) -> Self {
        Self {
            sequence,
            bytes: bytes.into(),
        }
    }
}

pub trait CanvasPersistenceByteStore {
    type Error: fmt::Debug + fmt::Display;

    fn load_checkpoint_bytes(&self) -> Result<Option<Vec<u8>>, Self::Error>;

    fn save_checkpoint_bytes(&mut self, bytes: Vec<u8>) -> Result<(), Self::Error>;

    fn append_log_entry_bytes(&mut self, sequence: u64, bytes: Vec<u8>) -> Result<(), Self::Error>;

    fn load_log_entry_bytes(
        &self,
        after_sequence: u64,
    ) -> Result<Vec<CanvasEncodedLogEntry>, Self::Error>;

    fn compact_log_entry_bytes(&mut self, through_sequence: u64) -> Result<(), Self::Error>;
}

#[derive(Debug, Eq, PartialEq)]
pub enum CanvasPersistenceByteStoreError<StoreError, CodecError> {
    Store(StoreError),
    Codec(CodecError),
    LogSequenceMismatch { key: u64, payload: u64 },
}

impl<StoreError, CodecError> fmt::Display
    for CanvasPersistenceByteStoreError<StoreError, CodecError>
where
    StoreError: fmt::Display,
    CodecError: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(f, "canvas persistence byte store error: {error}"),
            Self::Codec(error) => fmt::Display::fmt(error, f),
            Self::LogSequenceMismatch { key, payload } => write!(
                f,
                "canvas persistence log key sequence `{key}` does not match payload sequence `{payload}`"
            ),
        }
    }
}

impl<StoreError, CodecError> Error for CanvasPersistenceByteStoreError<StoreError, CodecError>
where
    StoreError: fmt::Debug + fmt::Display,
    CodecError: fmt::Debug + fmt::Display,
{
}

#[derive(Clone, Debug)]
pub struct CanvasPersistenceByteStoreAdapter<S, C = CanvasJsonPersistenceCodec> {
    store: S,
    codec: C,
}

impl<S> CanvasPersistenceByteStoreAdapter<S, CanvasJsonPersistenceCodec> {
    pub fn new(store: S) -> Self {
        Self::with_codec(store, CanvasJsonPersistenceCodec)
    }
}

impl<S, C> CanvasPersistenceByteStoreAdapter<S, C> {
    pub fn with_codec(store: S, codec: C) -> Self {
        Self { store, codec }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn store_mut(&mut self) -> &mut S {
        &mut self.store
    }

    pub fn into_store(self) -> S {
        self.store
    }

    pub fn codec(&self) -> &C {
        &self.codec
    }
}

impl<S, C> CanvasPersistenceStore for CanvasPersistenceByteStoreAdapter<S, C>
where
    S: CanvasPersistenceByteStore,
    C: CanvasPersistenceCodec,
{
    type Error = CanvasPersistenceByteStoreError<S::Error, C::Error>;

    fn load_checkpoint(&self) -> Result<Option<CanvasCheckpoint>, Self::Error> {
        self.store
            .load_checkpoint_bytes()
            .map_err(CanvasPersistenceByteStoreError::Store)?
            .map(|bytes| {
                self.codec
                    .decode_checkpoint(&bytes)
                    .map_err(CanvasPersistenceByteStoreError::Codec)
            })
            .transpose()
    }

    fn save_checkpoint(&mut self, checkpoint: CanvasCheckpoint) -> Result<(), Self::Error> {
        let bytes = self
            .codec
            .encode_checkpoint(&checkpoint)
            .map_err(CanvasPersistenceByteStoreError::Codec)?;
        self.store
            .save_checkpoint_bytes(bytes)
            .map_err(CanvasPersistenceByteStoreError::Store)
    }

    fn append_log_entry(&mut self, entry: CanvasLogEntry) -> Result<(), Self::Error> {
        let sequence = entry.sequence();
        let bytes = self
            .codec
            .encode_log_entry(&entry)
            .map_err(CanvasPersistenceByteStoreError::Codec)?;
        self.store
            .append_log_entry_bytes(sequence, bytes)
            .map_err(CanvasPersistenceByteStoreError::Store)
    }

    fn load_log_entries(&self, after_sequence: u64) -> Result<Vec<CanvasLogEntry>, Self::Error> {
        self.store
            .load_log_entry_bytes(after_sequence)
            .map_err(CanvasPersistenceByteStoreError::Store)?
            .into_iter()
            .map(|encoded| {
                let entry = self
                    .codec
                    .decode_log_entry(&encoded.bytes)
                    .map_err(CanvasPersistenceByteStoreError::Codec)?;
                if entry.sequence() != encoded.sequence {
                    return Err(CanvasPersistenceByteStoreError::LogSequenceMismatch {
                        key: encoded.sequence,
                        payload: entry.sequence(),
                    });
                }
                Ok(entry)
            })
            .collect()
    }

    fn compact_log_entries(&mut self, through_sequence: u64) -> Result<(), Self::Error> {
        self.store
            .compact_log_entry_bytes(through_sequence)
            .map_err(CanvasPersistenceByteStoreError::Store)
    }
}
