use super::*;
use crate::window::a11y::{A11ySubtreeBuilder, ROOT_NODE_ID, set_requested_active};
use crate::{AppContext, EmptyView, TestAppContext, WindowId, point, px};
use accesskit::{ActionRequest, Role, TreeId, TreeUpdate};
use slotmap::SlotMap;

const PARENT: NodeId = NodeId(1);
const MENU: NodeId = NodeId(2);
const ITEM: NodeId = NodeId(3);
const OUTSIDE: NodeId = NodeId(4);
const SYNTHETIC_GROUP: NodeId = NodeId(5);
const SYNTHETIC_LEAF: NodeId = NodeId(6);

fn start_active(a11y: &mut A11y) {
    a11y.set_requested_active_for_test(true);
    a11y.sync_active_flag();
    a11y.begin_frame();
}

fn fixture() -> A11y {
    let mut a11y = A11y::new(Arc::new(AtomicU64::new(0)), false, WindowId::from(7));
    start_active(&mut a11y);
    a11y
}

fn publish(a11y: &mut A11y) -> TreeUpdate {
    let update = a11y.end_frame();
    a11y.publish(&update, a11y.activation_generation(), Default::default());
    update
}

fn node(update: &TreeUpdate, id: NodeId) -> &Node {
    &update
        .nodes
        .iter()
        .find(|(node_id, _)| *node_id == id)
        .unwrap()
        .1
}

fn menu_leaf(a11y: &mut A11y) {
    let mut menu = Node::new(Role::Button);
    menu.add_action(Action::Click);
    assert!(a11y.nodes.push_leaf(MENU, menu));
}

#[::std::prelude::v1::test]
fn replayed_nodes_keep_synthetic_hierarchy_geometry_and_current_focus() {
    let mut a11y = fixture();
    let mut focus_ids = SlotMap::<FocusId, ()>::with_key();
    let menu_focus = focus_ids.insert(());
    let outside_focus = focus_ids.insert(());
    let mut button = Node::new(Role::Button);
    button.add_action(Action::Focus);
    assert!(a11y.nodes.push_leaf(OUTSIDE, button));
    a11y.record_focus_id(OUTSIDE, outside_focus);
    assert!(a11y.nodes.push(PARENT, Node::new(Role::Group)));

    let start = checkpoint(&a11y);
    let rect = accesskit::Rect::new(20., 40., 70., 90.);
    let mut menu = Node::new(Role::Menu);
    menu.add_action(Action::Focus);
    menu.set_bounds(rect);
    assert!(a11y.nodes.push(MENU, menu));
    let logical = Bounds::new(point(px(10.), px(20.)), crate::size(px(25.), px(25.)));
    let witness = point(px(22.), px(30.));
    a11y.record_node_bounds(MENU, logical, Some(witness));
    a11y.record_focus_id(MENU, menu_focus);
    assert!(a11y.nodes.push(ITEM, Node::new(Role::MenuItem)));
    a11y.record_active_descendant(ITEM);
    a11y.nodes.pop();
    let mut synthetic = A11ySubtreeBuilder::new(MENU, &mut a11y.nodes);
    assert!(synthetic.push_child(SYNTHETIC_LEAF, Node::new(Role::Label)));
    let mut group = Node::new(Role::Group);
    group.set_children([SYNTHETIC_LEAF]);
    assert!(synthetic.push_child(SYNTHETIC_GROUP, group));
    synthetic
        .parent_node()
        .set_children([ITEM, SYNTHETIC_GROUP]);
    a11y.nodes.pop();
    let fragment = capture(&a11y, &start).unwrap();
    a11y.nodes.pop();
    a11y.resolve_focus(Some(outside_focus), None);
    let original = publish(&mut a11y);
    assert_eq!(original.focus, OUTSIDE);

    a11y.begin_frame();
    assert!(a11y.nodes.push(PARENT, Node::new(Role::Group)));
    assert!(a11y.nodes.push_leaf(OUTSIDE, Node::new(Role::Label)));
    let start = checkpoint(&a11y);
    assert!(replay(&mut a11y, Some(&fragment), Point::default()));
    let recaptured = capture(&a11y, &start).unwrap();
    a11y.nodes.pop();
    a11y.resolve_focus(Some(menu_focus), None);
    let replayed = publish(&mut a11y);
    assert_eq!(node(&replayed, PARENT).children(), &[OUTSIDE, MENU]);
    for id in [MENU, ITEM, SYNTHETIC_GROUP, SYNTHETIC_LEAF] {
        assert_eq!(node(&replayed, id), node(&original, id));
    }
    assert_eq!(node(&replayed, MENU).bounds(), Some(rect));
    assert_eq!(a11y.published_node_witness(MENU), Some(witness));
    assert_eq!(a11y.published_focus_id(MENU), Some(menu_focus));
    assert_eq!(replayed.focus, ITEM);
    #[cfg(debug_assertions)]
    assert!(a11y.nodes.node_info[&SYNTHETIC_LEAF].synthetic);

    a11y.begin_frame();
    assert!(a11y.nodes.push(PARENT, Node::new(Role::Group)));
    assert!(can_replay(&a11y, Some(&recaptured), Point::default()));
}

