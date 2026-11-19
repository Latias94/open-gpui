//! Small vector-only chrome primitives using the core's published rectangles.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use dockspace::{
    geometry::LogicalRect,
    model::{DockspaceAxis, TabKey},
    runtime::{
        DeferredSurfaceAction, DockspaceDropDirection, DockspaceReceiverDescriptor,
        DockspaceVisualId, DockspaceWorkspaceStamp, PreparedSurfaceAction, RetainedTabStrip,
    },
};
use open_gpui::{
    App, Bounds, Corners, Div, Element, ElementId, GlobalElementId, HitboxId, Hsla,
    InspectorElementId, LayoutId, PathBuilder, Pixels, Role, Stateful, Window, accesskit, fill,
    point, px, quad, size,
};

use crate::adapter::DockspaceColors;

#[derive(Clone, Copy)]
pub(super) enum Icon {
    Close,
    Back,
    Forward,
    List,
    Reveal,
    Menu,
    Layout,
    Grip,
}

#[derive(Clone, Copy)]
pub(super) enum ControlKind {
    Tab(TabKey),
    Button,
    Splitter(DockspaceAxis),
    Junction,
    MenuRow,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ControlAction {
    Activate,
    Equalize,
    Close,
    Previous,
    Next,
    First,
    Last,
    Decrement,
    Increment,
    Minimum,
    Maximum,
    Up,
    Down,
    Dismiss,
}

/// A frozen witness may authorize multiple independent framework events. Each
/// event submits its own deferred copy; core revalidates its original target.
#[derive(Clone)]
pub(super) struct Action {
    pub(super) intent: DeferredSurfaceAction,
    pub(super) tab_focus: Option<(DockspaceVisualId, TabKey)>,
    pub(super) yields_content: bool,
}

pub(super) type ActionSlot = Rc<RefCell<Option<Action>>>;

impl From<PreparedSurfaceAction> for Action {
    fn from(action: PreparedSurfaceAction) -> Self {
        action.into_deferred().into()
    }
}

impl From<DeferredSurfaceAction> for Action {
    fn from(action: DeferredSurfaceAction) -> Self {
        Self {
            tab_focus: action.tab_focus_visual().zip(action.tab_focus_target()),
            intent: action,
            yields_content: false,
        }
    }
}

pub(super) struct Control {
    pub(super) id: DockspaceVisualId,
    pub(super) bounds: LogicalRect,
    pub(super) kind: ControlKind,
    pub(super) label: String,
    pub(super) selected: Option<bool>,
    pub(super) expanded: Option<bool>,
    pub(super) value: Option<(f64, f64, f64)>,
    pub(super) menu_owner: Option<DockspaceVisualId>,
    pub(super) opens_menu: Option<DockspaceVisualId>,
    pub(super) focused: bool,
    pub(super) receiver: Option<DockspaceReceiverDescriptor>,
    pub(super) actions: Vec<(ControlAction, ActionSlot)>,
    pub(super) tab_strip: Option<Rc<RetainedTabStrip>>,
    pub(super) menu_keys: Option<Rc<super::keyboard::MenuKeys>>,
}

impl Control {
    pub(super) fn new(
        id: DockspaceVisualId,
        bounds: LogicalRect,
        kind: ControlKind,
        label: String,
        receiver: Option<DockspaceReceiverDescriptor>,
    ) -> Self {
        Self {
            id,
            bounds: receiver.map_or(bounds, |receiver| receiver.bounds()),
            kind,
            label,
            selected: None,
            expanded: None,
            value: None,
            menu_owner: None,
            opens_menu: None,
            focused: false,
            receiver,
            actions: Vec::new(),
            tab_strip: None,
            menu_keys: None,
        }
    }

    pub(super) fn action(&mut self, key: ControlAction, action: Option<PreparedSurfaceAction>) {
        if let Some(action) = action {
            let mut action = Action::from(action);
            action.yields_content = key == ControlAction::Activate
                && matches!(self.kind, ControlKind::Tab(_) | ControlKind::MenuRow);
            self.actions
                .push((key, Rc::new(RefCell::new(Some(action)))));
        }
    }

    pub(super) fn slot(&self, key: ControlAction) -> Option<ActionSlot> {
        self.actions.iter().find_map(|(candidate, action)| {
            (*candidate == key && action.borrow().is_some()).then(|| action.clone())
        })
    }

    pub(super) fn role(&self) -> Role {
        match self.kind {
            ControlKind::Tab(_) => Role::Tab,
            ControlKind::Button => Role::Button,
            ControlKind::Splitter(_) | ControlKind::Junction => Role::Splitter,
            ControlKind::MenuRow => Role::MenuItem,
        }
    }

