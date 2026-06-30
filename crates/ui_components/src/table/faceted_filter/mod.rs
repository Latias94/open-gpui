use std::rc::Rc;

use open_gpui::{App, Window};

mod component;
mod render;
mod state;

pub use component::TableFacetedFilter;
pub use state::{TableFacetedFilterChange, TableFacetedFilterOptionState, TableFacetedFilterState};

pub(super) type TableFacetedFilterChangeHandler =
    Rc<dyn Fn(TableFacetedFilterChange, &mut Window, &mut App)>;
