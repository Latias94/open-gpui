use crate::legacy::binary_layout_policy::{
    BinaryChild, BinarySplitClass, initial_split_sizes, plan_sibling_promotion,
    resolve_split_sizes, transfer_central_after_split,
};
use crate::legacy::graph_assembly::{CanonicalSizeRef, DockGraphAssembly};
use crate::legacy::{CanonicalGraphVersion, DockItemId, DockSpaceId, SplitAxis};

#[test]
fn initial_split_matches_imgui_truncation_and_remainder() {
    let sizes = initial_split_sizes(101.0, 2.0, 32.0, 0.333).unwrap();
    assert_eq!(sizes.available, 99.0);
    assert_eq!(sizes.first, 32.0);
    assert_eq!(sizes.second, 67.0);
    assert_eq!(sizes.first + sizes.second, sizes.available);
}

#[test]
fn regular_resize_rounds_first_and_assigns_remainder_to_second() {
    let sizes = resolve_split_sizes(101.0, 2.0, 32.0, 0.5, None, None, None).unwrap();
    assert_eq!(sizes.available, 99.0);
    assert_eq!(sizes.first, 50.0);
    assert_eq!(sizes.second, 49.0);

    let constrained = resolve_split_sizes(40.0, 2.0, 32.0, 0.9, None, None, None).unwrap();
    assert_eq!(constrained.available, 38.0);
    assert_eq!(constrained.first, 34.0);
    assert_eq!(constrained.second, 4.0);
}

#[test]
fn central_descendant_receives_remainder_while_sibling_size_ref_is_preserved() {
    let central_second = resolve_split_sizes(
        400.0,
        2.0,
        32.0,
        0.5,
        Some(120.0),
        None,
        Some(BinaryChild::Second),
    )
    .unwrap();
    assert_eq!(central_second.first, 120.0);
    assert_eq!(central_second.second, 278.0);

    let central_first = resolve_split_sizes(
        400.0,
        2.0,
        32.0,
        0.5,
        None,
        Some(120.0),
        Some(BinaryChild::First),
    )
    .unwrap();
    assert_eq!(central_first.first, 278.0);
    assert_eq!(central_first.second, 120.0);
}

#[test]
fn regular_resize_uses_relative_size_refs_when_present() {
    let sizes = resolve_split_sizes(400.0, 2.0, 32.0, 0.9, Some(100.0), Some(300.0), None)
        .expect("valid size references should resolve");
    assert_eq!(sizes.first, 100.0);
    assert_eq!(sizes.second, 298.0);
}

#[test]
fn oversized_minimum_is_rejected_before_arithmetic_overflow() {
    let result = initial_split_sizes(f32::MAX, 0.0, f32::MAX, 0.5);
    assert!(result.is_err());
}

#[test]
fn imported_zero_share_ratios_remain_resolvable() {
    let zero = resolve_split_sizes(300.0, 2.0, 32.0, 0.0, None, None, None).unwrap();
    assert_eq!(zero.first, 32.0);
    assert_eq!(zero.second, 266.0);

    let one = resolve_split_sizes(300.0, 2.0, 32.0, 1.0, None, None, None).unwrap();
    assert_eq!(one.first, 298.0);
    assert_eq!(one.second, 0.0);
}

#[test]
fn sibling_promotion_preserves_parent_identity_and_size_ref() {
    let mut assembly = DockGraphAssembly::new();
    assembly.add_space("main").unwrap();
    let first = assembly
        .add_group("first", [DockItemId::from("A")], DockItemId::from("A"))
        .unwrap();
    let second = assembly
        .add_group("second", [DockItemId::from("B")], DockItemId::from("B"))
        .unwrap();
    let root = assembly
        .add_branch_with_size_ref(
            SplitAxis::Horizontal,
            first,
            second,
            0.5,
            Some(CanonicalSizeRef {
                width: 900.0,
                height: 600.0,
            }),
        )
        .unwrap();
    assembly.set_root("main", root).unwrap();
    let graph = assembly.publish(CanonicalGraphVersion::new(1, 0)).unwrap();
    let parent = graph
        .space(&DockSpaceId::from("main"))
        .unwrap()
        .root
        .unwrap();
    let (first, second, size_ref) = match graph.node(parent).unwrap() {
        crate::legacy::graph_assembly::CanonicalNode::Branch {
            first,
            second,
            size_ref,
            ..
        } => (*first, *second, *size_ref),
        _ => unreachable!(),
    };

    let plan = plan_sibling_promotion(parent, first, second, second, size_ref).unwrap();
    assert_eq!(plan.parent, parent);
    assert_eq!(plan.survivor, second);
    assert_eq!(plan.retired, [first, second]);
    assert_eq!(plan.remap.from, second);
    assert_eq!(plan.remap.to, parent);
    assert_eq!(plan.preserved_size_ref, size_ref);
}

#[test]
fn inner_split_transfers_central_to_inheritor_but_outer_split_keeps_it() {
    let mut assembly = DockGraphAssembly::new();
    assembly.add_space("main").unwrap();
    let target = assembly
        .add_group("target", [DockItemId::from("A")], DockItemId::from("A"))
        .unwrap();
    let sibling = assembly
        .add_group("sibling", [DockItemId::from("B")], DockItemId::from("B"))
        .unwrap();
    let root = assembly
        .add_branch(SplitAxis::Horizontal, target, sibling, 0.5)
        .unwrap();
    assembly.set_root("main", root).unwrap();
    assembly
        .set_central_node("main", Some(target), true, false)
        .unwrap();
    let graph = assembly.publish(CanonicalGraphVersion::new(1, 0)).unwrap();
    let central = graph
        .space(&DockSpaceId::from("main"))
        .unwrap()
        .central
        .node
        .unwrap();
    let root = graph
        .space(&DockSpaceId::from("main"))
        .unwrap()
        .root
        .unwrap();

    assert_eq!(
        transfer_central_after_split(Some(central), central, root, BinarySplitClass::Inner),
        Some(root)
    );
    assert_eq!(
        transfer_central_after_split(Some(central), central, root, BinarySplitClass::Outer),
        Some(central)
    );
}