    pub(super) fn key_action(&self, key: &str) -> Option<ControlAction> {
        use ControlAction::*;
        match (self.kind, key) {
            (_, "enter" | "space") => Some(Activate),
            (ControlKind::Button, "down" | "up") if self.opens_menu.is_some() => Some(Activate),
            (ControlKind::Tab(_), "left") | (ControlKind::MenuRow, "up") => Some(Previous),
            (ControlKind::Tab(_), "right") | (ControlKind::MenuRow, "down") => Some(Next),
            (ControlKind::Tab(_) | ControlKind::MenuRow, "home") => Some(First),
            (ControlKind::Tab(_) | ControlKind::MenuRow, "end") => Some(Last),
            (ControlKind::Tab(_), "delete" | "backspace") => Some(Close),
            (ControlKind::MenuRow, "escape") => Some(Dismiss),
            (ControlKind::Splitter(DockspaceAxis::Horizontal) | ControlKind::Junction, "left")
            | (ControlKind::Splitter(DockspaceAxis::Vertical), "up") => Some(Decrement),
            (ControlKind::Splitter(DockspaceAxis::Horizontal) | ControlKind::Junction, "right")
            | (ControlKind::Splitter(DockspaceAxis::Vertical), "down") => Some(Increment),
            (ControlKind::Junction, "up") => Some(Up),
            (ControlKind::Junction, "down") => Some(Down),
            (ControlKind::Splitter(_), "home") => Some(Minimum),
            (ControlKind::Splitter(_), "end") => Some(Maximum),
            _ => None,
        }
    }
}

pub(super) struct PresentationMenu {
    pub(super) id: DockspaceVisualId,
    pub(super) workspace: DockspaceWorkspaceStamp,
    pub(super) open: Option<super::presentation_menu::OpenMenu>,
    pub(super) scope: super::presentation_menu::Scope,
    pub(super) bounds: LogicalRect,
    pub(super) enabled: bool,
    pub(super) choices: Vec<(&'static str, Option<ActionSlot>)>,
}

/// Observes the control's actual hitbox for use as the original core receiver
/// authority, without inserting a second hitbox over the same rectangle.
pub(super) struct ControlElement {
    pub(super) inner: Stateful<Div>,
    pub(super) hitbox: Rc<Cell<Option<HitboxId>>>,
}

impl Element for ControlElement {
    type RequestLayoutState = <Stateful<Div> as Element>::RequestLayoutState;
    type PrepaintState = <Stateful<Div> as Element>::PrepaintState;

