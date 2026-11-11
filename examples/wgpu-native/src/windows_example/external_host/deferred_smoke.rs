//! Host-controlled acceptance using the ordinary renderer, original tokens and real Win32 queue.
//! No recording is submitted by the provider's draw callback. Input probes distinguish the
//! submitted receiver/button from a differently laid-out candidate with another receiver.

use super::*;
use open_gpui::{PlatformFrameSubmissionStatus, WindowPresentationFacts};

const TEXT_PROBE: &str = "dλ🦀";

#[derive(Clone, Copy)]
enum WithheldWindow {
    Minimized,
    Hidden,
}

#[derive(Clone)]
enum Phase {
    Hidden,
    HiddenProbe {
        frame: PendingFrame,
        tick: u32,
    },
    InitialShow {
        frame: PendingFrame,
    },
    CandidateA,
    HoldA {
        frame: PendingFrame,
        facts: WindowPresentationFacts,
        tick: u32,
    },
    SubmittedA {
        frame: PendingFrame,
    },
    SubmittedB {
        frame: PendingFrame,
    },
    InputCandidate,
    HeldInput {
        frame: PendingFrame,
        facts: WindowPresentationFacts,
        prefix: u64,
        tick: u32,
    },
    InputRecovery,
    SubmittedInput {
        frame: PendingFrame,
    },
    ResizeCandidate,
    Resized {
        original: PendingFrame,
        outer: RECT,
        prefix: u64,
        discarded: u64,
    },
    AbaRecovery {
        original: PendingFrame,
        middle: PendingFrame,
        prefix: u64,
    },
    SubmittedAba {
        frame: PendingFrame,
    },
    RejectCandidate,
    RejectionRecovery {
        rejected: PendingFrame,
        facts: WindowPresentationFacts,
        prefix: u64,
    },
    SubmittedRecovery {
        frame: PendingFrame,
    },
    Withheld {
        kind: WithheldWindow,
        facts: WindowPresentationFacts,
        prefix: u64,
        offers: u64,
        since: Instant,
        tick: u32,
    },
    Restored {
        kind: WithheldWindow,
    },
    SubmittedRestore {
        kind: WithheldWindow,
        frame: PendingFrame,
    },
    ResetBaseline,
    Baseline,
    CloseCandidate,
    Closing,
    Shutdown,
}

pub(super) struct Smoke {
    started: Instant,
    phase: Phase,
    initial_input: Option<Entity<HostInput>>,
    input_a: Option<Entity<HostInput>>,
    input_b: Option<Entity<HostInput>>,
    baseline: Option<smoke::Smoke>,
    close_frame: Option<PendingFrame>,
    close_prefix: u64,
    close_discarded: u64,
    producer_a: Option<[u8; 4]>,
    close_retention: Option<GpuRetention>,
}

impl Smoke {
    pub(super) fn new() -> Self {
        Self {
            started: Instant::now(),
            phase: Phase::Hidden,
            initial_input: None,
            input_a: None,
            input_b: None,
            baseline: None,
            close_frame: None,
            close_prefix: 0,
            close_discarded: 0,
            producer_a: None,
            close_retention: None,
        }
    }

    pub(super) fn auto_drain(&self) -> bool {
        matches!(self.phase, Phase::ResetBaseline | Phase::Baseline)
    }

