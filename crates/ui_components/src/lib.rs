#![warn(missing_docs)]

//! Concrete UI components for the Open GPUI component ecosystem.
//!
//! This crate sits above `open-gpui-ui-core`: it renders styled GPUI elements while consuming the
//! foundation vocabulary for sizing, tokens, accessibility, and focus.

pub mod button;
pub mod color;
pub mod prelude;
pub mod switch;

pub use button::{Button, ButtonColors, ButtonMetrics, ButtonState, ButtonVariant};
pub use color::ColorIntent;
pub use switch::{Switch, SwitchColors, SwitchMetrics, SwitchState};
