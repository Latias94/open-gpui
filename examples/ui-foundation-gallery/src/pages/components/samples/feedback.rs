use super::*;

/// One status cue sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusCueSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible sample title.
    pub title: &'static str,
    /// Resolved state.
    pub state: StatusCueState,
}

/// One empty state sample in the gallery.
#[derive(Debug, Clone, PartialEq)]
pub struct EmptyStateSample {
    /// Stable sample id.
    pub id: &'static str,
    /// Visible sample title.
    pub title: &'static str,
    /// Resolved state.
    pub state: EmptyStateState,
}

/// Returns status cue samples backed by real component state.
pub fn status_cue_samples(tokens: ThemeTokens) -> [StatusCueSample; 3] {
    [
        (
            "sync-warning",
            "Sync warning",
            "3 anchors need review",
            FeedbackIntent::Warning,
            Size::Small,
        ),
        (
            "healthy",
            "Healthy",
            "All queues clear",
            FeedbackIntent::Success,
            Size::Medium,
        ),
        (
            "indexing",
            "Indexing",
            "Indexing workspace",
            FeedbackIntent::Info,
            Size::Medium,
        ),
    ]
    .map(|(id, title, label, intent, size)| StatusCueSample {
        id,
        title,
        state: StatusCue::new(id, label)
            .intent(intent)
            .with_size(size)
            .tokens(tokens)
            .state(),
    })
}

/// Returns empty-state samples backed by real component state.
pub fn empty_state_samples(tokens: ThemeTokens) -> [EmptyStateSample; 2] {
    [
        (
            "no-results",
            "No results",
            "No matching releases",
            Some("Adjust filters or clear the current query."),
            FeedbackIntent::Neutral,
            Size::Medium,
        ),
        (
            "blocked",
            "Blocked",
            "Queue blocked",
            Some("Resolve failing checks before merging the next item."),
            FeedbackIntent::Danger,
            Size::Small,
        ),
    ]
    .map(
        |(id, title, state_title, description, intent, size)| EmptyStateSample {
            id,
            title,
            state: {
                let empty_state = EmptyState::new(id, state_title)
                    .intent(intent)
                    .with_size(size)
                    .tokens(tokens);
                match description {
                    Some(description) => empty_state.description(description).state(),
                    None => empty_state.state(),
                }
            },
        },
    )
}