    pub(super) fn advance(
        &mut self,
        host: &HostWindow,
        application: &ApplicationHandle,
        factory: &Factory,
        window: Option<&WindowHandle<Root>>,
        input: &RefCell<Option<Entity<HostInput>>>,
        loop_observation: &LoopObservation,
    ) -> Result<()> {
        if self.ready_to_shutdown() {
            return Ok(());
        }
        ensure!(
            self.started.elapsed() < Duration::from_secs(40),
            "deferred external-host smoke timed out during {}",
            self.phase_name()
        );
        if matches!(self.phase, Phase::Closing) {
            ensure!(
                factory.submitted.get() == self.close_prefix,
                "the host submitted the held close candidate"
            );
            if host.close_requested.get() || application.update(|cx| cx.windows().is_empty()) {
                self.phase = Phase::Shutdown;
            }
            return Ok(());
        }
        let Some(window) = window else {
            return Ok(());
        };
        let facts = presentation(application, window)?;
        match self.phase.clone() {
            Phase::Hidden => {
                let Some(frame) = factory.pending_frame() else {
                    return Ok(());
                };
                ensure!(
                    frame.submission.is_pending() && frame.recorded_count == 0,
                    "the first hidden frame was not retained as a real deferred packet"
                );
                assert_unpublished(facts)?;
                ensure!(
                    factory.submitted.get() == 0
                        && factory.observation.retained_recordings.get() == 0,
                    "recording the hidden frame submitted GPU work"
                );
                ensure!(
                    !unsafe { IsWindowVisible(host.hwnd.get()) }.as_bool(),
                    "the native HWND was shown before its original initial submission"
                );
                self.initial_input = Some(
                    input
                        .borrow()
                        .clone()
                        .context("the initial text receiver was not created")?,
                );
                ensure!(
                    factory.observation.published_button.get().is_none(),
                    "the hidden candidate published its button geometry/commit effects"
                );
                assert_hidden_pointer(host, application, window, &frame)?;
                smoke::post(host, WM_CHAR, WPARAM('!' as usize), LPARAM(1))?;
                host.post_input_fence(1)?;
                self.phase = Phase::HiddenProbe {
                    frame,
                    tick: host.timer_ticks.get(),
                };
            }
            Phase::HiddenProbe { frame, tick } => {
                if host.timer_ticks.get() == tick || host.last_input_fence.get() != 1 {
                    return Ok(());
                }
                assert_unpublished(facts)?;
                ensure!(
                    !unsafe { IsWindowVisible(host.hwnd.get()) }.as_bool(),
                    "native input revealed an unsubmitted initial window"
                );
                ensure!(
                    factory.count.get() == 0
                        && self.text(application, &self.initial_input)?.is_empty(),
                    "the hidden candidate published button or text effects"
                );
                ensure!(
                    factory.observation.published_button.get().is_none(),
                    "the hidden unsubmitted button entered the committed geometry journal"
                );
                assert_hidden_pointer(host, application, window, &frame)?;
                self.submit_original(factory, &frame)?;
                self.phase = Phase::InitialShow { frame };
            }
            Phase::InitialShow { frame } => {
                if !is_published(facts, &frame)
                    || facts.initial_presentation != WindowInitialPresentationStatus::Completed
                {
                    return Ok(());
                }
                ensure!(
                    facts.native_visible && unsafe { IsWindowVisible(host.hwnd.get()) }.as_bool(),
                    "the normal initial-presentation command did not show the submitted window"
                );
                ensure!(
                    factory.observation.drawn_count.get() == 0,
                    "unexpected initial image"
                );
                let initial = self
                    .initial_input
                    .as_ref()
                    .context("the initial receiver disappeared")?;
                let (a, b) = application.update(|cx| {
                    let focus = initial.read(cx).focus.clone();
                    let a = cx.new(|cx| {
                        let mut input = HostInput::new(factory.count.clone(), cx);
                        input.focus = focus.clone();
                        input
                    });
                    let b = cx.new(|cx| {
                        let mut input = HostInput::new(factory.count.clone(), cx);
                        input.focus = focus;
                        input
                    });
                    (a, b)
                });
                self.input_a = Some(a.clone());
                self.input_b = Some(b);
                stage(application, window, factory, 1, px(240.), a)?;
                self.phase = Phase::CandidateA;
            }
            Phase::CandidateA => {
                let Some(frame) = factory.pending_frame() else {
                    return Ok(());
                };
                if frame.recorded_count != 1 {
                    factory.drain_pending(true)?;
                    return Ok(());
                }
                ensure!(
                    frame.submission.is_pending() && factory.observation.drawn_count.get() == 0,
                    "candidate A replaced S0 before actual submission"
                );
                let b = self
                    .input_b
                    .as_ref()
                    .context("receiver B disappeared")?
                    .clone();
                stage(application, window, factory, 2, px(0.), b)?;
                self.phase = Phase::HoldA {
                    frame,
                    facts,
                    tick: host.timer_ticks.get(),
                };
            }
            Phase::HoldA {
                frame,
                facts: previous,
                tick,
            } => {
                if host.timer_ticks.get() == tick {
                    return Ok(());
                }
                assert_same_publication(previous, facts)?;
                ensure!(
                    factory.count.get() == 2 && factory.observation.drawn_count.get() == 0,
                    "the live B source changed the recorded A or submitted S0 image"
                );
                ensure!(
                    factory
                        .observation
                        .published_button
                        .get()
                        .map(|(generation, _)| generation)
                        == previous.present_submitted_generation,
                    "candidate A published button geometry before submission"
                );
                ensure!(
                    self.text(application, &self.input_a)?.is_empty()
                        && self.text(application, &self.input_b)?.is_empty(),
                    "an unsubmitted receiver received text"
                );
                self.submit_original(factory, &frame)?;
                ensure!(
                    factory.count.get() == 2 && factory.observation.drawn_count.get() == 1,
                    "submission reread the latest count instead of A's recorded producer input"
                );
                self.producer_a = Some(assert_producer_pixel(factory, &frame, [217, 89, 38, 255])?);
                self.phase = Phase::SubmittedA { frame };
            }
            Phase::SubmittedA { frame } => {
                if !is_published(facts, &frame) {
                    return Ok(());
                }
                let Some(b) = factory.pending_frame() else {
                    return Ok(());
                };
                ensure!(
                    b.recorded_count == 2
                        && b.submission.is_pending()
                        && b.submission.frame_generation() > frame.submission.frame_generation(),
                    "source B did not produce a distinct pending next candidate"
                );
                ensure!(
                    factory.observation.drawn_count.get() == 1,
                    "B was published before its actual submission"
                );
                ensure!(
                    factory
                        .observation
                        .published_button
                        .get()
                        .map(|(generation, _)| generation)
                        == Some(frame.submission.frame_generation()),
                    "candidate B published button geometry before submission"
                );
                self.submit_original(factory, &b)?;
                let pixel = assert_producer_pixel(factory, &b, [38, 140, 217, 255])?;
                ensure!(
                    Some(pixel) != self.producer_a,
                    "original A/B producer GPU pixels did not differ"
                );
                self.phase = Phase::SubmittedB { frame: b };
            }
            Phase::SubmittedB { frame } => {
                if !is_published(facts, &frame) {
                    return Ok(());
                }
                ensure!(
                    factory.observation.drawn_count.get() == 2,
                    "actual B submission did not publish B's frozen image"
                );
                stage(
                    application,
                    window,
                    factory,
                    2,
                    px(240.),
                    self.input_a
                        .as_ref()
                        .context("receiver A disappeared")?
                        .clone(),
                )?;
                self.phase = Phase::InputCandidate;
            }
            Phase::InputCandidate => {
                let Some(candidate) = factory.pending_frame() else {
                    return Ok(());
                };
                ensure!(
                    candidate.submission.is_pending() && candidate.recorded_count == 2,
                    "the input probe requires one original pending candidate"
                );
                let submitted_button = factory
                    .observation
                    .submitted_button_bounds
                    .get()
                    .context("submitted B has no button")?;
                let candidate_button = candidate
                    .button_bounds
                    .context("the input candidate has no button")?;
                ensure!(
                    !candidate_button.contains(&submitted_button.center()),
                    "the submitted/candidate button probe points overlap"
                );
                smoke::click_at(host, application, window, submitted_button)?;
                for unit in TEXT_PROBE.encode_utf16() {
                    smoke::post(host, WM_CHAR, WPARAM(unit as usize), LPARAM(1))?;
                }
                host.post_input_fence(2)?;
                self.phase = Phase::HeldInput {
                    frame: candidate,
                    facts,
                    prefix: factory.submitted.get(),
                    tick: host.timer_ticks.get(),
                };
            }
            Phase::HeldInput {
                frame,
                facts: previous,
                prefix,
                tick,
            } => {
                if host.timer_ticks.get() == tick || host.last_input_fence.get() != 2 {
                    return Ok(());
                }
                assert_same_publication(previous, facts)?;
                ensure!(
                    factory.submitted.get() == prefix && factory.count.get() == 3,
                    "the last-submitted native button stopped working while a candidate was pending"
                );
                ensure!(
                    self.text(application, &self.input_b)? == TEXT_PROBE
                        && self.text(application, &self.input_a)?.is_empty()
                        && self.text(application, &self.initial_input)?.is_empty(),
                    "native text did not stay with the last-submitted receiver B"
                );
                if frame.submission.is_pending() {
                    ensure!(
                        frame
                            .submission
                            .complete(PlatformWindowPresentOutcome::Rejected),
                        "the host could not reject the original pending input candidate"
                    );
                }
                drop_original(factory, &frame, prefix)?;
                self.phase = Phase::InputRecovery;
            }
            Phase::InputRecovery => {
                let Some(frame) = factory.pending_frame() else {
                    return Ok(());
                };
                ensure!(
                    frame.recorded_count == 3,
                    "fresh input candidate has the wrong producer input"
                );
                self.submit_original(factory, &frame)?;
                self.phase = Phase::SubmittedInput { frame };
            }
            Phase::SubmittedInput { frame } => {
                if !is_published(facts, &frame) {
                    return Ok(());
                }
                refresh(application, window)?;
                self.phase = Phase::ResizeCandidate;
            }
            Phase::ResizeCandidate => {
                let Some(original) = factory.pending_frame() else {
                    return Ok(());
                };
                ensure!(
                    original.submission.is_pending(),
                    "resize requires an unsubmitted original packet"
                );
                let mut outer = RECT::default();
                unsafe { GetWindowRect(host.hwnd.get(), &mut outer) }?;
                let prefix = factory.submitted.get();
                let discarded = factory.observation.discarded_packets.get();
                // SAFETY: Native resize and its callbacks run after every App update returns.
                unsafe {
                    SetWindowPos(
                        host.hwnd.get(),
                        None,
                        outer.left + 29,
                        outer.top + 19,
                        outer.right - outer.left + 96,
                        outer.bottom - outer.top + 64,
                        SWP_NOACTIVATE | SWP_NOZORDER,
                    )
                }?;
                ensure!(
                    host.client_extent()? != original.extent,
                    "the target resize did not change native extent"
                );
                self.phase = Phase::Resized {
                    original,
                    outer,
                    prefix,
                    discarded,
                };
            }
            Phase::Resized {
                original,
                outer,
                prefix,
                discarded,
            } => {
                ensure!(
                    !original.submission.is_pending(),
                    "native resize failed to invalidate the original token"
                );
                drop_original(factory, &original, prefix)?;
                ensure!(
                    factory.observation.discarded_packets.get() > discarded,
                    "resize retained the original unsubmitted recording"
                );
                let Some(middle) = factory.pending_frame() else {
                    return Ok(());
                };
                if middle.extent != host.client_extent()? {
                    return Ok(());
                }
                ensure!(
                    middle.submission.is_pending()
                        && middle.target_generation > original.target_generation,
                    "the resized candidate reused the original target generation"
                );
                unsafe {
                    SetWindowPos(
                        host.hwnd.get(),
                        None,
                        outer.left,
                        outer.top,
                        outer.right - outer.left,
                        outer.bottom - outer.top,
                        SWP_NOACTIVATE | SWP_NOZORDER,
                    )
                }?;
                ensure!(
                    host.client_extent()? == original.extent,
                    "the native same-size ABA did not restore the original extent"
                );
                self.phase = Phase::AbaRecovery {
                    original,
                    middle,
                    prefix,
                };
            }
            Phase::AbaRecovery {
                original,
                middle,
                prefix,
            } => {
                ensure!(
                    !middle.submission.is_pending(),
                    "the second resize left its superseded candidate pending"
                );
                drop_original(factory, &middle, prefix)?;
                ensure!(
                    !original
                        .submission
                        .complete(PlatformWindowPresentOutcome::Submitted),
                    "same-size ABA revived the original token"
                );
                let Some(frame) = factory.pending_frame() else {
                    return Ok(());
                };
                if frame.extent != original.extent {
                    return Ok(());
                }
                ensure!(
                    frame.target_generation > middle.target_generation
                        && !same_attempt(&frame.submission, &original.submission)
                        && !same_attempt(&frame.submission, &middle.submission),
                    "a superseded packet was retargeted to the restored same-sized HWND"
                );
                self.submit_original(factory, &frame)?;
                self.phase = Phase::SubmittedAba { frame };
            }
            Phase::SubmittedAba { frame } => {
                if !is_published(facts, &frame)
                    || !smoke::geometry_matches(host, application, window, factory)?
                {
                    return Ok(());
                }
                refresh(application, window)?;
                self.phase = Phase::RejectCandidate;
            }
            Phase::RejectCandidate => {
                let Some(rejected) = factory.pending_frame() else {
                    return Ok(());
                };
                let prefix = factory.submitted.get();
                ensure!(
                    rejected
                        .submission
                        .complete(PlatformWindowPresentOutcome::Rejected),
                    "explicit original-token rejection was not admitted"
                );
                ensure!(
                    rejected.submission.status() == PlatformFrameSubmissionStatus::Rejected,
                    "the original rejected token did not settle terminally"
                );
                drop_original(factory, &rejected, prefix)?;
                self.phase = Phase::RejectionRecovery {
                    rejected,
                    facts,
                    prefix,
                };
            }
            Phase::RejectionRecovery {
                rejected,
                facts: previous,
                prefix,
            } => {
                assert_same_publication(previous, facts)?;
                ensure!(
                    factory.submitted.get() == prefix,
                    "rejected work entered the GPU queue"
                );
                let Some(frame) = factory.pending_frame() else {
                    return Ok(());
                };
                ensure!(
                    !same_attempt(&frame.submission, &rejected.submission),
                    "rejection reused the original delivery attempt"
                );
                self.submit_original(factory, &frame)?;
                self.phase = Phase::SubmittedRecovery { frame };
            }
            Phase::SubmittedRecovery { frame } => {
                if !is_published(facts, &frame) {
                    return Ok(());
                }
                self.withhold(
                    WithheldWindow::Minimized,
                    host,
                    application,
                    window,
                    factory,
                    facts,
                )?;
            }
            Phase::Withheld {
                kind,
                facts: previous,
                prefix,
                offers,
                since,
                tick,
            } => {
                assert_same_publication(previous, facts)?;
                ensure!(
                    factory.submitted.get() == prefix
                        && factory.pending_frame().is_none()
                        && factory.observation.offered_frames.get() == offers,
                    "a minimized/hidden Presented window reached the provider before restore"
                );
                ensure!(
                    match kind {
                        WithheldWindow::Minimized => unsafe { IsIconic(host.hwnd.get()) }.as_bool(),
                        WithheldWindow::Hidden =>
                            !unsafe { IsWindowVisible(host.hwnd.get()) }.as_bool(),
                    },
                    "the real native withholding condition changed unexpectedly"
                );
                if since.elapsed() < Duration::from_millis(150) || host.timer_ticks.get() == tick {
                    return Ok(());
                }
                ensure!(
                    facts.initial_presentation == WindowInitialPresentationStatus::Completed,
                    "manual restore preceded the normal initial-presentation completion"
                );
                unsafe {
                    let _ = ShowWindow(
                        host.hwnd.get(),
                        match kind {
                            WithheldWindow::Minimized => SW_RESTORE,
                            WithheldWindow::Hidden => SW_SHOWNOACTIVATE,
                        },
                    );
                }
                refresh(application, window)?;
                self.phase = Phase::Restored { kind };
            }
            Phase::Restored { kind } => {
                ensure!(
                    unsafe { IsWindowVisible(host.hwnd.get()) }.as_bool()
                        && !unsafe { IsIconic(host.hwnd.get()) }.as_bool(),
                    "the host's real restore did not make presentation possible"
                );
                let Some(frame) = factory.pending_frame() else {
                    return Ok(());
                };
                self.submit_original(factory, &frame)?;
                self.phase = Phase::SubmittedRestore { kind, frame };
            }
            Phase::SubmittedRestore { kind, frame } => {
                if !is_published(facts, &frame)
                    || !smoke::geometry_matches(host, application, window, factory)?
                {
                    return Ok(());
                }
                match kind {
                    WithheldWindow::Minimized => {
                        self.withhold(
                            WithheldWindow::Hidden,
                            host,
                            application,
                            window,
                            factory,
                            facts,
                        )?;
                    }
                    WithheldWindow::Hidden => {
                        stage(
                            application,
                            window,
                            factory,
                            0,
                            px(0.),
                            self.initial_input
                                .as_ref()
                                .context("the initial receiver disappeared")?
                                .clone(),
                        )?;
                        self.phase = Phase::ResetBaseline;
                    }
                }
            }
            Phase::ResetBaseline => {
                if factory.observation.drawn_count.get() != 0
                    || facts.present_submitted_generation
                        != factory
                            .observation
                            .submitted_token
                            .borrow()
                            .as_ref()
                            .map(PlatformFrameSubmission::frame_generation)
                {
                    return Ok(());
                }
                self.baseline = Some(smoke::Smoke::new());
                self.phase = Phase::Baseline;
            }
            Phase::Baseline => {
                let baseline = self
                    .baseline
                    .as_mut()
                    .context("the synchronous smoke trajectory was not started")?;
                baseline.advance(
                    host,
                    application,
                    factory,
                    Some(window),
                    input,
                    loop_observation,
                )?;
                if baseline.ready_to_shutdown() {
                    let retained = factory.delivery.borrow().retention();
                    ensure!(
                        retained.completed < retained.prefix && retained.original_retained,
                        "close staging requires an original recording with completion not yet observed"
                    );
                    factory
                        .observation
                        .closing_prefix
                        .set(Some(retained.prefix));
                    refresh(application, window)?;
                    self.phase = Phase::CloseCandidate;
                }
            }
            Phase::CloseCandidate => {
                let Some(frame) = factory.pending_frame() else {
                    return Ok(());
                };
                ensure!(
                    frame.submission.is_pending(),
                    "close requires a real unsubmitted candidate"
                );
                self.close_prefix = factory.submitted.get();
                let retained = factory.delivery.borrow().retention();
                ensure!(
                    Some(retained.prefix) == factory.observation.closing_prefix.get()
                        && retained.prefix == self.close_prefix
                        && retained.original_retained,
                    "close lost the original submitted recording"
                );
                self.close_retention = Some(retained);
                self.close_discarded = factory.observation.discarded_packets.get();
                self.close_frame = Some(frame);
                smoke::post(host, WM_CLOSE, WPARAM(0), LPARAM(0))?;
                self.phase = Phase::Closing;
            }
            Phase::Closing => {
                unreachable!("closing is observed before querying the retired Window")
            }
            Phase::Shutdown => {}
        }
        Ok(())
    }

