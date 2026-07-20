//! Interactive subtree presentation page metadata.

/// Page title.
pub const TITLE: &str = "Presentation";
/// Page summary.
pub const SUMMARY: &str =
    "Committed geometry and visible/inert/hidden authority across one transformed subtree.";
/// Foundation signals rendered by this page.
pub const SIGNALS: &[&str] = &[
    "SubtreeTransform",
    "SubtreePresentation",
    "ElementGeometry",
    "MotionProjectionTransformSample",
    "TargetedEvent",
    "DragStartGeometry",
    "IME and accessibility projection",
];
