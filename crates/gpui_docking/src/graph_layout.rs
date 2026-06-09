use crate::{DockNodeId, split_fraction};
use open_gpui::{Bounds, Pixels, point, px, size};
use std::collections::HashMap;

use super::{DockGraph, DockNode, SplitAxis};

impl DockGraph {
    /// Computes layout bounds for a subtree into `out`.
    pub fn compute_layout(
        &self,
        root: DockNodeId,
        bounds: Bounds<Pixels>,
        out: &mut HashMap<DockNodeId, Bounds<Pixels>>,
    ) {
        let Some(node) = self.nodes.get(root) else {
            return;
        };

        out.insert(root, bounds);
        match node {
            DockNode::Tabs { .. } => {}
            DockNode::Floating { child } => {
                self.compute_layout(*child, bounds, out);
            }
            DockNode::Split {
                axis,
                children,
                fractions,
            } => {
                if children.is_empty() {
                    return;
                }

                let shares = split_fraction::cleaned_shares(children.len(), fractions);
                let mut cursor = 0.0_f32;
                let width = f32::from(bounds.size.width);
                let height = f32::from(bounds.size.height);
                let x = f32::from(bounds.origin.x);
                let y = f32::from(bounds.origin.y);

                for (child, share) in children.iter().copied().zip(shares) {
                    let (child_bounds, next_cursor) = match axis {
                        SplitAxis::Horizontal => {
                            let child_width = width * share;
                            (
                                Bounds::new(
                                    point(px(x + cursor), bounds.origin.y),
                                    size(px(child_width), bounds.size.height),
                                ),
                                cursor + child_width,
                            )
                        }
                        SplitAxis::Vertical => {
                            let child_height = height * share;
                            (
                                Bounds::new(
                                    point(bounds.origin.x, px(y + cursor)),
                                    size(bounds.size.width, px(child_height)),
                                ),
                                cursor + child_height,
                            )
                        }
                    };

                    cursor = next_cursor;
                    self.compute_layout(child, child_bounds, out);
                }
            }
        }
    }
}