    fn text(
        &self,
        application: &ApplicationHandle,
        input: &Option<Entity<HostInput>>,
    ) -> Result<String> {
        let input = input
            .as_ref()
            .context("the text probe receiver disappeared")?;
        Ok(application.update(|cx| input.read(cx).text.clone()))
    }

    fn withhold(
        &mut self,
        kind: WithheldWindow,
        host: &HostWindow,
        application: &ApplicationHandle,
        window: &WindowHandle<Root>,
        factory: &Factory,
        facts: WindowPresentationFacts,
    ) -> Result<()> {
        ensure!(
            facts.initial_presentation == WindowInitialPresentationStatus::Completed,
            "manual visibility controls require the original completed initial presentation"
        );
        let prefix = factory.submitted.get();
        if let Some(frame) = factory.pending_frame() {
            if frame.submission.is_pending() {
                ensure!(
                    frame
                        .submission
                        .complete(PlatformWindowPresentOutcome::Rejected),
                    "discarding residual work before native withholding was refused"
                );
            }
            drop_original(factory, &frame, prefix)?;
        }
        unsafe {
            let _ = ShowWindow(
                host.hwnd.get(),
                match kind {
                    WithheldWindow::Minimized => SW_MINIMIZE,
                    WithheldWindow::Hidden => SW_HIDE,
                },
            );
        }
        let offers = factory.observation.offered_frames.get();
        refresh(application, window)?;
        self.phase = Phase::Withheld {
            kind,
            facts,
            prefix,
            offers,
            since: Instant::now(),
            tick: host.timer_ticks.get(),
        };
        Ok(())
    }

