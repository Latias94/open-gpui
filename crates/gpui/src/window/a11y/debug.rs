//! Developer tooling for inspecting the accessibility tree.
//!
//! [`A11yDebug`] retains the last [`TreeUpdate`] sent to the platform adapter so
//! it can be serialized on demand (see
//! [`crate::Window::debug_a11y_tree_json`]). In `cfg(debug_assertions)` builds,
//! we capture extra info.

use accesskit::{Action, NodeId, TreeUpdate};
use open_gpui_collections::FxHashMap;

use crate::{Pixels, SharedString, Size};

#[derive(Default)]
pub(crate) struct FrameDebugInfo {
    pub viewport_size: Size<Pixels>,
    pub scale_factor: f32,
    pub tab_stop_count: usize,
    pub frame_generation: u64,
    pub activation_generation: u64,
}

struct CapturedFrame {
    rendered_at: String,
    frame_number: u64,
    window_title: Option<SharedString>,
    node_count: usize,
    tab_stop_count: usize,
    viewport_size: Size<Pixels>,
    scale_factor: f32,
    frame_generation: u64,
    activation_generation: u64,
}

#[cfg(debug_assertions)]
#[derive(Clone, Default)]
pub(crate) struct NodeDebugInfo {
    /// Whether the node was synthesized via
    /// [`crate::Element::a11y_synthetic_children`] rather than created from a
    /// real element with a role and ID.
    pub synthetic: bool,
    /// The type name of the `Render` view that was rendering when the node was
    /// created.
    pub view: Option<&'static str>,
    /// The [`ElementId`](crate::ElementId) of the creating element (the leaf of
    /// its `GlobalElementId`, not the full path). For a synthetic node, this is
    /// the real element whose `a11y_synthetic_children` produced it.
    pub element_id: Option<String>,
    /// Source location where the creating element was constructed.
    pub source_location: Option<&'static core::panic::Location<'static>>,
}

#[cfg(debug_assertions)]
#[derive(Clone, Default)]
pub(crate) struct NodeCreator {
    pub view: Option<&'static str>,
    pub element_id: Option<String>,
    pub source_location: Option<&'static core::panic::Location<'static>>,
}

#[derive(Default)]
pub(crate) struct A11yDebug {
    last_tree_update: Option<TreeUpdate>,
    last_gpui_focus: Option<NodeId>,
    last_active_descendant: Option<NodeId>,
    /// Monotonic counter incremented on each captured frame, so a re-dump makes
    /// it obvious whether the tree actually refreshed.
    frame_number: u64,
    /// Metadata about the most recently captured frame.
    last_frame: Option<CapturedFrame>,
    #[cfg(debug_assertions)]
    last_node_info: FxHashMap<NodeId, NodeDebugInfo>,
}

