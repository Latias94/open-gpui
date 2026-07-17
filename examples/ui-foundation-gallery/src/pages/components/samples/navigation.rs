use super::*;

/// One tab item sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct TabsItemSample {
    /// Stable tab value.
    pub value: &'static str,
    /// Visible label.
    pub label: &'static str,
    /// Panel copy shown for the sample.
    pub panel: &'static str,
    /// Whether the tab is disabled.
    pub disabled: bool,
}

/// One tabs sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct TabsSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Tab items.
    pub items: Vec<TabsItemSample>,
    /// Resolved state.
    pub state: TabsState,
}

impl TabsSample {
    /// Builds a tabs widget from the sample's resolved state and item descriptors.
    pub fn build_tabs(&self, tokens: ThemeTokens) -> Tabs {
        let tabs = self.items.iter().fold(
            Tabs::new(format!("component-tabs:{}", self.id))
                .orientation(self.state.orientation())
                .activation_mode(self.state.activation_mode())
                .with_size(self.state.size())
                .tokens(tokens),
            |tabs, item| {
                tabs.item(
                    TabsItem::new(
                        format!("component-tabs-item:{}:{}", self.id, item.value),
                        item.label,
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(open_gpui::FontWeight::BOLD)
                                    .child(item.label),
                            )
                            .child(div().text_xs().text_color(rgb(0x5a6472)).child(item.panel)),
                    )
                    .disabled(item.disabled),
                )
            },
        );

        if let Some(selected) = self.state.selected_value() {
            tabs.default_selected(selected)
        } else {
            tabs
        }
    }
}

/// One toolbar item sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolbarItemSample {
    /// Stable item value.
    pub value: &'static str,
    /// Visible or accessible label.
    pub label: &'static str,
    /// Icon glyph used by compact toolbar items.
    pub icon: Option<&'static str>,
    /// Item kind.
    pub kind: ToolbarItemKind,
    /// Whether the item is disabled.
    pub disabled: bool,
    /// Whether the toggle item is pressed.
    pub pressed: bool,
    /// Optional resolved action metadata used by typed action projection samples.
    pub action: Option<ResolvedActionState>,
}

/// One toolbar sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ToolbarSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Toolbar items.
    pub items: Vec<ToolbarItemSample>,
    /// Resolved state.
    pub state: ToolbarState,
}

impl ToolbarSample {
    /// Builds a toolbar widget from the sample's resolved state and item descriptors.
    pub fn build_toolbar(&self, tokens: ThemeTokens) -> Toolbar {
        let mut toolbar = Toolbar::new(
            format!("component-toolbar:{}", self.id),
            self.state.label().to_string(),
        )
        .orientation(self.state.orientation())
        .with_size(self.state.size())
        .tokens(tokens);

        if let Some(focused) = self.state.focused_value() {
            toolbar = toolbar.default_focused(focused);
        }

        for item in &self.items {
            let toolbar_item = if let Some(action) = item.action.as_ref() {
                ToolbarItem::from_resolved_action(action)
            } else {
                match item.kind {
                    ToolbarItemKind::Action => match item.icon {
                        Some(icon) => ToolbarItem::icon(item.value, icon, item.label),
                        None => ToolbarItem::action(item.value, item.label),
                    },
                    ToolbarItemKind::Toggle => match item.icon {
                        Some(icon) => ToolbarItem::toggle_icon(item.value, icon, item.label),
                        None => ToolbarItem::toggle(item.value, item.label),
                    }
                    .pressed(item.pressed),
                    ToolbarItemKind::Separator => ToolbarItem::separator(item.value),
                }
            }
            .disabled(item.disabled);
            toolbar = toolbar.item(toolbar_item);
        }

        toolbar
    }
}

/// One sidebar item sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct SidebarItemSample {
    /// Stable item value.
    pub value: &'static str,
    /// Visible or accessible label.
    pub label: &'static str,
    /// Icon glyph shown by the sample.
    pub icon: &'static str,
    /// Optional display-only badge text.
    pub badge: Option<&'static str>,
    /// Optional trailing action label.
    pub action_label: Option<&'static str>,
    /// Whether the item is disabled.
    pub disabled: bool,
}