    fn submit_original(&self, factory: &Factory, original: &PendingFrame) -> Result<()> {
        let pending = factory
            .pending_frame()
            .context("the original deferred packet disappeared")?;
        ensure!(
            same_attempt(&pending.submission, &original.submission)
                && pending.submission.is_pending()
                && pending.recorded_count == original.recorded_count
                && pending.extent == original.extent
                && pending.target_generation == original.target_generation,
            "host drain replaced or mutated the original recorded packet"
        );
        let before = factory.submitted.get();
        ensure!(
            factory.drain_pending(true)?,
            "the original packet did not enter the actual GPU queue"
        );
        ensure!(
            factory.submitted.get() == before + 1
                && factory.observation.drawn_count.get() == original.recorded_count
                && factory.observation.drawn_extent.get() == original.extent,
            "submission did not retain the original count, extent and queue prefix"
        );
        let token = factory.observation.submitted_token.borrow();
        ensure!(
            token
                .as_ref()
                .is_some_and(|token| same_attempt(token, &original.submission))
                && original.submission.status() == PlatformFrameSubmissionStatus::Submitted
                && !original
                    .submission
                    .complete(PlatformWindowPresentOutcome::Submitted),
            "actual submission did not complete exactly the original token once"
        );
        Ok(())
    }

