#![cfg(feature = "bench-support")]

//! Executed value consumers of the public randomized-tree API without GPUI test-support.
//! App, notification, drawing and release workflows belong to the companion benchmark.

use open_gpui::randomized_element_tree::{
    RandomizedElementTree, RandomizedElementTreeBounds, RandomizedElementTreeConfig,
    RandomizedElementTreeMutation, RandomizedElementTreeMutationKind,
    RandomizedElementTreeTopology,
};
use rand::{SeedableRng as _, rngs::StdRng};
use std::collections::BTreeSet;

#[test]
fn downstream_config_preserves_topology_builders_and_density_normalization() {
    for (seed, topology) in RandomizedElementTreeTopology::ALL.into_iter().enumerate() {
        let base = RandomizedElementTreeConfig::new(seed as u64, 32);
        assert_eq!(base.seed(), seed as u64);
        assert_eq!(base.element_count(), 32);
        assert_eq!(base.topology(), topology);
        assert_eq!(base.entity_density(), 0.0);
        assert_eq!(base.handler_density(), 0.0);
        assert_eq!(
            topology.is_tall(),
            matches!(
                topology,
                RandomizedElementTreeTopology::Deep | RandomizedElementTreeTopology::Narrow
            ),
        );
        let configured = base
            .with_topology(RandomizedElementTreeTopology::Mixed)
            .with_entity_density(0.75)
            .with_handler_density(0.25)
            .with_animation_density(0.5)
            .with_cached_views(true)
            .with_hover_styles(true);
        assert_eq!(configured.topology(), RandomizedElementTreeTopology::Mixed);
        assert_eq!(configured.entity_density(), 0.75);
        assert_eq!(configured.handler_density(), 0.25);
        assert_eq!(
            configured.with_cached_views(false).with_cached_views(true),
            configured,
        );
        assert_eq!(
            configured.with_hover_styles(false).with_hover_styles(true),
            configured,
        );
        assert_ne!(configured.with_cached_views(false), configured);
        assert_ne!(configured.with_hover_styles(false), configured);
        for (input, expected) in [
            (f64::NAN, 0.0),
            (f64::NEG_INFINITY, 0.0),
            (-1.0, 0.0),
            (0.4, 0.4),
            (2.0, 1.0),
            (f64::INFINITY, 1.0),
        ] {
            let normalized = configured
                .with_entity_density(input)
                .with_handler_density(input)
                .with_animation_density(input);
            assert_eq!(normalized.entity_density(), expected);
            assert_eq!(normalized.handler_density(), expected);
            assert_eq!(
                normalized,
                configured
                    .with_entity_density(expected)
                    .with_handler_density(expected)
                    .with_animation_density(expected),
            );
        }
    }
}

#[test]
fn downstream_bounds_sample_reproducibly_and_keep_fixed_and_normalized_ranges() {
    let bounds = RandomizedElementTreeBounds::new(8..=96)
        .with_topologies([
            RandomizedElementTreeTopology::Deep,
            RandomizedElementTreeTopology::Narrow,
        ])
        .with_entity_density(-1.0..=0.5)
        .with_handler_density(0.25..=0.25);
    let mut sampled_topologies = BTreeSet::new();
    for seed in 0..64 {
        let config = bounds.sample(&mut StdRng::seed_from_u64(seed));
        assert_eq!(
            config,
            bounds.clone().sample(&mut StdRng::seed_from_u64(seed)),
        );
        assert!((8..=96).contains(&config.element_count()));
        assert!(config.topology().is_tall());
        assert!((0.0..=0.5).contains(&config.entity_density()));
        assert_eq!(config.handler_density(), 0.25);
        sampled_topologies.insert(format!("{:?}", config.topology()));
    }
    assert_eq!(sampled_topologies.len(), 2);
    let all = RandomizedElementTreeBounds::new(0..=0).with_topologies([]);
    assert_eq!(all, RandomizedElementTreeBounds::new(0..=0));
    let config = all
        .with_entity_density(2.0..=f64::INFINITY)
        .with_handler_density(f64::NAN..=f64::NAN)
        .sample(&mut StdRng::seed_from_u64(23));
    assert_eq!(config.element_count(), 0);
    assert_eq!(config.entity_density(), 1.0);
    assert_eq!(config.handler_density(), 0.0);
}

