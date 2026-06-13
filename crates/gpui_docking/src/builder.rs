use crate::{
    DockCentralRegion, DockFloatingContainer, DockGraph, DockGraphValidationError, DockItemId,
    DockNode, DockNodeId, DockSpaceId, SplitAxis,
};

/// Convenience builder for programmatic dock layouts.
#[derive(Debug, Default)]
pub struct DockLayoutBuilder {
    graph: DockGraph,
}

impl DockLayoutBuilder {
    /// Creates an empty layout builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts a tabs node.
    pub fn tabs(&mut self, items: impl IntoIterator<Item = impl Into<DockItemId>>) -> DockNodeId {
        let items: Vec<DockItemId> = items.into_iter().map(Into::into).collect();
        let selected = items.first().cloned();
        self.graph.insert_node(DockNode::Tabs { items, selected })
    }

    /// Inserts a tabs node with an explicit selected item.
    pub fn tabs_with_selected(
        &mut self,
        items: impl IntoIterator<Item = impl Into<DockItemId>>,
        selected: impl Into<DockItemId>,
    ) -> DockNodeId {
        let items: Vec<DockItemId> = items.into_iter().map(Into::into).collect();
        let selected = selected.into();
        let selected = items.contains(&selected).then_some(selected);
        self.graph.insert_node(DockNode::Tabs { items, selected })
    }

    /// Inserts a split node with explicit children and fractions.
    pub fn split(
        &mut self,
        axis: SplitAxis,
        children: Vec<DockNodeId>,
        fractions: Vec<f32>,
    ) -> DockNodeId {
        self.graph.insert_node(DockNode::Split {
            axis,
            children,
            fractions,
        })
    }

    /// Inserts a horizontal two-child split.
    pub fn split_horizontal(
        &mut self,
        left: DockNodeId,
        right: DockNodeId,
        left_fraction: f32,
    ) -> DockNodeId {
        let left_fraction = left_fraction.clamp(0.0, 1.0);
        self.split(
            SplitAxis::Horizontal,
            vec![left, right],
            vec![left_fraction, 1.0 - left_fraction],
        )
    }

    /// Inserts a vertical two-child split.
    pub fn split_vertical(
        &mut self,
        top: DockNodeId,
        bottom: DockNodeId,
        top_fraction: f32,
    ) -> DockNodeId {
        let top_fraction = top_fraction.clamp(0.0, 1.0);
        self.split(
            SplitAxis::Vertical,
            vec![top, bottom],
            vec![top_fraction, 1.0 - top_fraction],
        )
    }

    /// Sets a root node for a dock space.
    pub fn set_root(&mut self, space: impl Into<DockSpaceId>, root: DockNodeId) {
        self.graph.set_root(space.into(), root);
    }

    /// Adds an in-window floating container.
    pub fn add_floating(
        &mut self,
        space: impl Into<DockSpaceId>,
        child: DockNodeId,
        bounds: open_gpui::Bounds<open_gpui::Pixels>,
    ) -> DockNodeId {
        let floating = self.graph.insert_node(DockNode::Floating { child });
        self.graph
            .floating_containers_mut(space.into())
            .push(DockFloatingContainer {
                node: floating,
                bounds,
            });
        floating
    }

    /// Sets central region semantics for a dock space.
    pub fn set_central_region(
        &mut self,
        space: impl Into<DockSpaceId>,
        central: DockCentralRegion,
    ) {
        self.graph.set_central_region(space, central);
    }

    /// Finishes the builder and returns a canonical graph without validation.
    pub fn build(mut self) -> DockGraph {
        self.simplify_graph();
        self.graph
    }

    /// Finishes the builder, validates reachable graph state, and returns a canonical graph.
    pub fn try_build(mut self) -> Result<DockGraph, DockGraphValidationError> {
        self.simplify_graph();
        self.graph.validate()?;
        Ok(self.graph)
    }

    fn simplify_graph(&mut self) {
        for space in self.graph.spaces() {
            self.graph.simplify_space(&space);
        }
    }
}

/// Specification for a common editor-style default layout.
#[derive(Debug, Clone)]
pub struct EditorDockLayoutSpec {
    /// Items in the left tab stack.
    pub left_items: Vec<DockItemId>,
    /// Items in the main tab stack.
    pub main_items: Vec<DockItemId>,
    /// Items in the bottom tab stack.
    pub bottom_items: Vec<DockItemId>,
    /// Fraction allocated to the left stack.
    pub left_fraction: f32,
    /// Fraction allocated to the top stack within the right split.
    pub main_fraction: f32,
    /// Selected item in the left stack.
    pub selected_left: Option<DockItemId>,
    /// Selected item in the main stack.
    pub selected_main: Option<DockItemId>,
    /// Selected item in the bottom stack.
    pub selected_bottom: Option<DockItemId>,
}

impl EditorDockLayoutSpec {
    /// Creates an editor-style layout specification.
    pub fn new(
        left_items: impl IntoIterator<Item = impl Into<DockItemId>>,
        main_items: impl IntoIterator<Item = impl Into<DockItemId>>,
        bottom_items: impl IntoIterator<Item = impl Into<DockItemId>>,
    ) -> Self {
        Self {
            left_items: left_items.into_iter().map(Into::into).collect(),
            main_items: main_items.into_iter().map(Into::into).collect(),
            bottom_items: bottom_items.into_iter().map(Into::into).collect(),
            left_fraction: 0.26,
            main_fraction: 0.72,
            selected_left: None,
            selected_main: None,
            selected_bottom: None,
        }
    }

    /// Sets the primary split fractions.
    pub fn with_fractions(mut self, left_fraction: f32, main_fraction: f32) -> Self {
        self.left_fraction = left_fraction;
        self.main_fraction = main_fraction;
        self
    }

    /// Sets selected tab identities.
    pub fn with_selected_items(
        mut self,
        selected_left: impl Into<DockItemId>,
        selected_main: impl Into<DockItemId>,
        selected_bottom: impl Into<DockItemId>,
    ) -> Self {
        self.selected_left = Some(selected_left.into());
        self.selected_main = Some(selected_main.into());
        self.selected_bottom = Some(selected_bottom.into());
        self
    }
}

impl DockGraph {
    /// Builds a common editor-style default layout.
    pub fn default_editor_layout(
        space: impl Into<DockSpaceId>,
        spec: EditorDockLayoutSpec,
    ) -> Self {
        let space = space.into();
        let mut builder = DockLayoutBuilder::new();
        let left = builder.editor_tabs(spec.left_items, spec.selected_left);
        let main = builder.editor_tabs(spec.main_items, spec.selected_main);
        let bottom = builder.editor_tabs(spec.bottom_items, spec.selected_bottom);
        let right = builder.split_vertical(main, bottom, spec.main_fraction);
        let root = builder.split_horizontal(left, right, spec.left_fraction);
        builder.set_root(space.clone(), root);
        builder.set_central_region(space, DockCentralRegion::with_node(main));
        builder.build()
    }
}

impl DockLayoutBuilder {
    fn editor_tabs(&mut self, items: Vec<DockItemId>, selected: Option<DockItemId>) -> DockNodeId {
        if let Some(selected) = selected {
            return self.tabs_with_selected(items, selected);
        }
        self.tabs(items)
    }
}