/// One sidebar section sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct SidebarSectionSample {
    /// Stable section value.
    pub value: &'static str,
    /// Visible section label.
    pub label: &'static str,
    /// Navigation items in this section.
    pub items: Vec<SidebarItemSample>,
}

/// One sidebar sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct SidebarSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Authored sections used to exercise the component's own resolution path.
    pub sections: Vec<SidebarSectionSample>,
    /// Resolved state.
    pub state: SidebarState,
}

/// Returns tabs samples backed by real component state.
pub fn tabs_samples(tokens: ThemeTokens) -> [TabsSample; 2] {
    let overview_items = vec![
        TabsItemSample {
            value: "overview",
            label: "Overview",
            panel: "Project snapshot and recent activity.",
            disabled: false,
        },
        TabsItemSample {
            value: "details",
            label: "Details",
            panel: "Important metadata and settings.",
            disabled: false,
        },
        TabsItemSample {
            value: "history",
            label: "History",
            panel: "Previous revisions and timeline.",
            disabled: true,
        },
    ];
    let workspace_items = vec![
        TabsItemSample {
            value: "profile",
            label: "Profile",
            panel: "Identity and display settings.",
            disabled: false,
        },
        TabsItemSample {
            value: "security",
            label: "Security",
            panel: "Keys, authentication, and access rules.",
            disabled: false,
        },
        TabsItemSample {
            value: "billing",
            label: "Billing",
            panel: "Plans, invoices, and payment method.",
            disabled: false,
        },
        TabsItemSample {
            value: "integrations",
            label: "Integrations",
            panel: "Connected apps and webhooks.",
            disabled: true,
        },
        TabsItemSample {
            value: "notifications",
            label: "Notifications",
            panel: "Email and product alerts.",
            disabled: false,
        },
        TabsItemSample {
            value: "appearance",
            label: "Appearance",
            panel: "Theme and density preferences.",
            disabled: false,
        },
        TabsItemSample {
            value: "advanced",
            label: "Advanced",
            panel: "Migration and power-user settings.",
            disabled: false,
        },
        TabsItemSample {
            value: "audit",
            label: "Audit",
            panel: "Security log retention and export controls.",
            disabled: false,
        },
        TabsItemSample {
            value: "members",
            label: "Members",
            panel: "Seat management and team invitations.",
            disabled: false,
        },
        TabsItemSample {
            value: "projects",
            label: "Projects",
            panel: "Default project templates and access.",
            disabled: false,
        },
        TabsItemSample {
            value: "automation",
            label: "Automation",
            panel: "Rules, scheduled jobs, and notification routing.",
            disabled: false,
        },
        TabsItemSample {
            value: "experiments",
            label: "Experiments",
            panel: "Feature flags and rollout controls.",
            disabled: false,
        },
    ];

    [
        TabsSample {
            id: "overview-tabs",
            title: "Overview",
            summary: "Automatic activation with roving focus and one disabled tab.",
            state: tabs_state(
                Orientation::Horizontal,
                TabsActivationMode::Automatic,
                Size::Medium,
                "overview",
                &overview_items,
                tokens,
            ),
            items: overview_items,
        },
        TabsSample {
            id: "workspace-tabs",
            title: "Workspace",
            summary: "Manual activation with vertical navigation.",
            state: tabs_state(
                Orientation::Vertical,
                TabsActivationMode::Manual,
                Size::Small,
                "profile",
                &workspace_items,
                tokens,
            ),
            items: workspace_items,
        },
    ]
}

