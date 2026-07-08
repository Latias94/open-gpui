use std::collections::BTreeMap;

use crate::{
    DevtoolsProbe, ProbeId, ProbeSnapshotError, SnapshotCollection, SnapshotDiagnostic,
    SnapshotKind, SnapshotProbe, SnapshotProbeSnapshot,
};

/// Registry of read-only devtools probes.
#[derive(Default)]
pub struct DevtoolsRegistry {
    probes: BTreeMap<ProbeId, Box<dyn DevtoolsProbe>>,
}

impl DevtoolsRegistry {
    /// Registers a probe.
    pub fn register(
        &mut self,
        probe: impl DevtoolsProbe + 'static,
    ) -> Result<(), DevtoolsRegistryError> {
        let id = probe.id().clone();
        if self.probes.contains_key(&id) {
            return Err(DevtoolsRegistryError::DuplicateProbe(id));
        }
        self.probes.insert(id, Box::new(probe));
        Ok(())
    }

    /// Registers a closure-backed snapshot probe.
    pub fn register_snapshot_probe<F>(
        &mut self,
        id: impl Into<String>,
        kind: SnapshotKind,
        snapshot: F,
    ) -> Result<(), DevtoolsRegistryError>
    where
        F: Fn() -> Result<SnapshotProbeSnapshot, ProbeSnapshotError> + Send + Sync + 'static,
    {
        let probe = SnapshotProbe::new(id, kind, snapshot)
            .map_err(DevtoolsRegistryError::InvalidProbeId)?;
        self.register(probe)
    }

    /// Removes a probe by id.
    pub fn unregister(&mut self, id: &ProbeId) -> bool {
        self.probes.remove(id).is_some()
    }

    /// Returns the number of registered probes.
    pub fn len(&self) -> usize {
        self.probes.len()
    }

    /// Returns true when no probes are registered.
    pub fn is_empty(&self) -> bool {
        self.probes.is_empty()
    }

    /// Collects all currently available snapshots.
    pub fn collect(&self) -> SnapshotCollection {
        let mut collection = SnapshotCollection::default();
        for (id, probe) in &self.probes {
            match probe.snapshot() {
                Ok(snapshot) => collection.snapshots.push(snapshot),
                Err(error) => {
                    collection.diagnostics.push(SnapshotDiagnostic {
                        probe_id: id.clone(),
                        message: error.to_string(),
                    });
                }
            }
        }
        collection
    }
}

/// Error returned while registering devtools probes.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DevtoolsRegistryError {
    /// A probe id could not be created.
    #[error("invalid devtools probe id: {0}")]
    InvalidProbeId(ProbeSnapshotError),
    /// A probe with this id is already registered.
    #[error("duplicate devtools probe: {0}")]
    DuplicateProbe(ProbeId),
}
