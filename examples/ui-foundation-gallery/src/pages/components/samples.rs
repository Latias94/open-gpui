//! Component sample descriptors and resolved-state builders for the foundation gallery.

use open_gpui::{
    AppContext, InteractiveElement, ParentElement, StatefulInteractiveElement, Styled, div,
    prelude::FluentBuilder, rgb,
};
use open_gpui_command::CommandContextStack;
use open_gpui_ui_components::{
    Accordion, AccordionItem, AccordionMode, AccordionState, ActionDescriptor,
    ActionIconDescriptor, Avatar, AvatarState, Badge, BadgeState, BadgeVariant, Breadcrumb,
    BreadcrumbItemDescriptor, BreadcrumbState, Button, ButtonState, ButtonVariant, Checkbox,
    CheckboxState, Collapsible, CollapsibleState, ComboboxGroupDescriptor,
    ComboboxOptionDescriptor, ComboboxState, ComboboxStateRequest, CommandGroupDescriptor,
    CommandIndexSnapshot, CommandIndexSnapshotMode, CommandItemDescriptor,
    CommandKeyBindingCaptureState, CommandKeyBindingEditorFilter,
    CommandKeyBindingEditorPreviewState, CommandKeyBindingEditorState, CommandLoadingState,
    CommandPaletteController, CommandPaletteProjection, CommandQueryMode, CommandSelectionMode,
    CommandShortcutInspectorState, CommandState, CommandStateDataSource, CommandStateRequest,
    CommandStatusItem, EmptyState, EmptyStateState, FeedbackIntent, Field, FieldState, IconButton,
    IconButtonState, Kbd, KbdState, Label, LabelState, Link, LinkState, ListboxGroupDescriptor,
    ListboxOptionDescriptor, ListboxState, NumberInput, NumberInputState, Progress, ProgressState,
    RadioGroupState, RadioItemDescriptor, ResolvedActionIcon, ResolvedActionState, ScrollAreaAxis,
    ScrollAreaState, ScrollResetPolicy, SelectState, SelectStateRequest, Separator, SeparatorState,
    SidebarCollapseMode, SidebarItemDescriptor, SidebarSectionDescriptor, SidebarSide,
    SidebarState, SidebarVariant, Skeleton, SkeletonState, Slider, SliderState,
    SplitterPanelDescriptor, SplitterState, StatusCue, StatusCueState, Switch, SwitchState, Table,
    TableBehaviorSnapshot, Tabs, TabsActivationMode, TabsItem, TabsItemDescriptor, TabsState, Tag,
    TagState, TagVariant, TextInput, TextInputDisplayMode, TextInputState, Textarea, TextareaState,
    Toast, ToastStack, ToastStackState, Toggle, ToggleGroup, ToggleGroupItem,
    ToggleGroupSelectionMode, ToggleGroupState, ToggleState, ToggleVariant, Toolbar, ToolbarItem,
    ToolbarItemDescriptor, ToolbarItemKind, ToolbarState, Tree, TreeBehaviorSnapshot,
    TreeItemDescriptor, TreeState, VirtualizedList, VirtualizedListBehaviorSnapshot,
    VirtualizedListItemDescriptor, VirtualizedListMetrics, VirtualizedListRowMeasureMode,
    VirtualizedListRowRenderContext, VirtualizedListScrollStrategy, VirtualizedListSelectionMode,
    VirtualizedListState,
};
use open_gpui_ui_core::{
    EscapeKeyPolicy, FocusRestoreIntent, InitialFocusIntent, Orientation, OutsidePressPolicy,
    OverlayPlacementAlignment, OverlayPlacementSide, Sizable, Size, TableAggregation,
    TableCellValue, TableColumn, TableColumnFacets, TableColumnGroup, TableColumnId,
    TableColumnPinning, TableColumnSizing, TableFacetValueCount, TableFilter, TablePagination,
    TableRow, TableRowPinning, TableSelectOption, TableSort, TableStageMode, TableState,
    ThemeTokens, UiPx, VirtualizerItemKey, VirtualizerSnapshot, VirtualizerSnapshotItem, ui_px,
};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use super::runtime::{current_tree_sample_items, record_virtualized_list_nested_action};

#[path = "samples/choice.rs"]
mod choice;
#[path = "samples/feedback.rs"]
mod feedback;
#[path = "samples/foundation.rs"]
mod foundation;
#[path = "samples/layout.rs"]
mod layout;
#[path = "samples/navigation.rs"]
mod navigation;
#[path = "samples/table.rs"]
mod table;
#[path = "samples/text.rs"]
mod text;
#[path = "samples/tree.rs"]
mod tree;
#[path = "samples/virtualized_list.rs"]
mod virtualized_list;

