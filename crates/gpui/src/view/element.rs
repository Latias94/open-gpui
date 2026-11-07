use super::{View, ViewCacheKey, ViewElementState};
use crate::{
    AnyElement, App, Bounds, Element, ElementId, EntityId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, Pixels, Style, StyleRefinement, Window,
};
use open_gpui_refineable::Refineable;
use std::{any::type_name, mem};

/// The common layout, prepaint and paint element for a [`View`].
#[doc(hidden)]
pub struct ViewElement<V: View> {
    view: Option<V>,
    entity_id: Option<EntityId>,
    cached_style: Option<StyleRefinement>,
    #[cfg(any(feature = "profiler", debug_assertions))]
    view_type_name: &'static str,
    #[cfg(debug_assertions)]
    source: &'static core::panic::Location<'static>,
}

impl<V: View> ViewElement<V> {
    /// Wrap a view using its entity identity, when present.
    #[track_caller]
    pub fn new(view: V) -> Self {
        Self::with_type_name(view, type_name::<V>())
    }

    #[track_caller]
    #[cfg_attr(
        not(any(feature = "profiler", debug_assertions)),
        expect(unused_variables)
    )]
    pub(super) fn with_type_name(view: V, view_type_name: &'static str) -> Self {
        Self {
            entity_id: view.entity_id(),
            view: Some(view),
            cached_style: None,
            #[cfg(any(feature = "profiler", debug_assertions))]
            view_type_name,
            #[cfg(debug_assertions)]
            source: core::panic::Location::caller(),
        }
    }

    pub(super) fn cached(mut self, style: StyleRefinement) -> Self {
        self.cached_style = Some(style);
        self
    }
}

impl<V: View> IntoElement for ViewElement<V> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

#[inline]
fn render_view<V: View>(view: V, window: &mut Window, cx: &mut App) -> AnyElement {
    View::render(view, window, cx).into_any_element()
}

impl<V: View> Element for ViewElement<V> {
    type RequestLayoutState = Option<AnyElement>;
    type PrepaintState = Option<AnyElement>;

    fn id(&self) -> Option<ElementId> {
        self.entity_id.map(ElementId::View)
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        #[cfg(debug_assertions)]
        return Some(self.source);
        #[cfg(not(debug_assertions))]
        return None;
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        if let Some(entity_id) = self.entity_id {
            #[cfg(debug_assertions)]
            window
                .a11y
                .view_type_names
                .insert(entity_id, self.view_type_name);
            #[cfg(feature = "profiler")]
            let view_type_name = self.view_type_name;
            let draw = move |window: &mut Window, cx: &mut App| {
                let source = self;
                let view = &mut source.view;
                request_layout_view(
                    entity_id,
                    source.cached_style.as_ref(),
                    window,
                    cx,
                    &mut |window, cx| {
                        render_view(view.take().expect("view render consumed once"), window, cx)
                    },
                )
            };
            #[cfg(feature = "profiler")]
            return window.profile_view_phase(
                view_type_name,
                crate::profiler::ViewPhase::RequestLayout,
                cx,
                draw,
            );
            #[cfg(not(feature = "profiler"))]
            return draw(window, cx);
        }
        request_layout_component(type_name::<V>(), window, cx, &mut |window, cx| {
            render_view(
                self.view.take().expect("component render consumed once"),
                window,
                cx,
            )
        })
    }

    fn prepaint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        element: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        if let Some(entity_id) = self.entity_id {
            #[cfg(feature = "profiler")]
            let view_type_name = self.view_type_name;
            let draw = move |window: &mut Window, cx: &mut App| {
                let source = self;
                prepaint_view(
                    entity_id,
                    global_id,
                    bounds,
                    element,
                    window,
                    cx,
                    &mut |window, cx| {
                        render_view(
                            source
                                .view
                                .take()
                                .expect("uncached view render consumed once"),
                            window,
                            cx,
                        )
                    },
                )
            };
            #[cfg(feature = "profiler")]
            return window.profile_view_phase(
                view_type_name,
                crate::profiler::ViewPhase::Prepaint,
                cx,
                draw,
            );
            #[cfg(not(feature = "profiler"))]
            return draw(window, cx);
        }
        prepaint_component(type_name::<V>(), element, window, cx)
    }

    fn paint(
        &mut self,
        global_id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut Self::RequestLayoutState,
        element: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(entity_id) = self.entity_id {
            let cached = self.cached_style.is_some();
            let draw = move |window: &mut Window, cx: &mut App| {
                let rendered = element;
                paint_view(entity_id, cached, global_id, rendered, window, cx)
            };
            #[cfg(feature = "profiler")]
            window.profile_view_phase(
                self.view_type_name,
                crate::profiler::ViewPhase::Paint,
                cx,
                draw,
            );
            #[cfg(not(feature = "profiler"))]
            draw(window, cx);
        } else {
            paint_component(type_name::<V>(), element, window, cx);
        }
    }
}

#[inline(never)]
fn request_layout_view(
    entity_id: EntityId,
    cached_style: Option<&StyleRefinement>,
    window: &mut Window,
    cx: &mut App,
    render: &mut dyn FnMut(&mut Window, &mut App) -> AnyElement,
) -> (LayoutId, Option<AnyElement>) {
    window.with_rendered_view(entity_id, |window| {
        let caching_disabled = window.is_inspector_picking(cx) || window.a11y.is_active();
        match cached_style {
            Some(style) if !caching_disabled && window.subtree_presentation().paints() => {
                let mut root_style = Style::default();
                root_style.refine(style);
                (window.request_layout(root_style, None, cx), None)
            }
            _ => {
                let mut element = render(window, cx);
                let layout = element.request_layout(window, cx);
                (layout, Some(element))
            }
        }
    })
}

