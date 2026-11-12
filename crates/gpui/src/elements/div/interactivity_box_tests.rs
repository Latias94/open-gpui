use super::*;
use crate::{TestAppContext, window::ElementArenaScope};

#[test]
fn div_built_outside_draw_outlives_arena_clear_and_drops_its_resources() {
    let resource = Rc::new(());
    let retained = Rc::downgrade(&resource);
    let element = div().retain_for_frame(resource);
    assert!(matches!(&element.interactivity, InteractivityBox::Heap(_)));

    let cx = TestAppContext::single();
    cx.update(|cx| {
        let scope = ElementArenaScope::enter(&cx.element_arena);
        drop(div());
        scope.exit().clear(cx);
    });

    let element = element.id("after-draw");
    assert_eq!(Element::id(&element), Some("after-draw".into()));
    assert!(retained.upgrade().is_some());
    drop(element);
    assert!(retained.upgrade().is_none());
}

#[test]
fn arena_div_resources_survive_nested_clear_until_outer_draw_ends() {
    let cx = TestAppContext::single();
    cx.update(|cx| {
        let outer_scope = ElementArenaScope::enter(&cx.element_arena);
        let outer_resource = Rc::new(());
        let outer_retained = Rc::downgrade(&outer_resource);
        let outer = div().retain_for_frame(outer_resource);
        assert!(matches!(&outer.interactivity, InteractivityBox::Arena(_)));

        let nested_scope = ElementArenaScope::enter(&cx.element_arena);
        let nested_resource = Rc::new(());
        let nested_retained = Rc::downgrade(&nested_resource);
        let nested = div().retain_for_frame(nested_resource);
        assert!(matches!(&nested.interactivity, InteractivityBox::Arena(_)));
        drop(nested);
        nested_scope.exit().clear(cx);

        let outer = outer.id("after-nested-draw");
        assert_eq!(Element::id(&outer), Some("after-nested-draw".into()));
        drop(outer);
        assert!(outer_retained.upgrade().is_some());
        assert!(nested_retained.upgrade().is_some());

        outer_scope.exit().clear(cx);
        assert!(outer_retained.upgrade().is_none());
        assert!(nested_retained.upgrade().is_none());
    });
}

#[cfg(any(feature = "inspector", debug_assertions))]
#[test]
fn div_preserves_call_site_on_heap_and_in_arena() {
    let cx = TestAppContext::single();
    cx.update(|cx| {
        for during_draw in [false, true] {
            let scope = during_draw.then(|| ElementArenaScope::enter(&cx.element_arena));
            let caller_line = line!() + 1;
            let element = div().id("caller-location");
            let location = Element::source_location(&element).unwrap();
            assert_eq!(location.file(), file!());
            assert_eq!(location.line(), caller_line);
            drop(element);
            if let Some(scope) = scope {
                scope.exit().clear(cx);
            }
        }
    });
}