impl A11yDebug {
    pub(crate) fn capture(
        &mut self,
        update: &TreeUpdate,
        gpui_focus: Option<NodeId>,
        active_descendant: Option<NodeId>,
        window_title: Option<&SharedString>,
        frame: FrameDebugInfo,
    ) {
        self.last_tree_update = Some(update.clone());
        self.last_gpui_focus = gpui_focus;
        self.last_active_descendant = active_descendant;
        self.frame_number += 1;
        self.last_frame = Some(CapturedFrame {
            rendered_at: chrono::Local::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, false),
            frame_number: self.frame_number,
            window_title: window_title.cloned(),
            node_count: update.nodes.len(),
            tab_stop_count: frame.tab_stop_count,
            viewport_size: frame.viewport_size,
            scale_factor: frame.scale_factor,
            frame_generation: frame.frame_generation,
            activation_generation: frame.activation_generation,
        });
    }

    #[cfg(debug_assertions)]
    pub(crate) fn capture_node_info(&mut self, node_info: &FxHashMap<NodeId, NodeDebugInfo>) {
        self.last_node_info = node_info.clone();
    }

    /// Release the captured tree, frame metadata, and provenance without resetting
    /// the monotonic capture counter.
    pub(crate) fn clear(&mut self) {
        self.last_tree_update = None;
        self.last_gpui_focus = None;
        self.last_active_descendant = None;
        self.last_frame = None;
        #[cfg(debug_assertions)]
        self.last_node_info.clear();
    }

    /// Serialize the last tree update to a readable JSON string. Node ids are
    /// replaced with short ephemeral ids (`a`, `b`, ..., `z`, `aa`, ...).
    pub(crate) fn to_json(&self) -> Option<String> {
        let update = self.last_tree_update.as_ref()?;

        let mut ephemeral: FxHashMap<NodeId, String> = FxHashMap::default();
        for (index, (id, _)) in update.nodes.iter().enumerate() {
            ephemeral.insert(*id, ephemeral_id(index));
        }

        let mut nodes = serde_json::Map::new();
        for (id, node) in &update.nodes {
            let key = ephemeral
                .get(id)
                .cloned()
                .unwrap_or_else(|| id.0.to_string());
            #[cfg(debug_assertions)]
            let provenance = self
                .last_node_info
                .get(id)
                .map(|info| NodeProvenance {
                    element_id: info.element_id.as_deref(),
                    view: info.view,
                    source_location: info.source_location.map(|loc| loc.to_string()),
                    // Only surface synthetic nodes; `false` is the default and
                    // would just be noise on every real node.
                    synthetic: info.synthetic.then_some(true),
                })
                .unwrap_or_default();
            #[cfg(not(debug_assertions))]
            let provenance = NodeProvenance::default();
            let value = node_to_json(*id, node, &ephemeral, &provenance);
            nodes.insert(key, value);
        }

        let frame = self.last_frame.as_ref().map(|frame| {
            serde_json::json!({
                "rendered_at": frame.rendered_at,
                "frame_number": frame.frame_number,
                "window_title": frame.window_title.as_ref().map(|title| title.to_string()),
                "node_count": frame.node_count,
                "tab_stop_count": frame.tab_stop_count,
                "viewport_size": {
                    "width": f32::from(frame.viewport_size.width),
                    "height": f32::from(frame.viewport_size.height),
                },
                "scale_factor": frame.scale_factor,
                "frame_generation": frame.frame_generation,
                "activation_generation": frame.activation_generation,
            })
        });

        let root = update
            .tree
            .as_ref()
            .map(|tree| tree.root)
            .and_then(|id| ephemeral.get(&id).cloned());

        let value = serde_json::json!({
            "root": root,
            "gpui_focus": self.last_gpui_focus.and_then(|id| ephemeral.get(&id).cloned()),
            "active_descendant_focus": self.last_active_descendant.and_then(|id| ephemeral.get(&id).cloned()),
            "frame": frame,
            "nodes": nodes,
        });
        Some(serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{}".to_string()))
    }
}

#[derive(Default)]
struct NodeProvenance<'a> {
    element_id: Option<&'a str>,
    view: Option<&'a str>,
    source_location: Option<String>,
    synthetic: Option<bool>,
}

