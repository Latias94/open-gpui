use open_gpui::WindowVisibility;

#[derive(Default)]
pub(super) struct MacWindowVisibilityObserver {
    last_visibility: Option<WindowVisibility>,
    callback: Option<Box<dyn FnMut(WindowVisibility)>>,
}

impl MacWindowVisibilityObserver {
    pub(super) fn is_registered(&self) -> bool {
        self.last_visibility.is_some()
    }

    pub(super) fn last_reported_visibility(&self) -> Option<WindowVisibility> {
        self.last_visibility
    }

    pub(super) fn set(
        &mut self,
        visibility: WindowVisibility,
        callback: Box<dyn FnMut(WindowVisibility)>,
    ) -> Option<Box<dyn FnMut(WindowVisibility)>> {
        self.last_visibility = Some(visibility);
        self.callback.replace(callback)
    }

    pub(super) fn take_transition(
        &mut self,
        visibility: WindowVisibility,
    ) -> Option<Box<dyn FnMut(WindowVisibility)>> {
        if self.last_visibility.is_none() || self.last_visibility == Some(visibility) {
            return None;
        }
        let callback = self.callback.take()?;
        self.last_visibility = Some(visibility);
        Some(callback)
    }

    pub(super) fn restore(
        &mut self,
        callback: Box<dyn FnMut(WindowVisibility)>,
    ) -> Option<Box<dyn FnMut(WindowVisibility)>> {
        if self.is_registered() && self.callback.is_none() {
            self.callback = Some(callback);
            None
        } else {
            Some(callback)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::RefCell, rc::Rc};

    fn deliver(observer: &mut MacWindowVisibilityObserver, visibility: WindowVisibility) {
        if let Some(mut callback) = observer.take_transition(visibility) {
            callback(visibility);
            drop(observer.restore(callback));
        }
    }

    #[test]
    fn registration_seeds_current_visibility_without_replaying_construction() {
        let mut observer = MacWindowVisibilityObserver::default();
        assert!(!observer.is_registered());
        assert!(observer.take_transition(WindowVisibility::Hidden).is_none());
        assert!(
            observer
                .take_transition(WindowVisibility::Visible)
                .is_none()
        );

        let transitions = Rc::new(RefCell::new(Vec::new()));
        let observed = transitions.clone();
        drop(observer.set(
            WindowVisibility::Visible,
            Box::new(move |visibility| observed.borrow_mut().push(visibility)),
        ));
        deliver(&mut observer, WindowVisibility::Visible);
        assert!(transitions.borrow().is_empty());

        deliver(&mut observer, WindowVisibility::Hidden);
        deliver(&mut observer, WindowVisibility::Hidden);
        deliver(&mut observer, WindowVisibility::Visible);
        deliver(&mut observer, WindowVisibility::Visible);
        assert_eq!(
            *transitions.borrow(),
            [WindowVisibility::Hidden, WindowVisibility::Visible],
        );
    }

    #[test]
    fn hidden_initial_visibility_reports_only_the_first_visible_transition() {
        let mut observer = MacWindowVisibilityObserver::default();
        let transitions = Rc::new(RefCell::new(Vec::new()));
        let observed = transitions.clone();
        drop(observer.set(
            WindowVisibility::Hidden,
            Box::new(move |visibility| observed.borrow_mut().push(visibility)),
        ));

        deliver(&mut observer, WindowVisibility::Hidden);
        deliver(&mut observer, WindowVisibility::Visible);
        assert_eq!(*transitions.borrow(), [WindowVisibility::Visible]);
    }

    #[test]
    fn callback_replacement_during_delivery_keeps_the_new_registration() {
        let observer = Rc::new(RefCell::new(MacWindowVisibilityObserver::default()));
        let transitions = Rc::new(RefCell::new(Vec::new()));
        let observed = transitions.clone();
        let reentrant_observer = Rc::downgrade(&observer);
        drop(observer.borrow_mut().set(
            WindowVisibility::Visible,
            Box::new(move |_| {
                let observer = reentrant_observer.upgrade().unwrap();
                let observed = observed.clone();
                let previous = observer.borrow_mut().set(
                    WindowVisibility::Hidden,
                    Box::new(move |visibility| observed.borrow_mut().push(visibility)),
                );
                drop(previous);
            }),
        ));

        let mut callback = observer
            .borrow_mut()
            .take_transition(WindowVisibility::Hidden)
            .unwrap();
        callback(WindowVisibility::Hidden);
        let superseded = observer.borrow_mut().restore(callback);
        assert!(superseded.is_some());
        drop(superseded);

        deliver(&mut observer.borrow_mut(), WindowVisibility::Visible);
        assert_eq!(*transitions.borrow(), [WindowVisibility::Visible]);
    }

    #[test]
    fn retirement_during_delivery_does_not_restore_the_callback() {
        let mut observer = MacWindowVisibilityObserver::default();
        drop(observer.set(WindowVisibility::Visible, Box::new(|_| {})));
        let callback = observer.take_transition(WindowVisibility::Hidden).unwrap();

        let retired = std::mem::take(&mut observer);
        drop(retired);
        assert!(observer.restore(callback).is_some());
        assert!(!observer.is_registered());
        assert!(
            observer
                .take_transition(WindowVisibility::Visible)
                .is_none()
        );
    }

    #[test]
    fn reentrant_observation_does_not_consume_an_undelivered_transition() {
        let mut observer = MacWindowVisibilityObserver::default();
        drop(observer.set(WindowVisibility::Visible, Box::new(|_| {})));
        let callback = observer.take_transition(WindowVisibility::Hidden).unwrap();
        assert!(
            observer
                .take_transition(WindowVisibility::Visible)
                .is_none()
        );
        drop(observer.restore(callback));
        assert!(
            observer
                .take_transition(WindowVisibility::Visible)
                .is_some()
        );
    }
}
