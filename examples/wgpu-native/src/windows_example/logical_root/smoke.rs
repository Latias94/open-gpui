use super::*;
use open_gpui::{KeyDownEvent, Keystroke, PlatformFrameSubmissionStatus, WindowPresentationFacts};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::time::Instant;

#[derive(Clone)]
struct Frame {
    token: PlatformFrameSubmission,
    output: Rc<renderer::Output>,
    count: u32,
}

#[derive(Clone, Copy, Debug)]
enum Phase {
    Initial,
    InitialSubmitted,
    Typed,
    TypedSubmitted,
    Clicked,
    ClickedSubmitted,
    CandidateA,
    SubmittedA,
    CandidateB,
    SubmittedB,
    InputCandidate,
    InputRecovery,
    InputSubmitted,
    GeometryCandidate(u8),
    ResizedCandidate,
    ResizedSubmitted,
    GeometrySubmitted(u8),
    Suspended,
    Hidden,
    Rejection,
    Recovery,
    RecoverySubmitted,
    CloseCandidate,
    Closing,
    Retiring,
    NativeSibling,
    Done,
}

pub(super) struct Smoke {
    started: Instant,
    phase: Phase,
    last: Option<Frame>,
    original: Option<Frame>,
    receiver_b: Option<Entity<HostInput>>,
    initial_field: u64,
    typed_field: u64,
    producer_a: Option<[u8; 4]>,
    held_prefix: u64,
    held_offers: u64,
    native_prefix: u64,
    native_text: String,
    close_text: (String, String),
    retained_output: Option<Rc<renderer::Output>>,
}