#[inline(never)]
fn prepaint_view(
    entity_id: EntityId,
    global_id: Option<&GlobalElementId>,
    bounds: Bounds<Pixels>,
    element: &mut Option<AnyElement>,
    window: &mut Window,
    cx: &mut App,
    render: &mut dyn FnMut(&mut Window, &mut App) -> AnyElement,
) -> Option<AnyElement> {
    window.set_view_id(entity_id);
    window.with_rendered_view(entity_id, |window| {
        if let Some(mut element) = element.take() {
            element.prepaint(window, cx);
            return Some(element);
        }
        window.with_element_state::<ViewElementState, _>(global_id.unwrap(), |state, window| {
            let clip_stack = window.clip_stack();
            let text_style = window.text_style();
            let opacity = window.element_opacity();
            let subtree_presentation = window.subtree_presentation();
            let subtree_transform = window.subtree_transform();
            if let Some(mut state) = state
                && state.cache_key.bounds == bounds
                && state.cache_key.clip_stack == clip_stack
                && state.cache_key.text_style == text_style
                && state.cache_key.opacity.to_bits() == opacity.to_bits()
                && state.cache_key.subtree_presentation == subtree_presentation
                && state.cache_key.subtree_transform == subtree_transform
                && window.can_reuse_paint(&state.paint_range)
                && !window.dirty_views.contains(&entity_id)
                && !window.refreshing
                && !window.portal_anchor_dependency_invalidates_view(entity_id)
                && !window.portal_anchor_capture_requires_fresh_prepaint()
                && !window.reveal_target_capture_requires_fresh_prepaint()
            {
                let start = window.prepaint_index();
                window.reuse_prepaint(state.prepaint_range.clone());
                cx.entities.extend_accessed(&state.accessed_entities);
                state.prepaint_range = start..window.prepaint_index();
                return (None, state);
            }
            let refreshing = mem::replace(&mut window.refreshing, true);
            let start = window.prepaint_index();
            let (element, accessed_entities) = cx.detect_accessed_entities(|cx| {
                let mut element = render(window, cx);
                let root_layout_id = element.request_layout(window, cx);
                element.layout_as_root(bounds.size.into(), window, cx);
                window.register_portal_anchor_root_layout_alias(root_layout_id);
                window.register_reveal_target_root_layout_alias(root_layout_id);
                element.prepaint_at(bounds.origin, window, cx);
                element
            });
            let end = window.prepaint_index();
            window.refreshing = refreshing;
            (
                Some(element),
                ViewElementState {
                    accessed_entities,
                    prepaint_range: start..end,
                    paint_range: Default::default(),
                    cache_key: ViewCacheKey {
                        bounds,
                        clip_stack,
                        text_style,
                        opacity,
                        subtree_presentation,
                        subtree_transform,
                    },
                },
            )
        })
    })
}

#[inline(never)]
fn paint_view(
    entity_id: EntityId,
    cached: bool,
    global_id: Option<&GlobalElementId>,
    element: &mut Option<AnyElement>,
    window: &mut Window,
    cx: &mut App,
) {
    window.with_rendered_view(entity_id, |window| {
        let caching_disabled = window.is_inspector_picking(cx) || window.a11y.is_active();
        if cached && !caching_disabled {
            window.with_atlas_texture_lease_paint_scope(|window| {
                window.with_element_state::<ViewElementState, _>(
                    global_id.unwrap(),
                    |state, window| {
                        let mut state = state.unwrap();
                        let start = window.paint_index();
                        let opacity = window.element_opacity();
                        let succeeded = if let Some(element) = element {
                            let refreshing = mem::replace(&mut window.refreshing, true);
                            let ((), accessed) =
                                cx.detect_accessed_entities(|cx| element.paint(window, cx));
                            state.accessed_entities.extend(accessed);
                            window.refreshing = refreshing;
                            if state.cache_key.opacity.to_bits() != opacity.to_bits() {
                                window
                                    .record_opacity_rebuild_view(entity_id, &state.prepaint_range);
                            }
                            state.cache_key.opacity = opacity;
                            true
                        } else {
                            let reused = window.reuse_paint(state.paint_range.clone());
                            if reused && state.cache_key.opacity.to_bits() != opacity.to_bits() {
                                window.request_animation_frame();
                            }
                            reused
                        };
                        if succeeded {
                            state.paint_range = start..window.paint_index();
                        } else {
                            window.on_next_frame(move |_, cx| cx.notify(entity_id));
                        }
                        ((), state)
                    },
                )
            });
        } else {
            element.as_mut().unwrap().paint(window, cx);
        }
    });
}

#[inline(never)]
fn request_layout_component(
    name: &'static str,
    window: &mut Window,
    cx: &mut App,
    render: &mut dyn FnMut(&mut Window, &mut App) -> AnyElement,
) -> (LayoutId, Option<AnyElement>) {
    window.with_id(ElementId::from(name), |window| {
        let mut element = render(window, cx);
        let layout = element.request_layout(window, cx);
        (layout, Some(element))
    })
}

#[inline(never)]
fn prepaint_component(
    name: &'static str,
    element: &mut Option<AnyElement>,
    window: &mut Window,
    cx: &mut App,
) -> Option<AnyElement> {
    window.with_id(ElementId::from(name), |window| {
        let mut element = element.take().unwrap();
        element.prepaint(window, cx);
        Some(element)
    })
}

#[inline(never)]
fn paint_component(
    name: &'static str,
    element: &mut Option<AnyElement>,
    window: &mut Window,
    cx: &mut App,
) {
    window.with_id(ElementId::from(name), |window| {
        element.as_mut().unwrap().paint(window, cx)
    });
}
