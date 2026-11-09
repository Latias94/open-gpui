use super::*;

struct MutableSpringView {
    config: SpringConfig,
    target: Pixels,
    initial: Option<Pixels>,
    playback: SpringPlayback,
    ancestor_id: &'static str,
    spring_id: &'static str,
    mounted: bool,
    samples: Rc<RefCell<Vec<Pixels>>>,
}

impl Render for MutableSpringView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().id(self.ancestor_id).when(self.mounted, |element| {
            let mut animation = SpringAnimation::new(self.config)
                .to(self.target)
                .with_epsilon(0.001)
                .playback(self.playback);
            if let Some(initial) = self.initial {
                animation = animation.from(initial);
            }
            let samples = self.samples.clone();
            element.child(
                div().with_spring(self.spring_id, animation, move |element, value| {
                    samples.borrow_mut().push(value);
                    element.left(value)
                }),
            )
        })
    }
}

fn open_mutable_spring(
    cx: &mut TestAppContext,
) -> (WindowHandle<MutableSpringView>, Rc<RefCell<Vec<Pixels>>>) {
    let samples = Rc::new(RefCell::new(Vec::new()));
    let window = cx.open_window(size(px(100.0), px(100.0)), {
        let samples = samples.clone();
        move |_, _| MutableSpringView {
            config: SpringConfig::new(100.0, 2.0, 1.0),
            target: px(100.0),
            initial: Some(px(20.0)),
            playback: SpringPlayback::Running,
            ancestor_id: "ancestor",
            spring_id: "spring",
            mounted: true,
            samples,
        }
    });
    cx.run_until_parked();
    (window, samples)
}

struct SpringSnapshot {
    spring: SpringState,
    target: f32,
    config: SpringConfig,
    initial: f32,
    playback: SpringPlayback,
    updated_at: Instant,
}

fn spring_snapshots<V: Render>(
    window: WindowHandle<V>,
    cx: &mut TestAppContext,
) -> Vec<SpringSnapshot> {
    cx.update_window(window.into(), |_, window, _| {
        window
            .rendered_frame
            .element_states
            .values()
            .filter_map(|state| {
                state
                    .inner
                    .downcast_ref::<Option<Rc<SpringStateOwner>>>()
                    .and_then(Option::as_ref)
                    .and_then(|owner| owner.committed.get())
                    .map(|state| SpringSnapshot {
                        spring: state.spring,
                        target: state.target,
                        config: state.config,
                        initial: state.initial,
                        playback: state.playback,
                        updated_at: state.updated_at,
                    })
            })
            .collect()
    })
    .unwrap()
}

fn spring_snapshot<V: Render>(window: WindowHandle<V>, cx: &mut TestAppContext) -> SpringSnapshot {
    let mut states = spring_snapshots(window, cx);
    assert_eq!(states.len(), 1);
    states.pop().unwrap()
}

fn assert_spring_close(actual: SpringState, expected: SpringState) {
    assert!(
        (actual.position - expected.position).abs() <= 1e-4 * (1.0 + expected.position.abs()),
        "actual {actual:?}, expected {expected:?}"
    );
    assert!(
        (actual.velocity - expected.velocity).abs() <= 1e-4 * (1.0 + expected.velocity.abs()),
        "actual {actual:?}, expected {expected:?}"
    );
}

