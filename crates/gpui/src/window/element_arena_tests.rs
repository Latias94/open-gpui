use super::*;
use crate::{TestAppContext, canvas, div};
use std::panic::{AssertUnwindSafe, catch_unwind};

struct EmptyView;

impl Render for EmptyView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

struct OpensWindowOnPaint {
    opened: Rc<Cell<bool>>,
    following_sibling_paints: Rc<Cell<usize>>,
}

impl Render for OpensWindowOnPaint {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let opened = self.opened.clone();
        let following_sibling_paints = self.following_sibling_paints.clone();
        div()
            .size_full()
            .child(canvas(
                |_, _, _| {},
                move |_, _, _, cx| {
                    if !opened.replace(true) {
                        cx.open_window(WindowOptions::default(), |_, cx| cx.new(|_| EmptyView))
                            .unwrap();
                    }
                },
            ))
            .child(div().child("after").child(canvas(
                |_, _, _| {},
                move |_, _, _, _| {
                    following_sibling_paints.set(following_sibling_paints.get() + 1);
                },
            )))
    }
}

#[test]
fn test_window_opened_during_draw_defers_arena_clear() {
    let mut cx = TestAppContext::single();
    let opened = Rc::new(Cell::new(false));
    let following_sibling_paints = Rc::new(Cell::new(0));
    let window = cx.add_window({
        let opened = opened.clone();
        let following_sibling_paints = following_sibling_paints.clone();
        move |_, _| OpensWindowOnPaint {
            opened,
            following_sibling_paints,
        }
    });

    assert!(opened.get());
    assert_eq!(cx.windows().len(), 2);
    assert!(following_sibling_paints.get() > 0);
    let paints = following_sibling_paints.get();
    for handle in cx.windows() {
        cx.update_window(handle, |_, window, cx| {
            window.refresh();
            window.draw(cx).clear(cx);
        })
        .unwrap();
    }
    assert!(following_sibling_paints.get() > paints);
    cx.update_window(window.into(), |_, window, cx| window.draw(cx).clear(cx))
        .unwrap();
}

#[test]
fn nested_app_scopes_restore_allocation_and_clear_authority() {
    let first = TestAppContext::single();
    let second = first.new_app();
    first.update(|cx| {
        let outer_scope = ElementArenaScope::enter(&cx.element_arena);
        let outer = with_element_arena(|arena| arena.alloc(|| 42u64));
        second.update(|cx| {
            let nested_scope = ElementArenaScope::enter(&cx.element_arena);
            let nested = with_element_arena(|arena| arena.alloc(|| 7u64));
            nested_scope.exit().clear(cx);
            assert!(catch_unwind(AssertUnwindSafe(|| *nested)).is_err());
        });
        assert_eq!(*outer, 42);
        let restored = with_element_arena(|arena| arena.alloc(|| 99u64));
        outer_scope.exit().clear(cx);
        assert!(catch_unwind(AssertUnwindSafe(|| *outer)).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| *restored)).is_err());
    });
}

#[test]
fn unwinding_a_nested_scope_preserves_outer_allocations_and_allows_cleanup() {
    let cx = TestAppContext::single();
    cx.update(|cx| {
        let outer_scope = ElementArenaScope::enter(&cx.element_arena);
        let outer = with_element_arena(|arena| arena.alloc(|| 42u64));
        let result = catch_unwind(AssertUnwindSafe(|| {
            let _nested_scope = ElementArenaScope::enter(&cx.element_arena);
            let _nested = with_element_arena(|arena| arena.alloc(|| 7u64));
            panic!("abandoned nested draw");
        }));
        assert!(result.is_err());
        assert_eq!(*outer, 42);
        assert!(CURRENT_ELEMENT_ARENA.with(|current| current.get().is_some()));
        outer_scope.exit().clear(cx);
        assert!(CURRENT_ELEMENT_ARENA.with(|current| current.get().is_none()));
        assert!(catch_unwind(AssertUnwindSafe(|| *outer)).is_err());
        let next_scope = ElementArenaScope::enter(&cx.element_arena);
        assert_eq!(*with_element_arena(|arena| arena.alloc(|| 99u64)), 99);
        next_scope.exit().clear(cx);
    });
}

#[test]
fn clear_token_rejects_another_app_without_clearing_its_allocations() {
    let first = TestAppContext::single();
    let second = first.new_app();
    let (token, first_value) = first.update(|cx| {
        let scope = ElementArenaScope::enter(&cx.element_arena);
        let value = with_element_arena(|arena| arena.alloc(|| 42u64));
        (scope.exit(), value)
    });
    second.update(|cx| {
        let scope = ElementArenaScope::enter(&cx.element_arena);
        let second_value = with_element_arena(|arena| arena.alloc(|| 7u64));
        assert!(catch_unwind(AssertUnwindSafe(|| token.clear(cx))).is_err());
        assert_eq!(*second_value, 7);
        assert_eq!(*first_value, 42);
        scope.exit().clear(cx);
    });
    first.update(|cx| ElementArenaScope::enter(&cx.element_arena).exit().clear(cx));
    assert!(catch_unwind(AssertUnwindSafe(|| *first_value)).is_err());
}

#[test]
fn scope_owns_its_allocation_and_token_keeps_only_identity() {
    let arena = Rc::new(RefCell::new(Arena::new(1024)));
    let allocation = Rc::downgrade(&arena);
    let scope = ElementArenaScope::enter(&arena);
    let value = with_element_arena(|arena| arena.alloc(|| 42u64));
    drop(arena);
    assert!(allocation.upgrade().is_some());
    assert_eq!(*value, 42);
    let token = scope.exit();
    assert!(allocation.upgrade().is_none());
    assert!(CURRENT_ELEMENT_ARENA.with(|current| current.get().is_none()));
    assert!(catch_unwind(AssertUnwindSafe(|| *value)).is_err());
    assert_eq!(token.arena.as_ptr(), allocation.as_ptr());

    let cx = TestAppContext::single();
    cx.update(|cx| {
        assert!(catch_unwind(AssertUnwindSafe(|| token.clear(cx))).is_err());
    });
}
