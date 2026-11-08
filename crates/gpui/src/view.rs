use crate::{
    AnyElement, AnyEntity, AnyWeakEntity, App, Bounds, Context, Element, Entity, EntityId,
    IntoElement, PaintIndex, Pixels, PrepaintStateIndex, Render, RenderOnce, StyleRefinement,
    SubtreePresentation, TextStyle, WeakEntity, Window, geometry::ResolvedSubtreeTransform,
};
use anyhow::Result;
use open_gpui_collections::FxHashSet;
use std::{
    any::{TypeId, type_name},
    fmt,
    ops::Range,
};

mod element;
pub use element::ViewElement;

struct ViewElementState {
    prepaint_range: Range<PrepaintStateIndex>,
    paint_range: Range<PaintIndex>,
    cache_key: ViewCacheKey,
    /// Entities accessed in any phase of the last draw, restored on prepaint reuse
    /// so the window remains subscribed even when paint callbacks are replayed.
    accessed_entities: FxHashSet<EntityId>,
}

#[derive(Default)]
struct ViewCacheKey {
    bounds: Bounds<Pixels>,
    clip_stack: crate::geometry::ClipStackSnapshot,
    text_style: TextStyle,
    opacity: f32,
    subtree_presentation: SubtreePresentation,
    subtree_transform: ResolvedSubtreeTransform,
}

/// A dynamically typed view handle that embeds through [`ViewElement`].
#[derive(Clone, Debug)]
pub struct AnyView {
    entity: AnyEntity,
    render: fn(&AnyView, &mut Window, &mut App) -> AnyElement,
    view_type_name: &'static str,
}

impl<V: Render> From<Entity<V>> for AnyView {
    fn from(value: Entity<V>) -> Self {
        AnyView {
            entity: value.into_any(),
            render: any_view::render::<V>,
            view_type_name: type_name::<V>(),
        }
    }
}

impl AnyView {
    /// Cache this entity-backed view using the supplied definite layout style.
    ///
    /// [`Context::notify`] invalidates the view and [`Window::refresh`] forces a rebuild.
    /// Inherited style, presentation, geometry and resource validity also constrain reuse.
    /// Reads made during paint remain dependencies. A non-view dependency notifies its
    /// windows; notify the owning view or refresh the window to rebuild cached content.
    /// Hover-dependent opacity discovered after prepaint schedules a follow-up rebuild;
    /// replay retains recorded opacity until that rebuild succeeds. External group changes
    /// require notifying the owning view because replay cannot discover a replacement group.
    #[track_caller]
    pub fn cached(self, style: StyleRefinement) -> ViewElement<AnyView> {
        self.into_element().cached(style)
    }

    /// Convert this to a weak handle.
    pub fn downgrade(&self) -> AnyWeakView {
        AnyWeakView {
            entity: self.entity.downgrade(),
            render: self.render,
            view_type_name: self.view_type_name,
        }
    }

    /// Downcast to an entity, retaining this handle when its type does not match.
    pub fn downcast<T: 'static>(self) -> Result<Entity<T>, Self> {
        match self.entity.downcast() {
            Ok(entity) => Ok(entity),
            Err(entity) => Err(Self {
                entity,
                render: self.render,
                view_type_name: self.view_type_name,
            }),
        }
    }

    /// Return the backing entity's concrete type.
    pub fn entity_type(&self) -> TypeId {
        self.entity.entity_type
    }

    /// Return the backing entity's identity.
    pub fn entity_id(&self) -> EntityId {
        self.entity.entity_id()
    }
}

impl PartialEq for AnyView {
    fn eq(&self, other: &Self) -> bool {
        self.entity == other.entity
    }
}

impl Eq for AnyView {}

impl View for AnyView {
    fn entity_id(&self) -> Option<EntityId> {
        Some(AnyView::entity_id(self))
    }

    #[cfg(not(gpui_hot_reload))]
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        (self.render)(&self, window, cx)
    }

    // The stored function pointer was captured from the original binary, so it must be looked up
    // in the jump table to reach the patched render.
    #[cfg(gpui_hot_reload)]
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        subsecond::HotFn::current(self.render).call((&self, window, cx))
    }
}

