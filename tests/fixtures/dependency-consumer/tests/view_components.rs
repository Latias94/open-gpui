use open_gpui::{
    AnyElement, AnyView, AnyWeakView, App, AppContext as _, Context, Element as _, ElementId,
    Empty, Entity, EntityId, FocusHandle, InteractiveElement as _, IntoElement, MouseButton,
    ParentElement as _, Render, RenderOnce, StyleRefinement, Styled as _, View, ViewElement,
    Window, WindowHandle, canvas, div, fill, px, rgb,
};

#[derive(IntoElement)]
struct Badge<T, const WIDTH: usize = 24>
where
    T: IntoElement + 'static,
{
    child: T,
}

impl<T, const WIDTH: usize> RenderOnce for Badge<T, WIDTH>
where
    T: IntoElement + 'static,
{
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        div().w(px(WIDTH as f32)).h(px(24.)).child(self.child)
    }
}

struct Document {
    value: usize,
}

impl Render for Document {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let value = self.value;
        canvas(
            |_, _, _| (),
            move |bounds, (), window, _| {
                window.paint_quad(fill(bounds, rgb(0x305080 + value as u32)));
            },
        )
        .size(px(24.))
    }
}

#[derive(IntoElement)]
enum PropsView<T, const STEP: usize = 1>
where
    T: IntoElement + 'static,
{
    Backed { state: Entity<Document>, child: T },
    Stateless(T),
}

impl<T, const STEP: usize> View for PropsView<T, STEP>
where
    T: IntoElement + 'static,
{
    fn entity_id(&self) -> Option<EntityId> {
        match self {
            Self::Backed { state, .. } => Some(state.entity_id()),
            Self::Stateless(_) => None,
        }
    }

    fn render(self, _: &mut Window, app: &mut App) -> impl IntoElement {
        match self {
            Self::Backed { state, child } => {
                let value = state.read(app).value;
                div()
                    .child(child)
                    .child(
                        canvas(
                            |_, _, _| (),
                            move |bounds, (), window, _| {
                                window.paint_quad(fill(bounds, rgb(0x305080 + value as u32)));
                            },
                        )
                        .size(px(24.)),
                    )
                    .on_mouse_down(MouseButton::Left, move |_, _, app| {
                        state.update(app, |document, cx| {
                            document.value += STEP;
                            cx.notify();
                        });
                    })
            }
            Self::Stateless(child) => div().child(child),
        }
    }
}

#[test]
fn downstream_derived_struct_and_manual_enum_preserve_generics_and_source_callers() {
    let badge = Badge::<_, 48> { child: Empty };
    assert_eq!(View::entity_id(&badge), None);
    let badge_line = line!() + 1;
    let badge: ViewElement<Badge<Empty, 48>> = badge.into_element();
    assert_eq!(badge.id(), None);

    let view = PropsView::<_, 3>::Stateless(Badge { child: Empty });
    assert_eq!(View::entity_id(&view), None);
    let view_line = line!() + 1;
    let view: ViewElement<PropsView<Badge<Empty>, 3>> = view.into_element();
    assert_eq!(view.id(), None);
    let direct: ViewElement<PropsView<Empty>> = ViewElement::new(PropsView::Stateless(Empty));
    assert_eq!(direct.id(), None);

    for (source, line) in [
        (badge.source_location(), badge_line),
        (view.source_location(), view_line),
    ] {
        #[cfg(debug_assertions)]
        {
            let source = source.unwrap();
            assert_eq!(source.file(), file!());
            assert_eq!(source.line(), line);
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = line;
            assert!(source.is_none());
        }
    }
}

#[test]
fn downstream_entity_props_cache_weak_and_interactive_composition_contracts_type_only() {
    let _composition_type_only: fn(
        &mut Window,
        &mut App,
        Entity<Document>,
        FocusHandle,
    ) -> AnyElement = |window, app, state, focus| {
        let props = PropsView::<_, 3>::Backed {
            state: state.clone(),
            child: Badge { child: Empty },
        };
        assert_eq!(View::entity_id(&props), Some(state.entity_id()));
        let props: ViewElement<PropsView<Badge<Empty>, 3>> = props.into_element();
        assert_eq!(props.id(), Some(ElementId::View(state.entity_id())));
        let _: ViewElement<Entity<Document>> = state.clone().into_element();
        let typed_cached: ViewElement<Entity<Document>> = state
            .clone()
            .cached(StyleRefinement::default().size(px(24.)));
        let erased = AnyView::from(state.clone());
        let _: ViewElement<AnyView> = erased.clone().into_element();
        let erased_cached: ViewElement<AnyView> = erased
            .clone()
            .cached(StyleRefinement::default().size(px(24.)));
        let weak: AnyWeakView = state.downgrade().into();
        let _: Option<Entity<Document>> = weak
            .upgrade()
            .and_then(|view| view.downcast::<Document>().ok());
        let _: AnyWeakView = erased.downgrade();
        let _: Entity<usize> = window.use_state(app, |_, _| 0);
        let rendered = View::render(
            PropsView::<_, 2>::Backed {
                state: state.clone(),
                child: Empty,
            },
            window,
            app,
        )
        .into_any_element();
        let keyboard_state = state.clone();
        div()
            .id("independent-view-composition")
            .track_focus(&focus)
            .child(div().id("props").child(props))
            .child(div().id("typed-cache").child(typed_cached))
            .child(div().id("erased-cache").child(erased_cached))
            .child(div().id("render-contract").child(rendered))
            .on_mouse_down(MouseButton::Left, move |_, window, app| {
                window.focus(&focus, app);
                state.update(app, |document, cx| {
                    document.value += 1;
                    cx.notify();
                });
            })
            .on_key_down(move |event, _, app| {
                if event.keystroke.key == "enter" {
                    keyboard_state.update(app, |document, cx| {
                        document.value += 2;
                        cx.notify();
                    });
                }
            })
            .into_any_element()
    };
    let _root_type_only: fn(&mut App) -> WindowHandle<Document> = |app| {
        app.open_window(Default::default(), |_, app| {
            app.new(|_| Document { value: 1 })
        })
        .unwrap()
    };
}