pub use choice::{
    CheckboxSample, ComboboxSample, CommandSample, ListboxGroupSample, ListboxOptionSample,
    ListboxSample, RadioGroupSample, RadioItemSample, SelectSample, SwitchSample, ToggleSample,
    checkbox_samples, combobox_samples, command_samples, listbox_samples, radio_group_samples,
    select_samples, switch_samples, toggle_samples,
};
pub use feedback::{EmptyStateSample, StatusCueSample, empty_state_samples, status_cue_samples};
pub use foundation::{
    AccordionSample, AvatarGroupSample, AvatarSample, BadgeSample, BreadcrumbSample, ButtonSample,
    CollapsibleSample, FoundationComponentSamples, IconButtonSample, KbdSample, LinkSample,
    NumberInputSample, ProgressSample, SeparatorSample, SkeletonSample, SliderSample, TagSample,
    ToastStackSample, ToggleGroupSample, accordion_samples, avatar_group_samples, avatar_samples,
    badge_samples, breadcrumb_samples, button_samples, collapsible_samples,
    foundation_component_samples, icon_button_samples, kbd_samples, link_samples,
    number_input_samples, progress_samples, separator_samples, skeleton_samples, slider_samples,
    tag_samples, toast_stack_samples, toggle_group_samples,
};
pub use layout::{
    ScrollAreaSample, SplitterPanelSample, SplitterSample, scroll_area_samples, splitter_samples,
};
pub use navigation::{
    SidebarItemSample, SidebarSample, SidebarSectionSample, TabsItemSample, TabsSample,
    ToolbarItemSample, ToolbarSample, sidebar_samples, tabs_samples, toolbar_samples,
};
pub(crate) use table::server_tree_table_state;
pub use table::{TableSample, TableSampleStateSummary, table_samples};
pub use text::{
    FieldSample, FieldTextareaSample, LabelSample, TextInputSample, TextareaSample, field_samples,
    field_textarea_samples, label_samples, text_input_samples, textarea_samples,
};
pub use tree::{TreeSample, TreeStateContractSample, tree_samples, tree_state_contract_samples};
pub use virtualized_list::{
    VirtualizedListSample, VirtualizedListSampleRenderer, VirtualizedListSampleStateSummary,
    VirtualizedListStateContractSample, virtualized_list_samples,
    virtualized_list_state_contract_samples,
};

macro_rules! impl_component_sample_selectors {
    ($ty:ident, $selector_family:literal) => {
        impl $ty {
            /// Returns the stable debug selector used by the gallery shell and tests.
            pub fn debug_selector(&self) -> String {
                format!("gallery:{}:{}", $selector_family, self.id)
            }
        }
    };
}

impl_component_sample_selectors!(ButtonSample, "component-button-sample");
impl_component_sample_selectors!(BadgeSample, "component-badge-sample");
impl_component_sample_selectors!(AccordionSample, "component-accordion-sample");
impl_component_sample_selectors!(CollapsibleSample, "component-collapsible-sample");
impl_component_sample_selectors!(SliderSample, "component-slider-sample");
impl_component_sample_selectors!(NumberInputSample, "component-number-input-sample");
impl_component_sample_selectors!(ToggleGroupSample, "component-toggle-group-sample");
impl_component_sample_selectors!(LinkSample, "component-link-sample");
impl_component_sample_selectors!(BreadcrumbSample, "component-breadcrumb-sample");
impl_component_sample_selectors!(TagSample, "component-tag-sample");
impl_component_sample_selectors!(ToastStackSample, "component-toast-stack-sample");
impl_component_sample_selectors!(IconButtonSample, "component-icon-button-sample");
impl_component_sample_selectors!(SwitchSample, "component-switch-sample");
impl_component_sample_selectors!(CheckboxSample, "component-checkbox-sample");
impl_component_sample_selectors!(RadioGroupSample, "component-radio-sample");
impl_component_sample_selectors!(ToggleSample, "component-toggle-sample");
impl_component_sample_selectors!(ToolbarSample, "component-toolbar-sample");
impl_component_sample_selectors!(SidebarSample, "component-sidebar-sample");
impl_component_sample_selectors!(TreeSample, "component-tree-sample");
impl_component_sample_selectors!(ListboxSample, "component-listbox-sample");
impl_component_sample_selectors!(SelectSample, "component-select-sample");
impl_component_sample_selectors!(ComboboxSample, "component-combobox-sample");
impl_component_sample_selectors!(CommandSample, "component-command-sample");
impl_component_sample_selectors!(LabelSample, "component-label-sample");
impl_component_sample_selectors!(TextInputSample, "component-text-input-sample");
impl_component_sample_selectors!(TextareaSample, "component-textarea-sample");
impl_component_sample_selectors!(FieldSample, "component-field-sample");
impl_component_sample_selectors!(FieldTextareaSample, "component-field-textarea-sample");
impl_component_sample_selectors!(TabsSample, "component-tabs-sample");
impl_component_sample_selectors!(TableSample, "component-table-sample");
impl_component_sample_selectors!(VirtualizedListSample, "component-virtualized-list-sample");
impl_component_sample_selectors!(ScrollAreaSample, "component-scroll-area-sample");
impl_component_sample_selectors!(SplitterSample, "component-splitter-sample");
impl_component_sample_selectors!(SeparatorSample, "component-separator-sample");
impl_component_sample_selectors!(KbdSample, "component-kbd-sample");
impl_component_sample_selectors!(ProgressSample, "component-progress-sample");
impl_component_sample_selectors!(SkeletonSample, "component-skeleton-sample");
impl_component_sample_selectors!(AvatarSample, "component-avatar-sample");
impl_component_sample_selectors!(AvatarGroupSample, "component-avatar-group-sample");
impl_component_sample_selectors!(StatusCueSample, "component-status-cue-sample");
impl_component_sample_selectors!(EmptyStateSample, "component-empty-state-sample");