    pub(super) fn ready_to_shutdown(&self) -> bool {
        matches!(self.phase, Phase::Shutdown)
    }

    pub(super) fn verify_detached(&self, factory: &Factory) -> Result<()> {
        ensure!(
            self.ready_to_shutdown(),
            "deferred smoke closed before acceptance completed"
        );
        self.baseline
            .as_ref()
            .context("the existing smoke assertions were not exercised")?
            .verify_detached(factory)?;
        let frame = self
            .close_frame
            .as_ref()
            .context("close did not retain an original unsubmitted candidate")?;
        ensure!(
            !frame.submission.is_pending()
                && !frame
                    .submission
                    .complete(PlatformWindowPresentOutcome::Submitted)
                && factory.pending_frame().is_none()
                && factory.observation.discarded_packets.get() > self.close_discarded,
            "shutdown did not drop and terminally invalidate the original unsubmitted recording"
        );
        ensure!(
            factory.submitted.get() == self.close_prefix
                && factory.observation.quiesced_prefix.get() == Some(self.close_prefix)
                && factory.observation.completed_prefix() >= self.close_prefix,
            "close submitted the held candidate or lost the original GPU completion prefix"
        );
        let entry = factory
            .observation
            .quiesce_entry
            .get()
            .context("missing quiesce entry custody")?;
        let waited = factory
            .observation
            .quiesce_after_wait
            .get()
            .context("missing custody after GPU wait")?;
        ensure!(
            entry.prefix == self.close_prefix
                && waited.prefix == self.close_prefix
                && entry.original_retained
                && waited.original_retained
                && entry.recordings == waited.recordings
                && waited.completed >= self.close_prefix,
            "quiescence did not retain the exact original recording through real GPU completion"
        );
        Ok(())
    }