impl Smoke {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            phase: Phase::Initial,
            last: None,
            original: None,
            receiver_b: None,
            initial_field: 0,
            typed_field: 0,
            producer_a: None,
            held_prefix: 0,
            held_offers: 0,
            native_prefix: 0,
            native_text: String::new(),
            close_text: (String::new(), String::new()),
            retained_output: None,
        }
    }

    pub fn logical_complete(&self) -> bool {
        matches!(self.phase, Phase::NativeSibling | Phase::Done)
    }

    pub fn complete(&self) -> bool {
        matches!(self.phase, Phase::Done)
    }

    pub fn advance(
        &mut self,
        root: &LogicalRoot,
        cx: &AsyncApp,
        factory: &Factory,
        native: &WindowHandle<Root>,
        native_input: &Entity<HostInput>,
        owned: bool,
    ) -> Result<()> {
        if self.complete() {
            return Ok(());
        }
        ensure!(
            self.started.elapsed() < Duration::from_secs(45),
            "logical-root smoke timed out in {:?}",
            self.phase
        );
        ensure!(
            root.native_creations
                == (
                    factory.renderers_created.get(),
                    factory.gpu.surfaces_created.load(Ordering::Relaxed)
                ),
            "logical-root consumption created an additional native renderer or surface"
        );
        let facts = if root.surface.is_closed() {
            None
        } else {
            Some(cx.update(|cx| {
                root.window.update(cx, |_, window, _| {
                    ensure!(
                        HasWindowHandle::window_handle(window).is_err(),
                        "logical root exposed an HWND"
                    );
                    Ok(window.presentation_facts())
                })
            })??)
        };
        if let Some(facts) = facts {
            ensure!(
                !facts.native_created && !facts.native_visible,
                "logical presentation published native-window facts: {facts:?}"
            );
        }
        match self.phase {
            Phase::Initial => {
                let Some(frame) = pending(root) else {
                    return Ok(());
                };
                ensure!(
                    frame.count == 0
                        && frame.token.is_pending()
                        && root.renderer.delivery.borrow().prefix == 0,
                    "logical initial output was submitted before host admission"
                );
                ensure!(
                    facts.unwrap().present_submitted_generation.is_none(),
                    "unsubmitted logical output was published"
                );
                cx.update(|cx| {
                    ensure!(
                        root.input.read(cx).text.is_empty()
                            && native_input.read(cx).text.is_empty(),
                        "root input was initialized with synthetic text"
                    );
                    ensure!(
                        root.input.read(cx).focus != native_input.read(cx).focus,
                        "native and logical roots shared field focus"
                    );
                    self.native_text = native_input.read(cx).text.clone();
                    Ok::<_, anyhow::Error>(())
                })?;
                self.submit(root, frame)?;
                self.phase = Phase::InitialSubmitted;
            }
            Phase::InitialSubmitted => {
                if !self.published(facts.unwrap()) {
                    return Ok(());
                }
                let image = self.read(root)?;
                image.check_ui()?;
                self.initial_field = image.field_hash();
                root.click(point(px(50.), px(336.)))?;
                ensure!(
                    root.surface
                        .with_input_handler(|handler| handler.replace_text_in_range(None, "lλ🦀"))?
                        .is_some(),
                    "logical initial field did not install its real input handler"
                );
                root.request_frame()?;
                self.phase = Phase::Typed;
            }
            Phase::Typed => {
                let Some(frame) = pending(root) else {
                    return Ok(());
                };
                self.submit(root, frame)?;
                self.phase = Phase::TypedSubmitted;
            }
            Phase::TypedSubmitted => {
                if !self.published(facts.unwrap()) {
                    return Ok(());
                }
                let image = self.read(root)?;
                image.check_ui()?;
                self.typed_field = image.field_hash();
                ensure!(
                    self.typed_field != self.initial_field,
                    "typing did not change GPUI field output pixels"
                );
                cx.update(|cx| {
                    ensure!(root.input.read(cx).text == "lλ🦀" && native_input.read(cx).text == self.native_text,
                        "logical text input changed the native receiver or bypassed its own receiver");
                    Ok::<_, anyhow::Error>(())
                })?;
                let native_count = factory.count.get();
                root.click(point(px(60.), px(120.)))?;
                ensure!(
                    root.renderer.count.get() == 1 && factory.count.get() == native_count,
                    "ordinary logical on_click did not change only its own source"
                );
                root.request_frame()?;
                self.phase = Phase::Clicked;
            }
            Phase::Clicked => {
                let Some(frame) = pending(root) else {
                    return Ok(());
                };
                ensure!(frame.count == 1, "actual logical click was not rendered");
                self.submit(root, frame)?;
                self.phase = Phase::ClickedSubmitted;
            }
            Phase::ClickedSubmitted => {
                if !self.published(facts.unwrap()) {
                    return Ok(());
                }
                let clicked = self.read(root)?;
                let pixel = clicked.pixel(80, 210)?;
                ensure!(
                    pixel[0] > pixel[2].saturating_add(30),
                    "logical click did not change the sampled producer in GPUI output: {pixel:?}"
                );
                let input_b = cx.update(|cx| {
                    let focus = root.input.read(cx).focus.clone();
                    cx.new(|cx| {
                        let mut receiver = HostInput::new(root.renderer.count.clone(), cx);
                        receiver.focus = focus;
                        receiver
                    })
                });
                self.receiver_b = Some(input_b);
                stage(root, cx, 3, px(240.), root.input.clone())?;
                self.phase = Phase::CandidateA;
            }
            Phase::CandidateA => {
                let Some(frame) = pending(root) else {
                    return Ok(());
                };
                if frame.count != 3 {
                    discard(root, &frame)?;
                    root.request_frame()?;
                    return Ok(());
                }
                stage(
                    root,
                    cx,
                    4,
                    px(0.),
                    self.receiver_b.as_ref().unwrap().clone(),
                )?;
                let frozen =
                    pending(root).context("source B discarded the valid original A packet")?;
                ensure!(
                    same(&frame.token, &frozen.token)
                        && Rc::ptr_eq(&frame.output, &frozen.output)
                        && frozen.count == 3
                        && root.renderer.count.get() == 4
                        && frame.token.is_pending(),
                    "source mutation B retargeted or invalidated the frozen A recording"
                );
                self.submit(root, frame)?;
                self.phase = Phase::SubmittedA;
            }
            Phase::SubmittedA => {
                if !self.published(facts.unwrap()) {
                    return Ok(());
                }
                let image = self.read(root)?;
                image.expect_color(266, 102, [0x27, 0x66, 0x50, 255])?;
                image.expect_color(26, 102, [0x16, 0x23, 0x34, 255])?;
                let pixel = image.pixel(80, 210)?;
                ensure!(
                    pixel[0] > pixel[2].saturating_add(30),
                    "recorded A did not preserve the warm image after source B: {pixel:?}"
                );
                self.producer_a = Some(pixel);
                root.request_frame()?;
                self.phase = Phase::CandidateB;
            }
            Phase::CandidateB => {
                let Some(frame) = pending(root) else {
                    return Ok(());
                };
                ensure!(frame.count == 4, "successor did not record source B");
                self.submit(root, frame)?;
                self.phase = Phase::SubmittedB;
            }
            Phase::SubmittedB => {
                if !self.published(facts.unwrap()) {
                    return Ok(());
                }
                let image = self.read(root)?;
                image.check_ui()?;
                let pixel = image.pixel(80, 210)?;
                ensure!(
                    pixel[2] > pixel[0].saturating_add(30) && self.producer_a != Some(pixel),
                    "original B output did not differ from original A: {pixel:?}"
                );
                stage(root, cx, 5, px(240.), root.input.clone())?;
                self.phase = Phase::InputCandidate;
            }
            Phase::InputCandidate => {
                let Some(frame) = pending(root) else {
                    return Ok(());
                };
                if frame.count != 5 {
                    discard(root, &frame)?;
                    root.request_frame()?;
                    return Ok(());
                }
                let published = facts.unwrap().present_submitted_generation;
                root.click(point(px(60.), px(120.)))?;
                ensure!(
                    root.renderer.count.get() == 6,
                    "pending successor replaced the submitted logical button geometry"
                );
                ensure!(
                    root.surface
                        .with_input_handler(|handler| handler.replace_text_in_range(None, "pλ🦀"))?
                        .is_some(),
                    "the submitted logical receiver disappeared behind the successor"
                );
                root.surface
                    .dispatch_input(PlatformInput::KeyDown(KeyDownEvent {
                        keystroke: Keystroke::parse("f6")?,
                        is_held: false,
                        prefer_character_input: false,
                    }))?;
                cx.update(|cx| {
                    let a = root.input.read(cx);
                    let b = self.receiver_b.as_ref().unwrap().read(cx);
                    ensure!(a.text == "lλ🦀" && b.text == "pλ🦀" && a.key_events == 0 && b.key_events == 1,
                        "logical input targeted the pending candidate instead of the submitted receiver");
                    ensure!(native_input.read(cx).text == self.native_text, "logical submitted input changed the native field");
                    ensure!(a.focus != native_input.read(cx).focus && b.focus != native_input.read(cx).focus,
                        "logical submitted fields reused native focus");
                    Ok::<_, anyhow::Error>(())
                })?;
                ensure!(
                    root.renderer.count.get() == 7
                        && !frame.token.is_pending()
                        && !frame
                            .token
                            .complete(PlatformWindowPresentOutcome::Submitted),
                    "input did not terminally cancel its successor"
                );
                let now = cx.update(|cx| {
                    root.window
                        .update(cx, |_, window, _| window.presentation_facts())
                })?;
                ensure!(
                    now.present_submitted_generation == published,
                    "pending input effects published unsubmitted output"
                );
                stage(
                    root,
                    cx,
                    8,
                    px(0.),
                    self.receiver_b.as_ref().unwrap().clone(),
                )?;
                self.phase = Phase::InputRecovery;
            }
            Phase::InputRecovery => {
                let Some(frame) = pending(root) else {
                    return Ok(());
                };
                if frame.count != 8 {
                    discard(root, &frame)?;
                    root.request_frame()?;
                    return Ok(());
                }
                ensure!(
                    frame.token.is_pending(),
                    "input recovery did not produce a fresh packet"
                );
                self.submit(root, frame)?;
                self.phase = Phase::InputSubmitted;
            }
            Phase::InputSubmitted => {
                if !self.published(facts.unwrap()) {
                    return Ok(());
                }
                let image = self.read(root)?;
                ensure!(
                    image.field_hash() != self.typed_field
                        && image.field_hash() != self.initial_field,
                    "submitted receiver edits did not change final GPUI field pixels: initial={}, typed={}, final={}",
                    self.initial_field,
                    self.typed_field,
                    image.field_hash()
                );
                root.request_frame()?;
                self.phase = Phase::GeometryCandidate(0);
            }
            Phase::GeometryCandidate(step) => {
                let Some(frame) = pending(root) else {
                    return Ok(());
                };
                let prefix = root.renderer.delivery.borrow().prefix;
                match step {
                    0 => {
                        root.set_geometry(size(px(704.), px(444.)), 1.)?;
                        invalid(&frame)?;
                        root.renderer.process_pending(false)?;
                        ensure!(
                            root.renderer.delivery.borrow().prefix == prefix,
                            "resize submitted its invalidated original packet"
                        );
                        self.original = Some(frame);
                        root.request_frame()?;
                        self.phase = Phase::ResizedCandidate;
                        return Ok(());
                    }
                    1 => {
                        root.set_geometry(size(px(320.), px(210.)), 2.)?;
                        ensure!(
                            root.surface.geometry().physical_size()
                                == size(DevicePixels(640), DevicePixels(420)),
                            "scale transition did not preserve physical size"
                        );
                    }
                    2 => {
                        root.set_geometry(size(px(0.), px(420.)), 1.)?;
                        invalid(&frame)?;
                        root.renderer.process_pending(false)?;
                        self.held_prefix = prefix;
                        self.held_offers = root.renderer.delivery.borrow().offers;
                        root.request_frame()?;
                        self.phase = Phase::Suspended;
                        return Ok(());
                    }
                    _ => unreachable!(),
                }
                invalid(&frame)?;
                root.renderer.process_pending(false)?;
                ensure!(
                    root.renderer.delivery.borrow().prefix == prefix,
                    "geometry invalidation submitted stale commands"
                );
                self.original = Some(frame);
                root.request_frame()?;
                self.phase = Phase::GeometrySubmitted(step);
            }
            Phase::ResizedCandidate => {
                let Some(frame) = pending(root) else {
                    return Ok(());
                };
                ensure!(
                    frame.output.extent == (704, 444),
                    "resize did not record the real enlarged output"
                );
                invalid(self.original.as_ref().unwrap())?;
                self.submit(root, frame)?;
                root.request_frame()?;
                self.phase = Phase::ResizedSubmitted;
            }
            Phase::ResizedSubmitted => {
                if !self.published(facts.unwrap()) {
                    return Ok(());
                }
                let Some(middle) = pending(root) else {
                    return Ok(());
                };
                let image = self.read(root)?;
                ensure!(
                    image.extent == (704, 444),
                    "actual resized GPU output used an old target"
                );
                image.check_ui()?;
                root.set_geometry(size(px(640.), px(420.)), 1.)?;
                invalid(&middle)?;
                invalid(self.original.as_ref().unwrap())?;
                root.renderer.process_pending(false)?;
                root.request_frame()?;
                self.phase = Phase::GeometrySubmitted(0);
            }
            Phase::GeometrySubmitted(step) => {
                let Some(frame) = pending(root) else {
                    return Ok(());
                };
                let original = self.original.as_ref().unwrap();
                ensure!(
                    !same(&frame.token, &original.token)
                        && !Rc::ptr_eq(&frame.output, &original.output)
                        && frame.output.basis.revision != original.output.basis.revision,
                    "geometry recovery reused an invalidated original packet"
                );
                if step == 0 {
                    ensure!(
                        frame.output.extent == original.output.extent,
                        "same-size ABA did not return to its original extent"
                    );
                } else {
                    ensure!(
                        frame.output.extent == original.output.extent
                            && frame.output.basis.scale != original.output.basis.scale,
                        "same-physical-size scale recovery lost its logical basis"
                    );
                }
                self.submit(root, frame)?;
                root.request_frame()?;
                self.phase = Phase::GeometryCandidate(step + 1);
            }
            Phase::Suspended => {
                ensure!(
                    root.renderer.delivery.borrow().offers == self.held_offers
                        && root.renderer.delivery.borrow().prefix == self.held_prefix
                        && pending(root).is_none()
                        && root.surface.geometry().physical_size().width.0 == 0,
                    "zero extent produced a fake 1x1 frame or submitted output"
                );
                root.set_geometry(size(px(640.), px(420.)), 1.)?;
                root.request_frame()?;
                self.phase = Phase::Rejection;
            }
            Phase::Rejection => {
                let Some(frame) = pending(root) else {
                    return Ok(());
                };
                ensure!(
                    frame.output.extent == (640, 420),
                    "zero extent did not resume at the real target size"
                );
                discard(root, &frame)?;
                self.original = Some(frame);
                root.request_frame()?;
                self.phase = Phase::Recovery;
            }
            Phase::Recovery => {
                let Some(frame) = pending(root) else {
                    return Ok(());
                };
                ensure!(
                    !same(&frame.token, &self.original.as_ref().unwrap().token),
                    "rejected candidate was reused for recovery"
                );
                self.submit(root, frame)?;
                root.request_frame()?;
                self.phase = Phase::RecoverySubmitted;
            }
            Phase::RecoverySubmitted => {
                if !self.published(facts.unwrap()) {
                    return Ok(());
                }
                let Some(frame) = pending(root) else {
                    return Ok(());
                };
                root.set_visible(false)?;
                invalid(&frame)?;
                root.renderer.process_pending(false)?;
                self.held_prefix = root.renderer.delivery.borrow().prefix;
                self.held_offers = root.renderer.delivery.borrow().offers;
                root.request_frame()?;
                self.phase = Phase::Hidden;
            }
            Phase::Hidden => {
                ensure!(
                    pending(root).is_none()
                        && root.renderer.delivery.borrow().offers == self.held_offers
                        && root.renderer.delivery.borrow().prefix == self.held_prefix,
                    "hidden logical output reached its renderer"
                );
                root.set_visible(true)?;
                root.request_frame()?;
                self.phase = Phase::CloseCandidate;
            }
            Phase::CloseCandidate => {
                let Some(frame) = pending(root) else {
                    return Ok(());
                };
                self.submit(root, frame)?;
                let mut delivery = root.renderer.delivery.borrow_mut();
                delivery.pin = Some(delivery.prefix);
                self.held_prefix = delivery.prefix;
                self.retained_output = Some(delivery.last_output.as_ref().unwrap().output.clone());
                drop(delivery);
                root.request_frame()?;
                self.phase = Phase::Closing;
            }
            Phase::Closing => {
                if !self.published(facts.unwrap()) {
                    return Ok(());
                }
                let Some(frame) = pending(root) else {
                    return Ok(());
                };
                {
                    let delivery = root.renderer.delivery.borrow();
                    let submitted = delivery
                        .in_flight
                        .iter()
                        .find(|entry| entry.prefix == self.held_prefix)
                        .context("close lost its retained original submitted recording")?;
                    ensure!(
                        Rc::ptr_eq(&submitted.output, self.retained_output.as_ref().unwrap())
                            && submitted.token.status() == PlatformFrameSubmissionStatus::Submitted,
                        "close replaced its original submitted output or authority"
                    );
                }
                self.close_text = cx.update(|cx| {
                    (
                        root.input.read(cx).text.clone(),
                        self.receiver_b.as_ref().unwrap().read(cx).text.clone(),
                    )
                });
                self.native_prefix = factory.submitted.get();
                root.surface.close()?;
                invalid(&frame)?;
                self.original = Some(frame);
                self.phase = Phase::Retiring;
            }
            Phase::Retiring => {
                if root
                    .renderer
                    .delivery
                    .borrow()
                    .ticket
                    .as_ref()
                    .is_none_or(|ticket| !ticket.snapshot().detached())
                {
                    return Ok(());
                }
                root.verify_shutdown(false)?;
                ensure!(
                    thread_ui_windows()? == root.thread_windows,
                    "logical rendering/input/close changed the thread's UI HWND identities"
                );
                ensure!(
                    root.renderer.delivery.borrow().prefix == self.held_prefix,
                    "close submitted its retained unsubmitted packet"
                );
                let image = self.read(root)?;
                image.check_ui()?;
                ensure!(
                    Rc::ptr_eq(
                        self.retained_output.as_ref().unwrap(),
                        &self.last.as_ref().unwrap().output
                    ),
                    "reading output after close changed the retained host owner"
                );
                if owned {
                    native_probe(cx, native, native_input)?;
                }
                self.phase = Phase::NativeSibling;
            }
            Phase::NativeSibling => {
                let text = cx.update(|cx| native_input.read(cx).text.clone());
                if text == self.native_text || factory.submitted.get() <= self.native_prefix {
                    return Ok(());
                }
                cx.update(|cx| {
                    ensure!(
                        root.input.read(cx).text == self.close_text.0
                            && self.receiver_b.as_ref().unwrap().read(cx).text == self.close_text.1,
                        "native sibling input changed the closed logical fields"
                    );
                    Ok::<_, anyhow::Error>(())
                })?;
                root.verify_shutdown(false)?;
                self.phase = Phase::Done;
                println!(
                    "Logical-root actual GPU smoke: final GPUI background/button/glyph/image and field pixel changes; original frozen A/B commands and outputs; submitted input under a pending successor; independent native fields/focus; resize ABA and equal-physical scale invalidation; zero-size suspension; rejection recovery; visibility invalidation; exact GPU-prefix quiescence and detached close with retained submitted/unsubmitted output; native sibling input and redraw; UI HWND identities and native renderer/surface creation counts unchanged"
                );
            }
            Phase::Done => {}
        }
        Ok(())
    }

    fn published(&self, facts: WindowPresentationFacts) -> bool {
        self.last.as_ref().is_some_and(|frame| {
            facts.present_submitted_generation == Some(frame.token.frame_generation())
        })
    }

    fn submit(&mut self, root: &LogicalRoot, frame: Frame) -> Result<()> {
        let before = root.renderer.delivery.borrow().prefix;
        let current =
            pending(root).context("the original logical packet disappeared before admission")?;
        ensure!(
            frame.token.window_id() == root.window.window_id()
                && same(&current.token, &frame.token)
                && Rc::ptr_eq(&current.output, &frame.output)
                && current.count == frame.count,
            "the host replaced the original logical commands/output before submission"
        );
        ensure!(
            root.renderer.process_pending(true)?,
            "the original logical packet did not enter the GPU queue"
        );
        ensure!(
            root.renderer.delivery.borrow().prefix == before + 1
                && frame.token.status() == PlatformFrameSubmissionStatus::Submitted
                && !frame
                    .token
                    .complete(PlatformWindowPresentOutcome::Submitted),
            "the exact original logical token was not submitted exactly once"
        );
        self.last = Some(frame);
        Ok(())
    }

    fn read(&self, root: &LogicalRoot) -> Result<PixelsImage> {
        let frame = self.last.as_ref().context("no submitted logical output")?;
        let (readback, index) = {
            let mut delivery = root.renderer.delivery.borrow_mut();
            let last = delivery
                .last_output
                .as_mut()
                .context("missing original logical GPU output")?;
            ensure!(
                same(&last.token, &frame.token)
                    && Rc::ptr_eq(&last.output, &frame.output)
                    && last.count == frame.count,
                "GPUI output readback was retargeted to a different packet"
            );
            (
                last.readback
                    .take()
                    .context("original GPUI output readback was already consumed")?,
                last.index.clone(),
            )
        };
        let (send, receive) = std::sync::mpsc::sync_channel(1);
        readback
            .buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = send.send(result);
            });
        root.renderer.gpu.device.poll(wgpu::PollType::Wait {
            submission_index: Some(index),
            timeout: Some(Duration::from_secs(5)),
        })?;
        receive.recv_timeout(Duration::from_secs(1))??;
        let mapped = readback.buffer.slice(..).get_mapped_range()?;
        let bytes = mapped.to_vec();
        drop(mapped);
        readback.buffer.unmap();
        Ok(PixelsImage {
            bytes,
            stride: readback.stride as usize,
            extent: frame.output.extent,
        })
    }
}

