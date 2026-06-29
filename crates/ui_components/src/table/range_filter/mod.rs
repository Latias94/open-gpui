use std::rc::Rc;

use open_gpui::{App, Window};

mod component;
mod render;
mod state;

pub use component::TableRangeFilter;
pub use state::{TableRangeFilterChange, TableRangeFilterState};

pub(super) type TableRangeFilterChangeHandler =
    Rc<dyn Fn(TableRangeFilterChange, &mut Window, &mut App)>;
