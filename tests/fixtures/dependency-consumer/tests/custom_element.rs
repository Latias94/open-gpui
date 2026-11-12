use open_gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, ParentElement as _, Pixels, Style, Window, div,
};
use std::panic::Location;

struct CustomElement {
    value: usize,
    source: &'static Location<'static>,
}

impl CustomElement {
    #[track_caller]
    fn new(value: usize) -> Self {
        Self {
            value,
            source: Location::caller(),
        }
    }
}

impl Element for CustomElement {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some("independent-custom-element".into())
    }

    fn source_location(&self) -> Option<&'static Location<'static>> {
        Some(self.source)
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        (window.request_layout(Style::default(), [], cx), ())
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Window,
        _: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        _: &mut Self::PrepaintState,
        _: &mut Window,
        _: &mut App,
    ) {
    }
}

#[test]
fn downstream_custom_element_needs_only_element_to_be_a_child() {
    let source_line = line!() + 1;
    let element = CustomElement::new(7);
    let element: CustomElement = element.into_element();
    assert_eq!(element.value, 7);
    assert_eq!(element.id(), Some("independent-custom-element".into()));
    assert_eq!(element.source_location().unwrap().file(), file!());
    assert_eq!(element.source_location().unwrap().line(), source_line);

    let _parent = div().child(element).children([CustomElement::new(8)]);
}

#[test]
fn downstream_any_element_erasure_preserves_the_custom_element_identity() {
    let mut element = CustomElement::new(41).into_any();
    let custom = element.downcast_mut::<CustomElement>().unwrap();
    custom.value += 1;
    let identity = std::ptr::from_mut(custom);

    let mut element = element
        .into_any()
        .into_any_element()
        .into_element()
        .into_any();
    assert!(element.downcast_mut::<AnyElement>().is_none());
    let custom = element.downcast_mut::<CustomElement>().unwrap();
    assert!(std::ptr::eq(custom, identity));
    assert_eq!(custom.value, 42);
}
