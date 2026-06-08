use open_gpui::{AnyView, App};
use std::{cell::OnceCell, fmt, rc::Rc};
use thiserror::Error;

type DockPanelFactory = Rc<dyn Fn(&mut App) -> AnyView>;

/// Error returned when reading already-instantiated panel view state fails.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DockPanelViewError {
    /// The panel is lazy and has not been rendered or otherwise resolved yet.
    #[error("lazy dock panel view has not been instantiated")]
    LazyViewNotInstantiated,
}

#[derive(Clone)]
pub(crate) struct DockPanelViewHandle {
    inner: Rc<DockPanelViewLifecycle>,
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

    pub(crate) fn view(&self) -> Result<&AnyView, DockPanelViewError> {
        self.inner.view()
    }

    pub(crate) fn has_view(&self) -> bool {
        self.inner.has_view()
    }

    pub(crate) fn resolve_view(&self, cx: &mut App) -> AnyView {
        self.inner.resolve_view(cx)
    }
}

impl DockPanelViewLifecycle {
    fn view(&self) -> Result<&AnyView, DockPanelViewError> {
        match &self.source {
            DockPanelViewSource::View(view) => Ok(view),
            DockPanelViewSource::Lazy { view, .. } => view
                .get()
                .ok_or(DockPanelViewError::LazyViewNotInstantiated),
        }
    }

    fn has_view(&self) -> bool {
        match &self.source {
            DockPanelViewSource::View(_) => true,
            DockPanelViewSource::Lazy { view, .. } => view.get().is_some(),
        }
    }

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
