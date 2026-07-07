use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSceneFrame {
    pub visible_document_bounds: Bounds<Pixels>,
    record_groups: Vec<CanvasSceneRecordGroup>,
    edge_items: Vec<CanvasSceneLayerItem>,
    tool_chrome: CanvasPaintInteractionFrame,
}

impl CanvasSceneFrame {
    pub fn from_paint_frame(frame: &CanvasPaintFrame) -> Self {
        let mut record_groups = Vec::new();
        let mut edge_items = Vec::new();

        for (ordinal, record) in frame.records.iter().enumerate() {
            match record.target {
                HitTarget::Edge(_) => edge_items.push(CanvasSceneLayerItem::from_record(
                    record,
                    ordinal,
                    CanvasSceneLayerPhase::EdgeBehindNodes,
                )),
                _ => record_groups.push(CanvasSceneRecordGroup::from_record(record, ordinal)),
            }
        }

        record_groups.sort_by_key(scene_record_group_sort_key);
        edge_items.sort_by(|left, right| {
            left.z_index
                .cmp(&right.z_index)
                .then_with(|| left.ordinal.cmp(&right.ordinal))
        });

        Self {
            visible_document_bounds: frame.visible_document_bounds,
            record_groups,
            edge_items,
            tool_chrome: frame.interaction.clone(),
        }
    }

    pub fn record_groups(&self) -> &[CanvasSceneRecordGroup] {
        &self.record_groups
    }

    pub fn edge_items(&self) -> &[CanvasSceneLayerItem] {
        &self.edge_items
    }

    pub fn tool_chrome(&self) -> &CanvasPaintInteractionFrame {
        &self.tool_chrome
    }

    pub fn ordered_layer_items(&self) -> Vec<CanvasSceneLayerItem> {
        let mut items = Vec::with_capacity(
            self.edge_items.len()
                + self
                    .record_groups
                    .iter()
                    .map(|group| group.phases.len())
                    .sum::<usize>(),
        );
        items.extend(self.edge_items.iter().cloned());
        for group in &self.record_groups {
            items.extend(group.layer_items());
        }
        items
    }
}

fn scene_record_group_sort_key(group: &CanvasSceneRecordGroup) -> (u8, i32, usize) {
    (
        u8::from(group.selected || group.structurally_selected),
        group.z_index,
        group.ordinal,
    )
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSceneRecordGroup {
    pub target: HitTarget,
    pub document_bounds: Bounds<Pixels>,
    pub view_bounds: Bounds<Pixels>,
    pub z_index: i32,
    pub ordinal: usize,
    pub record_index: usize,
    pub hidden: bool,
    pub locked: bool,
    pub hovered: bool,
    pub selected: bool,
    pub structurally_selected: bool,
    phases: Vec<CanvasSceneLayerPhase>,
}

impl CanvasSceneRecordGroup {
    fn from_record(record: &CanvasPaintRecord, ordinal: usize) -> Self {
        Self {
            target: record.target.clone(),
            document_bounds: record.document_bounds,
            view_bounds: record.view_bounds,
            z_index: record.z_index,
            ordinal,
            record_index: ordinal,
            hidden: record.hidden,
            locked: record.locked,
            hovered: record.hovered,
            selected: record.selected,
            structurally_selected: record.structurally_selected,
            phases: scene_record_phases(&record.target).to_vec(),
        }
    }

    pub fn phases(&self) -> &[CanvasSceneLayerPhase] {
        &self.phases
    }

    pub fn has_phase(&self, phase: CanvasSceneLayerPhase) -> bool {
        self.phases.contains(&phase)
    }

    pub fn layer_items(&self) -> impl Iterator<Item = CanvasSceneLayerItem> + '_ {
        self.phases
            .iter()
            .copied()
            .map(|phase| CanvasSceneLayerItem {
                phase,
                target: self.target.clone(),
                document_bounds: self.document_bounds,
                view_bounds: self.view_bounds,
                z_index: self.z_index,
                ordinal: self.ordinal,
                record_index: self.record_index,
            })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSceneLayerItem {
    pub phase: CanvasSceneLayerPhase,
    pub target: HitTarget,
    pub document_bounds: Bounds<Pixels>,
    pub view_bounds: Bounds<Pixels>,
    pub z_index: i32,
    pub ordinal: usize,
    pub record_index: usize,
}

impl CanvasSceneLayerItem {
    fn from_record(
        record: &CanvasPaintRecord,
        ordinal: usize,
        phase: CanvasSceneLayerPhase,
    ) -> Self {
        Self {
            phase,
            target: record.target.clone(),
            document_bounds: record.document_bounds,
            view_bounds: record.view_bounds,
            z_index: record.z_index,
            ordinal,
            record_index: ordinal,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum CanvasSceneLayerPhase {
    DocumentUnderlay,
    EdgeBehindNodes,
    RecordBody,
    RecordWidget,
    RecordChrome,
    EdgeAboveNodes,
    ToolChrome,
    HostPortal,
}

fn scene_record_phases(target: &HitTarget) -> &'static [CanvasSceneLayerPhase] {
    match target {
        HitTarget::Node(_) => &[
            CanvasSceneLayerPhase::RecordBody,
            CanvasSceneLayerPhase::RecordWidget,
            CanvasSceneLayerPhase::RecordChrome,
        ],
        HitTarget::Shape(_) => &[
            CanvasSceneLayerPhase::RecordBody,
            CanvasSceneLayerPhase::RecordChrome,
        ],
        HitTarget::Handle { .. } => &[CanvasSceneLayerPhase::RecordChrome],
        HitTarget::Edge(_) => &[CanvasSceneLayerPhase::EdgeBehindNodes],
    }
}
