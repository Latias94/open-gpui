use crate::{
    AnyElement, App, Bounds, Element, ElementGeometry, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, Window,
};
use std::rc::Rc;

/// Builds a wrapper that reports an element's geometry after a valid frame commits.
///
/// This is intentionally toolkit-neutral: callers provide the semantic identifier and decide how
/// to store or interpret the immutable snapshot. Failed transform scopes do not invoke the
/// listener, so applications never observe half-committed geometry.
pub fn measured_element(
    id: impl Into<ElementId>,
    child: impl IntoElement,
    listener: impl Fn(MeasuredElementSnapshot, &mut App) + 'static,
) -> MeasuredElement {
    MeasuredElement {
        id: id.into(),
        child: Some(child.into_any_element()),
        listener: Rc::new(listener),
    }
}

/// Geometry and identity committed for a [`MeasuredElement`] in one frame.
#[derive(Clone, Debug, PartialEq)]
pub struct MeasuredElementSnapshot {
    frame_generation: u64,
    id: ElementId,
    global_id: Option<GlobalElementId>,
    geometry: ElementGeometry,
}

impl MeasuredElementSnapshot {
    /// Returns the committed frame generation that produced this snapshot.
    pub const fn frame_generation(&self) -> u64 {
        self.frame_generation
    }

    /// Returns the semantic element identifier supplied to [`measured_element`].
    pub const fn id(&self) -> &ElementId {
        &self.id
    }

    /// Returns GPUI's stable global identifier for this element instance.
    pub const fn global_id(&self) -> Option<&GlobalElementId> {
        self.global_id.as_ref()
    }

    /// Returns the committed layout and displayed geometry.
    pub const fn geometry(&self) -> ElementGeometry {
        self.geometry
    }
}

