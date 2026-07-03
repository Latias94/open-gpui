use super::*;
use open_gpui::{KeyBinding, Keymap, actions};
use open_gpui_command::{
    CommandCenter, CommandContribution, CommandDescriptor, CommandProviderRefreshController,
    CommandProviderResponse, CommandProviderSource, CommandProviderStatus,
};

/// One switch sample in the gallery.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwitchSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible label.
    pub label: &'static str,
    /// Resolved state.
    pub state: SwitchState,
}

/// One checkbox sample in the gallery.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CheckboxSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible label.
    pub label: &'static str,
    /// Resolved state.
    pub state: CheckboxState,
}

/// One radio item sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct RadioItemSample {
    /// Stable item value.
    pub value: &'static str,
    /// Visible label.
    pub label: &'static str,
    /// Whether the item is disabled.
    pub disabled: bool,
}

/// One radio group sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct RadioGroupSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Resolved state.
    pub state: RadioGroupState,
}

/// One toggle sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ToggleSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible label.
    pub label: &'static str,
    /// Resolved state.
    pub state: ToggleState,
}

/// One listbox option sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ListboxOptionSample {
    /// Stable option value.
    pub value: &'static str,
    /// Visible option label.
    pub label: &'static str,
    /// Whether the option is disabled.
    pub disabled: bool,
}

/// One listbox group sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ListboxGroupSample {
    /// Stable group value.
    pub value: &'static str,
    /// Visible group label.
    pub label: &'static str,
    /// Options in this group.
    pub options: Vec<ListboxOptionSample>,
}

/// One listbox sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ListboxSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Resolved state.
    pub state: ListboxState,
}

/// One select sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct SelectSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Resolved state.
    pub state: SelectState,
}

/// One combobox sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ComboboxSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Resolved state.
    pub state: ComboboxState,
}

/// One command palette sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Standalone descriptors consumed by the concrete command renderer.
    pub items: Arc<[CommandItemDescriptor]>,
    /// Group descriptors consumed by the concrete command renderer.
    pub groups: Arc<[CommandGroupDescriptor]>,
    /// Optional caller-owned command index snapshot.
    pub index_snapshot: Option<CommandIndexSnapshot>,
    /// Command id that the sample dispatch adapter resolves from the active selection.
    pub dispatched_command_id: Option<String>,
    /// Latest dynamic provider status retained by the backing command center.
    pub provider_status: Option<CommandProviderStatus>,
    /// Persistent selected values for multi-select samples.
    pub selected_values: Arc<[String]>,
    /// Estimated visible row count for the result viewport.
    pub viewport_item_count: usize,
    /// Optional fixed row height for virtualized command results.
    pub row_height: Option<UiPx>,
    /// Overscan row budget for virtualized command results.
    pub overscan: usize,
    /// Resolved state.
    pub state: CommandState,
}

/// Returns switch samples backed by real component state.
pub fn switch_samples(tokens: ThemeTokens) -> [SwitchSample; 4] {
    [
        ("off", "Unchecked", false, false, Size::Medium),
        ("on", "Checked", true, false, Size::Medium),
        ("small", "Small checked", true, false, Size::Small),
        ("disabled", "Disabled", false, true, Size::Medium),
    ]
    .map(|(id, label, checked, disabled, size)| SwitchSample {
        id,
        label,
        state: Switch::new(id)
            .label(label)
            .checked(checked)
            .disabled(disabled)
            .with_size(size)
            .tokens(tokens)
            .state(),
    })
}

/// Returns checkbox samples backed by real component state.
pub fn checkbox_samples(tokens: ThemeTokens) -> [CheckboxSample; 6] {
    [
        (
            "unchecked",
            "Unchecked",
            false,
            false,
            false,
            false,
            false,
            Size::Medium,
        ),
        (
            "checked",
            "Checked",
            true,
            false,
            false,
            false,
            false,
            Size::Medium,
        ),
        (
            "mixed",
            "Indeterminate",
            false,
            true,
            false,
            false,
            false,
            Size::Medium,
        ),
        (
            "required",
            "Required",
            true,
            false,
            false,
            true,
            false,
            Size::Medium,
        ),
        (
            "invalid",
            "Invalid",
            false,
            false,
            false,
            true,
            true,
            Size::Medium,
        ),
        (
            "disabled",
            "Disabled",
            false,
            false,
            true,
            false,
            false,
            Size::Medium,
        ),
    ]
    .map(
        |(id, label, checked, indeterminate, disabled, required, invalid, size)| CheckboxSample {
            id,
            label,
            state: Checkbox::new(id)
                .label(label)
                .checked(checked)
                .indeterminate(indeterminate)
                .disabled(disabled)
                .required(required)
                .invalid(invalid)
                .with_size(size)
                .tokens(tokens)
                .state(),
        },
    )
}

