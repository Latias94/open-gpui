//! Independent Windows consumer using the production application loop and public frame readback.

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("unsupported: native hot-reload qualification requires Windows and a visible window");
    std::process::exit(2);
}

#[cfg(target_os = "windows")]
fn main() {
    if let Err(error) = native::run() {
        eprintln!("native hot-reload consumer failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(target_os = "windows")]
mod native {
    use hot_reload_panel::{
        Channel, INITIAL_COUNTER, PanelComponent, PanelProbe, ROW_HEIGHT, StatefulPanel,
    };
    use open_gpui::{
        AnyView, App, AppContext as _, Application, Context, Entity, IntoElement,
        ParentElement as _, PlatformWindowPresentOutcome, QuitMode, Render, StyleRefinement,
        Styled as _, Window, WindowBounds, WindowInitialPresentationStatus, WindowOptions, div, px,
        rgb, size,
    };
    use open_gpui_windows::WindowsPlatform;
    use serde_json::{Value, json};
    use std::{
        cell::{Cell, RefCell},
        env,
        fs::{File, OpenOptions},
        io::{self, Write as _},
        path::PathBuf,
        rc::Rc,
        time::{Duration, Instant, SystemTime, UNIX_EPOCH},
    };

    const CHANNELS: [&str; 3] = ["typed_cached", "stored_any_view", "render_once"];

    struct ObservationLog {
        file: RefCell<File>,
        started: Instant,
    }

    impl ObservationLog {
        fn emit(&self, mut observation: Value) -> io::Result<()> {
            observation["pid"] = json!(std::process::id());
            observation["elapsed_ms"] = json!(self.started.elapsed().as_millis());
            let mut file = self.file.borrow_mut();
            serde_json::to_writer(&mut *file, &observation)?;
            file.write_all(b"\n")?;
            file.flush()
        }
    }

    struct Lifecycle {
        root_released: Cell<bool>,
        probes: [Rc<PanelProbe>; 3],
        last_frame: RefCell<Option<Value>>,
    }

    impl Lifecycle {
        fn retired(&self, cx: &App) -> bool {
            cx.windows().is_empty()
                && self.root_released.get()
                && self.probes.iter().all(|probe| probe.is_released())
        }

        fn teardown(&self, cx: &App, stage: &str) -> Value {
            let mut observation = self
                .last_frame
                .borrow()
                .clone()
                .unwrap_or_else(|| json!({}));
            observation["stage"] = json!(stage);
            observation["remaining_windows"] = json!(cx.windows().len());
            observation["root_released"] = json!(self.root_released.get());
            observation["children_released"] =
                json!(self.probes.each_ref().map(|p| p.is_released()));
            observation["initializations"] =
                json!(self.probes.each_ref().map(|p| p.initializations()));
            observation["releases"] = json!(self.probes.each_ref().map(|p| p.releases()));
            observation["post_run_supported"] = json!(false);
            observation["platform_exit"] = json!("ExitProcess");
            observation
        }
    }

    struct Consumer {
        process_start_nonce: String,
        counter: u32,
        state_mutations: u32,
        typed: Entity<StatefulPanel>,
        erased: AnyView,
        lifecycle: Rc<Lifecycle>,
        renders: u64,
    }

    impl Consumer {
        fn mutate_counters_once(&mut self, cx: &mut Context<Self>) -> io::Result<()> {
            if self.state_mutations != 0 {
                return Ok(());
            }
            let erased = self
                .erased
                .clone()
                .downcast::<StatefulPanel>()
                .map_err(|_| io::Error::other("stored AnyView lost its concrete panel type"))?;
            if !self.lifecycle.probes[2].set_component_counter(53, cx) {
                return Err(io::Error::other("keyed component state is unavailable"));
            }
            self.typed.update(cx, |panel, cx| panel.set_counter(43, cx));
            erased.update(cx, |panel, cx| panel.set_counter(47, cx));
            self.counter = 41;
            self.state_mutations = 1;
            cx.notify();
            Ok(())
        }
    }

    impl Render for Consumer {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            self.renders += 1;
            let style = || StyleRefinement::default().w_full().h(px(ROW_HEIGHT));
            div()
                .size_full()
                .flex()
                .flex_col()
                .bg(rgb(0x101820))
                .child(self.typed.clone().cached(style()))
                .child(self.erased.clone().cached(style()))
                .child(PanelComponent::new(self.lifecycle.probes[2].clone()))
        }
    }

    fn observe(
        root: &Consumer,
        window: &Window,
        cx: &Context<Consumer>,
        previous_generation: Option<u64>,
    ) -> io::Result<Option<Value>> {
        let facts = window.presentation_facts();
        let Some(accepted) = facts.frame_accepted_generation else {
            return Ok(None);
        };
        let Some(attempt) = facts.latest_present_attempt else {
            return Ok(None);
        };
        if previous_generation == Some(accepted)
            || accepted == 0
            || facts.present_submitted_generation != Some(accepted)
            || facts.non_empty_presented_generation != Some(accepted)
            || attempt.generation != accepted
            || attempt.outcome != PlatformWindowPresentOutcome::Submitted
            || !attempt.contained_valid_primitives
            || facts.initial_presentation != WindowInitialPresentationStatus::Completed
            || !facts.native_visible
            || window.rendered_frame_revision() != accepted
        {
            return Ok(None);
        }
        let [Some(typed), Some(erased), Some(component)] = root
            .lifecycle
            .probes
            .each_ref()
            .map(|probe| probe.last_paint())
        else {
            return Ok(None);
        };
        let paints = [typed, erased, component];
        if paints.iter().enumerate().any(|(index, paint)| {
            paint.bounds.size.width <= px(0.0)
                || paint.bounds.size.height <= px(0.0)
                || !paint
                    .previous_generation
                    .checked_add(1)
                    .is_some_and(|generation| {
                        if index < 2 {
                            generation <= accepted
                        } else {
                            generation == accepted
                        }
                    })
        }) {
            return Ok(None);
        }
        let scale = window.scale_factor();
        if !scale.is_finite() || scale <= 0.0 {
            return Err(io::Error::other(
                "native window has an invalid scale factor",
            ));
        }
        let image = window
            .render_to_image()
            .map_err(|error| io::Error::other(format!("native frame readback failed: {error}")))?;
        let mut pixels = [[0_u8; 4]; 3];
        let mut sample_positions = [[0_u32; 2]; 3];
        for (index, paint) in paints.iter().enumerate() {
            let center = paint.bounds.center();
            let x = (center.x.as_f32() * scale).floor();
            let y = (center.y.as_f32() * scale).floor();
            if !x.is_finite()
                || !y.is_finite()
                || x < 0.0
                || y < 0.0
                || x >= image.width() as f32
                || y >= image.height() as f32
            {
                return Err(io::Error::other(
                    "a row's center lies outside the native frame",
                ));
            }
            let position = [x as u32, y as u32];
            pixels[index] = image.get_pixel(position[0], position[1]).0;
            sample_positions[index] = position;
        }
        let colors = paints.map(|paint| paint.recipe.color);
        let expected_pixels =
            colors.map(|color| [(color >> 16) as u8, (color >> 8) as u8, color as u8, 255]);
        let pixels_match = pixels
            .iter()
            .zip(expected_pixels)
            .all(|(actual, expected)| {
                actual
                    .iter()
                    .zip(expected)
                    .all(|(actual, expected)| actual.abs_diff(expected) <= 1)
            });
        let labels = paints.map(|paint| paint.recipe.label);
        let phase = if labels.iter().all(|label| label.ends_with("-READY")) {
            "READY"
        } else if labels.iter().all(|label| label.ends_with("-PATCHED")) {
            "PATCHED"
        } else {
            "MIXED"
        };
        let erased_owner = root
            .erased
            .clone()
            .downcast::<StatefulPanel>()
            .map_err(|_| io::Error::other("stored AnyView lost its concrete panel type"))?;
        let state = paints.map(|paint| paint.counter);
        let entity_ids = paints.map(|paint| format!("{:?}", paint.entity_id));
        let initializations = root
            .lifecycle
            .probes
            .each_ref()
            .map(|probe| probe.initializations());
        let entity_ids_match = typed.entity_id == root.typed.entity_id()
            && erased.entity_id == root.erased.entity_id();
        Ok(Some(json!({
            "stage": "frame",
            "channels": CHANNELS,
            "phase": phase,
            "root_entity_id": format!("{:?}", cx.entity_id()),
            "process_start_nonce": root.process_start_nonce,
            "root_counter": root.counter,
            "state_mutations": root.state_mutations,
            "root_renders": root.renders,
            "entity_ids": entity_ids,
            "entity_ids_match": entity_ids_match,
            "state": state,
            "live_state": [root.typed.read(cx).counter(), erased_owner.read(cx).counter()],
            "initializations": initializations,
            "colors": colors,
            "labels": labels,
            "recipe_outputs": paints.map(|paint| paint.recipe.output),
            "pixels": pixels,
            "expected_pixels": expected_pixels,
            "pixels_match": pixels_match,
            "sample_positions": sample_positions,
            "paint_generations": paints.map(|paint| paint.previous_generation + 1),
            "accepted": accepted,
            "submitted": facts.present_submitted_generation,
            "non_empty_presented": facts.non_empty_presented_generation,
            "rendered": window.rendered_frame_revision(),
            "native_visible": facts.native_visible,
            "native_created": facts.native_created,
            "initial_presentation": format!("{:?}", facts.initial_presentation),
            "window_id": format!("{:?}", window.window_handle().window_id()),
            "image_size": [image.width(), image.height()],
            "scale_factor": scale,
            "test_scheduler": cx.is_test(),
            "readback_source": "accepted_framework_scene",
        })))
    }

    pub fn run() -> io::Result<()> {
        let process_start_nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| io::Error::other(format!("process start clock failed: {error}")))?
            .as_nanos()
            .to_string();
        let _ = env_logger::try_init();
        let observations_path = env::var_os("OPEN_GPUI_HOT_RELOAD_OBSERVATIONS")
            .map(PathBuf::from)
            .ok_or_else(|| {
                io::Error::other("OPEN_GPUI_HOT_RELOAD_OBSERVATIONS must name a JSONL file")
            })?;
        let stop_path = env::var_os("OPEN_GPUI_HOT_RELOAD_STOP")
            .map(PathBuf::from)
            .ok_or_else(|| io::Error::other("OPEN_GPUI_HOT_RELOAD_STOP must name a stop file"))?;
        let checkpoint_path = env::var_os("OPEN_GPUI_HOT_RELOAD_CHECKPOINT").map(PathBuf::from);
        let log = Rc::new(ObservationLog {
            file: RefCell::new(
                OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(observations_path)?,
            ),
            started: Instant::now(),
        });
        let lifecycle = Rc::new(Lifecycle {
            root_released: Cell::new(false),
            probes: std::array::from_fn(|_| Rc::new(PanelProbe::default())),
            last_frame: RefCell::new(None),
        });
        let failure = Rc::new(RefCell::new(None::<String>));
        let application = Application::with_platform(Rc::new(
            WindowsPlatform::new(false)
                .map_err(|error| io::Error::other(format!("Windows platform failed: {error}")))?,
        ))
        .with_quit_mode(QuitMode::Explicit);
        let run_log = log.clone();
        let run_lifecycle = lifecycle.clone();
        let run_failure = failure.clone();
        application.run(move |cx| {
            let quit_log = run_log.clone();
            let quit_lifecycle = run_lifecycle.clone();
            cx.on_app_quit(move |cx| {
                if let Err(error) = quit_log.emit(quit_lifecycle.teardown(cx, "quit")) {
                    eprintln!("writing native quit observation failed: {error}");
                }
                async {}
            })
            .detach();
            let lifecycle = run_lifecycle.clone();
            let handle = cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(
                        size(px(640.0), px(ROW_HEIGHT * 3.0)),
                        cx,
                    )),
                    titlebar: None,
                    focus_on_appearing: false,
                    show: true,
                    inactive_frame_interval: None,
                    low_power_frame_interval: None,
                    thermal_frame_interval: None,
                    ..WindowOptions::default()
                },
                |_, cx| {
                    let typed_probe = lifecycle.probes[0].clone();
                    let erased_probe = lifecycle.probes[1].clone();
                    let typed =
                        cx.new(|cx| StatefulPanel::new(Channel::TypedCached, typed_probe, cx));
                    let erased =
                        cx.new(|cx| StatefulPanel::new(Channel::StoredAnyView, erased_probe, cx));
                    let erased = AnyView::from(erased);
                    cx.new(|cx| {
                        let released = lifecycle.clone();
                        cx.on_release(move |_, _| released.root_released.set(true))
                            .detach();
                        Consumer {
                            process_start_nonce,
                            counter: INITIAL_COUNTER,
                            state_mutations: 0,
                            typed,
                            erased,
                            lifecycle,
                            renders: 0,
                        }
                    })
                },
            );
            let handle = match handle {
                Ok(handle) => handle,
                Err(error) => {
                    let message = format!("opening native hot-reload window failed: {error}");
                    eprintln!("{message}");
                    if let Err(error) = run_log.emit(json!({
                        "stage": "error",
                        "error": message,
                    })) {
                        eprintln!("writing native error observation failed: {error}");
                    }
                    *run_failure.borrow_mut() = Some(message);
                    cx.quit();
                    return;
                }
            };
            cx.spawn(async move |cx| {
                let mut previous_generation = None;
                let mut previous_checkpoint = None;
                let mut closing = false;
                let mut retirement_polls = 0;
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    if closing {
                        let retired = cx.update(|cx| run_lifecycle.retired(cx));
                        retirement_polls += 1;
                        if retired || retirement_polls >= 100 {
                            cx.update(|cx| {
                                let stage = if retired {
                                    "retired"
                                } else {
                                    "retirement_timeout"
                                };
                                if let Err(error) = run_log.emit(run_lifecycle.teardown(cx, stage))
                                {
                                    eprintln!(
                                        "writing native retirement observation failed: {error}"
                                    );
                                }
                                cx.shutdown();
                                cx.quit();
                            });
                            return;
                        }
                        continue;
                    }
                    if stop_path.exists() {
                        let removal = handle.update(cx, |_, window, cx| window.remove_window(cx));
                        if let Err(error) = removal {
                            eprintln!("native window was already closed: {error}");
                        }
                        closing = true;
                        continue;
                    }
                    let checkpoint = checkpoint_path
                        .as_ref()
                        .and_then(|path| std::fs::read_to_string(path).ok())
                        .filter(|value| !value.is_empty());
                    let checkpoint_changed = checkpoint != previous_checkpoint;
                    let observation = handle
                        .update(cx, |root, window, cx| {
                            observe(
                                root,
                                window,
                                cx,
                                if checkpoint_changed {
                                    None
                                } else {
                                    previous_generation
                                },
                            )
                        })
                        .map_err(|error| error.to_string())
                        .and_then(|result| result.map_err(|error| error.to_string()));
                    let result = observation.and_then(|observation| {
                        let Some(mut observation) = observation else {
                            return Ok(());
                        };
                        observation["checkpoint"] = json!(checkpoint);
                        previous_checkpoint = checkpoint;
                        previous_generation = observation["accepted"].as_u64();
                        let mutate_state =
                            observation["phase"] == "READY" && observation["state_mutations"] == 0;
                        *run_lifecycle.last_frame.borrow_mut() = Some(observation.clone());
                        run_log.emit(observation).map_err(|error| {
                            format!("writing native frame observation failed: {error}")
                        })?;
                        if mutate_state {
                            handle
                                .update(cx, |root, _, cx| root.mutate_counters_once(cx))
                                .map_err(|error| error.to_string())?
                                .map_err(|error| error.to_string())?;
                        }
                        Ok(())
                    });
                    if let Err(message) = result {
                        eprintln!("native frame observation failed: {message}");
                        if let Err(error) = run_log.emit(json!({
                            "stage": "error",
                            "error": message,
                        })) {
                            eprintln!("writing native error observation failed: {error}");
                        }
                        *run_failure.borrow_mut() = Some(message);
                        let _ = handle.update(cx, |_, window, cx| window.remove_window(cx));
                        closing = true;
                    }
                }
            })
            .detach();
        });
        if let Some(error) = failure.borrow_mut().take() {
            return Err(io::Error::other(error));
        }
        Ok(())
    }
}