impl<V: Render> IntoElement for Entity<V> {
    type Element = ViewElement<Entity<V>>;

    #[track_caller]
    fn into_element(self) -> Self::Element {
        ViewElement::with_type_name(self, type_name::<V>())
    }

    #[track_caller]
    #[inline(never)]
    fn into_any_element(self) -> AnyElement {
        self.into_element().into_any()
    }
}

impl IntoElement for AnyView {
    type Element = ViewElement<AnyView>;

    #[track_caller]
    fn into_element(self) -> Self::Element {
        let view_type_name = self.view_type_name;
        ViewElement::with_type_name(self, view_type_name)
    }

    #[track_caller]
    #[inline(never)]
    fn into_any_element(self) -> AnyElement {
        self.into_element().into_any()
    }
}

/// A weak, dynamically typed view handle that does not retain its entity.
pub struct AnyWeakView {
    entity: AnyWeakEntity,
    render: fn(&AnyView, &mut Window, &mut App) -> AnyElement,
    view_type_name: &'static str,
}

impl AnyWeakView {
    /// Upgrade if the referenced entity has not been released.
    pub fn upgrade(&self) -> Option<AnyView> {
        let entity = self.entity.upgrade()?;
        Some(AnyView {
            entity,
            render: self.render,
            view_type_name: self.view_type_name,
        })
    }
}

impl<V: Render> From<WeakEntity<V>> for AnyWeakView {
    fn from(view: WeakEntity<V>) -> Self {
        AnyWeakView {
            entity: view.into(),
            render: any_view::render::<V>,
            view_type_name: type_name::<V>(),
        }
    }
}

impl PartialEq for AnyWeakView {
    fn eq(&self, other: &Self) -> bool {
        self.entity == other.entity
    }
}

impl fmt::Debug for AnyWeakView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnyWeakView")
            .field("entity_id", &self.entity.entity_id)
            .finish_non_exhaustive()
    }
}

mod any_view {
    use crate::{AnyElement, AnyView, App, IntoElement, Render, Window};

    pub(crate) fn render<V: Render>(
        view: &AnyView,
        window: &mut Window,
        cx: &mut App,
    ) -> AnyElement {
        let view = view.clone().downcast::<V>().unwrap();
        view.update(cx, |view, cx| {
            Render::render(view, window, cx).into_any_element()
        })
    }
}

/// A renderable with optional entity identity, shared by views and stateless components.
///
/// An identity establishes a reactive boundary and its own element-id space. Two siblings
/// at the same element-tree position must not use the same entity identity; their state
/// would collide. Nesting is scoped by the parent path. A view without identity behaves as
/// a stateless component. Implement this directly to combine parent-supplied props with an
/// existing backing entity; `Entity<T: Render>` and [`RenderOnce`] already implement it.
pub trait View: 'static + Sized {
    /// Return the backing entity's identity, or None for a stateless component.
    fn entity_id(&self) -> Option<EntityId>;

    /// Consume this view to produce its element tree.
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement;
}

impl<T: RenderOnce> View for T {
    fn entity_id(&self) -> Option<EntityId> {
        None
    }

    #[inline]
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        RenderOnce::render(self, window, cx)
    }
}

impl<T: Render> View for Entity<T> {
    fn entity_id(&self) -> Option<EntityId> {
        Some(Entity::entity_id(self))
    }

    #[inline]
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        self.update(cx, |this, cx| {
            Render::render(this, window, cx).into_any_element()
        })
    }
}

impl<T: Render> Entity<T> {
    /// Cache this view with a definite size supplied by style, independent of its contents.
    ///
    /// Notification or a window refresh rebuilds the subtree. For content-based measurement,
    /// embed the entity without caching. See [`AnyView::cached`] for other reuse constraints.
    #[track_caller]
    pub fn cached(self, style: StyleRefinement) -> ViewElement<Entity<T>> {
        ViewElement::with_type_name(self, type_name::<T>()).cached(style)
    }
}

/// A view that renders nothing.
pub struct EmptyView;

impl Render for EmptyView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        crate::Empty
    }
}

#[cfg(test)]
mod cache_tests;