/// Returns radio group samples backed by real component state.
pub fn radio_group_samples(tokens: ThemeTokens) -> [RadioGroupSample; 2] {
    let persona_items = vec![
        RadioItemSample {
            value: "personal",
            label: "Personal",
            disabled: false,
        },
        RadioItemSample {
            value: "team",
            label: "Team",
            disabled: false,
        },
        RadioItemSample {
            value: "enterprise",
            label: "Enterprise",
            disabled: true,
        },
    ];
    let region_items = vec![
        RadioItemSample {
            value: "asia",
            label: "Asia",
            disabled: false,
        },
        RadioItemSample {
            value: "europe",
            label: "Europe",
            disabled: false,
        },
        RadioItemSample {
            value: "americas",
            label: "Americas",
            disabled: false,
        },
    ];

    [
        RadioGroupSample {
            id: "persona-radios",
            title: "Persona",
            summary: "Vertical group with required metadata and one disabled item.",
            state: radio_group_state(
                Orientation::Vertical,
                Size::Medium,
                false,
                true,
                "team",
                &persona_items,
                tokens,
            ),
        },
        RadioGroupSample {
            id: "region-radios",
            title: "Region",
            summary: "Horizontal group with compact sizing.",
            state: radio_group_state(
                Orientation::Horizontal,
                Size::Small,
                false,
                false,
                "europe",
                &region_items,
                tokens,
            ),
        },
    ]
}

/// Returns toggle samples backed by real component state.
pub fn toggle_samples(tokens: ThemeTokens) -> [ToggleSample; 4] {
    [
        (
            "ghost-off",
            "Ghost off",
            ToggleVariant::Ghost,
            false,
            false,
            Size::Medium,
        ),
        (
            "ghost-on",
            "Ghost on",
            ToggleVariant::Ghost,
            true,
            false,
            Size::Medium,
        ),
        (
            "outline-on",
            "Outline on",
            ToggleVariant::Outline,
            true,
            false,
            Size::Small,
        ),
        (
            "outline-disabled",
            "Disabled",
            ToggleVariant::Outline,
            false,
            true,
            Size::Medium,
        ),
    ]
    .map(
        |(id, label, variant, pressed, disabled, size)| ToggleSample {
            id,
            label,
            state: Toggle::new(id, label)
                .variant(variant)
                .pressed(pressed)
                .disabled(disabled)
                .with_size(size)
                .tokens(tokens)
                .state(),
        },
    )
}

/// Returns listbox samples backed by real component state.
pub fn listbox_samples(tokens: ThemeTokens) -> [ListboxSample; 2] {
    let assigned_options = vec![
        ListboxOptionSample {
            value: "unassigned",
            label: "Unassigned",
            disabled: false,
        },
        ListboxOptionSample {
            value: "separator",
            label: "",
            disabled: true,
        },
    ];
    let assigned_groups = vec![
        ListboxGroupSample {
            value: "core",
            label: "Core team",
            options: vec![
                ListboxOptionSample {
                    value: "maya",
                    label: "Maya Chen",
                    disabled: false,
                },
                ListboxOptionSample {
                    value: "owen",
                    label: "Owen Patel",
                    disabled: false,
                },
                ListboxOptionSample {
                    value: "li",
                    label: "Li Wei",
                    disabled: true,
                },
            ],
        },
        ListboxGroupSample {
            value: "support",
            label: "Support",
            options: vec![
                ListboxOptionSample {
                    value: "nora",
                    label: "Nora Lee",
                    disabled: false,
                },
                ListboxOptionSample {
                    value: "sam",
                    label: "Sam Rivera",
                    disabled: false,
                },
            ],
        },
    ];
    let empty_options = Vec::new();
    let empty_groups = Vec::new();

    [
        ListboxSample {
            id: "assignee-listbox",
            summary: "Grouped listbox with shared roving navigation, typeahead, and one disabled option.",
            state: listbox_state(
                Size::Medium,
                false,
                "Assignee",
                Some("owen"),
                Some("maya"),
                &assigned_options,
                &assigned_groups,
                tokens,
            ),
        },
        ListboxSample {
            id: "empty-listbox",
            summary: "Empty state keeps a listbox role but has no tab stop.",
            state: listbox_state(
                Size::Small,
                false,
                "Empty list",
                None,
                None,
                &empty_options,
                &empty_groups,
                tokens,
            ),
        },
    ]
}