/// Returns toolbar samples backed by real component state.
pub fn toolbar_samples(tokens: ThemeTokens) -> [ToolbarSample; 2] {
    let redo_action = gallery_action(
        "redo",
        "Redo",
        "R",
        "Ctrl+Shift+Z",
        Some("Nothing to redo"),
        Some("Redo last edit"),
        Some("Reapplies the most recently undone edit"),
    );
    let editor_items = vec![
        ToolbarItemSample {
            value: "undo",
            label: "Undo",
            icon: Some("U"),
            kind: ToolbarItemKind::Action,
            disabled: false,
            pressed: false,
            action: None,
        },
        ToolbarItemSample {
            value: "redo",
            label: "Redo",
            icon: Some("R"),
            kind: ToolbarItemKind::Action,
            disabled: true,
            pressed: false,
            action: Some(redo_action),
        },
        ToolbarItemSample {
            value: "history-separator",
            label: "",
            icon: None,
            kind: ToolbarItemKind::Separator,
            disabled: true,
            pressed: false,
            action: None,
        },
        ToolbarItemSample {
            value: "bold",
            label: "Bold",
            icon: Some("B"),
            kind: ToolbarItemKind::Toggle,
            disabled: false,
            pressed: true,
            action: None,
        },
        ToolbarItemSample {
            value: "italic",
            label: "Italic",
            icon: Some("I"),
            kind: ToolbarItemKind::Toggle,
            disabled: false,
            pressed: false,
            action: None,
        },
        ToolbarItemSample {
            value: "save",
            label: "Save",
            icon: None,
            kind: ToolbarItemKind::Action,
            disabled: false,
            pressed: false,
            action: None,
        },
    ];
    let inspector_items = vec![
        ToolbarItemSample {
            value: "pin",
            label: "Pin",
            icon: Some("P"),
            kind: ToolbarItemKind::Toggle,
            disabled: false,
            pressed: true,
            action: None,
        },
        ToolbarItemSample {
            value: "refresh",
            label: "Refresh",
            icon: Some("R"),
            kind: ToolbarItemKind::Action,
            disabled: false,
            pressed: false,
            action: None,
        },
        ToolbarItemSample {
            value: "inspector-separator",
            label: "",
            icon: None,
            kind: ToolbarItemKind::Separator,
            disabled: true,
            pressed: false,
            action: None,
        },
        ToolbarItemSample {
            value: "details",
            label: "Details",
            icon: Some("D"),
            kind: ToolbarItemKind::Action,
            disabled: false,
            pressed: false,
            action: None,
        },
    ];

    [
        ToolbarSample {
            id: "editor-toolbar",
            summary: "Horizontal actions with separators, one disabled item, and pressed toggles.",
            state: toolbar_state(
                Orientation::Horizontal,
                Size::Small,
                "Editor toolbar",
                "bold",
                &editor_items,
                tokens,
            ),
            items: editor_items,
        },
        ToolbarSample {
            id: "inspector-toolbar",
            summary: "Vertical toolbar that keeps roving focus on command buttons.",
            state: toolbar_state(
                Orientation::Vertical,
                Size::Medium,
                "Inspector toolbar",
                "pin",
                &inspector_items,
                tokens,
            ),
            items: inspector_items,
        },
    ]
}

