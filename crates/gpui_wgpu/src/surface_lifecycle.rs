use open_gpui::{DevicePixels, Size, WindowPresentationShutdownTicket};
#[cfg(target_family = "wasm")]
use std::sync::Arc;
#[cfg(target_family = "wasm")]
use std::sync::atomic::AtomicBool;

/// Progress of an exact surface-presentation shutdown attempt.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WgpuSurfaceShutdownProgress {
    /// The exact ticket has claimed the renderer and GPU work is being drained.
    EnteredDraining,
    /// The exact ticket remains in flight and must be retried after more GPU progress.
    Draining,
    /// Surface-bound resources were released after the exact ticket finished draining.
    Quiesced,
    /// Another ticket owns shutdown, or the requested transition was unsafe.
    Rejected,
}

enum SurfacePresentationShutdown {
    Open,
    Draining {
        ticket: WindowPresentationShutdownTicket,
        #[cfg(target_family = "wasm")]
        web_completion: Option<Arc<AtomicBool>>,
    },
    Quiesced {
        ticket: WindowPresentationShutdownTicket,
        drain_completed: bool,
    },
}

impl Default for SurfacePresentationShutdown {
    fn default() -> Self {
        Self::Open
    }
}

impl SurfacePresentationShutdown {
    fn begin(
        &mut self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> WgpuSurfaceShutdownProgress {
        if shutdown.snapshot().quiesced() {
            return if self.is_quiesced_for(shutdown) {
                WgpuSurfaceShutdownProgress::Quiesced
            } else {
                WgpuSurfaceShutdownProgress::Rejected
            };
        }

        match self {
            Self::Open => {
                *self = Self::Draining {
                    ticket: shutdown.clone(),
                    #[cfg(target_family = "wasm")]
                    web_completion: None,
                };
                WgpuSurfaceShutdownProgress::EnteredDraining
            }
            Self::Draining { ticket, .. } if ticket.same_authority(shutdown) => {
                WgpuSurfaceShutdownProgress::Draining
            }
            Self::Quiesced { ticket, .. } if ticket.same_authority(shutdown) => {
                WgpuSurfaceShutdownProgress::Quiesced
            }
            Self::Draining { .. } | Self::Quiesced { .. } => WgpuSurfaceShutdownProgress::Rejected,
        }
    }

    fn is_active(&self) -> bool {
        !matches!(self, Self::Open)
    }

    fn is_draining_for(&self, shutdown: &WindowPresentationShutdownTicket) -> bool {
        matches!(
            self,
            Self::Draining { ticket, .. } if ticket.same_authority(shutdown)
        )
    }

    fn is_quiesced_for(&self, shutdown: &WindowPresentationShutdownTicket) -> bool {
        matches!(
            self,
            Self::Quiesced {
                ticket,
                drain_completed: true,
            } if ticket.same_authority(shutdown)
        ) && shutdown.snapshot().quiesced()
    }

    fn mark_quiesced(&mut self, shutdown: &WindowPresentationShutdownTicket) -> bool {
        let snapshot = shutdown.snapshot();
        if !self.is_draining_for(shutdown) || !snapshot.quiesced() || snapshot.native_terminal() {
            return false;
        }

        *self = Self::Quiesced {
            ticket: shutdown.clone(),
            drain_completed: true,
        };
        true
    }

    #[cfg(target_family = "wasm")]
    fn web_completion(
        &self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> Option<Arc<AtomicBool>> {
        match self {
            Self::Draining {
                ticket,
                web_completion,
            } if ticket.same_authority(shutdown) => web_completion.clone(),
            Self::Open | Self::Draining { .. } | Self::Quiesced { .. } => None,
        }
    }

    #[cfg(target_family = "wasm")]
    fn install_web_completion(
        &mut self,
        shutdown: &WindowPresentationShutdownTicket,
        completion: Arc<AtomicBool>,
    ) -> bool {
        match self {
            Self::Draining {
                ticket,
                web_completion,
            } if ticket.same_authority(shutdown) && web_completion.is_none() => {
                *web_completion = Some(completion);
                true
            }
            Self::Open | Self::Draining { .. } | Self::Quiesced { .. } => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SurfaceExtent {
    pub(super) width: u32,
    pub(super) height: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SurfaceSizeIntent {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) was_clamped: bool,
}

impl SurfaceSizeIntent {
    fn extent(self) -> Option<SurfaceExtent> {
        (self.width != 0 && self.height != 0).then_some(SurfaceExtent {
            width: self.width,
            height: self.height,
        })
    }

    fn from_size(size: Size<DevicePixels>, max_texture_size: u32) -> Self {
        let requested_width = size.width.0.max(0) as u32;
        let requested_height = size.height.0.max(0) as u32;
        let max_texture_size = max_texture_size.max(1);
        let width = requested_width.min(max_texture_size);
        let height = requested_height.min(max_texture_size);

        Self {
            width,
            height,
            was_clamped: width != requested_width || height != requested_height,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SurfaceConfigureTicket {
    intent_generation: u64,
    surface_generation: u64,
    pub(super) extent: SurfaceExtent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SurfaceRecreateTicket {
    intent_generation: u64,
    lost_surface_generation: u64,
    next_surface_generation: u64,
    pub(super) extent: SurfaceExtent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SurfaceRenderPlan {
    Deferred,
    Configure(SurfaceConfigureTicket),
    Recreate(SurfaceRecreateTicket),
    Acquire,
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SurfaceAcquireEvent {
    Success,
    Suboptimal,
    Outdated,
    Lost,
    Timeout,
    Occluded,
    Validation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum SurfaceAcquireDecision {
    UseFrame,
    Deferred,
    Recreate(SurfaceRecreateTicket),
    Rejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SurfaceOwnershipPhase {
    Attached,
    Detached,
    RecreatePending { lost_surface_generation: u64 },
    Terminal,
}

pub(super) struct WindowSurfaceRuntime {
    max_texture_size: u32,
    desired_size: SurfaceSizeIntent,
    intent_generation: u64,
    surface_generation: u64,
    configured_intent_generation: Option<u64>,
    configured_surface_generation: Option<u64>,
    reconfigure_requested: bool,
    phase: SurfaceOwnershipPhase,
    shutdown: SurfacePresentationShutdown,
}

impl WindowSurfaceRuntime {
    pub(super) fn new(
        size: Size<DevicePixels>,
        max_texture_size: u32,
    ) -> (Self, SurfaceSizeIntent) {
        let desired_size = SurfaceSizeIntent::from_size(size, max_texture_size);
        (
            Self {
                max_texture_size: max_texture_size.max(1),
                desired_size,
                intent_generation: 1,
                surface_generation: 1,
                configured_intent_generation: None,
                configured_surface_generation: None,
                reconfigure_requested: false,
                phase: SurfaceOwnershipPhase::Attached,
                shutdown: SurfacePresentationShutdown::default(),
            },
            desired_size,
        )
    }

    pub(super) fn request_resize(&mut self, size: Size<DevicePixels>) -> SurfaceSizeIntent {
        let desired_size = SurfaceSizeIntent::from_size(size, self.max_texture_size);
        if self.shutdown.is_active()
            || matches!(self.phase, SurfaceOwnershipPhase::Terminal)
            || desired_size == self.desired_size
        {
            return desired_size;
        }

        let Some(next_generation) = self.intent_generation.checked_add(1) else {
            self.reject();
            return desired_size;
        };

        self.desired_size = desired_size;
        self.intent_generation = next_generation;
        if desired_size.extent().is_none() {
            self.configured_intent_generation = None;
            self.configured_surface_generation = None;
            self.reconfigure_requested = false;
        }
        desired_size
    }

    pub(super) fn request_reconfigure(&mut self) {
        if !self.shutdown.is_active()
            && matches!(self.phase, SurfaceOwnershipPhase::Attached)
            && self.desired_size.extent().is_some()
        {
            self.reconfigure_requested = true;
        }
    }

    pub(super) fn prepare_render(&self) -> SurfaceRenderPlan {
        if matches!(self.phase, SurfaceOwnershipPhase::Terminal) {
            return SurfaceRenderPlan::Rejected;
        }
        if self.shutdown.is_active() {
            return SurfaceRenderPlan::Deferred;
        }

        let Some(extent) = self.desired_size.extent() else {
            return SurfaceRenderPlan::Deferred;
        };

        match self.phase {
            SurfaceOwnershipPhase::Attached => {
                if self.configured_intent_generation == Some(self.intent_generation)
                    && self.configured_surface_generation == Some(self.surface_generation)
                    && !self.reconfigure_requested
                {
                    SurfaceRenderPlan::Acquire
                } else {
                    SurfaceRenderPlan::Configure(SurfaceConfigureTicket {
                        intent_generation: self.intent_generation,
                        surface_generation: self.surface_generation,
                        extent,
                    })
                }
            }
            SurfaceOwnershipPhase::Detached => SurfaceRenderPlan::Deferred,
            SurfaceOwnershipPhase::RecreatePending {
                lost_surface_generation,
            } => self.surface_generation.checked_add(1).map_or(
                SurfaceRenderPlan::Rejected,
                |next_surface_generation| {
                    SurfaceRenderPlan::Recreate(SurfaceRecreateTicket {
                        intent_generation: self.intent_generation,
                        lost_surface_generation,
                        next_surface_generation,
                        extent,
                    })
                },
            ),
            SurfaceOwnershipPhase::Terminal => SurfaceRenderPlan::Rejected,
        }
    }

    pub(super) fn accept_configuration(&mut self, ticket: SurfaceConfigureTicket) -> bool {
        if !matches!(self.phase, SurfaceOwnershipPhase::Attached)
            || ticket.intent_generation != self.intent_generation
            || ticket.surface_generation != self.surface_generation
            || self.desired_size.extent() != Some(ticket.extent)
        {
            return false;
        }

        self.configured_intent_generation = Some(ticket.intent_generation);
        self.configured_surface_generation = Some(ticket.surface_generation);
        self.reconfigure_requested = false;
        true
    }

    pub(super) fn observe_acquire(&mut self, event: SurfaceAcquireEvent) -> SurfaceAcquireDecision {
        if !matches!(self.prepare_render(), SurfaceRenderPlan::Acquire) {
            return SurfaceAcquireDecision::Rejected;
        }

        match event {
            SurfaceAcquireEvent::Success => SurfaceAcquireDecision::UseFrame,
            SurfaceAcquireEvent::Suboptimal => {
                self.request_reconfigure();
                SurfaceAcquireDecision::UseFrame
            }
            SurfaceAcquireEvent::Outdated => {
                self.request_reconfigure();
                SurfaceAcquireDecision::Deferred
            }
            SurfaceAcquireEvent::Lost => {
                let lost_surface_generation = self.surface_generation;
                self.phase = SurfaceOwnershipPhase::RecreatePending {
                    lost_surface_generation,
                };
                self.configured_intent_generation = None;
                self.configured_surface_generation = None;
                self.reconfigure_requested = false;
                match self.prepare_render() {
                    SurfaceRenderPlan::Recreate(ticket) => SurfaceAcquireDecision::Recreate(ticket),
                    SurfaceRenderPlan::Deferred => SurfaceAcquireDecision::Deferred,
                    SurfaceRenderPlan::Rejected
                    | SurfaceRenderPlan::Configure(_)
                    | SurfaceRenderPlan::Acquire => SurfaceAcquireDecision::Rejected,
                }
            }
            SurfaceAcquireEvent::Timeout | SurfaceAcquireEvent::Occluded => {
                SurfaceAcquireDecision::Deferred
            }
            SurfaceAcquireEvent::Validation => {
                self.reject();
                SurfaceAcquireDecision::Rejected
            }
        }
    }

    pub(super) fn accept_recreation(&mut self, ticket: SurfaceRecreateTicket) -> bool {
        if !matches!(
            self.phase,
            SurfaceOwnershipPhase::RecreatePending {
                lost_surface_generation
            } if lost_surface_generation == ticket.lost_surface_generation
        ) || ticket.intent_generation != self.intent_generation
            || self.surface_generation.checked_add(1) != Some(ticket.next_surface_generation)
            || self.desired_size.extent() != Some(ticket.extent)
        {
            return false;
        }

        self.surface_generation = ticket.next_surface_generation;
        self.configured_intent_generation = Some(ticket.intent_generation);
        self.configured_surface_generation = Some(ticket.next_surface_generation);
        self.reconfigure_requested = false;
        self.phase = SurfaceOwnershipPhase::Attached;
        true
    }

    pub(super) fn reject_recreation(&mut self, ticket: SurfaceRecreateTicket) -> bool {
        if self.prepare_render() != SurfaceRenderPlan::Recreate(ticket) {
            return false;
        }

        self.reject();
        true
    }

    pub(super) fn detach_surface(&mut self) {
        if !matches!(self.phase, SurfaceOwnershipPhase::Terminal) {
            self.phase = SurfaceOwnershipPhase::Detached;
        }
        self.configured_intent_generation = None;
        self.configured_surface_generation = None;
        self.reconfigure_requested = false;
    }

    pub(super) fn release_surface_for_shutdown(
        &mut self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> bool {
        if !self.shutdown.is_draining_for(shutdown) {
            return false;
        }

        self.phase = SurfaceOwnershipPhase::Detached;
        self.configured_intent_generation = None;
        self.configured_surface_generation = None;
        self.reconfigure_requested = false;
        true
    }

    pub(super) fn attach_surface(&mut self, size: Size<DevicePixels>) -> Option<SurfaceSizeIntent> {
        if self.shutdown.is_active() || matches!(self.phase, SurfaceOwnershipPhase::Terminal) {
            return None;
        }

        let desired_size = self.request_resize(size);
        let Some(next_surface_generation) = self.surface_generation.checked_add(1) else {
            self.reject();
            return None;
        };
        self.surface_generation = next_surface_generation;
        self.configured_intent_generation = None;
        self.configured_surface_generation = None;
        self.reconfigure_requested = false;
        self.phase = SurfaceOwnershipPhase::Attached;
        Some(desired_size)
    }

    pub(super) fn reject(&mut self) {
        self.phase = SurfaceOwnershipPhase::Terminal;
        self.configured_intent_generation = None;
        self.configured_surface_generation = None;
        self.reconfigure_requested = false;
    }

    pub(super) fn begin_shutdown(
        &mut self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> WgpuSurfaceShutdownProgress {
        self.shutdown.begin(shutdown)
    }

    pub(super) fn shutdown_active(&self) -> bool {
        self.shutdown.is_active()
    }

    pub(super) fn is_draining_for(&self, shutdown: &WindowPresentationShutdownTicket) -> bool {
        self.shutdown.is_draining_for(shutdown)
    }

    pub(super) fn is_quiesced_for(&self, shutdown: &WindowPresentationShutdownTicket) -> bool {
        self.shutdown.is_quiesced_for(shutdown)
            && matches!(self.phase, SurfaceOwnershipPhase::Detached)
    }

    pub(super) fn mark_quiesced(&mut self, shutdown: &WindowPresentationShutdownTicket) -> bool {
        matches!(self.phase, SurfaceOwnershipPhase::Detached)
            && self.shutdown.mark_quiesced(shutdown)
    }

    pub(super) fn owner_release_is_safe(&self, has_submission: bool) -> bool {
        match &self.shutdown {
            SurfacePresentationShutdown::Open => !has_submission,
            SurfacePresentationShutdown::Draining { .. } => false,
            SurfacePresentationShutdown::Quiesced { ticket, .. } => {
                let ticket = ticket.clone();
                !has_submission && self.is_quiesced_for(&ticket)
            }
        }
    }

    #[cfg(target_family = "wasm")]
    pub(super) fn web_completion(
        &self,
        shutdown: &WindowPresentationShutdownTicket,
    ) -> Option<Arc<AtomicBool>> {
        self.shutdown.web_completion(shutdown)
    }

    #[cfg(target_family = "wasm")]
    pub(super) fn install_web_completion(
        &mut self,
        shutdown: &WindowPresentationShutdownTicket,
        completion: Arc<AtomicBool>,
    ) -> bool {
        self.shutdown.install_web_completion(shutdown, completion)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn size(width: i32, height: i32) -> Size<DevicePixels> {
        Size {
            width: DevicePixels(width),
            height: DevicePixels(height),
        }
    }

    fn configured_runtime() -> WindowSurfaceRuntime {
        let (mut runtime, initial) = WindowSurfaceRuntime::new(size(800, 600), 4096);
        assert_eq!((initial.width, initial.height), (800, 600));
        let SurfaceRenderPlan::Configure(ticket) = runtime.prepare_render() else {
            panic!("an attached non-zero surface must require initial configuration");
        };
        assert!(runtime.accept_configuration(ticket));
        assert_eq!(runtime.prepare_render(), SurfaceRenderPlan::Acquire);
        runtime
    }

    #[test]
    fn zero_extent_suspends_without_configuring_or_acquiring() {
        let (mut runtime, initial) = WindowSurfaceRuntime::new(size(0, 600), 4096);
        assert_eq!((initial.width, initial.height), (0, 600));
        assert_eq!(runtime.prepare_render(), SurfaceRenderPlan::Deferred);

        runtime.request_resize(size(800, 600));
        let SurfaceRenderPlan::Configure(ticket) = runtime.prepare_render() else {
            panic!("restoring a non-zero extent must configure at the render safe point");
        };
        assert!(runtime.accept_configuration(ticket));
        assert_eq!(runtime.prepare_render(), SurfaceRenderPlan::Acquire);

        runtime.request_resize(size(800, 0));
        assert_eq!(runtime.prepare_render(), SurfaceRenderPlan::Deferred);
    }

    #[test]
    fn resize_intents_coalesce_and_stale_configuration_cannot_commit() {
        let mut runtime = configured_runtime();

        runtime.request_resize(size(900, 700));
        let SurfaceRenderPlan::Configure(stale) = runtime.prepare_render() else {
            panic!("first resize must require configuration");
        };
        runtime.request_resize(size(1200, 900));
        assert!(!runtime.accept_configuration(stale));

        let SurfaceRenderPlan::Configure(current) = runtime.prepare_render() else {
            panic!("latest resize must replace the stale configuration intent");
        };
        assert_eq!(
            current.extent,
            SurfaceExtent {
                width: 1200,
                height: 900
            }
        );
        assert!(runtime.accept_configuration(current));
        assert_eq!(runtime.prepare_render(), SurfaceRenderPlan::Acquire);
    }

    #[test]
    fn suboptimal_frame_remains_usable_and_reconfigures_next_frame() {
        let mut runtime = configured_runtime();

        assert_eq!(
            runtime.observe_acquire(SurfaceAcquireEvent::Suboptimal),
            SurfaceAcquireDecision::UseFrame
        );
        assert!(matches!(
            runtime.prepare_render(),
            SurfaceRenderPlan::Configure(_)
        ));
    }

    #[test]
    fn lost_surface_uses_one_generation_bound_recreation_attempt() {
        let mut runtime = configured_runtime();

        let SurfaceAcquireDecision::Recreate(ticket) =
            runtime.observe_acquire(SurfaceAcquireEvent::Lost)
        else {
            panic!("surface loss must issue an exact recreation ticket");
        };
        assert_eq!(
            runtime.prepare_render(),
            SurfaceRenderPlan::Recreate(ticket)
        );
        assert!(runtime.reject_recreation(ticket));
        assert_eq!(runtime.prepare_render(), SurfaceRenderPlan::Rejected);
        assert!(!runtime.reject_recreation(ticket));
    }

    #[test]
    fn successful_recreation_advances_surface_generation() {
        let mut runtime = configured_runtime();
        let SurfaceAcquireDecision::Recreate(ticket) =
            runtime.observe_acquire(SurfaceAcquireEvent::Lost)
        else {
            panic!("surface loss must issue an exact recreation ticket");
        };

        assert!(runtime.accept_recreation(ticket));
        assert_eq!(runtime.prepare_render(), SurfaceRenderPlan::Acquire);
        assert!(!runtime.accept_recreation(ticket));
    }

    #[test]
    fn resize_after_loss_supersedes_the_old_recreation_ticket() {
        let mut runtime = configured_runtime();
        let SurfaceAcquireDecision::Recreate(stale) =
            runtime.observe_acquire(SurfaceAcquireEvent::Lost)
        else {
            panic!("surface loss must issue an exact recreation ticket");
        };

        runtime.request_resize(size(1280, 720));
        assert!(!runtime.accept_recreation(stale));
        assert!(!runtime.reject_recreation(stale));
        assert!(matches!(
            runtime.prepare_render(),
            SurfaceRenderPlan::Recreate(SurfaceRecreateTicket {
                extent: SurfaceExtent {
                    width: 1280,
                    height: 720
                },
                ..
            })
        ));
    }

    #[test]
    fn detached_surface_never_recreates_without_explicit_attachment() {
        let mut runtime = configured_runtime();
        runtime.detach_surface();
        runtime.request_resize(size(1024, 768));
        assert_eq!(runtime.prepare_render(), SurfaceRenderPlan::Deferred);

        let attached = runtime
            .attach_surface(size(1024, 768))
            .expect("a detached surface can accept an explicit replacement");
        assert_eq!((attached.width, attached.height), (1024, 768));
        assert!(matches!(
            runtime.prepare_render(),
            SurfaceRenderPlan::Configure(_)
        ));
    }

    #[test]
    fn acquisition_failures_have_deterministic_state_transitions() {
        let mut outdated = configured_runtime();
        assert_eq!(
            outdated.observe_acquire(SurfaceAcquireEvent::Outdated),
            SurfaceAcquireDecision::Deferred
        );
        assert!(matches!(
            outdated.prepare_render(),
            SurfaceRenderPlan::Configure(_)
        ));

        for event in [SurfaceAcquireEvent::Timeout, SurfaceAcquireEvent::Occluded] {
            let mut runtime = configured_runtime();
            assert_eq!(
                runtime.observe_acquire(event),
                SurfaceAcquireDecision::Deferred
            );
            assert_eq!(runtime.prepare_render(), SurfaceRenderPlan::Acquire);
        }

        let mut validation = configured_runtime();
        assert_eq!(
            validation.observe_acquire(SurfaceAcquireEvent::Validation),
            SurfaceAcquireDecision::Rejected
        );
        assert_eq!(validation.prepare_render(), SurfaceRenderPlan::Rejected);
    }

    #[test]
    fn requested_extent_is_clamped_before_configuration() {
        let (runtime, intent) = WindowSurfaceRuntime::new(size(5000, 3000), 2048);
        assert_eq!((intent.width, intent.height), (2048, 2048));
        assert!(intent.was_clamped);
        assert!(matches!(
            runtime.prepare_render(),
            SurfaceRenderPlan::Configure(SurfaceConfigureTicket {
                extent: SurfaceExtent {
                    width: 2048,
                    height: 2048
                },
                ..
            })
        ));
    }

    #[test]
    fn shutdown_requires_the_same_ticket_and_surface_release() {
        let ticket = WindowPresentationShutdownTicket::new(open_gpui::WindowId::from(7), 1);
        let same_ticket = ticket.clone();
        let same_generation_but_distinct_ticket =
            WindowPresentationShutdownTicket::new(open_gpui::WindowId::from(7), 1);
        let different_generation =
            WindowPresentationShutdownTicket::new(open_gpui::WindowId::from(7), 2);
        let mut runtime = configured_runtime();
        assert!(runtime.owner_release_is_safe(false));

        assert_eq!(
            runtime.begin_shutdown(&ticket),
            WgpuSurfaceShutdownProgress::EnteredDraining
        );
        assert!(!runtime.owner_release_is_safe(false));
        assert_eq!(
            runtime.begin_shutdown(&same_ticket),
            WgpuSurfaceShutdownProgress::Draining
        );
        assert_eq!(
            runtime.begin_shutdown(&same_generation_but_distinct_ticket),
            WgpuSurfaceShutdownProgress::Rejected
        );
        assert_eq!(
            runtime.begin_shutdown(&different_generation),
            WgpuSurfaceShutdownProgress::Rejected
        );
        assert!(runtime.is_draining_for(&same_ticket));
        assert!(!runtime.is_quiesced_for(&same_ticket));
        assert!(!runtime.mark_quiesced(&same_ticket));

        assert!(same_generation_but_distinct_ticket.acknowledge_quiesced());
        assert!(!runtime.mark_quiesced(&same_generation_but_distinct_ticket));
        assert!(runtime.is_draining_for(&same_ticket));

        assert!(ticket.acknowledge_quiesced());
        assert!(!runtime.mark_quiesced(&same_ticket));
        assert!(runtime.release_surface_for_shutdown(&same_ticket));
        assert!(runtime.mark_quiesced(&same_ticket));
        assert!(runtime.is_quiesced_for(&same_ticket));
        assert!(runtime.owner_release_is_safe(false));
        assert!(!runtime.owner_release_is_safe(true));
        assert_eq!(
            runtime.begin_shutdown(&same_ticket),
            WgpuSurfaceShutdownProgress::Quiesced
        );
    }
}
