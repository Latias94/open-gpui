use crate::{
    DockFloatingContainer, DockGraph, DockGraphValidationError, DockItemId, DockNode, DockNodeId,
    DockSpaceId, SplitAxis,
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
    pub fn tabs(
        &mut self,
        items: impl IntoIterator<Item = impl Into<DockItemId>>,
        active: usize,
    ) -> DockNodeId {
        let items: Vec<DockItemId> = items.into_iter().map(Into::into).collect();
        let active = active.min(items.len().saturating_sub(1));
        self.graph.insert_node(DockNode::Tabs { items, active })
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
    /// Active index for the left stack.
    pub active_left: usize,
    /// Active index for the main stack.
    pub active_main: usize,
    /// Active index for the bottom stack.
    pub active_bottom: usize,
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
            active_left: 0,
            active_main: 0,
            active_bottom: 0,
        }
    }

    /// Sets the primary split fractions.
    pub fn with_fractions(mut self, left_fraction: f32, main_fraction: f32) -> Self {
        self.left_fraction = left_fraction;
        self.main_fraction = main_fraction;
        self
    }

    /// Sets active tab indexes.
    pub fn with_active_indexes(
        mut self,
        active_left: usize,
        active_main: usize,
        active_bottom: usize,
    ) -> Self {
        self.active_left = active_left;
        self.active_main = active_main;
        self.active_bottom = active_bottom;
        self
    }
}

impl DockGraph {
    /// Builds a common editor-style default layout.
    pub fn default_editor_layout(
        space: impl Into<DockSpaceId>,
        spec: EditorDockLayoutSpec,
    ) -> Self {
        let mut builder = DockLayoutBuilder::new();
        let left = builder.tabs(spec.left_items, spec.active_left);
        let main = builder.tabs(spec.main_items, spec.active_main);
        let bottom = builder.tabs(spec.bottom_items, spec.active_bottom);
        let right = builder.split_vertical(main, bottom, spec.main_fraction);
        let root = builder.split_horizontal(left, right, spec.left_fraction);
        builder.set_root(space, root);
        builder.build()
    }
}