    pub(super) fn report(&self, factory: &Factory, observation: &LoopObservation) {
        if let Some(baseline) = &self.baseline {
            baseline.report(factory, observation);
        }
        println!(
            "Original recording custody (observed callback completion): close {:?}; quiesce entry {:?}; after wait {:?}",
            self.close_retention,
            factory.observation.quiesce_entry.get(),
            factory.observation.quiesce_after_wait.get()
        );
        println!(
            "Deferred external-host smoke: hidden initial submission/show gate; frozen A after source B; \
            original A/B producer GPU pixel readbacks; last-submitted button/text during a pending candidate; \
            cancelled-token refusal; resize/same-size ABA; \
            explicit rejection and fresh recovery; minimized/hidden pre-offer and restore; \
            native close with an unsubmitted candidate; \
            original GPU prefix quiesced and typed Detached, host continued through its own WM_QUIT"
        );
    }

    fn phase_name(&self) -> &'static str {
        match self.phase {
            Phase::Hidden | Phase::HiddenProbe { .. } => {
                "hidden initial candidate and blocked effects"
            }
            Phase::InitialShow { .. } => "normal initial submission/show completion",
            Phase::CandidateA
            | Phase::HoldA { .. }
            | Phase::SubmittedA { .. }
            | Phase::SubmittedB { .. } => "frozen A while the actual source becomes B",
            Phase::InputCandidate
            | Phase::HeldInput { .. }
            | Phase::InputRecovery
            | Phase::SubmittedInput { .. } => "last-submitted native button/text routing",
            Phase::ResizeCandidate
            | Phase::Resized { .. }
            | Phase::AbaRecovery { .. }
            | Phase::SubmittedAba { .. } => "native resize and same-size ABA",
            Phase::RejectCandidate
            | Phase::RejectionRecovery { .. }
            | Phase::SubmittedRecovery { .. } => "explicit rejection and original-window recovery",
            Phase::Withheld { .. } | Phase::Restored { .. } | Phase::SubmittedRestore { .. } => {
                "real minimized/hidden pre-offer and restore"
            }
            Phase::ResetBaseline | Phase::Baseline => {
                "existing button/F6/text/idle/geometry smoke assertions"
            }
            Phase::CloseCandidate | Phase::Closing | Phase::Shutdown => {
                "close with original unsubmitted work"
            }
        }
    }
}