fn node_to_json(
    id: NodeId,
    node: &accesskit::Node,
    ephemeral: &FxHashMap<NodeId, String>,
    provenance: &NodeProvenance,
) -> serde_json::Value {
    use serde_json::json;

    let mut map = serde_json::Map::new();
    map.insert("accesskit_id".into(), json!(id.0.to_string()));

    let children: Vec<String> = node
        .children()
        .iter()
        .map(|child| {
            ephemeral
                .get(child)
                .cloned()
                .unwrap_or_else(|| child.0.to_string())
        })
        .collect();
    if !children.is_empty() {
        map.insert("children".into(), json!(children));
    }

    // Provenance (debug builds only), ordered before the accessibility section.
    if let Some(element_id) = provenance.element_id {
        map.insert("element_id".into(), json!(element_id));
    }
    if let Some(view) = provenance.view {
        map.insert("view".into(), json!(view));
    }
    if let Some(source_location) = &provenance.source_location {
        map.insert("source_location".into(), json!(source_location));
    }
    if let Some(synthetic) = provenance.synthetic {
        map.insert("synthetic".into(), json!(synthetic));
    }

    // Accessibility semantics for this node, grouped together.
    let mut aria = serde_json::Map::new();
    aria.insert("role".into(), json!(format!("{:?}", node.role())));

    // Which action types the node supports. AccessKit keeps these in a private
    // bitset with no getter or iterator, so we probe each variant. `Action::n`
    // (from AccessKit's `enumn` feature) maps a discriminant to its variant,
    // returning `None` past the last one - so this can't drift out of sync with
    // AccessKit's `Action` enum the way a hand-maintained list would.
    let mut next_action = 0u8;
    let on_action: Vec<String> = std::iter::from_fn(move || {
        let action = Action::n(next_action)?;
        next_action += 1;
        Some(action)
    })
    .filter(|action| node.supports_action(*action))
    .map(|action| format!("{action:?}"))
    .collect();
    if !on_action.is_empty() {
        aria.insert("on_action".into(), json!(on_action));
    }

    // String properties.
    if let Some(v) = node.label() {
        aria.insert("label".into(), json!(v));
    }
    if let Some(v) = node.description() {
        aria.insert("description".into(), json!(v));
    }
    if let Some(v) = node.value() {
        aria.insert("value".into(), json!(v));
    }
    if let Some(v) = node.keyboard_shortcut() {
        aria.insert("keyboard_shortcut".into(), json!(v));
    }
    if let Some(v) = node.access_key() {
        aria.insert("access_key".into(), json!(v));
    }
    if let Some(v) = node.placeholder() {
        aria.insert("placeholder".into(), json!(v));
    }
    if let Some(v) = node.tooltip() {
        aria.insert("tooltip".into(), json!(v));
    }
    if let Some(v) = node.role_description() {
        aria.insert("role_description".into(), json!(v));
    }

    // Boolean / enum states.
    if let Some(v) = node.is_selected() {
        aria.insert("selected".into(), json!(v));
    }
    if let Some(v) = node.is_expanded() {
        aria.insert("expanded".into(), json!(v));
    }
    if let Some(v) = node.toggled() {
        aria.insert("toggled".into(), json!(format!("{v:?}")));
    }
    if let Some(v) = node.orientation() {
        aria.insert("orientation".into(), json!(format!("{v:?}")));
    }

    // Numeric properties.
    if let Some(v) = node.numeric_value() {
        aria.insert("numeric_value".into(), json!(v));
    }
    if let Some(v) = node.min_numeric_value() {
        aria.insert("min_numeric_value".into(), json!(v));
    }
    if let Some(v) = node.max_numeric_value() {
        aria.insert("max_numeric_value".into(), json!(v));
    }
    if let Some(v) = node.numeric_value_step() {
        aria.insert("numeric_value_step".into(), json!(v));
    }

    // Set / table properties.
    if let Some(v) = node.level() {
        aria.insert("level".into(), json!(v));
    }
    if let Some(v) = node.position_in_set() {
        aria.insert("position_in_set".into(), json!(v));
    }
    if let Some(v) = node.size_of_set() {
        aria.insert("size_of_set".into(), json!(v));
    }
    if let Some(v) = node.row_index() {
        aria.insert("row_index".into(), json!(v));
    }
    if let Some(v) = node.column_index() {
        aria.insert("column_index".into(), json!(v));
    }
    if let Some(v) = node.row_count() {
        aria.insert("row_count".into(), json!(v));
    }
    if let Some(v) = node.column_count() {
        aria.insert("column_count".into(), json!(v));
    }

    map.insert("aria".into(), serde_json::Value::Object(aria));

    serde_json::Value::Object(map)
}