/// Returns select samples backed by real component state.
pub fn select_samples(tokens: ThemeTokens) -> [SelectSample; 3] {
    let priority_options = vec![
        ListboxOptionSample {
            value: "low",
            label: "Low",
            disabled: false,
        },
        ListboxOptionSample {
            value: "normal",
            label: "Normal",
            disabled: false,
        },
        ListboxOptionSample {
            value: "blocked",
            label: "Blocked",
            disabled: true,
        },
    ];
    let priority_groups = vec![ListboxGroupSample {
        value: "urgent",
        label: "Urgent",
        options: vec![
            ListboxOptionSample {
                value: "high",
                label: "High",
                disabled: false,
            },
            ListboxOptionSample {
                value: "critical",
                label: "Critical",
                disabled: false,
            },
            ListboxOptionSample {
                value: "today",
                label: "Today",
                disabled: false,
            },
            ListboxOptionSample {
                value: "tomorrow",
                label: "Tomorrow",
                disabled: false,
            },
            ListboxOptionSample {
                value: "later",
                label: "Later",
                disabled: false,
            },
        ],
    }];
    let status_options = vec![
        ListboxOptionSample {
            value: "todo",
            label: "Todo",
            disabled: false,
        },
        ListboxOptionSample {
            value: "doing",
            label: "Doing",
            disabled: false,
        },
        ListboxOptionSample {
            value: "done",
            label: "Done",
            disabled: false,
        },
    ];
    let disabled_options = Vec::new();
    let disabled_groups = Vec::new();

    [
        SelectSample {
            id: "priority-select",
            summary: "Open select keeps stable trigger selection distinct from popup active state.",
            state: select_state(
                Size::Medium,
                false,
                Some(true),
                false,
                "Priority",
                "Choose priority",
                Some("critical"),
                Some("normal"),
                &priority_options,
                &priority_groups,
                tokens,
            ),
        },
        SelectSample {
            id: "status-select",
            summary: "Closed uncontrolled select with selected trigger label.",
            state: select_state(
                Size::Small,
                false,
                None,
                false,
                "Status",
                "Choose status",
                Some("doing"),
                Some("doing"),
                &status_options,
                &[],
                tokens,
            ),
        },
        SelectSample {
            id: "disabled-select",
            summary: "Disabled empty select suppresses popup presence and activation.",
            state: select_state(
                Size::Small,
                true,
                None,
                true,
                "Disabled",
                "Unavailable",
                None,
                None,
                &disabled_options,
                &disabled_groups,
                tokens,
            ),
        },
    ]
}

