#![warn(missing_docs)]

//! Concrete UI components for the Open GPUI component ecosystem.
//!
//! This crate sits above `open-gpui-ui-core`: it renders styled GPUI elements while consuming the
//! foundation vocabulary for sizing, tokens, accessibility, and focus.

pub mod button;
pub mod color;
pub mod field;
pub mod focus;
pub mod prelude;
pub mod switch;
pub mod text_input;
pub mod theme;

pub use button::{Button, ButtonColors, ButtonMetrics, ButtonState, ButtonVariant};
pub use color::{ColorIntent, ColorState};
pub use field::{Field, FieldColors, FieldMessage, FieldMetrics, FieldState};
pub use focus::{DEFAULT_FOCUS_RING_WIDTH, FocusRing, focus_ring_shadow};
pub use switch::{Switch, SwitchColors, SwitchMetrics, SwitchState};
pub use text_input::{TextInput, TextInputColors, TextInputMetrics, TextInputState};
pub use theme::{ThemeColor, ThemeMode, ThemeResolver, ThemeSnapshot};