/// Maps a 0-based index to a short id in the sequence `a, b, ..., z, aa, ab,
/// ...` (bijective base-26).
fn ephemeral_id(mut index: usize) -> String {
    let mut bytes = Vec::new();
    loop {
        bytes.push(b'a' + (index % 26) as u8);
        if index < 26 {
            break;
        }
        index = index / 26 - 1;
    }
    bytes.reverse();
    String::from_utf8(bytes).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use accesskit::{Node, Orientation, Role, Toggled, Tree, TreeId};
    use serde_json::{Value, json};

    fn tree(nodes: Vec<(NodeId, Node)>, root: NodeId, focus: NodeId) -> TreeUpdate {
        TreeUpdate {
            nodes,
            tree: Some(Tree::new(root)),
            tree_id: TreeId::ROOT,
            focus,
        }
    }

    fn snapshot(debug: &A11yDebug) -> Value {
        serde_json::from_str(&debug.to_json().expect("captured tree")).expect("valid tree JSON")
    }

    #[test]
    fn serializes_complete_official_semantics_and_raw_text() {
        let id = NodeId(u64::MAX);
        let label = "  Label \"raw\"\n日本語 🦀  ";
        let value = "  Ordinary value\nwith \"quotes\" and \\slashes  ";
        let title: SharedString = "  Window \"title\"\n日本語  ".into();
        let mut node = Node::new(Role::TextInput);
        node.set_label(label);
        node.set_description("Complete description");
        node.set_value(value);
        node.set_keyboard_shortcut("Control+Enter");
        node.set_access_key("E");
        node.set_placeholder("Enter text");
        node.set_tooltip("Tooltip text");
        node.set_role_description("Custom editor");
        node.set_selected(false);
        node.set_expanded(true);
        node.set_toggled(Toggled::Mixed);
        node.set_orientation(Orientation::Horizontal);
        node.set_numeric_value(3.5);
        node.set_min_numeric_value(0.0);
        node.set_max_numeric_value(10.0);
        node.set_numeric_value_step(0.5);
        node.set_level(2);
        node.set_position_in_set(3);
        node.set_size_of_set(5);
        node.set_row_index(0);
        node.set_column_index(1);
        node.set_row_count(8);
        node.set_column_count(9);
        for action in [
            Action::ShowContextMenu,
            Action::SetValue,
            Action::Focus,
            Action::Click,
        ] {
            node.add_action(action);
        }
        let mut update = tree(vec![(id, node)], id, id);
        let mut debug = A11yDebug::default();
        debug.capture(
            &update,
            Some(id),
            None,
            Some(&title),
            FrameDebugInfo::default(),
        );
        update.nodes[0].1.set_label("Changed after capture");
        update.nodes[0].1.set_value("Changed after capture");

        let dump = snapshot(&debug);
        assert_eq!(dump["frame"]["window_title"], title.as_ref());
        assert_eq!(dump["nodes"]["a"]["accesskit_id"], u64::MAX.to_string());
        assert_eq!(
            dump["nodes"]["a"]["aria"],
            json!({
                "role": "TextInput",
                "on_action": ["Click", "Focus", "SetValue", "ShowContextMenu"],
                "label": label,
                "description": "Complete description",
                "value": value,
                "keyboard_shortcut": "Control+Enter",
                "access_key": "E",
                "placeholder": "Enter text",
                "tooltip": "Tooltip text",
                "role_description": "Custom editor",
                "selected": false,
                "expanded": true,
                "toggled": "Mixed",
                "orientation": "Horizontal",
                "numeric_value": 3.5,
                "min_numeric_value": 0.0,
                "max_numeric_value": 10.0,
                "numeric_value_step": 0.5,
                "level": 2,
                "position_in_set": 3,
                "size_of_set": 5,
                "row_index": 0,
                "column_index": 1,
                "row_count": 8,
                "column_count": 9,
            })
        );
    }

    #[test]
    fn omits_absent_semantics_and_unresolved_focus() {
        let id = NodeId(42);
        let mut update = tree(vec![(id, Node::new(Role::Window))], id, id);
        let mut debug = A11yDebug::default();
        debug.capture(
            &update,
            Some(NodeId(99)),
            Some(NodeId(100)),
            None,
            FrameDebugInfo::default(),
        );

        let dump = snapshot(&debug);
        assert_eq!(dump["root"], "a");
        assert_eq!(dump["gpui_focus"], Value::Null);
        assert_eq!(dump["active_descendant_focus"], Value::Null);
        assert_eq!(dump["frame"]["window_title"], Value::Null);
        assert_eq!(
            dump["nodes"],
            json!({ "a": { "accesskit_id": "42", "aria": { "role": "Window" } } })
        );

        update.tree = None;
        debug.capture(&update, None, None, None, FrameDebugInfo::default());
        assert_eq!(snapshot(&debug)["root"], Value::Null);
    }

    #[test]
    fn remaps_tree_references_to_ephemeral_ids() {
        let ids: Vec<_> = (0..28).map(|index| NodeId(u64::MAX - index)).collect();
        let mut nodes: Vec<_> = ids
            .iter()
            .map(|id| (*id, Node::new(Role::Button)))
            .collect();
        nodes[26].1.set_role(Role::Window);
        nodes[26].1.set_children(
            ids.iter()
                .copied()
                .filter(|id| *id != ids[26])
                .collect::<Vec<_>>(),
        );
        let update = tree(nodes, ids[26], ids[27]);
        let mut debug = A11yDebug::default();
        debug.capture(
            &update,
            Some(ids[25]),
            Some(ids[27]),
            None,
            FrameDebugInfo::default(),
        );

        let dump = snapshot(&debug);
        assert_eq!(dump["root"], "aa");
        assert_eq!(dump["gpui_focus"], "z");
        assert_eq!(dump["active_descendant_focus"], "ab");
        assert_eq!(dump["nodes"].as_object().expect("node map").len(), 28);
        assert_eq!(dump["nodes"]["a"]["accesskit_id"], ids[0].0.to_string());
        assert_eq!(dump["nodes"]["aa"]["accesskit_id"], ids[26].0.to_string());
        assert_eq!(dump["nodes"]["ab"]["accesskit_id"], ids[27].0.to_string());
        assert_eq!(
            dump["nodes"]["aa"]["children"],
            json!([
                "a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p",
                "q", "r", "s", "t", "u", "v", "w", "x", "y", "z", "ab",
            ])
        );
    }

    #[test]
    fn preserves_frame_metadata_and_counts_captures() {
        let id = NodeId(7);
        let update = tree(vec![(id, Node::new(Role::Window))], id, id);
        let mut debug = A11yDebug::default();
        debug.capture(
            &update,
            None,
            None,
            None,
            FrameDebugInfo {
                viewport_size: Size {
                    width: crate::px(800.5),
                    height: crate::px(600.25),
                },
                scale_factor: 1.5,
                tab_stop_count: 3,
                frame_generation: 40,
                activation_generation: 5,
            },
        );

        let first = snapshot(&debug);
        let rendered_at = first["frame"]["rendered_at"]
            .as_str()
            .expect("capture timestamp");
        assert!(chrono::DateTime::parse_from_rfc3339(rendered_at).is_ok());
        assert_eq!(
            first["frame"],
            json!({
                "rendered_at": rendered_at,
                "frame_number": 1,
                "window_title": null,
                "node_count": 1,
                "tab_stop_count": 3,
                "viewport_size": { "width": 800.5, "height": 600.25 },
                "scale_factor": 1.5,
                "frame_generation": 40,
                "activation_generation": 5,
            })
        );
        assert_eq!(snapshot(&debug), first);

        debug.capture(
            &update,
            None,
            None,
            None,
            FrameDebugInfo {
                frame_generation: 57,
                activation_generation: 6,
                ..FrameDebugInfo::default()
            },
        );
        let second = snapshot(&debug);
        assert_eq!(second["frame"]["frame_number"], 2);
        assert_eq!(second["frame"]["frame_generation"], 57);
        assert_eq!(second["frame"]["activation_generation"], 6);
    }

    #[test]
    fn clear_releases_snapshot_without_resetting_capture_counter() {
        let id = NodeId(42);
        let update = tree(vec![(id, Node::new(Role::Window))], id, id);
        let title: SharedString = "Captured title".into();
        let mut debug = A11yDebug::default();
        assert!(debug.to_json().is_none());
        debug.clear();
        assert!(debug.to_json().is_none());

        #[cfg(debug_assertions)]
        debug.capture_node_info(&FxHashMap::from_iter([(
            id,
            NodeDebugInfo {
                element_id: Some("Captured element".into()),
                ..NodeDebugInfo::default()
            },
        )]));
        debug.capture(
            &update,
            Some(id),
            Some(id),
            Some(&title),
            FrameDebugInfo::default(),
        );
        assert_eq!(snapshot(&debug)["frame"]["frame_number"], 1);

        debug.clear();
        debug.clear();
        assert!(debug.to_json().is_none());
        assert!(debug.last_tree_update.is_none());
        assert!(debug.last_gpui_focus.is_none());
        assert!(debug.last_active_descendant.is_none());
        assert!(debug.last_frame.is_none());
        #[cfg(debug_assertions)]
        assert!(debug.last_node_info.is_empty());

        debug.capture(&update, None, None, None, FrameDebugInfo::default());
        let dump = snapshot(&debug);
        assert_eq!(dump["frame"]["frame_number"], 2);
        assert_eq!(dump["frame"]["window_title"], Value::Null);
        assert!(dump["nodes"]["a"].get("element_id").is_none());
    }

    #[cfg(debug_assertions)]
    #[test]
    fn debug_builds_serialize_captured_provenance() {
        let root = NodeId(10);
        let child = NodeId(20);
        let mut root_node = Node::new(Role::Window);
        root_node.set_children([child]);
        let update = tree(
            vec![(root, root_node), (child, Node::new(Role::Button))],
            root,
            child,
        );
        let creator = NodeCreator {
            view: Some("example::Editor"),
            element_id: Some("editor-root".into()),
            source_location: Some(core::panic::Location::caller()),
        };
        let info = NodeDebugInfo {
            synthetic: false,
            view: creator.view,
            element_id: creator.element_id.clone(),
            source_location: creator.source_location,
        };
        let mut node_info = FxHashMap::from_iter([
            (root, info.clone()),
            (
                child,
                NodeDebugInfo {
                    synthetic: true,
                    ..info
                },
            ),
        ]);
        let mut debug = A11yDebug::default();
        debug.capture_node_info(&node_info);
        node_info.clear();
        debug.capture(&update, None, None, None, FrameDebugInfo::default());

        let dump = snapshot(&debug);
        for key in ["a", "b"] {
            assert_eq!(dump["nodes"][key]["element_id"], "editor-root");
            assert_eq!(dump["nodes"][key]["view"], "example::Editor");
            assert_eq!(
                dump["nodes"][key]["source_location"],
                creator
                    .source_location
                    .expect("creator location")
                    .to_string()
            );
        }
        assert!(dump["nodes"]["a"].get("synthetic").is_none());
        assert_eq!(dump["nodes"]["b"]["synthetic"], true);
    }

    #[cfg(not(debug_assertions))]
    #[test]
    fn release_builds_export_semantics_without_provenance() {
        let id = NodeId(42);
        let mut node = Node::new(Role::TextInput);
        node.set_label("Release label");
        node.set_value("Release value");
        let update = tree(vec![(id, node)], id, id);
        let mut debug = A11yDebug::default();
        debug.capture(&update, None, None, None, FrameDebugInfo::default());

        assert_eq!(
            snapshot(&debug)["nodes"]["a"],
            json!({
                "accesskit_id": "42",
                "aria": { "role": "TextInput", "label": "Release label", "value": "Release value" },
            })
        );
    }
}