/// Returns combobox samples backed by real component state.
pub fn combobox_samples(tokens: ThemeTokens) -> [ComboboxSample; 3] {
    let framework_options = vec![
        ListboxOptionSample {
            value: "react",
            label: "React",
            disabled: false,
        },
        ListboxOptionSample {
            value: "solid",
            label: "Solid",
            disabled: false,
        },
        ListboxOptionSample {
            value: "ember",
            label: "Ember",
            disabled: true,
        },
    ];
    let framework_groups = vec![ListboxGroupSample {
        value: "meta",
        label: "Meta",
        options: vec![
            ListboxOptionSample {
                value: "remix",
                label: "Remix",
                disabled: false,
            },
            ListboxOptionSample {
                value: "relay",
                label: "Relay",
                disabled: false,
            },
        ],
    }];
    let empty_options = vec![ListboxOptionSample {
        value: "rust",
        label: "Rust",
        disabled: false,
    }];
    let disabled_options = Vec::new();

    [
        ComboboxSample {
            id: "framework-combobox",
            summary: "Editable combobox keeps stable selected value while query filtering changes the visible list.",
            state: combobox_state(
                Size::Medium,
                false,
                Some(true),
                false,
                "Framework",
                "Search frameworks",
                "re",
                Some("solid"),
                Some("react"),
                &framework_options,
                &framework_groups,
                tokens,
            ),
        },
        ComboboxSample {
            id: "empty-combobox",
            summary: "Filtered empty state keeps the selected value independent from query text.",
            state: combobox_state(
                Size::Small,
                false,
                Some(true),
                false,
                "Empty search",
                "Search stack",
                "zz",
                None,
                None,
                &empty_options,
                &[],
                tokens,
            ),
        },
        ComboboxSample {
            id: "disabled-combobox",
            summary: "Disabled combobox preserves query metadata but suppresses popup presence.",
            state: combobox_state(
                Size::Small,
                true,
                None,
                true,
                "Disabled search",
                "Unavailable",
                "",
                None,
                None,
                &disabled_options,
                &[],
                tokens,
            ),
        },
    ]
}

static VIRTUALIZED_COMMAND_ITEMS: LazyLock<Arc<[CommandItemDescriptor]>> = LazyLock::new(|| {
    (0..10_000)
        .map(|index| {
            CommandItemDescriptor::new(
                format!("command-{index:04}"),
                format!("Command item {index:04}"),
            )
            .keyword(format!("release-{index:04}"))
        })
        .collect::<Vec<_>>()
        .into()
});

actions!(
    gallery_registry_command,
    [OpenRegistryCommand, SaveRegistryCommand,]
);