fn same_attempt(a: &PlatformFrameSubmission, b: &PlatformFrameSubmission) -> bool {
    a.window_id() == b.window_id()
        && a.frame_generation() == b.frame_generation()
        && a.attempt_id() == b.attempt_id()
}

fn assert_producer_pixel(
    factory: &Factory,
    frame: &PendingFrame,
    expected: [u8; 4],
) -> Result<[u8; 4]> {
    let pixel = factory.read_producer_pixel(&frame.submission)?;
    ensure!(
        pixel
            .iter()
            .zip(expected)
            .all(|(&actual, expected)| actual.abs_diff(expected) <= 1),
        "original producer GPU pixel {pixel:?} differs from expected RGBA8 {expected:?}"
    );
    Ok(pixel)
}

fn presentation(
    application: &ApplicationHandle,
    window: &WindowHandle<Root>,
) -> Result<WindowPresentationFacts> {
    application
        .update(|cx| window.update(cx, |_, window, _| window.presentation_facts()))
        .map_err(Into::into)
}

fn assert_hidden_pointer(
    host: &HostWindow,
    application: &ApplicationHandle,
    window: &WindowHandle<Root>,
    frame: &PendingFrame,
) -> Result<()> {
    use windows::Win32::{Foundation::POINT, Graphics::Gdi::ClientToScreen};

    let bounds = frame
        .button_bounds
        .context("the hidden candidate button was not laid out")?;
    let scale = application.update(|cx| window.update(cx, |_, window, _| window.scale_factor()))?;
    let center = bounds.to_device_pixels(scale).center();
    let mut point = POINT {
        x: center.x.0,
        y: center.y.0,
    };
    // SAFETY: These native queries neither dispatch an input transaction nor borrow App.
    ensure!(
        unsafe { ClientToScreen(host.hwnd.get(), &mut point) }.as_bool(),
        "reading the hidden button's native screen position"
    );
    ensure!(
        unsafe { WindowFromPoint(point) } != host.hwnd.get(),
        "the hidden unsubmitted HWND became a native pointer target"
    );
    Ok(())
}