#[open_gpui::test]
fn elapsed_time_uses_the_previous_target_and_config_before_a_retarget(cx: &mut TestAppContext) {
    let (window, _) = open_mutable_spring(cx);
    let before = spring_snapshot(window, cx);
    let next_config = SpringConfig::new(400.0, 30.0, 1.0);

    cx.executor().advance_clock(Duration::from_millis(50));
    window
        .update(cx, |view, _, cx| {
            view.config = next_config;
            view.target = px(-200.0);
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    let retargeted = spring_snapshot(window, cx);
    let elapsed = retargeted
        .updated_at
        .duration_since(before.updated_at)
        .as_secs_f32();
    assert_spring_close(
        retargeted.spring,
        before.config.step(before.spring, before.target, elapsed),
    );
    assert!(retargeted.spring.position > before.spring.position);
    assert!(retargeted.spring.velocity > 0.0);
    assert_eq!(retargeted.target, -200.0);
    assert_eq!(retargeted.config, next_config);

    cx.executor().advance_clock(Duration::from_millis(20));
    assert!(simulate_next_frame(&window, cx) > 0);
    let advanced = spring_snapshot(window, cx);
    let elapsed = advanced
        .updated_at
        .duration_since(retargeted.updated_at)
        .as_secs_f32();
    assert_spring_close(
        advanced.spring,
        next_config.step(retargeted.spring, -200.0, elapsed),
    );
}

#[open_gpui::test]
fn from_only_initializes_new_global_element_identities(cx: &mut TestAppContext) {
    let (window, samples) = open_mutable_spring(cx);
    assert_eq!(*samples.borrow(), vec![px(20.0)]);

    window
        .update(cx, |view, _, cx| {
            view.initial = Some(px(-50.0));
            view.playback = SpringPlayback::Completed;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(spring_snapshot(window, cx).initial, 20.0);
    assert_eq!(*samples.borrow().last().unwrap(), px(100.0));

    window
        .update(cx, |view, _, cx| {
            view.playback = SpringPlayback::Cancelled;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(*samples.borrow().last().unwrap(), px(20.0));
    let cancelled = spring_snapshot(window, cx);
    assert_eq!(cancelled.spring.velocity, 0.0);
    assert_eq!(cancelled.playback, SpringPlayback::Cancelled);

    window
        .update(cx, |view, _, cx| {
            view.ancestor_id = "replacement-ancestor";
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(spring_snapshot(window, cx).initial, -50.0);
    assert_eq!(*samples.borrow().last().unwrap(), px(-50.0));

    window
        .update(cx, |view, _, cx| {
            view.initial = Some(px(75.0));
            view.spring_id = "replacement-spring";
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(spring_snapshot(window, cx).initial, 75.0);
    assert_eq!(*samples.borrow().last().unwrap(), px(75.0));
}

#[open_gpui::test]
fn unmount_retires_state_and_remount_starts_from_the_new_initial_value(cx: &mut TestAppContext) {
    let (window, samples) = open_mutable_spring(cx);
    cx.executor().advance_clock(Duration::from_millis(50));
    assert!(simulate_next_frame(&window, cx) > 0);
    assert!(*samples.borrow().last().unwrap() > px(20.0));

    window
        .update(cx, |view, _, cx| {
            view.mounted = false;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    assert!(spring_snapshots(window, cx).is_empty());
    let unmounted_samples = samples.borrow().len();
    assert!(simulate_next_frame(&window, cx) > 0);
    assert_eq!(simulate_next_frame(&window, cx), 0);
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(samples.borrow().len(), unmounted_samples);

    window
        .update(cx, |view, _, cx| {
            view.initial = Some(px(40.0));
            view.mounted = true;
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    let remounted = spring_snapshot(window, cx);
    assert_eq!(remounted.spring.position, 40.0);
    assert_eq!(remounted.spring.velocity, 0.0);
    assert_eq!(*samples.borrow().last().unwrap(), px(40.0));
    cx.executor().advance_clock(Duration::from_millis(50));
    assert!(simulate_next_frame(&window, cx) > 0);
    assert!(*samples.borrow().last().unwrap() > px(40.0));
}

#[open_gpui::test]
fn reduced_motion_snaps_a_running_spring_and_later_targets_can_animate(cx: &mut TestAppContext) {
    let (window, samples) = open_mutable_spring(cx);
    cx.executor().advance_clock(Duration::from_millis(50));
    assert!(simulate_next_frame(&window, cx) > 0);
    assert!(*samples.borrow().last().unwrap() < px(100.0));

    cx.update(|app| app.set_reduce_motion(true));
    cx.run_until_parked();
    let reduced = spring_snapshot(window, cx);
    assert_eq!(reduced.spring.position, 100.0);
    assert_eq!(reduced.spring.velocity, 0.0);
    assert!(simulate_next_frame(&window, cx) > 0);
    assert_eq!(simulate_next_frame(&window, cx), 0);

    window
        .update(cx, |view, _, cx| {
            view.target = px(200.0);
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(*samples.borrow().last().unwrap(), px(200.0));
    assert_eq!(simulate_next_frame(&window, cx), 0);
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();

    cx.update(|app| app.set_reduce_motion(false));
    cx.run_until_parked();
    window
        .update(cx, |view, _, cx| {
            view.target = px(300.0);
            cx.notify();
        })
        .unwrap();
    cx.run_until_parked();
    assert_eq!(*samples.borrow().last().unwrap(), px(200.0));
    cx.executor().advance_clock(Duration::from_millis(50));
    assert!(simulate_next_frame(&window, cx) > 0);
    let resumed = *samples.borrow().last().unwrap();
    assert!(resumed > px(200.0));
    assert!(resumed < px(300.0));
}

#[open_gpui::test]
fn settled_spring_publishes_the_exact_endpoint_and_stops_requesting_frames(
    cx: &mut TestAppContext,
) {
    let (window, samples) = open_mutable_spring(cx);
    cx.executor().advance_clock(Duration::from_secs(30));
    assert!(simulate_next_frame(&window, cx) > 0);
    let settled = spring_snapshot(window, cx);
    assert_eq!(settled.spring.position, 100.0);
    assert_eq!(settled.spring.velocity, 0.0);
    assert_eq!(*samples.borrow().last().unwrap(), px(100.0));
    assert_eq!(simulate_next_frame(&window, cx), 0);
    let settled_samples = samples.borrow().len();
    cx.executor().advance_clock(Duration::from_secs(1));
    cx.run_until_parked();
    assert_eq!(samples.borrow().len(), settled_samples);
}

struct ProjectedTarget {
    coordinate: f32,
    label: String,
}

impl SpringTarget for ProjectedTarget {
    type Output = (String, Pixels);

    fn target(&self) -> f32 {
        self.coordinate
    }

    fn resolve(&self, value: f32) -> Self::Output {
        (self.label.clone(), px(value))
    }
}

struct ProjectedSpringView {
    samples: Rc<RefCell<Vec<(String, String, Pixels)>>>,
    child_paints: Rc<Cell<usize>>,
}

impl Render for ProjectedSpringView {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let samples = self.samples.clone();
        let consumed = String::from("consumed animator capture");
        let mapped_child_paints = self.child_paints.clone();
        let appended_child_paints = self.child_paints.clone();
        div()
            .size_full()
            .with_spring(
                "projected",
                SpringAnimation::new(SpringConfig::new(100.0, 2.0, 1.0))
                    .to(ProjectedTarget {
                        coordinate: 10.0,
                        label: "latest projection".into(),
                    })
                    .from(ProjectedTarget {
                        coordinate: 0.0,
                        label: "initial projection".into(),
                    }),
                move |element, (label, value)| {
                    samples.borrow_mut().push((consumed, label, value));
                    element.left(value)
                },
            )
            .map_element(move |element| {
                element.child(
                    crate::canvas(
                        |_, _, _| (),
                        move |_, (), _, _| {
                            mapped_child_paints.set(mapped_child_paints.get() + 1);
                        },
                    )
                    .size(px(10.0)),
                )
            })
            .child(
                crate::canvas(
                    |_, _, _| (),
                    move |_, (), _, _| {
                        appended_child_paints.set(appended_child_paints.get() + 1);
                    },
                )
                .size(px(10.0)),
            )
    }
}

#[open_gpui::test]
fn custom_targets_fn_once_animators_and_parent_mapping_reach_paint(cx: &mut TestAppContext) {
    let samples = Rc::new(RefCell::new(Vec::new()));
    let child_paints = Rc::new(Cell::new(0));
    let window = cx.open_window(size(px(100.0), px(100.0)), {
        let samples = samples.clone();
        let child_paints = child_paints.clone();
        move |_, _| ProjectedSpringView {
            samples,
            child_paints,
        }
    });
    cx.run_until_parked();
    assert_eq!(
        *samples.borrow(),
        vec![(
            "consumed animator capture".into(),
            "latest projection".into(),
            px(0.0),
        )]
    );
    assert_eq!(child_paints.get(), 2);

    cx.executor().advance_clock(Duration::from_millis(300));
    assert!(simulate_next_frame(&window, cx) > 0);
    assert!(samples.borrow().last().unwrap().2 > px(10.0));
    assert_eq!(child_paints.get(), samples.borrow().len() * 2);
    assert!(samples.borrow().iter().all(|(capture, label, _)| {
        capture == "consumed animator capture" && label == "latest projection"
    }));
}

#[open_gpui::test]
fn a_settled_spring_draws_directly_without_a_rendered_view(cx: &mut TestAppContext) {
    let visual = cx.add_empty_window();
    let sample = Rc::new(Cell::new(None));
    visual.draw(crate::Point::default(), size(px(80.0), px(40.0)), {
        let sample = sample.clone();
        move |_, _| {
            div().size(px(20.0)).with_spring(
                "direct-static-spring",
                SpringAnimation::new(SpringConfig::new(100.0, 10.0, 1.0)).to(1.0),
                move |element, value| {
                    sample.set(Some(value));
                    element
                },
            )
        }
    });
    assert_eq!(sample.get(), Some(1.0));
}