/// Returns command palette samples backed by real component state.
pub fn command_samples(tokens: ThemeTokens) -> [CommandSample; 6] {
    let ranked_items: Arc<[CommandItemDescriptor]> = vec![
        CommandItemDescriptor::new("archive", "Archive").keyword("file"),
        CommandItemDescriptor::new("open-file", "Open File").shortcut("Ctrl+O"),
        CommandItemDescriptor::new("file-action", "Launcher").shortcut("Ctrl+L"),
    ]
    .into();
    let ranked_groups: Arc<[CommandGroupDescriptor]> = vec![
        CommandGroupDescriptor::new("view", "View").item(
            CommandItemDescriptor::new("toggle-sidebar", "Toggle Sidebar")
                .keyword("layout")
                .shortcut("Ctrl+B"),
        ),
    ]
    .into();
    let multi_items: Arc<[CommandItemDescriptor]> = vec![
        CommandItemDescriptor::new("open-file", "Open File").shortcut("Ctrl+O"),
        CommandItemDescriptor::new("new-file", "New File").shortcut("Ctrl+N"),
        CommandItemDescriptor::new("delete-file", "Delete File").disabled(true),
    ]
    .into();
    let virtualized_items = VIRTUALIZED_COMMAND_ITEMS.clone();
    let indexed_snapshot = CommandIndexSnapshot::new("workspace-index-v3")
        .mode(CommandIndexSnapshotMode::PreRankedFilter)
        .loading(CommandLoadingState::new(
            "Refreshing command index",
            Some(45),
        ))
        .item(CommandItemDescriptor::new("recent-open", "Open Recent").keyword("file"))
        .item(CommandItemDescriptor::new("open-file", "Open File").shortcut("Ctrl+O"))
        .item(CommandItemDescriptor::new("archive", "Archive").keyword("file"))
        .group(
            CommandGroupDescriptor::new("workspace", "Workspace")
                .item(CommandItemDescriptor::new("switch-window", "Switch Window"))
                .item(CommandItemDescriptor::new("close-window", "Close Window").disabled(true)),
        );
    let mut command_center = CommandCenter::new("gallery-command-center-v1");
    command_center
        .register_source(
            "global",
            "gallery",
            [
                CommandContribution::new(
                    CommandDescriptor::new("workspace.open", "Open Workspace")
                        .group("Workspace")
                        .keyword("project"),
                ),
                CommandContribution::new(
                    CommandDescriptor::new("workspace.save", "Save Workspace")
                        .group("Workspace")
                        .keyword("persist"),
                ),
            ],
        )
        .unwrap();
    let mut keymap = Keymap::default();
    keymap.add_bindings([
        KeyBinding::new("ctrl-p", OpenRegistryCommand, None),
        KeyBinding::new("ctrl-shift-p", OpenRegistryCommand, None),
        KeyBinding::new("ctrl-s", SaveRegistryCommand, None),
    ]);
    command_center
        .register_action("workspace.open", OpenRegistryCommand)
        .register_action("workspace.save", SaveRegistryCommand);
    let dispatched_command_id = command_center
        .actions()
        .action_for_command("workspace.open")
        .map(|action| action.command_id().to_owned())
        .unwrap();
    let command_center_snapshot =
        CommandIndexSnapshot::from_registry_snapshot(&command_center.snapshot_for_keymap(&keymap))
            .mode(CommandIndexSnapshotMode::PreRankedFilter);
    let provider_query = "alpha";
    let mut provider_center = CommandCenter::new("gallery-provider-center-v1");
    let _provider_registration = provider_center.register_provider(
        "recent-provider",
        |request: &open_gpui_command::CommandProviderRequest| {
            let query = request.query().trim();
            let query = if query.is_empty() { "recent" } else { query };
            CommandProviderResponse::ready().source(CommandProviderSource::new(
                "workspace",
                "recent-provider-results",
                [
                    CommandContribution::new(
                        CommandDescriptor::new(
                            format!("provider.open.{query}"),
                            format!("Open {query} from provider"),
                        )
                        .group("Provider")
                        .keyword("recent"),
                    ),
                    CommandContribution::new(
                        CommandDescriptor::new(
                            format!("provider.reveal.{query}"),
                            format!("Reveal {query} provider result"),
                        )
                        .group("Provider")
                        .keyword("dynamic"),
                    ),
                ],
            ))
        },
    );
    let mut provider_controller = CommandProviderRefreshController::new("recent-provider")
        .with_loading_message("Searching provider commands");
    let provider_projection = provider_controller
        .refresh_provider(&mut provider_center, provider_query)
        .expect("gallery provider is registered")
        .expect("gallery provider response is valid");
    let provider_status = provider_projection
        .provider_status()
        .expect("gallery provider status is projected")
        .clone();
    let provider_snapshot =
        CommandIndexSnapshot::from_registry_snapshot(provider_projection.snapshot())
            .mode(CommandIndexSnapshotMode::PreRankedFilter);

    [
        command_sample_from_local(
            "ranked-search",
            "Ranked query keeps stable selected value while label and value matches outrank keyword-only commands.",
            Size::Medium,
            false,
            Some(true),
            false,
            "Ranked commands",
            "Search commands",
            "file",
            CommandSelectionMode::Single,
            Some("open-file"),
            Vec::<String>::new(),
            Some("open-file"),
            None,
            ranked_items,
            ranked_groups,
            true,
            8,
            None,
            6,
            tokens,
        ),
        command_sample_from_local(
            "multi-select",
            "Multi-select keeps selected chips even when query filtering hides a command.",
            Size::Small,
            false,
            Some(true),
            false,
            "Bulk commands",
            "Filter commands",
            "new",
            CommandSelectionMode::Multiple,
            None,
            vec!["open-file".to_string(), "new-file".to_string()],
            Some("new-file"),
            None,
            multi_items,
            Arc::from([]),
            false,
            6,
            None,
            4,
            tokens,
        ),
        command_sample_from_local(
            "virtualized-index",
            "Ten-thousand command results render through the fixed-row virtualizer.",
            Size::Small,
            false,
            Some(true),
            false,
            "Virtualized commands",
            "Search large index",
            "",
            CommandSelectionMode::Single,
            Some("command-0000"),
            Vec::<String>::new(),
            Some("command-0000"),
            None,
            virtualized_items,
            Arc::from([]),
            false,
            7,
            Some(ui_px(28.0)),
            4,
            tokens,
        ),
        command_sample_from_snapshot(
            "indexed-loading",
            "App-owned pre-ranked snapshot carries revision and loading metadata without a registry.",
            Size::Small,
            false,
            Some(true),
            false,
            "Indexed commands",
            "Search indexed commands",
            "file",
            CommandSelectionMode::Single,
            Some("open-file"),
            Vec::<String>::new(),
            Some("recent-open"),
            indexed_snapshot,
            false,
            6,
            None,
            4,
            tokens,
        ),
        CommandSample {
            dispatched_command_id: Some(dispatched_command_id),
            ..command_sample_from_snapshot(
                "registry-dispatch",
                "CommandCenter-backed palette projects keymap shortcuts and records the dispatched command id.",
                Size::Small,
                false,
                Some(true),
                false,
                "Registry commands",
                "Search registered commands",
                "workspace",
                CommandSelectionMode::Single,
                Some("workspace.open"),
                Vec::<String>::new(),
                Some("workspace.open"),
                command_center_snapshot,
                true,
                6,
                None,
                4,
                tokens,
            )
        },
        CommandSample {
            provider_status: Some(provider_status),
            ..command_sample_from_snapshot(
                "provider-search",
                "CommandCenter provider refresh projects query-specific dynamic commands into the palette snapshot.",
                Size::Small,
                false,
                Some(true),
                false,
                "Provider commands",
                "Search provider commands",
                provider_query,
                CommandSelectionMode::Single,
                Some("provider.open.alpha"),
                Vec::<String>::new(),
                Some("provider.open.alpha"),
                provider_snapshot,
                false,
                6,
                None,
                4,
                tokens,
            )
        },
    ]
}

