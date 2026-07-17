//! Collision-free render identity and debug selector composition.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::sync::Arc;

use open_gpui::{ElementId, SharedString};
use sha2::{Digest as _, Sha256};

use crate::action::ResolvedActionIcon;

/// Encodes one selector segment while preserving common human-readable identifiers.
///
/// Colons delimit selector segments and percent signs introduce escapes. Encoding both makes the
/// mapping injective without changing identifiers that use neither reserved character.
pub(crate) fn debug_selector_segment(value: &str) -> String {
    value.replace('%', "%25").replace(':', "%3A")
}

/// Encodes an element id without collapsing distinct enum variants through `Display`.
///
/// String ids keep their familiar selector spelling. Every other variant starts with `%00`, a
/// prefix that a string id cannot produce because literal percent signs are escaped first.
pub(crate) fn debug_selector_element_id(id: &ElementId) -> String {
    match id {
        ElementId::Name(name) => debug_selector_segment(name),
        ElementId::View(entity) => format!("%00v{}", entity.as_u64()),
        ElementId::Integer(value) => format!("%00i{value}"),
        ElementId::Uuid(uuid) => format!("%00u{uuid}"),
        ElementId::FocusHandle(focus) => {
            format!("%00f{}", debug_selector_segment(&format!("{focus:?}")))
        }
        ElementId::NamedInteger(name, value) => {
            let name = debug_selector_segment(name);
            format!("%00n{}-{name}-{value}", name.len())
        }
        ElementId::Path(path) => {
            format!("%00p{}", debug_selector_segment(&format!("{path:?}")))
        }
        ElementId::CodeLocation(location) => {
            let file = debug_selector_segment(location.file());
            format!(
                "%00l{}-{file}-{}-{}",
                file.len(),
                location.line(),
                location.column()
            )
        }
        ElementId::NamedChild(parent, name) => {
            let parent = debug_selector_element_id(parent);
            let name = debug_selector_segment(name);
            format!("%00c{}-{parent}-{}-{name}", parent.len(), name.len())
        }
        ElementId::OpaqueId(bytes) => {
            let bytes = bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            format!("%00o{bytes}")
        }
    }
}

/// Canonical authored identity facts plus their opaque collision-resistant token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AuthoredSnapshotFingerprint {
    token: String,
    #[cfg(test)]
    canonical: Arc<[u8]>,
}

impl AuthoredSnapshotFingerprint {
    fn as_str(&self) -> &str {
        &self.token
    }

    #[cfg(test)]
    fn canonical(&self) -> &[u8] {
        &self.canonical
    }
}

/// Builds a length-prefixed canonical snapshot and an opaque SHA-256 token.
#[derive(Debug, Default)]
pub(crate) struct AuthoredSnapshot {
    bytes: Vec<u8>,
}

impl AuthoredSnapshot {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn field(mut self, value: impl AsRef<str>) -> Self {
        self.push_field(0x01, value.as_ref().as_bytes());
        self
    }

    pub(crate) fn optional_field(mut self, value: Option<&str>) -> Self {
        match value {
            Some(value) => self.push_field(0x02, value.as_bytes()),
            None => self.bytes.push(0x03),
        }
        self
    }

    pub(crate) fn resolved_icon(mut self, icon: Option<&ResolvedActionIcon>) -> Self {
        let Some(icon) = icon else {
            self.bytes.push(0x05);
            return self;
        };

        self.bytes.push(0x06);
        self = self
            .field(icon.descriptor().name())
            .optional_field(icon.descriptor().fallback_label_ref())
            .optional_field(icon.label());
        if let Some(diagnostic) = icon.diagnostic() {
            self = self
                .field(diagnostic.icon_name())
                .field(diagnostic.message());
        } else {
            self.bytes.push(0x07);
        }
        self
    }

    pub(crate) fn opaque_fingerprint(mut self, fingerprint: &AuthoredSnapshotFingerprint) -> Self {
        self.push_opaque_fingerprint(fingerprint);
        self
    }

    pub(crate) fn finish(self) -> AuthoredSnapshotFingerprint {
        let digest = Sha256::digest(&self.bytes);
        let mut encoded = String::with_capacity(digest.len() * 2);
        for byte in digest {
            write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
        }
        AuthoredSnapshotFingerprint {
            token: encoded,
            #[cfg(test)]
            canonical: self.bytes.into(),
        }
    }

