use crate::{
    DockDropDelivery, DockSpaceId, DockViewportIdentity, DockViewportResolvedDropRoute,
    interaction::DockRuntimeDragSession,
};
use open_gpui::{AnyWindowHandle, WindowId};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DockViewportRoutedDropPreview {
    identity: DockViewportIdentity,
    pub(crate) preview: crate::drop_preview::DockDropPreview,
    delivery: DockDropDelivery,
    pub(crate) payload_title: String,
}

impl DockViewportRoutedDropPreview {
    fn new(
        space: DockSpaceId,
        window_id: WindowId,
        preview: crate::drop_preview::DockDropPreview,
        delivery: DockDropDelivery,
        payload_title: impl Into<String>,
    ) -> Self {
        Self {
            identity: DockViewportIdentity::new(space, window_id),
            preview,
            delivery,
            payload_title: payload_title.into(),
        }
    }

    fn matches(&self, space: &DockSpaceId, window_id: WindowId) -> bool {
        self.identity.matches(space, window_id)
    }

    fn space(&self) -> &DockSpaceId {
        self.identity.space()
    }

    fn window_id(&self) -> WindowId {
        self.identity.window_id()
    }

    fn delivery(&self) -> &DockDropDelivery {
        &self.delivery
    }
}

#[derive(Debug, Default)]
pub(crate) struct DockViewportRoutedDropPreviewStore {
    current: Option<DockViewportRoutedDropPreview>,
}

impl DockViewportRoutedDropPreviewStore {
    pub(crate) fn preview_for(
        &self,
        space: &DockSpaceId,
        window_id: WindowId,
    ) -> Option<DockViewportRoutedDropPreview> {
        self.current
            .as_ref()
            .filter(|preview| preview.matches(space, window_id))
            .cloned()
    }

    #[cfg(test)]
    pub(crate) fn delivery_for_drag_session(
        &self,
        session: Option<&DockRuntimeDragSession>,
    ) -> Option<DockDropDelivery> {
        let session = session?;
        let preview = self.current.as_ref()?;
        let delivery = preview.delivery();
        if delivery.drag_session_id() != Some(session.id()) {
            return None;
        }
        Some(delivery.clone())
    }

    pub(crate) fn update(
        &mut self,
        resolution: &DockViewportResolvedDropRoute,
        payload_title: impl Into<String>,
        mut window_for_space: impl FnMut(&DockSpaceId) -> Option<AnyWindowHandle>,
    ) -> (bool, Vec<AnyWindowHandle>) {
        let payload_title = payload_title.into();
        let next = match resolution.route() {
            crate::DockViewportDropRoute::KnownViewport { .. } => {
                routed_drop_preview_from_delivery(resolution.delivery(), payload_title)
            }
            crate::DockViewportDropRoute::Local { .. }
            | crate::DockViewportDropRoute::TearOff
            | crate::DockViewportDropRoute::Unavailable
            | crate::DockViewportDropRoute::Rejected(_) => None,
        };
        self.replace(next, &mut window_for_space)
    }

    pub(crate) fn clear(
        &mut self,
        mut window_for_space: impl FnMut(&DockSpaceId) -> Option<AnyWindowHandle>,
    ) -> (bool, Vec<AnyWindowHandle>) {
        self.replace(None, &mut window_for_space)
    }

    pub(crate) fn clear_if_window_matches(
        &mut self,
        window_id: WindowId,
        mut window_for_space: impl FnMut(&DockSpaceId) -> Option<AnyWindowHandle>,
    ) -> (bool, Vec<AnyWindowHandle>) {
        if self
            .current
            .as_ref()
            .is_some_and(|preview| preview.window_id() == window_id)
        {
            self.replace(None, &mut window_for_space)
        } else {
            (false, Vec::new())
        }
    }

    pub(crate) fn clear_for_drag_session(
        &mut self,
        session: Option<&DockRuntimeDragSession>,
        mut window_for_space: impl FnMut(&DockSpaceId) -> Option<AnyWindowHandle>,
    ) -> (bool, Vec<AnyWindowHandle>) {
        let Some(session) = session else {
            return (false, Vec::new());
        };
        if self
            .current
            .as_ref()
            .is_some_and(|preview| preview.delivery().drag_session_id() == Some(session.id()))
        {
            self.replace(None, &mut window_for_space)
        } else {
            (false, Vec::new())
        }
    }

    fn replace(
        &mut self,
        next: Option<DockViewportRoutedDropPreview>,
        window_for_space: &mut impl FnMut(&DockSpaceId) -> Option<AnyWindowHandle>,
    ) -> (bool, Vec<AnyWindowHandle>) {
        if self.current == next {
            return (false, Vec::new());
        }

        let mut windows = Vec::new();
        if let Some(current) = self.current.as_ref() {
            push_unique_window(&mut windows, window_for_space(current.space()));
        }
        if let Some(next) = next.as_ref() {
            push_unique_window(&mut windows, window_for_space(next.space()));
        }
        self.current = next;
        (true, windows)
    }
}

fn routed_drop_preview_from_delivery(
    delivery: &DockDropDelivery,
    payload_title: String,
) -> Option<DockViewportRoutedDropPreview> {
    let (space, window_id, resolved) = delivery.routed_preview_target()?;
    Some(DockViewportRoutedDropPreview::new(
        space.clone(),
        window_id,
        crate::drop_preview::DockDropPreview::from_resolved_target(resolved)?,
        delivery.clone(),
        payload_title,
    ))
}

fn push_unique_window(windows: &mut Vec<AnyWindowHandle>, window: Option<AnyWindowHandle>) {
    let Some(window) = window else {
        return;
    };
    if windows
        .iter()
        .any(|existing| existing.window_id() == window.window_id())
    {
        return;
    }
    windows.push(window);
}
