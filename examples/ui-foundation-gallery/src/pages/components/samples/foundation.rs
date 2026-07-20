use super::*;

/// One button sample in the gallery.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ButtonSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible label.
    pub label: &'static str,
    /// Resolved state.
    pub state: ButtonState,
}

/// One badge sample in the gallery.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BadgeSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible label.
    pub label: &'static str,
    /// Resolved state.
    pub state: BadgeState,
}

/// One accordion sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct AccordionSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample title.
    pub title: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Resolved state.
    pub state: AccordionState,
    /// Concrete items rendered by the sample.
    pub items: Vec<AccordionItem>,
}

/// One collapsible sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct CollapsibleSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Resolved state.
    pub state: CollapsibleState,
    /// Visible content copy.
    pub content: &'static str,
}

/// One slider sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct SliderSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Resolved state.
    pub state: SliderState,
}

/// One number input sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct NumberInputSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Resolved state.
    pub state: NumberInputState,
}

/// One toggle group sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ToggleGroupSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Resolved state.
    pub state: ToggleGroupState,
}

/// One link sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Resolved state.
    pub state: LinkState,
}

/// One breadcrumb sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct BreadcrumbSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Resolved state.
    pub state: BreadcrumbState,
}

/// One tag sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct TagSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Resolved state.
    pub state: TagState,
}

/// One toast stack sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct ToastStackSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Resolved state.
    pub state: ToastStackState,
}

/// One icon button sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct IconButtonSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible icon glyph.
    pub icon: &'static str,
    /// Resolved state.
    pub state: IconButtonState,
}

/// One separator sample in the gallery.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeparatorSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible sample title.
    pub title: &'static str,
    /// Resolved state.
    pub state: SeparatorState,
}

/// One keyboard shortcut sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct KbdSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Resolved state.
    pub state: KbdState,
}

/// One progress sample in the gallery.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProgressSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Accessible progress label.
    pub label: &'static str,
    /// Resolved state.
    pub state: ProgressState,
}

/// One skeleton sample in the gallery.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkeletonSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible sample title.
    pub title: &'static str,
    /// Resolved state.
    pub state: SkeletonState,
}

/// One avatar sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct AvatarSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Resolved state.
    pub state: AvatarState,
}

/// One avatar group sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct AvatarGroupSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Sample summary.
    pub summary: &'static str,
    /// Visible child avatars.
    pub avatars: Vec<AvatarSample>,
    /// Overflow counter label.
    pub count_label: &'static str,
}

/// Returns button samples backed by real component state.
pub fn button_samples(tokens: ThemeTokens) -> [ButtonSample; 6] {
    [
        (
            "default",
            "Default",
            ButtonVariant::Default,
            Size::Medium,
            false,
            false,
        ),
        (
            "secondary",
            "Secondary",
            ButtonVariant::Secondary,
            Size::Medium,
            false,
            false,
        ),
        (
            "outline",
            "Outline",
            ButtonVariant::Outline,
            Size::Small,
            false,
            false,
        ),
        (
            "destructive",
            "Destructive",
            ButtonVariant::Destructive,
            Size::Medium,
            false,
            false,
        ),
        (
            "selected",
            "Selected",
            ButtonVariant::Ghost,
            Size::Medium,
            false,
            true,
        ),
        (
            "disabled",
            "Disabled",
            ButtonVariant::Default,
            Size::Medium,
            true,
            false,
        ),
    ]
    .map(
        |(id, label, variant, size, disabled, selected)| ButtonSample {
            id,
            label,
            state: Button::new(id, label)
                .variant(variant)
                .with_size(size)
                .disabled(disabled)
                .selected(selected)
                .tokens(tokens)
                .state(),
        },
    )
}

/// Returns badge samples backed by real component state.
pub fn badge_samples(tokens: ThemeTokens) -> [BadgeSample; 4] {
    [
        ("default", "Live", BadgeVariant::Default, Size::Medium),
        ("secondary", "Beta", BadgeVariant::Secondary, Size::Medium),
        (
            "destructive",
            "Risk",
            BadgeVariant::Destructive,
            Size::Medium,
        ),
        ("outline", "Neutral", BadgeVariant::Outline, Size::Small),
    ]
    .map(|(id, label, variant, size)| BadgeSample {
        id,
        label,
        state: Badge::new(id, label)
            .variant(variant)
            .with_size(size)
            .tokens(tokens)
            .state(),
    })
}