    fn push_field(&mut self, tag: u8, value: &[u8]) {
        self.bytes.push(tag);
        self.bytes
            .extend_from_slice(&(value.len() as u64).to_le_bytes());
        self.bytes.extend_from_slice(value);
    }

    fn push_opaque_fingerprint(&mut self, fingerprint: &AuthoredSnapshotFingerprint) {
        self.push_field(0x08, fingerprint.as_str().as_bytes());
    }
}

/// Authored identity input for one stable-value occurrence.
#[derive(Debug, Clone)]
pub(crate) struct StableValueRenderIdentityInput {
    value: String,
    authored_snapshot: AuthoredSnapshotFingerprint,
}

impl StableValueRenderIdentityInput {
    pub(crate) fn new(
        value: impl Into<String>,
        authored_snapshot: AuthoredSnapshotFingerprint,
    ) -> Self {
        Self {
            value: value.into(),
            authored_snapshot,
        }
    }
}

/// Shared render identity for stable values that may become ambiguous.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StableValueRenderIdentity {
    pub(crate) element_id: ElementId,
    pub(crate) debug_selector: String,
    occurrence_fingerprint: AuthoredSnapshotFingerprint,
}

impl StableValueRenderIdentity {
    pub(crate) fn resolve(
        component: &str,
        component_id: &str,
        part: &str,
        inputs: impl IntoIterator<Item = StableValueRenderIdentityInput>,
    ) -> Vec<Self> {
        let inputs = inputs.into_iter().collect::<Vec<_>>();
        let value_counts = inputs.iter().fold(BTreeMap::new(), |mut counts, input| {
            *counts.entry(input.value.clone()).or_insert(0usize) += 1;
            counts
        });
        let mut snapshots = BTreeMap::<String, AuthoredSnapshot>::new();
        for input in &inputs {
            if value_counts
                .get(input.value.as_str())
                .is_some_and(|count| *count > 1)
            {
                let snapshot = snapshots.entry(input.value.clone()).or_default();
                snapshot.push_opaque_fingerprint(&input.authored_snapshot);
            }
        }
        let snapshots = snapshots
            .into_iter()
            .map(|(value, snapshot)| (value, snapshot.finish()))
            .collect::<BTreeMap<_, _>>();

        inputs
            .into_iter()
            .enumerate()
            .map(|(index, input)| {
                let selector_value = debug_selector_segment(&input.value);
                let ambiguous = value_counts
                    .get(input.value.as_str())
                    .is_some_and(|count| *count > 1);
                if ambiguous {
                    let snapshot = snapshots
                        .get(input.value.as_str())
                        .expect("ambiguous values must have an authored snapshot");
                    let base_id = ElementId::named_usize(
                        format!("{component}-{part}-{}", input.value),
                        index,
                    );
                    let element_id = ElementId::NamedChild(
                        Arc::new(base_id),
                        SharedString::from(format!("snapshot-{}", snapshot.as_str())),
                    );
                    let occurrence_fingerprint = AuthoredSnapshot::new()
                        .field(&input.value)
                        .field(index.to_string())
                        .opaque_fingerprint(snapshot)
                        .finish();
                    let debug_selector = format!(
                        "{component}:{component_id}:duplicate-{part}:{index}:{selector_value}:snapshot:{}",
                        snapshot.as_str()
                    );
                    Self {
                        element_id,
                        debug_selector,
                        occurrence_fingerprint,
                    }
                } else {
                    let element_id = format!("{component}-{part}-{}", input.value).into();
                    let debug_selector =
                        format!("{component}:{component_id}:{part}:{selector_value}");
                    let occurrence_fingerprint = AuthoredSnapshot::new()
                        .field("unique")
                        .field(&input.value)
                        .finish();
                    Self {
                        element_id,
                        debug_selector,
                        occurrence_fingerprint,
                    }
                }
            })
            .collect()
    }

    pub(crate) fn occurrence_fingerprint(&self) -> &AuthoredSnapshotFingerprint {
        &self.occurrence_fingerprint
    }
}

/// Shared render identity for activatable stable-value items.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StableValueItemRenderIdentity {
    pub(crate) element_id: ElementId,
    pub(crate) debug_selector: String,
    pub(crate) activation_state_key: ElementId,
}

