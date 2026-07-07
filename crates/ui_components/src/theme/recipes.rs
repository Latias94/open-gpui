#[allow(dead_code)]
const THEME_RECIPE_CATALOG: &[&str] = &[
    "alert_dialog_colors",
    "avatar_colors",
    "avatar_group_count_colors",
    "badge_colors",
    "button_colors",
    "checkbox_colors",
    "combobox_colors",
    "command_colors",
    "dialog_colors",
    "feedback_colors",
    "field_colors",
    "hover_card_colors",
    "kbd_colors",
    "label_colors",
    "listbox_colors",
    "menu_colors",
    "popover_colors",
    "progress_colors",
    "radio_group_colors",
    "select_colors",
    "separator_colors",
    "sheet_colors",
    "skeleton_colors",
    "switch_colors",
    "table_toolbar_colors",
    "text_input_colors",
    "textarea_colors",
    "tooltip_colors",
    "virtualized_list_colors",
];

#[path = "recipes/action.rs"]
mod action;
#[path = "recipes/choice.rs"]
mod choice;
#[path = "recipes/data.rs"]
mod data;
#[path = "recipes/display.rs"]
mod display;
#[path = "recipes/form.rs"]
mod form;
#[path = "recipes/overlay.rs"]
mod overlay;
