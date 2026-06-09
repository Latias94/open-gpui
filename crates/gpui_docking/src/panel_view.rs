use crate::DockItemId;
use open_gpui::{AnyView, App};
use std::{cell::OnceCell, collections::HashMap, fmt, rc::Rc};

type DockPanelFactory = Rc<dyn Fn(&mut App) -> AnyView>;

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
            }),
        }
    }

    pub(crate) fn resolve_view(&self, cx: &mut App) -> AnyView {
        self.inner.resolve_view(cx)
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
}

impl fmt::Debug for DockPanelViewHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl fmt::Debug for DockPanelViewLifecycle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.source.fmt(f)
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
