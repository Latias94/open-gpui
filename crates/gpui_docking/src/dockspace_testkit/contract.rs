use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DockspaceAdapterTag {
    Headless,
    GpuiHost,
    Native,
    Benchmark,
}

impl DockspaceAdapterTag {
    pub(crate) const fn fixture_name(self) -> &'static str {
        match self {
            Self::Headless => "headless",
            Self::GpuiHost => "GPUI host",
            Self::Native => "native",
            Self::Benchmark => "benchmark",
        }
    }

    pub(crate) fn supported_capabilities(self) -> BTreeSet<DockspaceAdapterCapability> {
        use DockspaceAdapterCapability as Capability;

        let capabilities: &[Capability] = match self {
            Self::Headless => &[
                Capability::SemanticOperation,
                Capability::MoveItemToItem,
                Capability::MoveItemToTab,
                Capability::MoveItemToEmptySpace,
                Capability::FloatItem,
                Capability::MoveFloating,
                Capability::ItemPayload,
                Capability::WholeFloatingPayload,
                Capability::CrossViewport,
            ],
            Self::GpuiHost => &[
                Capability::SemanticOperation,
                Capability::InteractionScript,
                Capability::MoveItemToItem,
                Capability::ItemPayload,
                Capability::CrossViewport,
            ],
            Self::Native => &[
                Capability::SemanticOperation,
                Capability::InteractionScript,
                Capability::MoveItemToItem,
                Capability::MoveItemToEmptySpace,
                Capability::ItemPayload,
                Capability::CrossViewport,
                Capability::NativeCapture,
            ],
            Self::Benchmark => &[
                Capability::SemanticOperation,
                Capability::MoveItemToItem,
                Capability::MoveItemToTab,
                Capability::MoveItemToEmptySpace,
                Capability::FloatItem,
                Capability::MoveFloating,
                Capability::ItemPayload,
                Capability::WholeFloatingPayload,
                Capability::CrossViewport,
                Capability::Performance,
            ],
        };
        capabilities.iter().copied().collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DockspaceAdapterCapability {
    SemanticOperation,
    InteractionScript,
    MoveItemToItem,
    MoveItemToTab,
    MoveItemToEmptySpace,
    FloatItem,
    MoveFloating,
    ItemPayload,
    WholeGroupPayload,
    WholeFloatingPayload,
    RetainedSubtreePayload,
    CrossViewport,
    NativeCapture,
    CommitReceipt,
    Recovery,
    ProviderFirstReservation,
    ShutdownDrain,
    Performance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DockspaceOracleKind {
    Graph,
    Layout,
    Presentation,
    FocusSelection,
    ViewportRoute,
    NativeLifetime,
    Performance,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DockspaceAdapterContract {
    pub(crate) adapter: DockspaceAdapterTag,
    pub(crate) required_capabilities: BTreeSet<DockspaceAdapterCapability>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub(crate) optional_capabilities: BTreeSet<DockspaceAdapterCapability>,
    pub(crate) required_oracles: BTreeSet<DockspaceOracleKind>,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub(crate) optional_oracles: BTreeSet<DockspaceOracleKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DockspacePrepareDecision {
    Prepared,
    NoOp,
    Cancelled,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DockspaceCommitObservation {
    Committed,
    AlreadyCommitted,
    Superseded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DockspaceDragTerminal {
    Committed,
    Unchanged,
    Superseded,
    Cancelled,
    Recovered,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DockspaceProgressState {
    ForwardOnlyPrepared,
    RecoveryPending,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum DockspaceReceiptExpectation {
    None,
    New { alias: String },
    SameAs { alias: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DockspaceStepExpectation {
    pub(crate) prepare: DockspacePrepareDecision,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) commit: Option<DockspaceCommitObservation>,
    pub(crate) terminal: DockspaceDragTerminal,
    pub(crate) receipt: DockspaceReceiptExpectation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DockspaceFaultKind {
    Escape,
    CaptureLoss,
    SourceLoss,
    TargetLoss,
    DependencyChanged,
    ProviderFailure,
    DestinationLoss,
    ApplicationShutdown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DockspaceFaultPoint {
    BeforePrepare,
    AfterPrepareBeforeCommit,
    AfterProviderReceiptBeforeGraphCommit,
    AfterGraphCommitBeforeSubmittedSemantics,
    AfterSubmittedSemantics,
    DuringShutdownDrain,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DockspaceFaultInjection {
    pub(crate) kind: DockspaceFaultKind,
    pub(crate) point: DockspaceFaultPoint,
}
