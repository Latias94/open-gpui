use std::rc::Rc;

use open_gpui::{App, Window};

mod component;
mod state;

pub use component::TableGlobalFilter;
pub use state::{TableGlobalFilterChange, TableGlobalFilterState};

pub(super) type TableGlobalFilterChangeHandler =
    Rc<dyn Fn(TableGlobalFilterChange, &mut Window, &mut App)>;
