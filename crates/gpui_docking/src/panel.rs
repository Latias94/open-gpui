use crate::{DockHost, DockItemId};
use open_gpui::{AnyView, Context};
use std::{cell::OnceCell, collections::HashMap, fmt, rc::Rc};

type DockPanelFactory = Rc<dyn Fn(&mut Context<DockHost>) -> AnyView>;

/// Renderable content and metadata for one dock item.
pub struct DockPanel {
    inner: Rc<DockPanelInner>,
}

#[derive(Clone)]
struct DockPanelInner {
    title: String,
    closable: bool,
    content: DockPanelContent,
}

#[derive(Clone)]
enum DockPanelContent {
    View(AnyView),
    Lazy {
        factory: DockPanelFactory,
        view: OnceCell<AnyView>,
    },
}

impl fmt::Debug for DockPanel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DockPanel")
            .field("title", &self.title())
            .field("closable", &self.is_closable())
            .field("content", &self.inner.content)
            .finish()
    }
}

impl fmt::Debug for DockPanelContent {
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

impl Clone for DockPanel {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl DockPanel {
    /// Creates a panel registration with the default close policy.
    pub fn new(title: impl Into<String>, view: impl Into<AnyView>) -> Self {
        Self {
            inner: Rc::new(DockPanelInner {
                title: title.into(),
                closable: true,
                content: DockPanelContent::View(view.into()),
            }),
        }
    }

    /// Creates a lazily instantiated panel registration with the default close policy.
    pub fn lazy(
        title: impl Into<String>,
        factory: impl Fn(&mut Context<DockHost>) -> AnyView + 'static,
    ) -> Self {
        Self {
            inner: Rc::new(DockPanelInner {
                title: title.into(),
                closable: true,
                content: DockPanelContent::Lazy {
                    factory: Rc::new(factory),
                    view: OnceCell::new(),
                },
            }),
        }
    }

    /// Sets whether the panel can be closed by future interaction layers.
    pub fn closable(mut self, closable: bool) -> Self {
        Rc::make_mut(&mut self.inner).closable = closable;
        self
    }

    /// Returns the panel title shown in tab chrome.
    pub fn title(&self) -> &str {
        &self.inner.title
    }

    /// Returns whether the panel can be closed by future interaction layers.
    pub fn is_closable(&self) -> bool {
        self.inner.closable
    }

    /// Returns the already-instantiated GPUI view used as this panel's rendered root.
    ///
    /// Lazy panels instantiate when rendered through [`Self::resolve_view`]. Calling this before a
    /// lazy panel has rendered will panic.
    pub fn view(&self) -> &AnyView {
        match &self.inner.content {
            DockPanelContent::View(view) => view,
            DockPanelContent::Lazy { view, .. } => view
                .get()
                .expect("lazy dock panel has not been instantiated"),
        }
    }

    /// Returns true when this panel has an instantiated view.
    pub fn has_view(&self) -> bool {
        match &self.inner.content {
            DockPanelContent::View(_) => true,
            DockPanelContent::Lazy { view, .. } => view.get().is_some(),
        }
    }

    /// Returns the panel view, instantiating lazy panels on first render.
    pub fn resolve_view(&self, cx: &mut Context<DockHost>) -> AnyView {
        match &self.inner.content {
            DockPanelContent::View(view) => view.clone(),
            DockPanelContent::Lazy { factory, view } => view.get_or_init(|| factory(cx)).clone(),
        }
    }
}

/// Result of resolving a dock item against a panel registry.
#[derive(Debug)]
pub enum DockPanelResolution<'a> {
    /// The dock item has registered panel content.
    Registered(&'a DockPanel),
    /// The dock item exists in the graph but has no registered content.
    Missing {
        /// The missing dock item id.
        item: &'a DockItemId,
    },
}

impl DockPanelResolution<'_> {
    /// Returns true when the item has no registered panel content.
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing { .. })
    }
}

/// Registry mapping pure graph item IDs to renderable panel roots.
#[derive(Debug, Default)]
pub struct DockPanelRegistry {
    panels: HashMap<DockItemId, DockPanel>,
}

impl DockPanelRegistry {
    /// Creates an empty panel registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a panel for a dock item, returning any previous registration.
    pub fn register(&mut self, item: impl Into<DockItemId>, panel: DockPanel) -> Option<DockPanel> {
        self.panels.insert(item.into(), panel)
    }

    /// Registers a view with a title for a dock item.
    pub fn register_view(
        &mut self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        view: impl Into<AnyView>,
    ) -> Option<DockPanel> {
        self.register(item, DockPanel::new(title, view))
    }

    /// Registers a lazily created view factory for a dock item.
    pub fn register_factory(
        &mut self,
        item: impl Into<DockItemId>,
        title: impl Into<String>,
        factory: impl Fn(&mut Context<DockHost>) -> AnyView + 'static,
    ) -> Option<DockPanel> {
        self.register(item, DockPanel::lazy(title, factory))
    }

    /// Returns a registered panel by dock item id.
    pub fn get(&self, item: &DockItemId) -> Option<&DockPanel> {
        self.panels.get(item)
    }

    /// Resolves a dock item to either registered content or a missing-panel state.
    pub fn resolve<'a>(&'a self, item: &'a DockItemId) -> DockPanelResolution<'a> {
        self.get(item)
            .map(DockPanelResolution::Registered)
            .unwrap_or(DockPanelResolution::Missing { item })
    }

    /// Returns true when a dock item has registered content.
    pub fn contains(&self, item: &DockItemId) -> bool {
        self.panels.contains_key(item)
    }

    /// Returns the number of registered panels.
    pub fn len(&self) -> usize {
        self.panels.len()
    }

    /// Returns true when no panels are registered.
    pub fn is_empty(&self) -> bool {
        self.panels.is_empty()
    }
}