impl StableValueItemRenderIdentity {
    pub(crate) fn from_render_identity(
        identity: StableValueRenderIdentity,
        activation_kind: &str,
    ) -> Self {
        let StableValueRenderIdentity {
            element_id,
            debug_selector,
            ..
        } = identity;
        let activation_identity = ElementId::NamedChild(
            Arc::new(element_id.clone()),
            SharedString::new_static("activation"),
        );
        let activation_state_key = ElementId::NamedChild(
            Arc::new(activation_identity),
            SharedString::from(activation_kind.to_owned()),
        );

        Self {
            element_id,
            debug_selector,
            activation_state_key,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selector_segments_cannot_alias_delimiter_shaped_values() {
        assert_eq!(debug_selector_segment("workspace"), "workspace");
        assert_eq!(debug_selector_segment("a:b"), "a%3Ab");
        assert_eq!(debug_selector_segment("a%3Ab"), "a%253Ab");
        assert_ne!(
            debug_selector_segment("a:b"),
            debug_selector_segment("a%3Ab")
        );
    }

    #[test]
    fn element_id_variants_cannot_alias_their_display_text() {
        let name = ElementId::Name("1".into());
        let integer = ElementId::Integer(1);
        let reserved_name = ElementId::Name("%00i1".into());

        assert_eq!(debug_selector_element_id(&name), "1");
        assert_eq!(debug_selector_element_id(&integer), "%00i1");
        assert_eq!(debug_selector_element_id(&reserved_name), "%2500i1");
        assert_ne!(
            debug_selector_element_id(&name),
            debug_selector_element_id(&integer)
        );
        assert_ne!(
            debug_selector_element_id(&integer),
            debug_selector_element_id(&reserved_name)
        );
    }

    #[test]
    fn ambiguous_occurrences_are_stable_only_within_the_same_authored_snapshot() {
        fn resolve(labels: [&str; 2]) -> Vec<StableValueItemRenderIdentity> {
            StableValueRenderIdentity::resolve(
                "toolbar",
                "snapshot-probe",
                "item",
                labels.map(|label| {
                    StableValueRenderIdentityInput::new(
                        "duplicate",
                        AuthoredSnapshot::new().field(label).finish(),
                    )
                }),
            )
            .into_iter()
            .map(|identity| StableValueItemRenderIdentity::from_render_identity(identity, "action"))
            .collect()
        }

        let initial = resolve(["Alpha", "Beta"]);
        let equivalent = resolve(["Alpha", "Beta"]);
        let reordered = resolve(["Beta", "Alpha"]);

        assert_eq!(initial, equivalent);
        for (old, new) in initial.iter().zip(reordered.iter()) {
            assert_ne!(old.element_id, new.element_id);
            assert_ne!(old.debug_selector, new.debug_selector);
            assert_ne!(old.activation_state_key, new.activation_state_key);
        }
    }

    #[test]
    fn unique_value_identity_does_not_churn_when_authored_metadata_changes() {
        let resolve = |label| {
            StableValueRenderIdentity::resolve(
                "sidebar",
                "snapshot-probe",
                "item",
                [StableValueRenderIdentityInput::new(
                    "unique",
                    AuthoredSnapshot::new().field(label).finish(),
                )],
            )
            .pop()
            .expect("the unique item should resolve")
        };

        let initial = resolve("Before");
        let updated = resolve("After");
        assert_eq!(initial.element_id, updated.element_id);
        assert_eq!(initial.debug_selector, updated.debug_selector);
    }

    #[test]
    fn authored_snapshot_encoding_preserves_field_and_option_boundaries() {
        let equivalent_a = AuthoredSnapshot::new()
            .field("alpha")
            .field("beta")
            .finish();
        let equivalent_b = AuthoredSnapshot::new()
            .field("alpha")
            .field("beta")
            .finish();
        let shifted_boundary = AuthoredSnapshot::new()
            .field("alph")
            .field("abeta")
            .finish();
        let absent = AuthoredSnapshot::new().optional_field(None).finish();
        let present_empty = AuthoredSnapshot::new().optional_field(Some("")).finish();

        assert_eq!(equivalent_a, equivalent_b);
        assert_ne!(equivalent_a, shifted_boundary);
        assert_ne!(absent, present_empty);
    }

    #[test]
    fn nested_fingerprints_store_only_the_fixed_size_opaque_token() {
        let child = AuthoredSnapshot::new()
            .field("private authored metadata".repeat(100))
            .finish();
        let parent = AuthoredSnapshot::new().opaque_fingerprint(&child).finish();

        assert_eq!(parent.canonical().len(), 1 + 8 + 64);
        assert!(
            !parent
                .canonical()
                .windows("private authored metadata".len())
                .any(|window| window == b"private authored metadata")
        );
    }
}