/// Grouped samples for newly completed foundation components.
#[derive(Debug, Clone, PartialEq)]
pub struct FoundationComponentSamples {
    /// Accordion samples.
    pub accordions: [AccordionSample; 1],
    /// Collapsible samples.
    pub collapsibles: [CollapsibleSample; 1],
    /// Slider samples.
    pub sliders: [SliderSample; 2],
    /// Number input samples.
    pub number_inputs: [NumberInputSample; 2],
    /// Toggle group samples.
    pub toggle_groups: [ToggleGroupSample; 2],
    /// Link samples.
    pub links: [LinkSample; 2],
    /// Breadcrumb samples.
    pub breadcrumbs: [BreadcrumbSample; 1],
    /// Tag samples.
    pub tags: [TagSample; 3],
    /// Toast stack samples.
    pub toast_stacks: [ToastStackSample; 1],
}

/// Returns samples for the foundation component completion slice.
pub fn foundation_component_samples(tokens: ThemeTokens) -> FoundationComponentSamples {
    FoundationComponentSamples {
        accordions: accordion_samples(tokens),
        collapsibles: collapsible_samples(tokens),
        sliders: slider_samples(tokens),
        number_inputs: number_input_samples(tokens),
        toggle_groups: toggle_group_samples(tokens),
        links: link_samples(tokens),
        breadcrumbs: breadcrumb_samples(tokens),
        tags: tag_samples(tokens),
        toast_stacks: toast_stack_samples(tokens),
    }
}

/// Returns accordion samples backed by real component state.
pub fn accordion_samples(tokens: ThemeTokens) -> [AccordionSample; 1] {
    let items = vec![
        AccordionItem::new("scope", "Scope", "Component contracts, samples, and tests."),
        AccordionItem::new(
            "risk",
            "Risk",
            "Breaking changes are acceptable before launch.",
        ),
        AccordionItem::new(
            "done",
            "Done",
            "Exported state and gallery coverage are required.",
        )
        .disabled(true),
    ];
    let accordion = Accordion::new("shipping")
        .mode(AccordionMode::Multiple)
        .collapsible(true)
        .default_open_values(["scope", "risk"])
        .tokens(tokens);
    let state = items
        .iter()
        .cloned()
        .fold(accordion, |accordion, item| accordion.item(item))
        .state();

    [AccordionSample {
        id: "shipping",
        title: "Shipping checklist",
        summary: "Multiple open panels with one disabled item.",
        state,
        items,
    }]
}

/// Returns collapsible samples backed by real component state.
pub fn collapsible_samples(tokens: ThemeTokens) -> [CollapsibleSample; 1] {
    [CollapsibleSample {
        id: "release-notes",
        summary: "Controlled disclosure content that keeps trigger and panel roles separate.",
        content: "Release notes stay mounted only when the disclosure is open.",
        state: Collapsible::new("release-notes", "Release notes")
            .default_open(true)
            .tokens(tokens)
            .state(),
    }]
}

/// Returns slider samples backed by real component state.
pub fn slider_samples(tokens: ThemeTokens) -> [SliderSample; 2] {
    [
        (
            "volume",
            "Volume",
            72.0,
            0.0,
            100.0,
            1.0,
            false,
            Size::Medium,
        ),
        (
            "threshold",
            "Threshold",
            42.0,
            0.0,
            50.0,
            5.0,
            true,
            Size::Small,
        ),
    ]
    .map(
        |(id, label, value, min, max, step, disabled, size)| SliderSample {
            id,
            state: Slider::new(id, label)
                .value(value)
                .min(min)
                .max(max)
                .step(step)
                .disabled(disabled)
                .with_size(size)
                .tokens(tokens)
                .state(),
        },
    )
}

/// Returns number input samples backed by real component state.
pub fn number_input_samples(tokens: ThemeTokens) -> [NumberInputSample; 2] {
    [
        ("workers", "Workers", 6.0, 1.0, 12.0, 1.0, false, false),
        ("budget", "Budget", 85.0, 0.0, 100.0, 5.0, false, true),
    ]
    .map(
        |(id, label, value, min, max, step, read_only, invalid)| NumberInputSample {
            id,
            state: NumberInput::new(id, label)
                .value(value)
                .min(min)
                .max(max)
                .step(step)
                .read_only(read_only)
                .invalid(invalid)
                .tokens(tokens)
                .state(),
        },
    )
}

/// Returns toggle group samples backed by real component state.
pub fn toggle_group_samples(tokens: ThemeTokens) -> [ToggleGroupSample; 2] {
    let alignment = ToggleGroup::new("alignment", "Alignment")
        .item(ToggleGroupItem::new("left", "Left"))
        .item(ToggleGroupItem::new("center", "Center"))
        .item(ToggleGroupItem::new("right", "Right").disabled(true))
        .selected_values(["left"])
        .default_focused("center")
        .selection_required(true)
        .tokens(tokens)
        .state();
    let formatting = ToggleGroup::new("formatting", "Formatting")
        .mode(ToggleGroupSelectionMode::Multiple)
        .item(ToggleGroupItem::new("bold", "Bold"))
        .item(ToggleGroupItem::new("italic", "Italic"))
        .item(ToggleGroupItem::new("code", "Code"))
        .selected_values(["bold", "code"])
        .tokens(tokens)
        .state();

    [
        ToggleGroupSample {
            id: "alignment",
            summary: "Required single selection with disabled item skip.",
            state: alignment,
        },
        ToggleGroupSample {
            id: "formatting",
            summary: "Multiple stable values selected at once.",
            state: formatting,
        },
    ]
}

