#![warn(missing_docs)]

//! Renderer-neutral foundation primitives for the Open GPUI component ecosystem.
//!
//! This crate intentionally stays below the styled component layer. It provides stable vocabulary
//! for sizing, adaptive layout, tokens, accessibility, focus, and overlay helpers that are useful
//! across future component crates without depending on the GPUI runtime or renderer types.

pub mod a11y;
pub mod active_descendant;
pub mod adaptive;
pub mod collection;
pub mod controllable_state;
pub mod focus;
pub mod geometry;
pub mod grid_viewport;
pub mod overlay;
pub mod prelude;
pub mod sizing;
pub mod table;
pub mod tokens;
pub mod virtualizer;

pub use a11y::*;
pub use active_descendant::*;
pub use adaptive::*;
pub use collection::*;
pub use controllable_state::*;
pub use focus::*;
pub use geometry::*;
pub use grid_viewport::*;
pub use overlay::*;
pub use sizing::*;
pub use table::*;
pub use tokens::*;
pub use virtualizer::*;