/// Returns sidebar samples backed by real component state.
pub fn sidebar_samples(tokens: ThemeTokens) -> [SidebarSample; 3] {
    let workspace_sections = vec![
        SidebarSectionSample {
            value: "workspace",
            label: "Workspace",
            items: vec![
                SidebarItemSample {
                    value: "dashboard",
                    label: "Dashboard",
                    icon: "D",
                    badge: None,
                    action_label: None,
                    disabled: false,
                },
                SidebarItemSample {
                    value: "projects",
                    label: "Projects",
                    icon: "P",
                    badge: Some("12"),
                    action_label: None,
                    disabled: false,
                },
                SidebarItemSample {
                    value: "inbox",
                    label: "Inbox",
                    icon: "I",
                    badge: Some("4"),
                    action_label: None,
                    disabled: false,
                },
                SidebarItemSample {
                    value: "archive",
                    label: "Archive",
                    icon: "A",
                    badge: None,
                    action_label: None,
                    disabled: true,
                },
                SidebarItemSample {
                    value: "duplicate-probe",
                    label: "Duplicate workspace",
                    icon: "W",
                    badge: None,
                    action_label: None,
                    disabled: false,
                },
            ],
        },
        SidebarSectionSample {
            value: "account",
            label: "Account",
            items: vec![
                SidebarItemSample {
                    value: "settings",
                    label: "Settings",
                    icon: "S",
                    badge: None,
                    action_label: None,
                    disabled: false,
                },
                SidebarItemSample {
                    value: "billing",
                    label: "Billing",
                    icon: "B",
                    badge: None,
                    action_label: Some("new"),
                    disabled: false,
                },
                SidebarItemSample {
                    value: "duplicate-probe",
                    label: "Duplicate account",
                    icon: "A",
                    badge: None,
                    action_label: None,
                    disabled: false,
                },
            ],
        },
    ];
    let icon_sections = vec![SidebarSectionSample {
        value: "primary",
        label: "Primary",
        items: vec![
            SidebarItemSample {
                value: "home",
                label: "Home",
                icon: "H",
                badge: None,
                action_label: None,
                disabled: false,
            },
            SidebarItemSample {
                value: "search",
                label: "Search",
                icon: "S",
                badge: None,
                action_label: None,
                disabled: false,
            },
            SidebarItemSample {
                value: "reports",
                label: "Reports",
                icon: "R",
                badge: Some("8"),
                action_label: None,
                disabled: false,
            },
        ],
    }];
    let long_sections = vec![SidebarSectionSample {
        value: "reports",
        label: "Reports",
        items: vec![
            SidebarItemSample {
                value: "overview",
                label: "Overview",
                icon: "O",
                badge: None,
                action_label: None,
                disabled: false,
            },
            SidebarItemSample {
                value: "traffic",
                label: "Traffic",
                icon: "T",
                badge: None,
                action_label: None,
                disabled: false,
            },
            SidebarItemSample {
                value: "funnels",
                label: "Funnels",
                icon: "F",
                badge: None,
                action_label: None,
                disabled: false,
            },
            SidebarItemSample {
                value: "retention",
                label: "Retention",
                icon: "R",
                badge: None,
                action_label: None,
                disabled: false,
            },
            SidebarItemSample {
                value: "quality",
                label: "Quality",
                icon: "Q",
                badge: None,
                action_label: None,
                disabled: false,
            },
            SidebarItemSample {
                value: "alerts",
                label: "Alerts",
                icon: "A",
                badge: Some("3"),
                action_label: None,
                disabled: false,
            },
            SidebarItemSample {
                value: "exports",
                label: "Exports",
                icon: "E",
                badge: None,
                action_label: None,
                disabled: true,
            },
            SidebarItemSample {
                value: "history",
                label: "History",
                icon: "H",
                badge: None,
                action_label: None,
                disabled: false,
            },
            SidebarItemSample {
                value: "forecast",
                label: "Forecast",
                icon: "F",
                badge: None,
                action_label: None,
                disabled: false,
            },
            SidebarItemSample {
                value: "segments",
                label: "Segments",
                icon: "S",
                badge: None,
                action_label: None,
                disabled: false,
            },
        ],
    }];

    [
        SidebarSample {
            id: "workspace-sidebar",
            summary: "Expanded navigation with one explicit disabled item and a cross-section duplicate pair that fails closed.",
            sections: workspace_sections.clone(),
            state: sidebar_state(
                SidebarSide::Left,
                SidebarVariant::Docked,
                SidebarCollapseMode::Icon,
                false,
                Size::Medium,
                "Workspace navigation",
                "projects",
                None,
                &workspace_sections,
                tokens,
            ),
        },
        SidebarSample {
            id: "icon-sidebar",
            summary: "Icon collapse hides visible text while preserving explicit item labels.",
            sections: icon_sections.clone(),
            state: sidebar_state(
                SidebarSide::Left,
                SidebarVariant::Inset,
                SidebarCollapseMode::Icon,
                true,
                Size::Small,
                "Icon navigation",
                "reports",
                Some("reports"),
                &icon_sections,
                tokens,
            ),
        },
        SidebarSample {
            id: "long-sidebar",
            summary: "Constrained long navigation remains scrollable and skips disabled items.",
            sections: long_sections.clone(),
            state: sidebar_state(
                SidebarSide::Right,
                SidebarVariant::Floating,
                SidebarCollapseMode::None,
                false,
                Size::Small,
                "Reports navigation",
                "alerts",
                Some("quality"),
                &long_sections,
                tokens,
            ),
        },
    ]
}