/// Returns link samples backed by real component state.
pub fn link_samples(tokens: ThemeTokens) -> [LinkSample; 2] {
    [
        LinkSample {
            id: "docs",
            state: Link::new("docs", "Component docs", "/docs/components")
                .external(true)
                .tokens(tokens)
                .state(),
        },
        LinkSample {
            id: "disabled",
            state: Link::new("disabled", "Disabled target", "/disabled")
                .disabled(true)
                .tokens(tokens)
                .state(),
        },
    ]
}

/// Returns breadcrumb samples backed by real component state.
pub fn breadcrumb_samples(tokens: ThemeTokens) -> [BreadcrumbSample; 1] {
    [BreadcrumbSample {
        id: "project",
        state: Breadcrumb::new("project", "Project path")
            .item(BreadcrumbItemDescriptor::new("home", "Home").href("/"))
            .item(BreadcrumbItemDescriptor::new("ui", "UI").href("/ui"))
            .item(BreadcrumbItemDescriptor::new("components", "Components").current(true))
            .tokens(tokens)
            .state(),
    }]
}

/// Returns tag samples backed by real component state.
pub fn tag_samples(tokens: ThemeTokens) -> [TagSample; 3] {
    [
        ("ready", "ready", "Ready", TagVariant::Default, true, false),
        (
            "blocked",
            "blocked",
            "Blocked",
            TagVariant::Destructive,
            false,
            false,
        ),
        (
            "archived",
            "archived",
            "Archived",
            TagVariant::Outline,
            true,
            true,
        ),
    ]
    .map(
        |(id, value, label, variant, removable, disabled)| TagSample {
            id,
            state: Tag::new(id, value, label)
                .variant(variant)
                .removable(removable)
                .disabled(disabled)
                .tokens(tokens)
                .state(),
        },
    )
}

/// Returns toast stack samples backed by real component state.
pub fn toast_stack_samples(tokens: ThemeTokens) -> [ToastStackSample; 1] {
    [ToastStackSample {
        id: "notifications",
        state: ToastStack::new("notifications", "Notifications")
            .max_visible(2)
            .toast(
                Toast::new("saved", "Saved")
                    .description("Settings are synced.")
                    .intent(FeedbackIntent::Success)
                    .live(open_gpui_ui_core::LivePoliteness::Off)
                    .action("Undo"),
            )
            .toast(
                Toast::new("queued", "Queued")
                    .description("Release job will start shortly.")
                    .intent(FeedbackIntent::Info)
                    .live(open_gpui_ui_core::LivePoliteness::Off)
                    .timeout(Duration::from_secs(8)),
            )
            .toast(
                Toast::new("expired", "Expired")
                    .live(open_gpui_ui_core::LivePoliteness::Off)
                    .elapsed(Duration::from_secs(8))
                    .timeout(Duration::from_secs(2)),
            )
            .tokens(tokens)
            .state(),
    }]
}

/// Returns icon button samples backed by real component state.
pub fn icon_button_samples(tokens: ThemeTokens) -> [IconButtonSample; 4] {
    [
        (
            "search",
            "?",
            "Search",
            ButtonVariant::Ghost,
            false,
            Size::Medium,
        ),
        (
            "add",
            "+",
            "Add item",
            ButtonVariant::Outline,
            false,
            Size::Small,
        ),
        (
            "delete",
            "!",
            "Delete item",
            ButtonVariant::Destructive,
            false,
            Size::Medium,
        ),
        (
            "locked",
            "x",
            "Locked action",
            ButtonVariant::Ghost,
            true,
            Size::Medium,
        ),
    ]
    .map(
        |(id, icon, accessible_label, variant, disabled, size)| IconButtonSample {
            id,
            icon,
            state: IconButton::new(id, icon, accessible_label)
                .variant(variant)
                .disabled(disabled)
                .with_size(size)
                .tokens(tokens)
                .state(),
        },
    )
}

