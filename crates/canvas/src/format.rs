use crate::{CanvasSnapshot, DocumentError};

pub const CANVAS_DOCUMENT_FORMAT_VERSION: u32 = 1;
pub const CANVAS_DOCUMENT_MIN_SUPPORTED_FORMAT_VERSION: u32 = 1;
pub const CANVAS_SNAPSHOT_MIGRATIONS: &[CanvasSnapshotMigration] = &[];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanvasSnapshotMigration {
    pub from_version: u32,
    pub to_version: u32,
}

pub fn default_document_format_version() -> u32 {
    CANVAS_DOCUMENT_FORMAT_VERSION
}

pub fn validate_canvas_document_format_version(format_version: u32) -> Result<(), DocumentError> {
    if format_version < CANVAS_DOCUMENT_MIN_SUPPORTED_FORMAT_VERSION
        || format_version > CANVAS_DOCUMENT_FORMAT_VERSION
    {
        return Err(DocumentError::UnsupportedFormatVersion {
            expected: CANVAS_DOCUMENT_FORMAT_VERSION,
            found: format_version,
        });
    }

    Ok(())
}

pub fn migrate_canvas_snapshot(
    mut snapshot: CanvasSnapshot,
) -> Result<CanvasSnapshot, DocumentError> {
    validate_canvas_document_format_version(snapshot.format_version)?;

    for migration in CANVAS_SNAPSHOT_MIGRATIONS {
        if snapshot.format_version == migration.from_version {
            snapshot.format_version = migration.to_version;
        }
    }

    if snapshot.format_version != CANVAS_DOCUMENT_FORMAT_VERSION {
        return Err(DocumentError::UnsupportedFormatVersion {
            expected: CANVAS_DOCUMENT_FORMAT_VERSION,
            found: snapshot.format_version,
        });
    }

    Ok(snapshot)
}
