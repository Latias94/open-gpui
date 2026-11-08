//! Stateful rendering recipes for the independent native hot-reload consumer.

use open_gpui::{
    App, Bounds, Context, EntityId, IntoElement, ParentElement as _, Pixels, Render, RenderOnce,
    Styled as _, WeakEntity, Window, canvas, div, fill, px, rgb,
};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

pub const ROW_HEIGHT: f32 = 80.0;
pub const INITIAL_COUNTER: u32 = 37;

#[derive(Clone, Copy, Debug)]
pub enum Channel {
    TypedCached,
    StoredAnyView,
}

#[derive(Clone, Copy, Debug)]
pub struct Recipe {
    pub label: &'static str,
    pub color: u32,
    pub output: u32,
}

#[derive(Clone, Copy, Debug)]
pub struct PaintReceipt {
    pub recipe: Recipe,
    pub entity_id: EntityId,
    pub counter: u32,
    pub bounds: Bounds<Pixels>,
    pub previous_generation: u64,
}

#[derive(Default)]
pub struct PanelProbe {
    paint: RefCell<Option<PaintReceipt>>,
    initializations: Cell<u32>,
    releases: Cell<u32>,
    component_state: RefCell<Option<WeakEntity<ComponentCounter>>>,
}

impl PanelProbe {
    pub fn last_paint(&self) -> Option<PaintReceipt> {
        *self.paint.borrow()
    }

    pub fn initializations(&self) -> u32 {
        self.initializations.get()
    }

    pub fn releases(&self) -> u32 {
        self.releases.get()
    }

    pub fn is_released(&self) -> bool {
        self.initializations.get() > 0 && self.releases.get() == self.initializations.get()
    }

    pub fn set_component_counter(&self, value: u32, cx: &mut App) -> bool {
        let Some(state) = self
            .component_state
            .borrow()
            .as_ref()
            .and_then(WeakEntity::upgrade)
        else {
            return false;
        };
        state.update(cx, |state, cx| {
            state.value = value;
            cx.notify();
        });
        true
    }
}

pub struct StatefulPanel {
    channel: Channel,
    counter: u32,
    probe: Rc<PanelProbe>,
}

impl StatefulPanel {
    pub fn new(channel: Channel, probe: Rc<PanelProbe>, cx: &mut Context<Self>) -> Self {
        probe.initializations.set(probe.initializations.get() + 1);
        let released = probe.clone();
        cx.on_release(move |_, _| released.releases.set(released.releases.get() + 1))
            .detach();
        Self {
            channel,
            counter: INITIAL_COUNTER,
            probe,
        }
    }

    pub fn counter(&self) -> u32 {
        self.counter
    }

    pub fn set_counter(&mut self, value: u32, cx: &mut Context<Self>) {
        self.counter = value;
        cx.notify();
    }
}

impl Render for StatefulPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let recipe = match self.channel {
            Channel::TypedCached => Recipe {
                label: "typed-READY",
                color: 0x334455,
                output: 101,
            },
            Channel::StoredAnyView => Recipe {
                label: "erased-READY",
                color: 0x556677,
                output: 202,
            },
        };
        row(recipe, cx.entity_id(), self.counter, self.probe.clone())
    }
}

#[derive(IntoElement)]
pub struct PanelComponent {
    probe: Rc<PanelProbe>,
}

impl PanelComponent {
    pub fn new(probe: Rc<PanelProbe>) -> Self {
        Self { probe }
    }
}

struct ComponentCounter {
    value: u32,
}

impl RenderOnce for PanelComponent {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let probe = self.probe;
        let state = window.use_keyed_state("hot-reload-component-counter", cx, |_, cx| {
            probe.initializations.set(probe.initializations.get() + 1);
            *probe.component_state.borrow_mut() = Some(cx.weak_entity());
            let released = probe.clone();
            cx.on_release(move |_, _| released.releases.set(released.releases.get() + 1))
                .detach();
            ComponentCounter {
                value: INITIAL_COUNTER,
            }
        });
        let recipe = Recipe {
            label: "component-READY",
            color: 0x778899,
            output: 303,
        };
        row(recipe, state.entity_id(), state.read(cx).value, probe)
    }
}

fn row(
    recipe: Recipe,
    entity_id: EntityId,
    counter: u32,
    probe: Rc<PanelProbe>,
) -> impl IntoElement {
    div()
        .relative()
        .w_full()
        .h(px(ROW_HEIGHT))
        .flex_shrink_0()
        .child(
            canvas(
                |_, _, _| (),
                move |bounds, (), window, _| {
                    window.paint_quad(fill(bounds, rgb(recipe.color)));
                    *probe.paint.borrow_mut() = Some(PaintReceipt {
                        recipe,
                        entity_id,
                        counter,
                        bounds,
                        previous_generation: window.rendered_frame_revision(),
                    });
                },
            )
            .size_full(),
        )
        .child(
            div()
                .absolute()
                .top(px(8.0))
                .left(px(12.0))
                .text_size(px(14.0))
                .text_color(rgb(0xffffff))
                .child(format!(
                    "{} counter={} output={}",
                    recipe.label, counter, recipe.output
                )),
        )
}