#[test]
fn downstream_snapshot_recreation_keeps_deterministic_shape_and_stable_ids() {
    for seed in 0..4 {
        let tree = RandomizedElementTree::new(seed, 32);
        let snapshot = tree.snapshot();
        assert_eq!(snapshot, RandomizedElementTree::new(seed, 32).snapshot());
        assert_eq!(snapshot.descendant_count(), 32);
        assert_eq!(snapshot.entity_count(), 0);
        assert_eq!(snapshot.handler_count(), 0);
        assert_eq!(
            snapshot.element_ids().into_iter().collect::<BTreeSet<_>>(),
            (0..32).collect(),
        );
        match seed {
            0 => assert_eq!(snapshot.max_depth(), 1),
            1 => assert_eq!(snapshot.max_depth(), 32),
            _ => assert!((1..=32).contains(&snapshot.max_depth())),
        }
        let recreated = RandomizedElementTree::from_snapshot(snapshot.clone());
        assert_eq!(recreated.snapshot(), snapshot);
        assert_eq!(tree.snapshot(), snapshot);
    }
    let mut empty = RandomizedElementTree::new(7, 0);
    assert_eq!(empty.snapshot().descendant_count(), 0);
    assert_eq!(empty.snapshot().max_depth(), 0);
    assert!(empty.snapshot().element_ids().is_empty());
    assert!(empty.random_element_id().is_none());
    assert!(empty.remove_child().is_none());
}

#[test]
fn downstream_seeded_mutations_replay_and_reconstruction_keeps_next_element_id() {
    for seed in [0, 1, 2, 3, 7, 23] {
        let mut first = RandomizedElementTree::new(seed, 24);
        let mut replay = RandomizedElementTree::new(seed, 24);
        for _ in 0..64 {
            assert_eq!(first.mutate(), replay.mutate());
            assert_eq!(first.snapshot(), replay.snapshot());
        }
    }
    let mut tree = RandomizedElementTree::new(0, 16);
    let original = tree.snapshot();
    let RandomizedElementTreeMutation::Inserted { element_id, .. } = tree.insert_child() else {
        panic!("insertion must return its new stable identity");
    };
    assert_eq!(element_id, 16);
    assert_eq!(tree.snapshot().descendant_count(), 17);
    let before_removal = tree.snapshot();
    let RandomizedElementTreeMutation::Removed {
        element_id,
        removed_element_count,
        ..
    } = tree
        .remove_child()
        .expect("a nonempty tree can remove a subtree")
    else {
        panic!("removal must describe the removed subtree");
    };
    let remaining = tree.snapshot();
    assert_eq!(
        remaining.descendant_count(),
        before_removal.descendant_count() - removed_element_count,
    );
    assert!(!remaining.element_ids().contains(&element_id));
    assert_eq!(original.descendant_count(), 16);
    let mut recreated = RandomizedElementTree::from_snapshot(remaining.clone());
    assert_eq!(recreated.snapshot(), remaining);
    let RandomizedElementTreeMutation::Inserted { element_id, .. } = recreated.insert_child()
    else {
        panic!("recreated trees must preserve insertion state");
    };
    assert_eq!(element_id, 17);
    assert!(recreated.snapshot().element_ids().contains(&element_id));
}

#[test]
fn downstream_mutation_inventory_exposes_all_seventeen_workload_controls() {
    use RandomizedElementTreeMutationKind::*;

    assert_eq!(
        RandomizedElementTreeMutationKind::ALL,
        [
            RootColor,
            RootOpacity,
            RootBorder,
            RootBounds,
            RootClipping,
            RootVisibility,
            ChildColor,
            ChildOpacity,
            ChildBorder,
            ChildBounds,
            ChildClipping,
            ChildVisibility,
            ChildText,
            Reorder,
            Insert,
            Remove,
            RemoveLeaf,
        ],
    );
}