/// An element wrapper that reports its own committed geometry without affecting child layout.
pub struct MeasuredElement {
    id: ElementId,
    child: Option<AnyElement>,
    listener: Rc<dyn Fn(MeasuredElementSnapshot, &mut App) + 'static>,
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
        if let Ok(geometry) = window.try_element_geometry(bounds) {
            let id = self.id.clone();
            let global_id = global_id.cloned();
            let listener = self.listener.clone();
            window.record_prepaint_commit(move |frame_generation, cx| {
                listener(
                    MeasuredElementSnapshot {
                        frame_generation,
                        id: id.clone(),
                        global_id: global_id.clone(),
                        geometry,
                    },
                    cx,
                );
            });
        }
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
        AnyView, AppContext as _, Context, Empty, Entity, ParentElement as _, Point, Render,
        StyleRefinement, Styled as _, SubtreeTransform, SubtreeTransformExt as _,
        SubtreeTransformOrigin, TestAppContext, VisualContext as _, VisualTestContext, div, point,
        px, size,
    };
    use std::{
        cell::{Cell, RefCell},
        rc::Rc,
    };

    struct CachedMeasuredChild {
        renders: Rc<Cell<usize>>,
        reported: Rc<RefCell<Vec<MeasuredElementSnapshot>>>,
    }

    impl Render for CachedMeasuredChild {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            self.renders.set(self.renders.get() + 1);
            let reported = self.reported.clone();
            measured_element(
                "cached-measured-child",
                div().size_full(),
                move |snapshot, _| reported.borrow_mut().push(snapshot),
            )
        }
    }

    struct CachedMeasuredRoot {
        child: Entity<CachedMeasuredChild>,
        translated: bool,
    }

    impl Render for CachedMeasuredRoot {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            let translation = if self.translated {
                point(px(100.0), px(40.0))
            } else {
                Point::default()
            };
            AnyView::from(self.child.clone())
                .cached(StyleRefinement::default().w(px(20.0)).h(px(10.0)))
                .with_subtree_transform(SubtreeTransform::try_translation(translation).unwrap())
        }
    }

    #[crate::test]
    fn measured_element_reports_nested_committed_layout_and_displayed_geometry(
        cx: &mut TestAppContext,
    ) {
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
                        move |snapshot, _| report_child.borrow_mut().push(snapshot),
                    )),
                    move |snapshot, _| report_root.borrow_mut().push(snapshot),
                )
                .with_subtree_transform(
                    SubtreeTransform::try_new(
                        size(2.0, 3.0),
                        point(px(5.0), px(7.0)),
                        SubtreeTransformOrigin::TOP_LEFT,
                    )
                    .unwrap(),
                )
            },
        );

        let reported = reported.borrow();
        let root = reported
            .iter()
            .find(|entry| entry.id() == &ElementId::from("semantic-root".to_string()))
            .expect("root measured geometry");
        let child = reported
            .iter()
            .find(|entry| entry.id() == &ElementId::from("semantic-child".to_string()))
            .expect("child measured geometry");

        assert_eq!(
            root.geometry().layout_bounds(),
            Bounds::new(point(px(10.0), px(20.0)), size(px(100.0), px(80.0)))
        );
        assert_eq!(
            root.geometry().displayed_bounds(),
            Bounds::new(point(px(15.0), px(27.0)), size(px(200.0), px(240.0)))
        );
        assert_eq!(
            child.geometry().layout_bounds(),
            Bounds::new(point(px(10.0), px(20.0)), size(px(30.0), px(20.0)))
        );
        assert_eq!(
            child.geometry().displayed_bounds(),
            Bounds::new(point(px(15.0), px(27.0)), size(px(60.0), px(60.0)))
        );
        assert_eq!(
            root.geometry()
                .local_to_window_point(point(px(2.0), px(3.0)))
                .unwrap(),
            point(px(19.0), px(36.0))
        );
        assert_eq!(
            root.geometry()
                .window_to_local_point(point(px(19.0), px(36.0)))
                .unwrap(),
            point(px(2.0), px(3.0))
        );
        assert!(
            root.global_id()
                .is_some_and(|id| id.to_string().contains("semantic-root"))
        );
        assert!(
            child
                .global_id()
                .is_some_and(|id| id.to_string().contains("semantic-child"))
        );
        assert_eq!(root.frame_generation(), child.frame_generation());
    }

    #[crate::test]
    fn cached_measured_element_tracks_transform_only_commits(cx: &mut TestAppContext) {
        let renders = Rc::new(Cell::new(0));
        let reported = Rc::new(RefCell::new(Vec::new()));
        let (root, cx) = cx.add_window_view({
            let renders = renders.clone();
            let reported = reported.clone();
            move |_, cx| CachedMeasuredRoot {
                child: cx.new(|_| CachedMeasuredChild { renders, reported }),
                translated: false,
            }
        });

        cx.update(|window, cx| window.draw(cx).clear());
        let initial_renders = renders.get();
        let first = reported.borrow().last().cloned().unwrap();

        cx.update(|window, cx| window.draw(cx).clear());
        assert_eq!(renders.get(), initial_renders);
        let replayed = reported.borrow().last().cloned().unwrap();
        assert!(replayed.frame_generation() > first.frame_generation());
        assert_eq!(replayed.geometry(), first.geometry());

        cx.update_window_entity(&root, |root, _, cx| {
            root.translated = true;
            cx.notify();
        });
        cx.update(|window, cx| window.draw(cx).clear());

        assert!(renders.get() > initial_renders);
        let translated = reported.borrow().last().cloned().unwrap();
        assert_eq!(
            translated.geometry().layout_bounds(),
            first.geometry().layout_bounds()
        );
        assert_eq!(
            translated.geometry().displayed_bounds().origin,
            first.geometry().displayed_bounds().origin + point(px(100.0), px(40.0))
        );
    }

    #[crate::test]
    fn failed_transform_scope_does_not_publish_measured_geometry(cx: &mut TestAppContext) {
        let reported = Rc::new(Cell::new(0));
        let window = cx.add_window(|_, _| Empty);
        let mut cx = VisualTestContext::from_window(window.into(), cx);

        cx.draw(Point::default(), size(px(20.0), px(10.0)), |_, _| {
            let reported = reported.clone();
            measured_element("failed-measurement", div().size_full(), move |_, _| {
                reported.set(reported.get() + 1)
            })
            .with_subtree_transform(
                SubtreeTransform::try_new(
                    size(f32::MAX, 1.0),
                    Point::default(),
                    SubtreeTransformOrigin::TOP_LEFT,
                )
                .unwrap(),
            )
        });

        assert_eq!(reported.get(), 0);
    }
}
