//! Component conformance bindings for the foundation gallery.

pub use open_gpui_ui_components::{COMPONENT_CONFORMANCE_GATES, ComponentConformanceGate};
use open_gpui_ui_components::{ComponentA11yEvidence, component_a11y_evidence};

/// One renderer-neutral accessibility claim tied to a representative gallery sample selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentA11yClaim {
    /// Component or component part covered by the claim.
    pub component: &'static str,
    /// Stable sample selector prefix used by the gallery renderer.
    pub selector_prefix: &'static str,
}

impl ComponentA11yClaim {
    /// Returns the component-owned accessibility evidence for this gallery selector binding.
    pub fn evidence(self) -> &'static ComponentA11yEvidence {
        component_a11y_evidence(self.component)
            .expect("gallery a11y claim must reference component-owned evidence")
    }
}

/// Gallery selector bindings for component-owned representative accessibility evidence.
pub const COMPONENT_A11Y_CLAIMS: &[ComponentA11yClaim] = &[
    ComponentA11yClaim {
        component: "IconButton",
        selector_prefix: "gallery:component-icon-button-sample",
    },
    ComponentA11yClaim {
        component: "Checkbox",
        selector_prefix: "gallery:component-checkbox-sample",
    },
    ComponentA11yClaim {
        component: "Slider",
        selector_prefix: "gallery:component-slider-sample",
    },
    ComponentA11yClaim {
        component: "NumberInput",
        selector_prefix: "gallery:component-number-input-sample",
    },
    ComponentA11yClaim {
        component: "Progress",
        selector_prefix: "gallery:component-progress-sample",
    },
    ComponentA11yClaim {
        component: "Listbox option",
        selector_prefix: "gallery:component-listbox-sample",
    },
    ComponentA11yClaim {
        component: "Tree item",
        selector_prefix: "gallery:component-tree-sample",
    },
    ComponentA11yClaim {
        component: "VirtualizedList row",
        selector_prefix: "gallery:component-virtualized-list-sample",
    },
    ComponentA11yClaim {
        component: "VirtualizedList structural row",
        selector_prefix: "gallery:component-virtualized-list-sample",
    },
    ComponentA11yClaim {
        component: "Splitter handle",
        selector_prefix: "gallery:component-splitter-sample",
    },
];