fn pending(root: &LogicalRoot) -> Option<Frame> {
    let delivery = root.renderer.delivery.borrow();
    let packet = delivery.pending.as_ref()?;
    Some(Frame {
        token: packet.token.clone(),
        output: packet.output.clone(),
        count: packet.count,
    })
}

fn same(a: &PlatformFrameSubmission, b: &PlatformFrameSubmission) -> bool {
    a.window_id() == b.window_id()
        && a.frame_generation() == b.frame_generation()
        && a.attempt_id() == b.attempt_id()
}

fn invalid(frame: &Frame) -> Result<()> {
    ensure!(
        !frame.token.is_pending()
            && !frame
                .token
                .complete(PlatformWindowPresentOutcome::Submitted),
        "geometry/visibility/input/close left its original token submittable"
    );
    Ok(())
}

fn discard(root: &LogicalRoot, frame: &Frame) -> Result<()> {
    ensure!(
        frame.token.complete(PlatformWindowPresentOutcome::Rejected),
        "the original logical rejection was refused"
    );
    invalid(frame)?;
    root.renderer.process_pending(false)?;
    Ok(())
}

fn stage(
    root: &LogicalRoot,
    cx: &AsyncApp,
    count: u32,
    shift: Pixels,
    input: Entity<HostInput>,
) -> Result<()> {
    cx.update(|cx| {
        root.window.update(cx, |view, _, cx| {
            view.count.set(count);
            view.button_shift = shift;
            view.input = input;
            cx.notify();
        })
    })?;
    root.request_frame()
}

