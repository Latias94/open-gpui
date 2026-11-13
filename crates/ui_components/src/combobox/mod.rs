//! Combobox component built from editable text input, overlay, and listbox state.
//!
//! Query text, selected identity, externally filtered results, and open state can
//! be controlled independently. Use `selected_item` to supply a selected label
//! before it appears in any result page. External pages retain the last resolved
//! selected label across loading, paging, and errors; unique current labels win.
//!
//! [`ComboboxQuerySession`] is an optional application-owned stale-reply boundary.
//! The component never starts a provider or assumes ownership of application tasks.
//! `results_content` composes around the owned listbox inside the existing popup;
//! the callback must render its supplied listbox element exactly once.
//! `option_content` composes individual result rows with application thumbnails,
//! labels, and metadata. Use [`crate::listbox::ListboxRowAction`] for nested actions
//! or editors so their gestures stay separate from result selection.
//!
//! Contracts follow Base UI's independent input/selection/open and externally
//! filtered item channels, with shadcn's popup/list/status composition translated
//! through the existing GPUI text, listbox, and window overlay owners.
//!
//! Source contracts: [Base UI AriaCombobox](https://github.com/mui/base-ui/blob/fb9fbbf35e72bd4a5494b678c85d2ea0457fed60/packages/react/src/combobox/root/AriaCombobox.tsx)
//! and [shadcn Combobox](https://github.com/shadcn-ui/ui/blob/dd41d19e2f80ff6891951f7ced74c2c9a650b065/apps/v4/registry/bases/base/ui/combobox.tsx).

mod descriptor;
mod model;
mod query;
mod render_plan;
mod runtime;
mod style;

pub use descriptor::{
    ComboboxGroup, ComboboxGroupDescriptor, ComboboxOption, ComboboxOptionDescriptor,
};
pub use model::{ComboboxOpenMode, ComboboxSelection, ComboboxState, ComboboxStateRequest};
pub use query::{
    ComboboxFilterMode, ComboboxQueryRequest, ComboboxQuerySession, ComboboxResults,
    ComboboxResultsStatus,
};
pub use runtime::Combobox;
pub use style::{ComboboxColors, ComboboxMetrics};
