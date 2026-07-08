use super::*;

#[test]
fn surface_manifest_classifies_public_surface_once() {
    use std::collections::{BTreeMap, BTreeSet};

    let mut owners = BTreeMap::new();
    for entry in surface_manifest() {
        let previous = owners.insert(entry.name.clone(), entry.owner);
        assert!(
            previous.is_none(),
            "`{}` appears in multiple public surface owner classes",
            entry.name
        );
    }

    let covered_classes = owners.values().copied().collect::<BTreeSet<_>>();
    for expected_class in [
        PublicSurfaceOwnerClass::OfficialComponent,
        PublicSurfaceOwnerClass::RendererNeutralStateContract,
        PublicSurfaceOwnerClass::GpuiAdapterHelper,
        PublicSurfaceOwnerClass::DeprecatedRemovalTarget,
        PublicSurfaceOwnerClass::InternalImplementationDetail,
    ] {
        assert!(
            covered_classes.contains(&expected_class),
            "public surface owner map should contain at least one {expected_class:?} entry"
        );
    }
}

#[test]
fn surface_manifest_aligns_adjacent_gallery_statuses() {
    let manifest = surface_manifest();

    for owner in PUBLIC_SURFACE_OWNER_MAP {
        let expected_status = component_contract_entry(owner.name)
            .map(|entry| entry.gallery_status)
            .unwrap_or(SurfaceGalleryStatus::NotInGallery);
        if expected_status == SurfaceGalleryStatus::NotInGallery {
            continue;
        }

        let entries = manifest
            .iter()
            .filter(|entry| entry.name == owner.name)
            .collect::<Vec<_>>();
        assert_eq!(
            entries.len(),
            1,
            "contract gallery surface `{}` should have exactly one manifest owner",
            owner.name
        );
        assert_eq!(
            entries[0].gallery_status, expected_status,
            "contract gallery surface `{}` changed manifest gallery status",
            owner.name
        );
    }
}

#[test]
fn surface_manifest_homes_point_to_real_sources() {
    let source_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let lib_source = std::fs::read_to_string(source_dir.join("lib.rs"))
        .unwrap_or_else(|error| panic!("failed to read lib.rs: {error}"));
    let gpui_adapter_source = public_module_source(&lib_source, "gpui_adapter")
        .expect("lib.rs should expose a gpui_adapter module");

    for entry in surface_manifest() {
        if entry.home == "removed" {
            continue;
        } else if entry.home == "gpui_adapter" {
            assert!(
                gpui_adapter_source.contains(entry.name.as_str()),
                "`{}` should stay exported through the gpui_adapter owner group",
                entry.name
            );
        } else {
            let path = source_dir.join(entry.home.as_str());
            assert!(
                path.is_file() || path.is_dir(),
                "`{}` owner home `{}` should point to a real source file or module directory",
                entry.name,
                entry.home
            );
        }
    }
}

#[test]
fn surface_manifest_tracks_exports_gallery_and_docs_contracts() {
    use std::collections::BTreeSet;

    let manifest = surface_manifest();
    let names = manifest
        .iter()
        .map(|entry| entry.name.as_str())
        .collect::<BTreeSet<_>>();
    for required in [
        "Button",
        "Listbox",
        "Select",
        "Combobox",
        "Command",
        "Tooltip",
        "Dialog",
        "TableBehaviorSnapshot",
        "TreeState",
        "VirtualizedListState",
        "GpuiOverlayAdapterConfig",
        "TextInputController",
        "primitives::trigger_a11y",
    ] {
        assert!(
            names.contains(required),
            "surface manifest should include `{required}`"
        );
    }

    let component_contract = include_str!("../../../../docs/ui/component-contract.md");
    let verification = include_str!("../../../../docs/verification.md");
    for entry in &manifest {
        match entry.owner {
            PublicSurfaceOwnerClass::OfficialComponent => {
                assert!(
                    entry.root_export,
                    "{} should be exported from crate root",
                    entry.name
                );
                assert!(
                    entry.prelude_export,
                    "{} should be exported from prelude",
                    entry.name
                );
                assert!(
                    matches!(
                        entry.gallery_status,
                        SurfaceGalleryStatus::OfficialComponent
                            | SurfaceGalleryStatus::OfficialOverlay
                    ),
                    "official manifest entry `{}` should be present in a gallery catalog",
                    entry.name
                );
            }
            PublicSurfaceOwnerClass::OfficialComponentRecipe => {
                assert!(
                    entry.root_export,
                    "{} should be exported from crate root",
                    entry.name
                );
                assert!(
                    !entry.prelude_export,
                    "{} should stay out of prelude; import component recipes from the crate root or owner module",
                    entry.name
                );
                assert_eq!(
                    entry.gallery_status,
                    SurfaceGalleryStatus::NotInGallery,
                    "component recipe `{}` should be documented by docs/signals rather than standalone catalog status",
                    entry.name
                );
            }
            PublicSurfaceOwnerClass::GpuiAdapterHelper => {
                assert!(
                    entry.adapter_only,
                    "{} should be flagged adapter-only",
                    entry.name
                );
                assert!(
                    !entry.prelude_export,
                    "adapter-only surface `{}` must not leak into prelude",
                    entry.name
                );
            }
            PublicSurfaceOwnerClass::DiagnosticSurface => {
                assert!(
                    entry.diagnostic_only,
                    "{} should be flagged diagnostic-only",
                    entry.name
                );
            }
            PublicSurfaceOwnerClass::RendererNeutralStateContract
            | PublicSurfaceOwnerClass::DeprecatedRemovalTarget => {}
            PublicSurfaceOwnerClass::InternalImplementationDetail => {
                assert!(
                    !entry.root_export && !entry.prelude_export,
                    "internal anatomy `{}` must stay out of crate root and prelude exports",
                    entry.name
                );
            }
        }

        match entry.primitive_status {
            SurfacePrimitiveStatus::PublicPrimitiveModule => {
                assert!(
                    entry.home.starts_with("primitives/"),
                    "primitive manifest entry `{}` should point to primitives source",
                    entry.name
                );
            }
            SurfacePrimitiveStatus::RemovedPrimitiveModule => {
                assert_eq!(
                    entry.home, "removed",
                    "removed primitive `{}` should not point at a compatibility file",
                    entry.name
                );
            }
            SurfacePrimitiveStatus::NotPrimitive => {}
        }

        let Some(token) = entry.docs_token else {
            continue;
        };
        match entry.docs_status {
            SurfaceDocsStatus::ComponentCatalog => {
                assert!(
                    names.contains(entry.name.as_str()),
                    "component catalog surface `{}` should remain in manifest",
                    entry.name
                );
            }
            SurfaceDocsStatus::ComponentContract => {
                assert!(
                    component_contract.contains(token),
                    "component contract docs should mention manifest token `{token}`"
                );
            }
            SurfaceDocsStatus::ComponentContractOrVerification => {
                assert!(
                    component_contract.contains(token) || verification.contains(token),
                    "component contract or verification docs should mention manifest token `{token}`"
                );
            }
            SurfaceDocsStatus::Verification => {
                assert!(
                    verification.contains(token)
                        || verification.contains("primitive_deletion_target_inventory"),
                    "verification docs should mention removed manifest token `{token}`"
                );
            }
        }
    }
}

