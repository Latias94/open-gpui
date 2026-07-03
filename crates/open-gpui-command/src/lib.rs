#![warn(missing_docs)]

//! Command registry, projection, history, and GPUI action adapters.
//!
//! This crate owns application command metadata and deterministic projections. It deliberately
//! avoids depending on concrete UI components, so command palettes, menus, and plugin-like modules
//! can share one command domain model.

mod availability;
mod center;
pub mod gpui;
mod history;
mod menu;
mod provider;
mod refresh;
mod registry;
mod scope;

pub use availability::{
    CommandAvailability, CommandAvailabilityMap, CommandAvailabilityResolver,
    command_effective_availability,
};
pub use center::{CommandCenter, CommandProviderRegistration, CommandSourceRegistration};
pub use gpui::{
    CommandDispatchOutcome, CommandShortcutDiagnostic, CommandShortcutDiagnosticKind,
    GpuiCommandAction, GpuiCommandActionMap, command_shortcut_label,
    command_shortcut_label_from_keymap,
};
pub use history::{CommandHistoryEntry, CommandUsageHistory, MemoryCommandHistory};
pub use menu::{CommandMenuCommand, CommandMenuEntry, CommandMenuSubmenu, CommandMenuTree};
pub use provider::{
    CommandProvider, CommandProviderApplyOutcome, CommandProviderId, CommandProviderRequest,
    CommandProviderRequestId, CommandProviderResponse, CommandProviderSource,
    CommandProviderStaleResponse, CommandProviderState, CommandProviderStatus,
};
pub use refresh::{CommandProviderRefreshController, CommandProviderRefreshProjection};
pub use registry::{
    CommandContribution, CommandDescriptor, CommandRegistry, CommandRegistryError,
    CommandRegistrySnapshot, CommandSourceId,
};
pub use scope::{
    CommandProjectionDiagnostic, CommandProjectionDiagnosticKind, CommandScopeId,
    CommandScopeProjection, ScopedCommandRegistry,
};
