#![warn(missing_docs)]

//! Foundation primitives for the Open GPUI component ecosystem.
//!
//! This crate intentionally stays below the styled component layer. It provides stable vocabulary
//! for sizing, adaptive layout, tokens, accessibility, focus, and overlay helpers that are useful
//! across future component crates.

pub mod a11y;
pub mod adaptive;
pub mod focus;
pub mod overlay;
pub mod prelude;
pub mod sizing;
pub mod tokens;

pub use a11y::*;
pub use adaptive::*;
pub use focus::*;
pub use overlay::*;
pub use sizing::*;
pub use tokens::*;
