//! Interactive subtree presentation page metadata.

/// Page title.
pub const TITLE: &str = "Presentation";
/// Page summary.
pub const SUMMARY: &str =
    "Committed layout and displayed geometry across one interactive transformed subtree.";
/// Foundation signals rendered by this page.
pub const SIGNALS: &[&str] = &[
    "SubtreeTransform",
    "ElementGeometry",
    "MotionProjectionTransformSample",
    "TargetedEvent",
    "DragStartGeometry",
    "IME and accessibility projection",
];