fn native_probe(
    cx: &AsyncApp,
    window: &WindowHandle<Root>,
    input: &Entity<HostInput>,
) -> Result<()> {
    use windows::Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::{
            Input::KeyboardAndMouse::SetFocus,
            WindowsAndMessaging::{PostMessageW, WM_CHAR},
        },
    };
    let hwnd = cx.update(|cx| {
        window.update(cx, |_, window, cx| {
            input.read(cx).focus.clone().focus(window, cx);
            window.refresh();
            let RawWindowHandle::Win32(handle) = HasWindowHandle::window_handle(window)?.as_raw()
            else {
                anyhow::bail!("native sibling lost its HWND");
            };
            Ok::<_, anyhow::Error>(HWND(handle.hwnd.get() as *mut std::ffi::c_void))
        })
    })??;
    // SAFETY: The native sibling owns this live HWND; input is posted after the App borrow ends.
    unsafe {
        let _ = SetFocus(Some(hwnd));
        PostMessageW(Some(hwnd), WM_CHAR, WPARAM('n' as usize), LPARAM(1))?;
    }
    Ok(())
}

struct PixelsImage {
    bytes: Vec<u8>,
    stride: usize,
    extent: (u32, u32),
}

impl PixelsImage {
    fn pixel(&self, x: u32, y: u32) -> Result<[u8; 4]> {
        ensure!(
            x < self.extent.0 && y < self.extent.1,
            "GPUI output pixel outside the original extent"
        );
        let start = y as usize * self.stride + x as usize * 4;
        Ok(self.bytes[start..start + 4].try_into()?)
    }

