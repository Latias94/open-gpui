use std::rc::Rc;

use open_gpui::{App, Window};

mod component;
mod state;

pub use component::TablePredicateFilter;
pub use state::{
    TablePredicateFilterChange, TablePredicateFilterOperator,
    TablePredicateFilterOperatorOptionState, TablePredicateFilterState,
};

pub(super) type TablePredicateFilterChangeHandler =
    Rc<dyn Fn(TablePredicateFilterChange, &mut Window, &mut App)>;