#[::std::prelude::v1::test]
fn rejected_frames_and_rolled_back_captures_never_gain_replay_authority() {
    let mut a11y = fixture();
    let start = checkpoint(&a11y);
    menu_leaf(&mut a11y);
    let accepted = capture(&a11y, &start).unwrap();
    publish(&mut a11y);

    a11y.begin_frame();
    let transaction = a11y.prepaint_checkpoint();
    let start = checkpoint(&a11y);
    assert!(replay(&mut a11y, Some(&accepted), Point::default()));
    let rolled_back = capture(&a11y, &start).unwrap();
    a11y.rollback_prepaint(transaction);
    assert!(!a11y.nodes.has_node(MENU));
    assert!(capture(&a11y, &start).is_none());
    assert!(
        node(&publish(&mut a11y), ROOT_NODE_ID)
            .children()
            .is_empty()
    );

    a11y.begin_frame();
    assert!(!can_replay(&a11y, Some(&rolled_back), Point::default()));
    let start = checkpoint(&a11y);
    assert!(replay(&mut a11y, Some(&accepted), Point::default()));
    let refused = capture(&a11y, &start).unwrap();
    a11y.end_frame();
    a11y.discard_candidate_frame();
    assert!(capture(&a11y, &start).is_none());

    a11y.begin_frame();
    assert!(!can_replay(&a11y, Some(&refused), Point::default()));
    assert!(can_replay(&a11y, Some(&accepted), Point::default()));
}

#[::std::prelude::v1::test]
fn activation_parent_and_collisions_qualify_before_any_mutation() {
    let mut a11y = fixture();
    assert!(a11y.nodes.push(PARENT, Node::new(Role::Group)));
    let start = checkpoint(&a11y);
    menu_leaf(&mut a11y);
    assert!(a11y.nodes.push_leaf(ITEM, Node::new(Role::Label)));
    a11y.record_focus_without_id(FocusId::default());
    let fragment = capture(&a11y, &start).unwrap();
    a11y.nodes.pop();
    publish(&mut a11y);

    a11y.begin_frame();
    assert!(!replay(&mut a11y, None, Point::default()));
    assert!(!replay(&mut a11y, Some(&fragment), Point::default()));
    assert!(a11y.nodes.push(PARENT, Node::new(Role::Group)));
    assert!(!replay(&mut a11y, Some(&fragment), point(px(1.), px(0.))));
    assert!(a11y.nodes.push_leaf(ITEM, Node::new(Role::Label)));
    assert!(!replay(&mut a11y, Some(&fragment), Point::default()));
    assert!(!a11y.nodes.has_node(MENU));
    assert!(a11y.candidate_missing_focus.is_empty());
    a11y.nodes.pop();
    assert_eq!(node(&publish(&mut a11y), PARENT).children(), &[ITEM]);

    let mut other_window = fixture();
    assert!(other_window.nodes.push(PARENT, Node::new(Role::Group)));
    assert!(!can_replay(
        &other_window,
        Some(&fragment),
        Point::default()
    ));

    set_requested_active(&a11y.active_state, false);
    a11y.sync_active_flag();
    assert!(replay(&mut a11y, None, point(px(1.), px(0.))));
    assert!(capture(&a11y, &checkpoint(&a11y)).is_none());
    start_active(&mut a11y);
    assert!(a11y.nodes.push(PARENT, Node::new(Role::Group)));
    assert!(!can_replay(&a11y, Some(&fragment), Point::default()));
}