fn listbox_state(
    size: Size,
    disabled: bool,
    label: &str,
    selected: Option<&str>,
    active: Option<&str>,
    options: &[ListboxOptionSample],
    groups: &[ListboxGroupSample],
    tokens: ThemeTokens,
) -> ListboxState {
    ListboxState::resolve(
        size,
        disabled,
        label,
        selected,
        active,
        None,
        "No options",
        groups.iter().map(listbox_group_descriptor),
        options.iter().map(listbox_option_descriptor),
        tokens,
    )
}

#[allow(clippy::too_many_arguments)]
fn select_state(
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    label: &str,
    placeholder: &str,
    selected: Option<&str>,
    active: Option<&str>,
    options: &[ListboxOptionSample],
    groups: &[ListboxGroupSample],
    tokens: ThemeTokens,
) -> SelectState {
    SelectState::resolve(
        size,
        disabled,
        open,
        default_open,
        label,
        placeholder,
        selected,
        active,
        groups.iter().map(listbox_group_descriptor),
        options.iter().map(listbox_option_descriptor),
        OverlayPlacementSide::Bottom,
        OverlayPlacementAlignment::Start,
        OutsidePressPolicy::DismissAndConsume,
        InitialFocusIntent::FirstFocusable,
        FocusRestoreIntent::Trigger,
        tokens,
    )
}

#[allow(clippy::too_many_arguments)]
fn combobox_state(
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    label: &str,
    placeholder: &str,
    query: &str,
    selected: Option<&str>,
    active: Option<&str>,
    options: &[ListboxOptionSample],
    groups: &[ListboxGroupSample],
    tokens: ThemeTokens,
) -> ComboboxState {
    ComboboxState::resolve(
        size,
        disabled,
        false,
        open,
        default_open,
        label,
        placeholder,
        query,
        selected,
        active,
        "No results",
        groups.iter().map(combobox_group_descriptor),
        options.iter().map(combobox_option_descriptor),
        OverlayPlacementSide::Bottom,
        OverlayPlacementAlignment::Start,
        OutsidePressPolicy::DismissAndConsume,
        InitialFocusIntent::None,
        FocusRestoreIntent::Trigger,
        tokens,
    )
}

