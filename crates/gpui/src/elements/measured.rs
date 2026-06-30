use crate::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Window,
};
use std::rc::Rc;

/// Builds a wrapper that reports an element's layout-pass bounds during prepaint.
///
/// This is intentionally toolkit-neutral: callers provide the semantic identifier
/// and decide how to store or interpret the reported bounds.
pub fn measured_element(
    id: impl Into<ElementId>,
    child: impl IntoElement,
    listener: impl Fn(&ElementId, Bounds<Pixels>, Option<&GlobalElementId>, &mut Window, &mut App)
    + 'static,
) -> MeasuredElement {
    MeasuredElement {
        id: id.into(),
        child: Some(child.into_any_element()),
        listener: Rc::new(listener),
    }
}

/// An element wrapper that reports its own computed bounds without affecting child layout.
pub struct MeasuredElement {
    id: ElementId,
    child: Option<AnyElement>,
    listener: Rc<
        dyn Fn(&ElementId, Bounds<Pixels>, Option<&GlobalElementId>, &mut Window, &mut App)
            + 'static,
    >,
}

impl Element for MeasuredElement {
    type RequestLayoutState = AnyElement;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut child = self.child.take().expect("measured element child missing");
        let layout_id = child.request_layout(window, cx);
        (layout_id, child)
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        (self.listener)(&self.id, bounds, global_id, window, cx);
        child.prepaint(window, cx);
    }

    fn paint(
        &mut self,
        _global_id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        child: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        child.paint(window, cx);
    }
}

impl IntoElement for MeasuredElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Empty, ParentElement as _, Styled as _, TestAppContext, VisualTestContext, div, point, px,
        size,
    };
    use std::{cell::RefCell, rc::Rc};

    #[derive(Clone, Debug)]
    struct MeasuredBounds {
        id: ElementId,
        bounds: Bounds<Pixels>,
        global_id: Option<String>,
    }

    #[crate::test]
    fn measured_element_reports_nested_layout_pass_bounds(cx: &mut TestAppContext) {
        let reported = Rc::new(RefCell::new(Vec::new()));
        let window = cx.add_window(|_, _| Empty);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        cx.draw(
            point(px(10.0), px(20.0)),
            size(px(100.0), px(80.0)),
            |_, _| {
                let report_root = reported.clone();
                let report_child = reported.clone();

                measured_element(
                    "semantic-root".to_string(),
                    div().size_full().child(measured_element(
                        "semantic-child".to_string(),
                        div().w(px(30.0)).h(px(20.0)),
                        move |id, bounds, global_id, _, _| {
                            report_child.borrow_mut().push(MeasuredBounds {
                                id: id.clone(),
                                bounds,
                                global_id: global_id.map(ToString::to_string),
                            });
                        },
                    )),
                    move |id, bounds, global_id, _, _| {
                        report_root.borrow_mut().push(MeasuredBounds {
                            id: id.clone(),
                            bounds,
                            global_id: global_id.map(ToString::to_string),
                        });
                    },
                )
            },
        );

        let reported = reported.borrow();
        let root = reported
            .iter()
            .find(|entry| entry.id == ElementId::from("semantic-root".to_string()))
            .expect("root measured bounds");
        let child = reported
            .iter()
            .find(|entry| entry.id == ElementId::from("semantic-child".to_string()))
            .expect("child measured bounds");

        assert_eq!(root.bounds.origin, point(px(10.0), px(20.0)));
        assert_eq!(root.bounds.size, size(px(100.0), px(80.0)));
        assert_eq!(child.bounds.origin, point(px(10.0), px(20.0)));
        assert_eq!(child.bounds.size, size(px(30.0), px(20.0)));
        assert!(
            root.global_id
                .as_deref()
                .is_some_and(|id| id.contains("semantic-root"))
        );
        assert!(
            child
                .global_id
                .as_deref()
                .is_some_and(|id| id.contains("semantic-child"))
        );
    }
}
