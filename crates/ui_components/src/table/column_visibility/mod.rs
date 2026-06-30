use std::rc::Rc;

use open_gpui::{App, Window};

mod component;
mod render;
mod state;

pub use component::TableColumnVisibility;
pub use state::{
    TableColumnVisibilityAction, TableColumnVisibilityChange, TableColumnVisibilityItemState,
    TableColumnVisibilityState,
};

pub(super) type TableColumnVisibilityChangeHandler =
    Rc<dyn Fn(TableColumnVisibilityChange, &mut Window, &mut App)>;