#[::std::prelude::v1::test]
fn unsupported_scopes_and_external_parent_edits_are_ineligible() {
    let mut a11y = fixture();
    let start = checkpoint(&a11y);
    menu_leaf(&mut a11y);
    let fragment = capture(&a11y, &start).unwrap();
    publish(&mut a11y);
    a11y.begin_frame();
    for scope in [
        AccessibilityTreeScope::ModalRoot,
        AccessibilityTreeScope::ModalDescendant,
        AccessibilityTreeScope::Excluded,
    ] {
        let start = checkpoint(&a11y);
        {
            let _scope = a11y.nodes.enter_scope(scope);
            assert!(!replay(&mut a11y, Some(&fragment), Point::default()));
        }
        assert!(capture(&a11y, &start).is_none());
    }
    let start = checkpoint(&a11y);
    let normal_parent = a11y.nodes.reserve_deferred_parent().unwrap();
    let portal_parent = a11y.nodes.reserve_window_portal_parent().unwrap();
    assert!(capture(&a11y, &start).is_none());
    {
        let _deferred = a11y.nodes.enter_deferred_parent_scope(normal_parent);
        assert!(!replay(&mut a11y, Some(&fragment), Point::default()));
    }
    {
        let _portal = a11y.nodes.enter_window_portal_scope(portal_parent);
        assert!(!replay(&mut a11y, Some(&fragment), Point::default()));
    }
    {
        let _clip = a11y.nodes.enter_clip_owner_scope();
        assert!(!replay(&mut a11y, Some(&fragment), Point::default()));
    }
    {
        let validity = crate::geometry::SubtreeGeometryValidity::new(None);
        let _geometry = a11y.nodes.enter_geometry_validity(Some(validity));
        let _identity = a11y.nodes.enter_geometry_validity(None);
        assert!(!replay(&mut a11y, Some(&fragment), Point::default()));
    }
    assert!(!a11y.nodes.has_node(MENU));

    let start = checkpoint(&a11y);
    block_capture(&mut a11y);
    assert!(capture(&a11y, &start).is_none());
    let start = checkpoint(&a11y);
    a11y.nodes
        .current_node_mut()
        .unwrap()
        .set_label("Changed parent");
    assert!(capture(&a11y, &start).is_none());
    let start = checkpoint(&a11y);
    assert!(a11y.nodes.push(PARENT, Node::new(Role::Group)));
    assert!(capture(&a11y, &start).is_none());
}

#[crate::test]
fn retained_native_action_listeners_keep_order_and_state_through_rejected_replay(
    cx: &mut TestAppContext,
) {
    let handle = cx.add_window(|_, _| EmptyView);
    let calls = Rc::new(RefCell::new(Vec::new()));
    cx.update_window(handle.into(), |_, window, cx| {
        start_active(&mut window.a11y);
        let start = checkpoint(&window.a11y);
        menu_leaf(&mut window.a11y);
        let prepaint = capture(&window.a11y, &start).unwrap();
        let start = checkpoint(&window.a11y);
        for name in ["first", "second"] {
            let calls = calls.clone();
            let mut count = 0;
            window.on_a11y_action(MENU, Action::Click, move |_, _, _| {
                count += 1;
                calls.borrow_mut().push((name, count));
            });
        }
        let paint = capture(&window.a11y, &start).unwrap();
        assert!(paint.nodes.is_empty());
        publish(&mut window.a11y);
        let accepted_revision = window.a11y.published_revision_for_test();
        let activation = window.a11y.activation_generation();
        let request = || ActionRequest {
            action: Action::Click,
            target_node: MENU,
            target_tree: TreeId::ROOT,
            data: None,
        };
        assert!(window.handle_a11y_action(activation, request(), cx));

        window.a11y.begin_frame();
        assert!(replay(&mut window.a11y, Some(&prepaint), Point::default()));
        assert!(replay(&mut window.a11y, Some(&paint), Point::default()));
        window.a11y.end_frame();
        window.a11y.discard_candidate_frame();
        assert_eq!(window.a11y.published_revision_for_test(), accepted_revision);
        assert!(window.handle_a11y_action(activation, request(), cx));

        window.a11y.begin_frame();
        assert!(replay(&mut window.a11y, Some(&prepaint), Point::default()));
        assert!(replay(&mut window.a11y, Some(&paint), Point::default()));
        publish(&mut window.a11y);
        assert!(window.handle_a11y_action(activation, request(), cx));
    })
    .unwrap();
    assert_eq!(
        calls.borrow().as_slice(),
        &[
            ("first", 1),
            ("second", 1),
            ("first", 2),
            ("second", 2),
            ("first", 3),
            ("second", 3),
        ]
    );
}