#[test]
fn primitive_owner_map_classifies_every_public_primitive_module_once() {
    use std::collections::BTreeMap;

    let modules = public_primitive_modules_from_mod();
    let mut owners = BTreeMap::new();
    for entry in PUBLIC_SURFACE_OWNER_MAP
        .iter()
        .filter(|entry| entry.name.starts_with("primitives::"))
    {
        let module = entry
            .name
            .strip_prefix("primitives::")
            .expect("primitive owner entry should use primitives:: prefix");
        let previous = owners.insert(module.to_owned(), entry.owner);
        assert!(
            previous.is_none(),
            "primitive module `{module}` should have exactly one owner class"
        );
    }

    owners.retain(|_, owner| *owner != PublicSurfaceOwnerClass::DeprecatedRemovalTarget);
    assert_eq!(
        owners.keys().cloned().collect::<Vec<_>>(),
        modules,
        "every remaining public primitives module should be explicitly classified after U2 removes shallow aliases"
    );
}

#[test]
fn primitive_deletion_target_inventory_blocks_removed_shallow_reexports() {
    let deletion_targets = PUBLIC_SURFACE_OWNER_MAP
        .iter()
        .filter(|entry| entry.owner == PublicSurfaceOwnerClass::DeprecatedRemovalTarget)
        .map(|entry| {
            entry
                .name
                .strip_prefix("primitives::")
                .unwrap_or_else(|| panic!("deletion target `{}` should be a primitive", entry.name))
                .to_owned()
        })
        .collect::<Vec<_>>();

    assert_eq!(
        deletion_targets,
        vec![
            "active_descendant".to_string(),
            "collection".to_string(),
            "controllable_state".to_string(),
            "overlay".to_string(),
        ],
        "U2 should delete only the known shallow primitive pass-through modules"
    );

    let primitives_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/primitives");
    let public_modules = public_primitive_modules_from_mod();
    for module in deletion_targets {
        let source_path = primitives_dir.join(format!("{module}.rs"));
        assert!(
            !source_path.exists(),
            "removed shallow primitive module `{module}` should not keep a compatibility file"
        );
        assert!(
            !public_modules.contains(&module),
            "removed shallow primitive module `{module}` should not stay in primitives/mod.rs"
        );
    }
}

#[test]
fn primitive_modules_do_not_reexport_ui_core_as_pass_through_aliases() {
    let primitives_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/primitives");
    let entries = std::fs::read_dir(&primitives_dir)
        .unwrap_or_else(|error| panic!("failed to read {primitives_dir:?}: {error}"));
    let mut offenders = Vec::new();

    for entry in entries {
        let path = entry
            .unwrap_or_else(|error| panic!("failed to read primitive source entry: {error}"))
            .path();
        if path.file_name().is_some_and(|name| name == "mod.rs")
            || path.extension().is_none_or(|extension| extension != "rs")
        {
            continue;
        }

        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {path:?}: {error}"));
        if source.contains("pub use open_gpui_ui_core::") {
            offenders.push(
                path.file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("<unknown>")
                    .to_owned(),
            );
        }
    }

    assert_eq!(
        offenders,
        Vec::<String>::new(),
        "ui_components::primitives must own adapter behavior, not pass through ui_core aliases"
    );
}