    fn id(&self) -> Option<ElementId> {
        Element::id(&self.inner)
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn a11y_role(&self) -> Option<Role> {
        self.inner.a11y_role()
    }
    fn write_a11y_info(&self, node: &mut accesskit::Node) {
        self.inner.write_a11y_info(node);
    }
    fn infer_a11y_scroll_into_view(&self) -> bool {
        self.inner.infer_a11y_scroll_into_view()
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.inner.request_layout(id, inspector, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let state = self
            .inner
            .prepaint(id, inspector, bounds, request, window, cx);
        self.hitbox.set(state.0.as_ref().map(|hitbox| hitbox.id));
        state
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request: &mut Self::RequestLayoutState,
        prepared: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.inner
            .paint(id, inspector, bounds, request, prepared, window, cx);
    }
}

pub(super) fn paint_icon(
    bounds: Bounds<Pixels>,
    icon: Icon,
    enabled: bool,
    colors: &DockspaceColors,
    window: &mut Window,
) {
    let center = bounds.center();
    let radius = px(4.0)
        .min(bounds.size.width / 4.0)
        .min(bounds.size.height / 4.0);
    let color = if enabled {
        colors.muted_text
    } else {
        colors.muted_text.opacity(0.4)
    };
    if matches!(icon, Icon::Menu) {
        for offset in [-1.0, 0.0, 1.0] {
            window.paint_quad(fill(
                Bounds::new(
                    center + point(radius * offset - px(1.0), -px(1.0)),
                    size(px(2.0), px(2.0)),
                ),
                color,
            ));
        }
        return;
    }
    if matches!(icon, Icon::Grip) {
        let dot = radius / 3.0;
        for x in [-1.0, 1.0] {
            for y in [-1.0, 0.0, 1.0] {
                window.paint_quad(fill(
                    Bounds::new(
                        center + point(radius * x / 2.0 - dot / 2.0, radius * y - dot / 2.0),
                        size(dot, dot),
                    ),
                    color,
                ));
            }
        }
        return;
    }
    let mut path = PathBuilder::stroke(px(1.5));
    let mut line = |from, to| {
        path.move_to(from);
        path.line_to(to);
    };
    match icon {
        Icon::Close => {
            line(
                center + point(-radius, -radius),
                center + point(radius, radius),
            );
            line(
                center + point(-radius, radius),
                center + point(radius, -radius),
            );
        }
        Icon::Back | Icon::Forward => {
            let direction = if matches!(icon, Icon::Back) {
                -1.0
            } else {
                1.0
            };
            line(
                center + point(-radius * direction / 2.0, -radius),
                center + point(radius * direction / 2.0, px(0.0)),
            );
            line(
                center + point(radius * direction / 2.0, px(0.0)),
                center + point(-radius * direction / 2.0, radius),
            );
        }
        Icon::List => {
            for offset in [-1.0, 0.0, 1.0] {
                line(
                    center + point(-radius, radius * offset),
                    center + point(radius, radius * offset),
                );
            }
        }
        Icon::Reveal => {
            line(
                center + point(-radius, radius / 2.0),
                center + point(px(0.0), -radius / 2.0),
            );
            line(
                center + point(px(0.0), -radius / 2.0),
                center + point(radius, radius / 2.0),
            );
        }
        Icon::Layout => {
            let left = center.x - radius;
            let right = center.x + radius;
            let top = center.y - radius;
            let bottom = center.y + radius;
            line(point(left, top), point(right, top));
            line(point(right, top), point(right, bottom));
            line(point(right, bottom), point(left, bottom));
            line(point(left, bottom), point(left, top));
            line(point(center.x, top), point(center.x, bottom));
        }
        Icon::Menu | Icon::Grip => {}
    }
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

#[derive(Clone, Copy)]
pub(super) struct GuideState {
    pub(super) direction: DockspaceDropDirection,
    pub(super) active: bool,
    pub(super) eligible: bool,
}

pub(super) fn paint_guide(
    bounds: Bounds<Pixels>,
    state: GuideState,
    colors: &DockspaceColors,
    window: &mut Window,
) {
    let background = if !state.eligible {
        colors.disabled_guide
    } else if state.active {
        colors.active_guide
    } else {
        colors.guide
    };
    let border = if state.active {
        colors.accent
    } else {
        colors.border
    };
    let cue = if !state.eligible {
        colors.muted_text.opacity(0.45)
    } else if state.active {
        colors.accent
    } else {
        colors.muted_text
    };
    window.paint_quad(quad(
        bounds,
        px(4.0),
        background,
        px(1.0),
        border,
        Default::default(),
    ));

    let inset = px(5.0)
        .min(bounds.size.width / 4.0)
        .min(bounds.size.height / 4.0);
    let pane = Bounds::new(
        bounds.origin + point(inset, inset),
        size(
            bounds.size.width - inset * 2.0,
            bounds.size.height - inset * 2.0,
        ),
    );
    window.paint_quad(quad(
        pane,
        px(1.0),
        cue.opacity(0.0),
        px(1.0),
        cue,
        Default::default(),
    ));
    let interior = Bounds::new(
        pane.origin + point(px(1.0), px(1.0)),
        size(
            (pane.size.width - px(2.0)).max(px(0.0)),
            (pane.size.height - px(2.0)).max(px(0.0)),
        ),
    );
    let edge = px(4.0)
        .min(interior.size.width / 2.0)
        .min(interior.size.height / 2.0);
    let emphasis = match state.direction {
        DockspaceDropDirection::Center => interior,
        DockspaceDropDirection::Left => {
            Bounds::new(interior.origin, size(edge, interior.size.height))
        }
        DockspaceDropDirection::Right => Bounds::new(
            point(
                interior.origin.x + interior.size.width - edge,
                interior.origin.y,
            ),
            size(edge, interior.size.height),
        ),
        DockspaceDropDirection::Top => {
            Bounds::new(interior.origin, size(interior.size.width, edge))
        }
        DockspaceDropDirection::Bottom => Bounds::new(
            point(
                interior.origin.x,
                interior.origin.y + interior.size.height - edge,
            ),
            size(interior.size.width, edge),
        ),
    };
    window.paint_quad(fill(
        emphasis,
        cue.opacity(if state.direction == DockspaceDropDirection::Center {
            0.35
        } else {
            0.85
        }),
    ));
}

pub(super) fn paint_contained_frame(
    bounds: Bounds<Pixels>,
    colors: &DockspaceColors,
    window: &mut Window,
) {
    window.paint_drop_shadows(bounds, Corners::all(px(6.0)), &colors.shadow);
    window.paint_quad(quad(
        bounds,
        px(6.0),
        colors.pane,
        px(1.0),
        colors.border,
        Default::default(),
    ));
}

pub(super) fn paint_title(bounds: Bounds<Pixels>, color: Hsla, window: &mut Window) {
    window.paint_quad(quad(
        bounds,
        Corners {
            top_left: px(5.0),
            top_right: px(5.0),
            ..Default::default()
        },
        color,
        px(0.0),
        color.opacity(0.0),
        Default::default(),
    ));
}

pub(super) fn paint_tab(
    bounds: Bounds<Pixels>,
    background: Hsla,
    selected: bool,
    colors: &DockspaceColors,
    window: &mut Window,
) {
    window.paint_quad(fill(bounds, background));
    if selected {
        window.paint_quad(fill(
            Bounds::new(bounds.origin, size(bounds.size.width, px(2.0))),
            colors.accent,
        ));
    } else {
        window.paint_quad(fill(
            Bounds::new(
                point(
                    bounds.origin.x + bounds.size.width - px(1.0),
                    bounds.origin.y + px(7.0),
                ),
                size(px(1.0), (bounds.size.height - px(14.0)).max(px(0.0))),
            ),
            colors.border,
        ));
    }
}