    fn expect_color(&self, x: u32, y: u32, expected: [u8; 4]) -> Result<()> {
        let pixel = self.pixel(x, y)?;
        ensure!(
            pixel.iter().zip(expected).all(|(a, b)| a.abs_diff(b) <= 2),
            "GPUI output ({x}, {y}): expected {expected:?}, got {pixel:?}"
        );
        Ok(())
    }

    fn white_pixels(&self, x: u32, y: u32, width: u32, height: u32) -> Result<usize> {
        let mut count = 0;
        for y in y..y + height {
            for x in x..x + width {
                let pixel = self.pixel(x, y)?;
                count += usize::from(
                    pixel[0] > 180 && pixel[1] > 180 && pixel[2] > 180 && pixel[3] > 200,
                );
            }
        }
        Ok(count)
    }

    fn check_ui(&self) -> Result<()> {
        self.expect_color(4, 4, [0x16, 0x23, 0x34, 255])?;
        self.expect_color(26, 102, [0x27, 0x66, 0x50, 255])?;
        ensure!(
            self.white_pixels(24, 24, 580, 32)? > 40,
            "isolated GPUI title glyph region is empty"
        );
        ensure!(
            self.white_pixels(36, 108, 180, 28)? > 20,
            "GPUI button text was not rasterized"
        );
        ensure!(
            self.white_pixels(32, 328, 540, 32)? > 40,
            "GPUI field text was not rasterized"
        );
        let image = self.pixel(80, 210)?;
        ensure!(
            image[3] == 255 && image[..3] != [0x16, 0x23, 0x34] && image[..3] != [0, 0, 0],
            "imported producer did not reach final GPUI output: {image:?}"
        );
        Ok(())
    }

    fn field_hash(&self) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for y in 320..376 {
            let row = y * self.stride;
            for byte in &self.bytes[row + 24 * 4..row + 604 * 4] {
                hash = (hash ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3);
            }
        }
        hash
    }
}
