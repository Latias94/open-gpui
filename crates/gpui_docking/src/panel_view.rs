use crate::DockItemId;
use open_gpui::{AnyView, App, Entity, FocusHandle, Focusable, Render};
use std::{cell::OnceCell, collections::HashMap, fmt, rc::Rc};

type DockPanelFactory = Rc<dyn Fn(&mut App) -> AnyView>;
type DockPanelFocusResolver = Rc<dyn Fn(&AnyView, &App) -> Option<FocusHandle>>;

#[derive(Clone)]
pub(crate) struct DockPanelViewHandle {
    inner: Rc<DockPanelViewLifecycle>,
}

#[derive(Debug, Default)]
pub(crate) struct DockPanelViewStore {
    views: HashMap<DockItemId, DockPanelViewHandle>,
}

struct DockPanelViewLifecycle {
    source: DockPanelViewSource,
    focus_resolver: Option<DockPanelFocusResolver>,
}

enum DockPanelViewSource {
    View(AnyView),
    Lazy {
        factory: DockPanelFactory,
        view: OnceCell<AnyView>,
    },
}

impl DockPanelViewHandle {
    pub(crate) fn from_view(view: impl Into<AnyView>) -> Self {
        Self {
            inner: Rc::new(DockPanelViewLifecycle {
                source: DockPanelViewSource::View(view.into()),
                focus_resolver: None,
            }),
        }
    }

    pub(crate) fn focusable_view<V>(view: Entity<V>) -> Self
    where
        V: Focusable + Render,
    {
        Self {
            inner: Rc::new(DockPanelViewLifecycle {
                source: DockPanelViewSource::View(view.into()),
                focus_resolver: Some(focus_resolver::<V>()),
            }),
        }
    }

    pub(crate) fn lazy(factory: impl Fn(&mut App) -> AnyView + 'static) -> Self {
        Self {
            inner: Rc::new(DockPanelViewLifecycle {
                source: DockPanelViewSource::Lazy {
                    factory: Rc::new(factory),
                    view: OnceCell::new(),
                },
                focus_resolver: None,
            }),
        }
    }

    pub(crate) fn lazy_focusable<V>(factory: impl Fn(&mut App) -> Entity<V> + 'static) -> Self
    where
        V: Focusable + Render,
    {
        Self {
            inner: Rc::new(DockPanelViewLifecycle {
                source: DockPanelViewSource::Lazy {
                    factory: Rc::new(move |cx| factory(cx).into()),
                    view: OnceCell::new(),
                },
                focus_resolver: Some(focus_resolver::<V>()),
            }),
        }
    }

    pub(crate) fn resolve_view(&self, cx: &mut App) -> AnyView {
        self.inner.resolve_view(cx)
    }

    pub(crate) fn focus_handle(&self, cx: &mut App) -> Option<FocusHandle> {
        self.inner.focus_handle(cx)
    }
}

impl DockPanelViewStore {
    pub(crate) fn register(
        &mut self,
        item: DockItemId,
        view: DockPanelViewHandle,
    ) -> Option<DockPanelViewHandle> {
        self.views.insert(item, view)
    }

    pub(crate) fn view(&self, item: &DockItemId) -> Option<DockPanelViewHandle> {
        self.views.get(item).cloned()
    }

    pub(crate) fn contains(&self, item: &DockItemId) -> bool {
        self.views.contains_key(item)
    }
}

impl DockPanelViewLifecycle {
    fn resolve_view(&self, cx: &mut App) -> AnyView {
        match &self.source {
            DockPanelViewSource::View(view) => view.clone(),
            DockPanelViewSource::Lazy { factory, view } => view.get_or_init(|| factory(cx)).clone(),
        }
    }

    fn focus_handle(&self, cx: &mut App) -> Option<FocusHandle> {
        let Some(resolve_focus) = &self.focus_resolver else {
            return None;
        };
        let view = self.resolve_view(cx);
        resolve_focus(&view, cx)
    }
}

impl fmt::Debug for DockPanelViewHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl fmt::Debug for DockPanelViewLifecycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DockPanelViewLifecycle")
            .field("source", &self.source)
            .field("focusable", &self.focus_resolver.is_some())
            .finish()
    }
}

impl fmt::Debug for DockPanelViewSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::View(_) => f.write_str("View"),
            Self::Lazy { view, .. } => f
                .debug_struct("Lazy")
                .field("instantiated", &view.get().is_some())
                .finish(),
        }
    }
}

fn focus_resolver<V>() -> DockPanelFocusResolver
where
    V: Focusable + Render,
{
    Rc::new(|view, cx| {
        view.clone()
            .downcast::<V>()
            .ok()
            .map(|entity| entity.focus_handle(cx))
    })
}
