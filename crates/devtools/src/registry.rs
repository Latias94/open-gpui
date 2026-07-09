use std::collections::BTreeMap;

use crate::{
    CaptureProvider, DevtoolsCapture, DevtoolsCaptureProvider, DevtoolsProbe, DevtoolsTargetTree,
    ProbeId, ProbeSnapshotError, SnapshotCollection, SnapshotDiagnostic, SnapshotKind,
    SnapshotProbe, SnapshotProbeSnapshot,
};

/// Registry of read-only devtools probes and capture providers.
#[derive(Default)]
pub struct DevtoolsRegistry {
    probes: BTreeMap<ProbeId, Box<dyn DevtoolsProbe>>,
    capture_providers: BTreeMap<ProbeId, Box<dyn DevtoolsCaptureProvider>>,
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
        if self.capture_providers.contains_key(&id) {
            return Err(DevtoolsRegistryError::DuplicateCaptureProvider(id));
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

    /// Registers a capture provider.
    pub fn register_capture_provider(
        &mut self,
        provider: impl DevtoolsCaptureProvider + 'static,
    ) -> Result<(), DevtoolsRegistryError> {
        let id = provider.id().clone();
        if self.capture_providers.contains_key(&id) || self.probes.contains_key(&id) {
            return Err(DevtoolsRegistryError::DuplicateCaptureProvider(id));
        }
        self.capture_providers.insert(id, Box::new(provider));
        Ok(())
    }

    /// Registers a closure-backed capture provider.
    pub fn register_capture_provider_fn<F>(
        &mut self,
        id: impl Into<String>,
        capture: F,
    ) -> Result<(), DevtoolsRegistryError>
    where
        F: Fn() -> Result<DevtoolsCapture, ProbeSnapshotError> + Send + Sync + 'static,
    {
        let provider =
            CaptureProvider::new(id, capture).map_err(DevtoolsRegistryError::InvalidProbeId)?;
        self.register_capture_provider(provider)
    }

    /// Removes a probe by id.
    pub fn unregister(&mut self, id: &ProbeId) -> bool {
        self.probes.remove(id).is_some()
    }

    /// Removes a capture provider by id.
    pub fn unregister_capture_provider(&mut self, id: &ProbeId) -> bool {
        self.capture_providers.remove(id).is_some()
    }

    /// Returns the number of registered probes.
    pub fn len(&self) -> usize {
        self.probes.len()
    }

    /// Returns the number of registered capture providers.
    pub fn capture_provider_len(&self) -> usize {
        self.capture_providers.len()
    }

    /// Returns the total number of registered probes and capture providers.
    pub fn total_len(&self) -> usize {
        self.probes.len() + self.capture_providers.len()
    }

    /// Returns true when no probes or capture providers are registered.
    pub fn is_empty(&self) -> bool {
        self.probes.is_empty() && self.capture_providers.is_empty()
    }

    /// Collects all currently available snapshots.
    pub fn collect(&self) -> SnapshotCollection {
        let mut collection = SnapshotCollection::default();
        for (id, probe) in &self.probes {
            match probe.snapshot() {
                Ok(snapshot) => collection.snapshots.push(snapshot),
                Err(error) => {
                    collection
                        .diagnostics
                        .push(SnapshotDiagnostic::collection_failed(
                            id.clone(),
                            error.to_string(),
                        ));
                }
            }
        }
        collection
    }

    /// Collects a target/domain/event capture while preserving legacy snapshots.
    pub fn collect_capture(&self) -> DevtoolsCapture {
        let legacy_capture = if self.probes.is_empty() {
            DevtoolsCapture::default()
        } else {
            DevtoolsCapture::from_snapshot_collection(self.collect())
        };
        let mut targets = legacy_capture.targets.targets;
        let mut domains = legacy_capture.domains;
        let mut events = legacy_capture.events;
        let mut snapshots = legacy_capture.snapshots;
        let mut diagnostics = legacy_capture.diagnostics;

        for (id, provider) in &self.capture_providers {
            match provider.capture() {
                Ok(capture) => {
                    let capture = capture.sanitized();
                    targets.extend(capture.targets.targets);
                    domains.extend(capture.domains);
                    events.extend(capture.events);
                    snapshots.extend(capture.snapshots);
                    diagnostics.extend(capture.diagnostics);
                }
                Err(error) => {
                    diagnostics.push(SnapshotDiagnostic::collection_failed(
                        id.clone(),
                        error.to_string(),
                    ));
                }
            }
        }

        DevtoolsCapture::new(
            DevtoolsTargetTree::new(targets),
            domains,
            events,
            snapshots,
            diagnostics,
        )
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
    /// A capture provider with this id is already registered.
    #[error("duplicate devtools capture provider: {0}")]
    DuplicateCaptureProvider(ProbeId),
}
