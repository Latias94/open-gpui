use super::*;

/// One scroll area sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ScrollAreaSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Rows or cells rendered inside the sample viewport.
    pub items: Vec<&'static str>,
    /// Resolved state.
    pub state: ScrollAreaState,
}

/// One splitter panel sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitterPanelSample {
    /// Stable panel id.
    pub id: &'static str,
    /// Visible title.
    pub title: &'static str,
    /// Panel body copy.
    pub body: &'static str,
    /// Panel descriptor.
    pub descriptor: SplitterPanelDescriptor,
}

/// One splitter sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct SplitterSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Splitter panels.
    pub panels: Vec<SplitterPanelSample>,
    /// Resolved state.
    pub state: SplitterState,
}

/// Returns scroll area samples backed by real component state.
pub fn scroll_area_samples(_tokens: ThemeTokens) -> [ScrollAreaSample; 3] {
    [
        ScrollAreaSample {
            id: "activity-log",
            title: "Activity log",
            summary: "Vertical viewport with stable metadata and preserved offset.",
            items: vec![
                "09:12  Indexed 128 records",
                "09:13  Synced component tokens",
                "09:15  Rebuilt preview cache",
                "09:17  Published validation report",
                "09:21  Accepted keyboard navigation update",
                "09:24  Queued layout smoke test",
                "09:28  Completed gallery startup path",
                "09:34  Updated engineering memory",
                "09:39  Prepared review notes",
            ],
            state: ScrollAreaState::resolve(
                "activity-log",
                ScrollAreaAxis::Vertical,
                Size::Medium,
                ScrollResetPolicy::Preserve,
                None,
            ),
        },
        ScrollAreaSample {
            id: "release-queue",
            title: "Release queue",
            summary: "Horizontal overflow for fixed-width operational lanes.",
            items: vec![
                "Intake",
                "Design",
                "Implementation",
                "Verification",
                "Docs",
                "Release",
                "Follow-up",
            ],
            state: ScrollAreaState::resolve(
                "release-queue",
                ScrollAreaAxis::Horizontal,
                Size::Small,
                ScrollResetPolicy::Preserve,
                None,
            ),
        },
        ScrollAreaSample {
            id: "data-grid",
            title: "Data grid",
            summary: "Two-axis viewport with explicit view-key reset semantics.",
            items: vec![
                "Component / Axis / Reset / Metrics",
                "Tabs / horizontal / preserve / medium",
                "ScrollArea / both / reset-on-key-change / small",
                "Menu / vertical / preserve / medium",
                "Dialog / none / preserve / medium",
                "Popover / none / preserve / medium",
                "ContextMenu / point / preserve / medium",
            ],
            state: ScrollAreaState::resolve(
                "data-grid",
                ScrollAreaAxis::Both,
                Size::Small,
                ScrollResetPolicy::ResetOnKeyChange,
                Some("components".to_string()),
            ),
        },
    ]
}

/// Returns splitter samples backed by real component state.
pub fn splitter_samples(_tokens: ThemeTokens) -> [SplitterSample; 2] {
    let workspace_panels = vec![
        SplitterPanelSample {
            id: "navigator",
            title: "Navigator",
            body: "Folders, symbols, and filters.",
            descriptor: SplitterPanelDescriptor::new("navigator", 0.24)
                .min_fraction(0.18)
                .max_fraction(0.34),
        },
        SplitterPanelSample {
            id: "editor",
            title: "Editor",
            body: "Primary working surface.",
            descriptor: SplitterPanelDescriptor::new("editor", 0.56)
                .min_fraction(0.42)
                .max_fraction(0.72),
        },
        SplitterPanelSample {
            id: "inspector",
            title: "Inspector",
            body: "Metadata and actions.",
            descriptor: SplitterPanelDescriptor::new("inspector", 0.2)
                .min_fraction(0.12)
                .max_fraction(0.28)
                .collapsible(true),
        },
    ];
    let details_panels = vec![
        SplitterPanelSample {
            id: "summary",
            title: "Summary",
            body: "Resizable header keeps context visible.",
            descriptor: SplitterPanelDescriptor::new("summary", 0.32)
                .min_fraction(0.2)
                .max_fraction(0.45)
                .collapsible(true)
                .collapsed(true)
                .collapsed_fraction(0.08),
        },
        SplitterPanelSample {
            id: "details",
            title: "Details",
            body: "Scrollable content can own the remaining space.",
            descriptor: SplitterPanelDescriptor::new("details", 0.68)
                .min_fraction(0.42)
                .max_fraction(0.92),
        },
    ];

    [
        SplitterSample {
            id: "workspace-split",
            title: "Workspace split",
            summary: "Horizontal panels constrained by min and max fractions.",
            state: SplitterState::resolve(
                "workspace-split",
                Orientation::Horizontal,
                Size::Medium,
                false,
                workspace_panels
                    .iter()
                    .map(|panel| panel.descriptor.clone()),
            ),
            panels: workspace_panels,
        },
        SplitterSample {
            id: "details-split",
            title: "Details split",
            summary: "Vertical stack with a collapsed but restorable panel.",
            state: SplitterState::resolve(
                "details-split",
                Orientation::Vertical,
                Size::Small,
                false,
                details_panels.iter().map(|panel| panel.descriptor.clone()),
            ),
            panels: details_panels,
        },
    ]
}
