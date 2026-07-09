//! DevTools adapters for `open-gpui-motion` frame-demand facts.

use open_gpui_motion::{
    MotionFrameDemand, MotionFrameDriver, MotionFrameHostResetReason, MotionFrameReason,
};

use crate::{
    SnapshotProbeSnapshot, SnapshotRedactionSummary, SnapshotTree,
    adapters::snapshot_node_with_payload,
    timeline::{TimelineEventSnapshot, TimelineSnapshot},
};

/// Converts a single frame-demand decision into a DevTools tree.
pub fn motion_frame_demand_probe_snapshot(demand: MotionFrameDemand) -> SnapshotProbeSnapshot {
    SnapshotProbeSnapshot::new(SnapshotTree::new([motion_demand_node(
        ["motion", "frame-demand"],
        "Frame demand",
        demand,
    )]))
    .with_redaction(SnapshotRedactionSummary::default())
}

/// Converts a public motion frame driver into a DevTools tree.
pub fn motion_frame_driver_probe_snapshot(driver: &MotionFrameDriver) -> SnapshotProbeSnapshot {
    let mut root = snapshot_node_with_payload(
        ["motion", "frame-driver"],
        "Frame driver",
        serde_json::json!({
            "last_elapsed_ms": driver.last_elapsed().as_millis(),
            "requested_frames": driver.requested_frames(),
            "last_reset_reason": driver
                .last_reset_reason()
                .map(motion_frame_host_reset_reason_label),
        }),
    );
    root = root.with_child(motion_demand_node(
        ["motion", "frame-driver", "last-demand"],
        "Last frame demand",
        driver.last_frame_demand(),
    ));

    SnapshotProbeSnapshot::new(SnapshotTree::new([root]))
        .with_redaction(SnapshotRedactionSummary::default())
}

/// Converts a single frame-demand decision into a DevTools timeline tree.
pub fn motion_frame_demand_timeline_snapshot(demand: MotionFrameDemand) -> TimelineSnapshot {
    TimelineSnapshot::new(
        "motion-frame-demand",
        "Motion frame demand",
        [
            TimelineEventSnapshot::new("frame-demand", "Frame demand", "motion", 0).with_payload(
                serde_json::json!({
                    "needs_frame": demand.needs_frame(),
                    "reason": demand.reason().map(motion_frame_reason_label),
                }),
            ),
        ],
    )
}

/// Converts a single frame-demand decision into a DevTools timeline probe snapshot.
pub fn motion_frame_demand_timeline_probe_snapshot(
    demand: MotionFrameDemand,
) -> SnapshotProbeSnapshot {
    motion_frame_demand_timeline_snapshot(demand).probe_snapshot()
}

fn motion_demand_node<I, S>(
    id_parts: I,
    label: impl AsRef<str>,
    demand: MotionFrameDemand,
) -> crate::SnapshotNode
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    snapshot_node_with_payload(
        id_parts,
        label,
        serde_json::json!({
            "needs_frame": demand.needs_frame(),
            "reason": demand.reason().map(motion_frame_reason_label),
        }),
    )
}

fn motion_frame_reason_label(reason: MotionFrameReason) -> &'static str {
    match reason {
        MotionFrameReason::UpdateRender => "update-render",
        _ => "unknown",
    }
}

fn motion_frame_host_reset_reason_label(reason: MotionFrameHostResetReason) -> &'static str {
    match reason {
        MotionFrameHostResetReason::Retarget => "retarget",
        MotionFrameHostResetReason::Cancel => "cancel",
        MotionFrameHostResetReason::Finish => "finish",
        MotionFrameHostResetReason::PruneTerminal => "prune-terminal",
        MotionFrameHostResetReason::MotionIdentityChanged => "motion-identity-changed",
    }
}
