//! Command runtime context stack.

use open_gpui::KeyContext;

use crate::CommandScopeId;

/// App-owned command context stack.
///
/// Scope ids drive command registry projection. Key contexts drive GPUI keymap shortcut projection
/// and diagnostics. Both are ordered from broadest context to focused context.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommandContextStack {
    scope_ids: Vec<CommandScopeId>,
    key_contexts: Vec<KeyContext>,
}

impl CommandContextStack {
    /// Creates an empty command context stack.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a stack from command scopes.
    pub fn from_scopes(scopes: impl IntoIterator<Item = impl Into<CommandScopeId>>) -> Self {
        let mut stack = Self::new();
        stack.set_scopes(scopes);
        stack
    }

    /// Adds or moves a command scope to the focused end of the stack.
    pub fn scope(mut self, scope_id: impl Into<CommandScopeId>) -> Self {
        self.push_scope(scope_id);
        self
    }

    /// Replaces command scopes.
    pub fn set_scopes(
        &mut self,
        scopes: impl IntoIterator<Item = impl Into<CommandScopeId>>,
    ) -> &mut Self {
        self.scope_ids.clear();
        for scope_id in scopes {
            self.push_scope(scope_id);
        }
        self
    }

    /// Adds or moves a command scope to the focused end of the stack.
    pub fn push_scope(&mut self, scope_id: impl Into<CommandScopeId>) -> &mut Self {
        let scope_id = scope_id.into();
        if scope_id.as_str().is_empty() {
            return self;
        }
        self.scope_ids.retain(|candidate| candidate != &scope_id);
        self.scope_ids.push(scope_id);
        self
    }

    /// Clears command scopes while keeping key contexts.
    pub fn clear_scopes(&mut self) -> &mut Self {
        self.scope_ids.clear();
        self
    }

    /// Returns command scopes from broadest to focused.
    pub fn scope_ids(&self) -> &[CommandScopeId] {
        &self.scope_ids
    }

    /// Adds a GPUI key context to the focused end of the stack.
    pub fn key_context(mut self, context: KeyContext) -> Self {
        self.push_key_context(context);
        self
    }

    /// Replaces GPUI key contexts.
    pub fn set_key_contexts(
        &mut self,
        contexts: impl IntoIterator<Item = KeyContext>,
    ) -> &mut Self {
        self.key_contexts.clear();
        for context in contexts {
            self.push_key_context(context);
        }
        self
    }

    /// Adds a GPUI key context to the focused end of the stack.
    pub fn push_key_context(&mut self, context: KeyContext) -> &mut Self {
        if !context.is_empty() {
            self.key_contexts.push(context);
        }
        self
    }

    /// Clears GPUI key contexts while keeping command scopes.
    pub fn clear_key_contexts(&mut self) -> &mut Self {
        self.key_contexts.clear();
        self
    }

    /// Returns GPUI key contexts from broadest to focused.
    pub fn key_contexts(&self) -> &[KeyContext] {
        &self.key_contexts
    }

    /// Returns true when the stack has neither command scopes nor key contexts.
    pub fn is_empty(&self) -> bool {
        self.scope_ids.is_empty() && self.key_contexts.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use open_gpui::KeyContext;

    use crate::CommandContextStack;

    #[test]
    fn context_stack_moves_repeated_scope_to_focused_end() {
        let stack = CommandContextStack::new()
            .scope("global")
            .scope("workspace")
            .scope("editor")
            .scope("workspace");

        assert_eq!(
            stack
                .scope_ids()
                .iter()
                .map(|scope| scope.as_str())
                .collect::<Vec<_>>(),
            ["global", "editor", "workspace"]
        );
    }

    #[test]
    fn context_stack_preserves_key_context_depth_order() {
        let stack = CommandContextStack::new()
            .key_context(KeyContext::parse("Workspace").unwrap())
            .key_context(KeyContext::parse("Editor vim_mode=normal").unwrap());

        assert_eq!(stack.key_contexts().len(), 2);
        assert_eq!(
            stack.key_contexts()[0]
                .primary()
                .map(|entry| entry.key.as_ref()),
            Some("Workspace")
        );
        assert_eq!(
            stack.key_contexts()[1]
                .primary()
                .map(|entry| entry.key.as_ref()),
            Some("Editor")
        );
    }
}