/// Returns separator samples backed by real component state.
pub fn separator_samples(tokens: ThemeTokens) -> [SeparatorSample; 3] {
    [
        (
            "section-rule",
            "Section rule",
            Orientation::Horizontal,
            false,
            Size::Medium,
        ),
        (
            "panel-divider",
            "Panel divider",
            Orientation::Vertical,
            false,
            Size::Large,
        ),
        (
            "decorative-rule",
            "Decorative rule",
            Orientation::Horizontal,
            true,
            Size::Small,
        ),
    ]
    .map(
        |(id, title, orientation, decorative, size)| SeparatorSample {
            id,
            title,
            state: Separator::new(id)
                .orientation(orientation)
                .decorative(decorative)
                .with_size(size)
                .tokens(tokens)
                .state(),
        },
    )
}

/// Returns keyboard shortcut samples backed by real component state.
pub fn kbd_samples(tokens: ThemeTokens) -> [KbdSample; 3] {
    [
        ("command-palette", "Ctrl+K", Size::Medium),
        ("save", "Ctrl+S", Size::Small),
        ("confirm", "Enter", Size::Large),
    ]
    .map(|(id, label, size)| KbdSample {
        id,
        state: Kbd::new(id, label).with_size(size).tokens(tokens).state(),
    })
}

/// Returns progress samples backed by real component state.
pub fn progress_samples(tokens: ThemeTokens) -> [ProgressSample; 3] {
    [
        ("sync", "Sync progress", Some(64.0), Size::Medium),
        ("complete", "Complete progress", Some(100.0), Size::Large),
        ("indexing", "Indexing", None, Size::Small),
    ]
    .map(|(id, label, value_percent, size)| {
        let progress = Progress::new(id, label).with_size(size).tokens(tokens);
        let progress = match value_percent {
            Some(value) => progress.value(value),
            None => progress.indeterminate(),
        };

        ProgressSample {
            id,
            label,
            state: progress.state(),
        }
    })
}

/// Returns skeleton samples backed by real component state.
pub fn skeleton_samples(tokens: ThemeTokens) -> [SkeletonSample; 3] {
    [
        ("body-line", "Body line", false, Size::Medium),
        ("compact-line", "Compact line", true, Size::Small),
        ("headline", "Headline", false, Size::Large),
    ]
    .map(|(id, title, subtle, size)| SkeletonSample {
        id,
        title,
        state: Skeleton::new(id)
            .subtle(subtle)
            .with_size(size)
            .tokens(tokens)
            .state(),
    })
}

/// Returns avatar samples backed by real component state.
pub fn avatar_samples(tokens: ThemeTokens) -> [AvatarSample; 4] {
    [
        (
            "ada",
            "Ada Lovelace",
            None,
            None,
            "Ada Lovelace",
            Size::Medium,
        ),
        (
            "current-user",
            "Grace Hopper",
            None,
            Some("ME"),
            "Current user",
            Size::Large,
        ),
        (
            "source-user",
            "Katherine Johnson",
            Some("asset://avatars/katherine.png"),
            None,
            "Katherine profile photo",
            Size::Small,
        ),
        ("empty", "  ", None, None, "Anonymous avatar", Size::Small),
    ]
    .map(|(id, name, source, fallback, accessible_label, size)| {
        let avatar = Avatar::new(id, name)
            .accessible_label(accessible_label)
            .with_size(size)
            .tokens(tokens);
        let avatar = match source {
            Some(source) => avatar.source(source),
            None => avatar,
        };
        let avatar = match fallback {
            Some(fallback) => avatar.fallback(fallback),
            None => avatar,
        };

        AvatarSample {
            id,
            state: avatar.state(),
        }
    })
}

/// Returns avatar group samples backed by real component state.
pub fn avatar_group_samples(tokens: ThemeTokens) -> [AvatarGroupSample; 1] {
    [AvatarGroupSample {
        id: "team",
        summary: "Compact overlapping roster with overflow count",
        avatars: vec![
            AvatarSample {
                id: "team-ada",
                state: Avatar::new("team-ada", "Ada Lovelace")
                    .accessible_label("Ada Lovelace")
                    .with_size(Size::Medium)
                    .tokens(tokens)
                    .state(),
            },
            AvatarSample {
                id: "team-grace",
                state: Avatar::new("team-grace", "Grace Hopper")
                    .accessible_label("Grace Hopper")
                    .with_size(Size::Medium)
                    .tokens(tokens)
                    .state(),
            },
            AvatarSample {
                id: "team-katherine",
                state: Avatar::new("team-katherine", "Katherine Johnson")
                    .accessible_label("Katherine Johnson")
                    .with_size(Size::Medium)
                    .tokens(tokens)
                    .state(),
            },
            AvatarSample {
                id: "team-margaret",
                state: Avatar::new("team-margaret", "Margaret Hamilton")
                    .accessible_label("Margaret Hamilton")
                    .with_size(Size::Medium)
                    .tokens(tokens)
                    .state(),
            },
        ],
        count_label: "+1",
    }]
}