fn is_published(facts: WindowPresentationFacts, frame: &PendingFrame) -> bool {
    facts.frame_accepted_generation == Some(frame.submission.frame_generation())
        && facts.present_submitted_generation == Some(frame.submission.frame_generation())
}

fn assert_unpublished(facts: WindowPresentationFacts) -> Result<()> {
    ensure!(
        facts.frame_accepted_generation.is_none()
            && facts.present_submitted_generation.is_none()
            && facts.non_empty_presented_generation.is_none()
            && facts.initial_presentation == WindowInitialPresentationStatus::Pending
            && !facts.native_visible,
        "the hidden unsubmitted initial candidate published presentation effects: {facts:?}"
    );
    Ok(())
}

fn assert_same_publication(
    before: WindowPresentationFacts,
    after: WindowPresentationFacts,
) -> Result<()> {
    ensure!(
        before.frame_accepted_generation == after.frame_accepted_generation
            && before.present_submitted_generation == after.present_submitted_generation
            && before.non_empty_presented_generation == after.non_empty_presented_generation,
        "an unsubmitted candidate changed accepted/submitted publication: {before:?} -> {after:?}"
    );
    Ok(())
}

fn stage(
    application: &ApplicationHandle,
    window: &WindowHandle<Root>,
    factory: &Factory,
    count: u32,
    button_shift: Pixels,
    input: Entity<HostInput>,
) -> Result<()> {
    application.update(|cx| {
        window.update(cx, |root, _, cx| {
            factory.count.set(count);
            root.button_shift = button_shift;
            root.host_input = Some(input.into());
            cx.notify();
        })
    })?;
    Ok(())
}

fn refresh(application: &ApplicationHandle, window: &WindowHandle<Root>) -> Result<()> {
    application.update(|cx| window.update(cx, |_, window, _| window.refresh()))?;
    Ok(())
}

fn drop_original(factory: &Factory, original: &PendingFrame, prefix: u64) -> Result<()> {
    ensure!(
        !original.submission.is_pending()
            && !original
                .submission
                .complete(PlatformWindowPresentOutcome::Submitted),
        "a cancelled or rejected original token admitted stale completion"
    );
    if factory
        .pending_frame()
        .is_some_and(|pending| same_attempt(&pending.submission, &original.submission))
    {
        ensure!(
            !factory.drain_pending(true)?,
            "a terminal original packet entered the GPU queue"
        );
    }
    ensure!(
        factory.submitted.get() == prefix,
        "discarding an original packet submitted GPU work"
    );
    Ok(())
}
