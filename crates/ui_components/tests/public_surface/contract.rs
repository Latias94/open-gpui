use std::collections::{BTreeMap, BTreeSet};

use open_gpui_ui_components::component_contract::{
    COMPONENT_CONTRACT_GLOBAL_SCENARIOS, COMPONENT_CONTRACT_ROWS, PublicApiExport, PublicApiTier,
    common_public_exports, component_contract_entry, component_contract_metadata,
    default_public_exports, diagnostic_public_exports,
};

fn exports_by_name(
    exports: impl IntoIterator<Item = &'static PublicApiExport>,
) -> BTreeMap<&'static str, &'static PublicApiExport> {
    let mut by_name = BTreeMap::new();
    for export in exports {
        let previous = by_name.insert(export.name(), export);
        assert!(
            previous.is_none(),
            "public export `{}` has duplicate owners `{}` and `{}`",
            export.name(),
            previous.map_or("<none>", |entry| entry.owner()),
            export.owner(),
        );
    }
    by_name
}

#[test]
fn official_components_match_typed_public_exports() {
    let mut contract_ids = BTreeSet::new();
    for entry in COMPONENT_CONTRACT_ROWS {
        assert!(
            contract_ids.insert(entry.id().as_str()),
            "duplicate component contract id `{}`",
            entry.id().as_str(),
        );
        assert!(entry.revision().value() > 0);
        assert!(!entry.family().as_str().trim().is_empty());

        let mut requirements = BTreeSet::new();
        for scenario in entry.required_scenarios() {
            assert!(!scenario.trim().is_empty());
            assert!(
                requirements.insert(*scenario),
                "component contract `{}` repeats required scenario `{scenario}`",
                entry.id().as_str(),
            );
        }
    }
    assert_eq!(contract_ids.len(), 48);

    let common = exports_by_name(common_public_exports());
    let default = exports_by_name(default_public_exports());
    let diagnostics = exports_by_name(diagnostic_public_exports());

    assert!(component_contract_entry("NotAComponent").is_none());
    assert!(component_contract_metadata("NotAComponent").is_none());

    for entry in COMPONENT_CONTRACT_ROWS {
        let id = entry.id().as_str();
        assert!(
            common.contains_key(id),
            "official component contract `{id}` is missing from the common public interface",
        );
        assert!(
            default.contains_key(id),
            "official component contract `{id}` is missing from the default public interface",
        );
    }

    assert!(
        common
            .values()
            .all(|entry| entry.tier() == PublicApiTier::Common
                && entry.owner() == "open_gpui_ui_components::{root,common,prelude}")
    );
    assert!(default.values().all(|entry| {
        matches!(
            entry.tier(),
            PublicApiTier::Common | PublicApiTier::Extended
        )
    }));
    assert!(
        diagnostics
            .values()
            .all(|entry| entry.tier() == PublicApiTier::Diagnostic)
    );

    for name in diagnostics.keys() {
        assert!(
            !common.contains_key(name) && !default.contains_key(name),
            "diagnostic export `{name}` leaked into a common/default interface",
        );
    }
    assert!(diagnostics.contains_key("TableBehaviorSnapshot"));
    assert!(default.contains_key("TableVirtualizerSnapshot"));

    assert_eq!(
        COMPONENT_CONTRACT_GLOBAL_SCENARIOS,
        &[
            "gallery.component-contract.metadata",
            "public-api.component-contract.exports",
        ]
    );
}