fn tabs_state(
    orientation: Orientation,
    activation_mode: TabsActivationMode,
    size: Size,
    selected: &str,
    items: &[TabsItemSample],
    tokens: ThemeTokens,
) -> TabsState {
    TabsState::resolve(
        orientation,
        activation_mode,
        size,
        TabsSelectionAuthority::Uncontrolled(Some(selected)),
        None,
        items
            .iter()
            .map(|item| TabsItemDescriptor::new(item.value, item.label).disabled(item.disabled)),
        tokens,
    )
}

fn sidebar_state(
    side: SidebarSide,
    variant: SidebarVariant,
    collapse_mode: SidebarCollapseMode,
    collapsed: bool,
    size: Size,
    label: &str,
    selected: &str,
    focused: Option<&str>,
    sections: &[SidebarSectionSample],
    tokens: ThemeTokens,
) -> SidebarState {
    SidebarState::resolve(
        side,
        variant,
        collapse_mode,
        collapsed,
        false,
        label,
        Some(selected),
        focused,
        sections.iter().map(|section| {
            SidebarSectionDescriptor::new(section.value, section.label).items(
                section.items.iter().map(|item| {
                    let mut descriptor =
                        SidebarItemDescriptor::new(item.value, item.label).icon(item.icon);
                    if let Some(badge) = item.badge {
                        descriptor = descriptor.badge(badge);
                    }
                    if let Some(action_label) = item.action_label {
                        descriptor = descriptor.action_label(action_label);
                    }
                    descriptor.disabled(item.disabled)
                }),
            )
        }),
        size,
        tokens,
    )
}

fn toolbar_state(
    orientation: Orientation,
    size: Size,
    label: &str,
    focused: &str,
    items: &[ToolbarItemSample],
    tokens: ThemeTokens,
) -> ToolbarState {
    ToolbarState::resolve(
        orientation,
        size,
        false,
        label,
        Some(focused),
        items.iter().map(|item| {
            let descriptor = match item.kind {
                ToolbarItemKind::Action => item
                    .action
                    .as_ref()
                    .map(ToolbarItemDescriptor::from_resolved_action)
                    .unwrap_or_else(|| ToolbarItemDescriptor::action(item.value, item.label)),
                ToolbarItemKind::Toggle => {
                    ToolbarItemDescriptor::toggle(item.value, item.label).pressed(item.pressed)
                }
                ToolbarItemKind::Separator => ToolbarItemDescriptor::separator(item.value),
            };
            descriptor.disabled(item.disabled)
        }),
        tokens,
    )
}

fn gallery_action(
    value: &'static str,
    label: &'static str,
    icon_label: &'static str,
    shortcut: &'static str,
    disabled_reason: Option<&'static str>,
    tooltip: Option<&'static str>,
    accessibility_description: Option<&'static str>,
) -> ResolvedActionState {
    let mut descriptor = ActionDescriptor::new(value, label)
        .icon(ActionIconDescriptor::new(value).fallback_label(icon_label))
        .shortcut(shortcut);
    if let Some(reason) = disabled_reason {
        descriptor = descriptor.disabled_reason(reason);
    }
    if let Some(tooltip) = tooltip {
        descriptor = descriptor.tooltip(tooltip);
    }
    if let Some(description) = accessibility_description {
        descriptor = descriptor.accessibility_description(description);
    }
    descriptor.resolve_with(&|icon: &ActionIconDescriptor| {
        ResolvedActionIcon::resolved(
            icon.clone(),
            icon.fallback_label_ref().unwrap_or_else(|| icon.name()),
        )
    })
}