#[allow(clippy::too_many_arguments)]
fn command_sample_from_local(
    id: &'static str,
    summary: &'static str,
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    label: &str,
    placeholder: &str,
    query: &str,
    selection_mode: CommandSelectionMode,
    selected: Option<&str>,
    selected_values: Vec<String>,
    active: Option<&str>,
    loading: Option<CommandLoadingState>,
    items: Arc<[CommandItemDescriptor]>,
    groups: Arc<[CommandGroupDescriptor]>,
    dialog: bool,
    viewport_item_count: usize,
    row_height: Option<UiPx>,
    overscan: usize,
    tokens: ThemeTokens,
) -> CommandSample {
    let state = CommandState::resolve(
        size,
        disabled,
        open,
        default_open,
        dialog,
        label,
        placeholder,
        query,
        CommandQueryMode::Uncontrolled,
        selection_mode,
        selected,
        selected_values.iter().cloned(),
        active,
        loading,
        "No results",
        dialog.then_some("Command palette".to_string()),
        dialog.then_some("Run a workspace command".to_string()),
        groups.iter().cloned(),
        items.iter().cloned(),
        OutsidePressPolicy::DismissAndConsume,
        EscapeKeyPolicy::Dismiss,
        InitialFocusIntent::FirstFocusable,
        FocusRestoreIntent::Trigger,
        tokens,
    );
    CommandSample {
        id,
        summary,
        items,
        groups,
        index_snapshot: None,
        dispatched_command_id: None,
        provider_status: None,
        selected_values: selected_values.into(),
        viewport_item_count,
        row_height,
        overscan,
        state,
    }
}

#[allow(clippy::too_many_arguments)]
fn command_sample_from_snapshot(
    id: &'static str,
    summary: &'static str,
    size: Size,
    disabled: bool,
    open: Option<bool>,
    default_open: bool,
    label: &str,
    placeholder: &str,
    query: &str,
    selection_mode: CommandSelectionMode,
    selected: Option<&str>,
    selected_values: Vec<String>,
    active: Option<&str>,
    snapshot: CommandIndexSnapshot,
    dialog: bool,
    viewport_item_count: usize,
    row_height: Option<UiPx>,
    overscan: usize,
    tokens: ThemeTokens,
) -> CommandSample {
    let state = CommandState::resolve_from_index_snapshot(
        size,
        disabled,
        open,
        default_open,
        dialog,
        label,
        placeholder,
        query,
        CommandQueryMode::Uncontrolled,
        selection_mode,
        selected,
        selected_values.iter().cloned(),
        active,
        None,
        "No results",
        dialog.then_some("Command palette".to_string()),
        dialog.then_some("Run a workspace command".to_string()),
        snapshot.clone(),
        OutsidePressPolicy::DismissAndConsume,
        EscapeKeyPolicy::Dismiss,
        InitialFocusIntent::FirstFocusable,
        FocusRestoreIntent::Trigger,
        tokens,
    );
    CommandSample {
        id,
        summary,
        items: Arc::from([]),
        groups: Arc::from([]),
        index_snapshot: Some(snapshot),
        dispatched_command_id: None,
        provider_status: None,
        selected_values: selected_values.into(),
        viewport_item_count,
        row_height,
        overscan,
        state,
    }
}

fn listbox_group_descriptor(group: &ListboxGroupSample) -> ListboxGroupDescriptor {
    ListboxGroupDescriptor::new(group.value, group.label)
        .options(group.options.iter().map(listbox_option_descriptor))
}

fn listbox_option_descriptor(option: &ListboxOptionSample) -> ListboxOptionDescriptor {
    ListboxOptionDescriptor::option(option.value, option.label).disabled(option.disabled)
}

fn combobox_group_descriptor(group: &ListboxGroupSample) -> ComboboxGroupDescriptor {
    ComboboxGroupDescriptor::new(group.value, group.label)
        .options(group.options.iter().map(combobox_option_descriptor))
}

fn combobox_option_descriptor(option: &ListboxOptionSample) -> ComboboxOptionDescriptor {
    ComboboxOptionDescriptor::new(option.value, option.label).disabled(option.disabled)
}

fn radio_group_state(
    orientation: Orientation,
    size: Size,
    disabled: bool,
    required: bool,
    selected: &str,
    items: &[RadioItemSample],
    tokens: ThemeTokens,
) -> RadioGroupState {
    RadioGroupState::resolve(
        orientation,
        size,
        disabled,
        required,
        Some(selected),
        None,
        items
            .iter()
            .map(|item| RadioItemDescriptor::new(item.value, item.label).disabled(item.disabled)),
        tokens,
    )
}
