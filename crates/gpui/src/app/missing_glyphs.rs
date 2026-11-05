use super::App;
use crate::{AsyncApp, MissingGlyph, Subscription};
use std::{
    cell::RefCell,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    rc::Rc,
};

type MissingGlyphCallback = Box<dyn FnMut(&[MissingGlyph], &mut App) + 'static>;

struct MissingGlyphCallbackEntry {
    registration: Rc<()>,
    callback: Option<MissingGlyphCallback>,
}

#[derive(Default)]
pub(super) struct MissingGlyphCallbackSlot {
    entry: RefCell<Option<MissingGlyphCallbackEntry>>,
}

impl MissingGlyphCallbackSlot {
    fn replace(&self, callback: MissingGlyphCallback) -> Rc<()> {
        let registration = Rc::new(());
        let previous = self.entry.borrow_mut().replace(MissingGlyphCallbackEntry {
            registration: registration.clone(),
            callback: Some(callback),
        });
        drop(previous);
        registration
    }

    fn invoke(&self, missing_glyphs: &[MissingGlyph], cx: &mut App) {
        let Some((registration, mut callback)) =
            self.entry.borrow_mut().as_mut().and_then(|entry| {
                entry
                    .callback
                    .take()
                    .map(|callback| (entry.registration.clone(), callback))
            })
        else {
            return;
        };
        let result = catch_unwind(AssertUnwindSafe(|| callback(missing_glyphs, cx)));

        let mut callback = Some(callback);
        {
            let mut entry = self.entry.borrow_mut();
            if let Some(entry) = entry
                .as_mut()
                .filter(|entry| Rc::ptr_eq(&entry.registration, &registration))
            {
                std::mem::swap(&mut entry.callback, &mut callback);
            }
        }
        drop(callback);

        if let Err(payload) = result {
            resume_unwind(payload);
        }
    }

    fn remove(&self, registration: &Rc<()>) -> Option<MissingGlyphCallbackEntry> {
        let mut entry = self.entry.borrow_mut();
        if entry
            .as_ref()
            .is_some_and(|entry| Rc::ptr_eq(&entry.registration, registration))
        {
            entry.take()
        } else {
            None
        }
    }
}

async fn deliver_when_app_available(
    cx: &AsyncApp,
    callback: &MissingGlyphCallbackSlot,
    missing_glyphs: &[MissingGlyph],
    batch_generation: usize,
) {
    loop {
        let Some(app) = cx.app.upgrade() else {
            return;
        };
        let borrow_released = match app.try_borrow_mut() {
            Ok(mut app) => {
                app.update(|cx| {
                    if !cx.quitting
                        && cx.text_system().missing_glyph_generation() == batch_generation
                    {
                        callback.invoke(missing_glyphs, cx);
                    }
                });
                return;
            }
            Err(_) => app.wait_for_app_borrow_release(),
        };
        drop(app);
        if borrow_released.await.is_err() {
            return;
        }
    }
}

impl App {
    /// Invokes a callback with grapheme clusters that exhausted font fallback.
    ///
    /// Registering a callback replaces the previous callback and enables missing-glyph
    /// detection. The callback runs on the foreground executor after shaping has
    /// released its internal locks. Queued reports are delivered to the current callback.
    /// Dropping the current subscription disables detection and resets queued reports;
    /// dropping a replaced subscription leaves the current callback active.
    /// Delivery waits for an active App borrow to end and discards reports invalidated
    /// by a subscription reset or successful font installation before dispatch.
    ///
    /// Reports are queued without blocking shaping, then deduplicated before delivery.
    /// The bounded queue can drop reports on overflow. Dropped reports may be reported
    /// again when the text is reshaped; no retry is scheduled automatically.
    pub fn on_missing_glyphs(
        &self,
        callback: impl FnMut(&[MissingGlyph], &mut App) + 'static,
    ) -> Subscription {
        let registration = self.missing_glyph_callback.replace(Box::new(callback));

        if let Some(mut receiver) = self.text_system.take_missing_glyph_receiver() {
            let callback = self.missing_glyph_callback.clone();
            let task = self.spawn(async move |cx| {
                while let Ok(missing_glyphs) = receiver.recv().await {
                    let batch_generation = receiver.generation();
                    deliver_when_app_available(cx, &callback, &missing_glyphs, batch_generation)
                        .await;
                }
            });
            let previous_task = self.missing_glyph_task.replace(Some(task));
            drop(previous_task);
        }
        self.text_system.enable_missing_glyph_reporting();

        let callback = self.missing_glyph_callback.clone();
        let text_system = self.text_system.clone();
        Subscription::new(move || {
            let removed = callback.remove(&registration);
            if removed.is_some() {
                text_system.disable_missing_glyph_reporting();
            }
            drop(removed);
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FallbackFontClass, TestAppContext};
    use std::cell::Cell;

    struct OnDrop(Option<Box<dyn FnOnce()>>);

    impl Drop for OnDrop {
        fn drop(&mut self) {
            if let Some(callback) = self.0.take() {
                callback();
            }
        }
    }

    fn missing_glyph(grapheme: &'static str) -> MissingGlyph {
        MissingGlyph::new(grapheme.into(), FallbackFontClass::Proportional)
    }

    fn report(cx: &TestAppContext, grapheme: &'static str) {
        cx.update(|cx| {
            cx.text_system()
                .report_missing_glyphs_in_test(vec![missing_glyph(grapheme)]);
        });
    }

    #[test]
    fn missing_glyph_callbacks_follow_subscription_lifetime() {
        let cx = TestAppContext::single();
        let observed = Rc::new(RefCell::new(Vec::new()));
        let subscription = cx.update(|cx| {
            let observed = observed.clone();
            cx.on_missing_glyphs(move |missing_glyphs, _| {
                observed.borrow_mut().extend_from_slice(missing_glyphs);
            })
        });
        report(&cx, "active");
        cx.run_until_parked();
        assert_eq!(observed.borrow().as_slice(), &[missing_glyph("active")]);

        let second_observed = Rc::new(RefCell::new(Vec::new()));
        let second_subscription = cx.update(|cx| {
            let second_observed = second_observed.clone();
            cx.on_missing_glyphs(move |missing_glyphs, _| {
                second_observed
                    .borrow_mut()
                    .extend_from_slice(missing_glyphs);
            })
        });
        report(&cx, "replacement");
        cx.run_until_parked();
        assert_eq!(observed.borrow().as_slice(), &[missing_glyph("active")]);
        assert_eq!(
            second_observed.borrow().as_slice(),
            &[missing_glyph("replacement")]
        );

        drop(subscription);
        report(&cx, "after old drop");
        cx.run_until_parked();
        assert_eq!(
            second_observed.borrow().as_slice(),
            &[
                missing_glyph("replacement"),
                missing_glyph("after old drop")
            ]
        );

        drop(second_subscription);
        report(&cx, "inactive");
        cx.run_until_parked();
        assert_eq!(second_observed.borrow().len(), 2);
    }

    #[test]
    fn queued_reports_are_delivered_to_the_current_callback() {
        let cx = TestAppContext::single();
        let old_calls = Rc::new(Cell::new(0));
        let first = cx.update(|cx| {
            let old_calls = old_calls.clone();
            cx.on_missing_glyphs(move |_, _| old_calls.set(old_calls.get() + 1))
        });
        report(&cx, "queued before replacement");

        let observed = Rc::new(RefCell::new(Vec::new()));
        let current = cx.update(|cx| {
            let observed = observed.clone();
            cx.on_missing_glyphs(move |missing_glyphs, _| {
                observed.borrow_mut().extend_from_slice(missing_glyphs);
            })
        });
        drop(first);
        cx.run_until_parked();
        assert_eq!(old_calls.get(), 0);
        assert_eq!(
            observed.borrow().as_slice(),
            &[missing_glyph("queued before replacement")]
        );
        drop(current);
    }

    #[test]
    fn reentrant_foreground_pumps_wait_for_the_app_borrow_and_preserve_the_receiver() {
        let cx = TestAppContext::single();
        let observed = Rc::new(RefCell::new(Vec::new()));
        let subscription = cx.update(|cx| {
            let observed = observed.clone();
            cx.on_missing_glyphs(move |missing_glyphs, _| {
                observed.borrow_mut().extend_from_slice(missing_glyphs);
            })
        });
        cx.update(|app| {
            app.text_system()
                .report_missing_glyphs_in_test(vec![missing_glyph("during app borrow")]);
            for _ in 0..2 {
                app.background_executor.run_until_parked();
                assert!(observed.borrow().is_empty());
                assert!(
                    app.missing_glyph_task
                        .borrow()
                        .as_ref()
                        .is_some_and(|task| !task.is_ready())
                );
            }
        });
        cx.run_until_parked();
        assert_eq!(
            observed.borrow().as_slice(),
            &[missing_glyph("during app borrow")]
        );

        report(&cx, "after app borrow");
        cx.run_until_parked();
        assert_eq!(
            observed.borrow().as_slice(),
            &[
                missing_glyph("during app borrow"),
                missing_glyph("after app borrow")
            ]
        );
        drop(subscription);
    }

    #[test]
    fn replacing_the_callback_while_waiting_delivers_the_ready_batch_to_its_replacement() {
        let cx = TestAppContext::single();
        let old_calls = Rc::new(Cell::new(0));
        let first = cx.update(|cx| {
            let old_calls = old_calls.clone();
            cx.on_missing_glyphs(move |_, _| old_calls.set(old_calls.get() + 1))
        });
        let observed = Rc::new(RefCell::new(Vec::new()));
        let current = cx.update(|app| {
            let generation = app.text_system().missing_glyph_generation();
            app.text_system()
                .report_missing_glyphs_in_test(vec![missing_glyph("ready before replacement")]);
            app.background_executor.run_until_parked();
            assert_eq!(old_calls.get(), 0);

            let observed = observed.clone();
            let current = app.on_missing_glyphs(move |missing_glyphs, _| {
                observed.borrow_mut().extend_from_slice(missing_glyphs);
            });
            drop(first);
            assert_eq!(app.text_system().missing_glyph_generation(), generation);
            app.background_executor.run_until_parked();
            assert_eq!(old_calls.get(), 0);
            current
        });
        assert!(observed.borrow().is_empty());
        cx.run_until_parked();
        assert_eq!(old_calls.get(), 0);
        assert_eq!(
            observed.borrow().as_slice(),
            &[missing_glyph("ready before replacement")]
        );
        drop(current);
    }

    #[test]
    fn dropping_the_current_subscription_while_waiting_discards_its_ready_batch() {
        let cx = TestAppContext::single();
        let calls = Rc::new(Cell::new(0));
        let subscription = cx.update(|cx| {
            let calls = calls.clone();
            cx.on_missing_glyphs(move |_, _| calls.set(calls.get() + 1))
        });
        cx.update(|app| {
            app.text_system()
                .report_missing_glyphs_in_test(vec![missing_glyph("ready before cancellation")]);
            app.background_executor.run_until_parked();
            assert_eq!(calls.get(), 0);
            drop(subscription);
            app.background_executor.run_until_parked();
            assert_eq!(calls.get(), 0);
        });
        cx.run_until_parked();
        assert_eq!(calls.get(), 0);
        assert!(cx.read(|cx| cx.missing_glyph_callback.entry.borrow().is_none()));
    }

    #[test]
    fn resubscribing_while_waiting_discards_the_old_ready_batch_and_receives_new_reports() {
        let cx = TestAppContext::single();
        let old_calls = Rc::new(Cell::new(0));
        let first = cx.update(|cx| {
            let old_calls = old_calls.clone();
            cx.on_missing_glyphs(move |_, _| old_calls.set(old_calls.get() + 1))
        });
        let observed = Rc::new(RefCell::new(Vec::new()));
        let current = cx.update(|app| {
            let generation = app.text_system().missing_glyph_generation();
            app.text_system()
                .report_missing_glyphs_in_test(vec![missing_glyph("obsolete ready batch")]);
            app.background_executor.run_until_parked();
            assert_eq!(old_calls.get(), 0);
            drop(first);

            let observed = observed.clone();
            let current = app.on_missing_glyphs(move |missing_glyphs, _| {
                observed.borrow_mut().extend_from_slice(missing_glyphs);
            });
            assert_ne!(app.text_system().missing_glyph_generation(), generation);
            app.text_system()
                .report_missing_glyphs_in_test(vec![missing_glyph("new subscription report")]);
            app.background_executor.run_until_parked();
            current
        });
        assert!(observed.borrow().is_empty());
        cx.run_until_parked();
        assert_eq!(old_calls.get(), 0);
        assert_eq!(
            observed.borrow().as_slice(),
            &[missing_glyph("new subscription report")]
        );
        report(&cx, "obsolete ready batch");
        cx.run_until_parked();
        assert_eq!(
            observed.borrow().as_slice(),
            &[
                missing_glyph("new subscription report"),
                missing_glyph("obsolete ready batch")
            ]
        );
        drop(current);
    }

    #[test]
    fn successful_installation_reset_while_waiting_discards_the_old_ready_batch() {
        let cx = TestAppContext::single();
        let observed = Rc::new(RefCell::new(Vec::new()));
        let subscription = cx.update(|cx| {
            let observed = observed.clone();
            cx.on_missing_glyphs(move |missing_glyphs, _| {
                observed.borrow_mut().extend_from_slice(missing_glyphs);
            })
        });
        cx.update(|app| {
            let generation = app.text_system().missing_glyph_generation();
            app.text_system()
                .report_missing_glyphs_in_test(vec![missing_glyph("before installation")]);
            app.background_executor.run_until_parked();
            assert!(observed.borrow().is_empty());

            app.text_system()
                .add_fonts(Vec::new())
                .expect("NoopTextSystem accepts installation requests");
            assert_ne!(app.text_system().missing_glyph_generation(), generation);
            app.text_system()
                .report_missing_glyphs_in_test(vec![missing_glyph("after installation")]);
            app.background_executor.run_until_parked();
            assert!(observed.borrow().is_empty());
        });
        cx.run_until_parked();
        assert_eq!(
            observed.borrow().as_slice(),
            &[missing_glyph("after installation")]
        );
        report(&cx, "before installation");
        cx.run_until_parked();
        assert_eq!(
            observed.borrow().as_slice(),
            &[
                missing_glyph("after installation"),
                missing_glyph("before installation")
            ]
        );
        drop(subscription);
    }

    #[test]
    fn terminal_app_shutdown_while_waiting_discards_the_ready_batch() {
        let cx = TestAppContext::single();
        let calls = Rc::new(Cell::new(0));
        let subscription = cx.update(|cx| {
            let calls = calls.clone();
            cx.on_missing_glyphs(move |_, _| calls.set(calls.get() + 1))
        });
        cx.update(|app| {
            app.text_system()
                .report_missing_glyphs_in_test(vec![missing_glyph("ready before shutdown")]);
            app.background_executor.run_until_parked();
            assert_eq!(calls.get(), 0);
            app.shutdown_for_native_exit_test();
        });
        assert!(cx.read(|app| app.quitting));
        cx.run_until_parked();
        assert_eq!(calls.get(), 0);
        drop(subscription);
    }

    #[test]
    fn dropping_the_current_subscription_resets_queued_and_seen_reports() {
        let cx = TestAppContext::single();
        let old_calls = Rc::new(Cell::new(0));
        let first = cx.update(|cx| {
            let old_calls = old_calls.clone();
            cx.on_missing_glyphs(move |_, _| old_calls.set(old_calls.get() + 1))
        });
        report(&cx, "seen before drop");
        cx.run_until_parked();
        assert_eq!(old_calls.get(), 1);
        report(&cx, "queued before drop");
        drop(first);

        let observed = Rc::new(RefCell::new(Vec::new()));
        let current = cx.update(|cx| {
            let observed = observed.clone();
            cx.on_missing_glyphs(move |missing_glyphs, _| {
                observed.borrow_mut().extend_from_slice(missing_glyphs);
            })
        });
        report(&cx, "seen before drop");
        cx.run_until_parked();
        assert_eq!(old_calls.get(), 1);
        assert_eq!(
            observed.borrow().as_slice(),
            &[missing_glyph("seen before drop")]
        );
        drop(current);
    }

    #[test]
    fn callback_can_replace_itself_and_drop_its_replaced_subscription() {
        let cx = TestAppContext::single();
        let original: Rc<RefCell<Option<Subscription>>> = Rc::default();
        let replacement: Rc<RefCell<Option<Subscription>>> = Rc::default();
        let original_calls = Rc::new(Cell::new(0));
        let observed = Rc::new(RefCell::new(Vec::new()));
        *original.borrow_mut() = Some(cx.update(|cx| {
            let original = original.clone();
            let replacement = replacement.clone();
            let original_calls = original_calls.clone();
            let observed = observed.clone();
            cx.on_missing_glyphs(move |_, cx| {
                original_calls.set(original_calls.get() + 1);
                let observed = observed.clone();
                *replacement.borrow_mut() = Some(cx.on_missing_glyphs(move |missing_glyphs, _| {
                    observed.borrow_mut().extend_from_slice(missing_glyphs);
                }));
                let subscription = original.borrow_mut().take();
                drop(subscription);
                cx.text_system()
                    .report_missing_glyphs_in_test(vec![missing_glyph("after replacement")]);
            })
        }));
        report(&cx, "original");
        cx.run_until_parked();
        assert!(original.borrow().is_none());
        assert_eq!(original_calls.get(), 1);
        assert_eq!(
            observed.borrow().as_slice(),
            &[missing_glyph("after replacement")]
        );
        drop(replacement.borrow_mut().take());
    }

    #[test]
    fn callback_can_drop_its_current_subscription_without_being_restored() {
        let cx = TestAppContext::single();
        let subscription: Rc<RefCell<Option<Subscription>>> = Rc::default();
        let calls = Rc::new(Cell::new(0));
        *subscription.borrow_mut() = Some(cx.update(|cx| {
            let subscription = subscription.clone();
            let calls = calls.clone();
            cx.on_missing_glyphs(move |_, cx| {
                calls.set(calls.get() + 1);
                let current = subscription.borrow_mut().take();
                drop(current);
                cx.text_system()
                    .report_missing_glyphs_in_test(vec![missing_glyph("after cancellation")]);
            })
        }));
        report(&cx, "cancel during callback");
        cx.run_until_parked();
        report(&cx, "after callback");
        cx.run_until_parked();
        assert_eq!(calls.get(), 1);
        assert!(subscription.borrow().is_none());
        assert!(cx.read(|cx| cx.missing_glyph_callback.entry.borrow().is_none()));
    }

    #[test]
    fn invoking_the_current_callback_again_during_delivery_is_inert() {
        let cx = TestAppContext::single();
        let calls = Rc::new(Cell::new(0));
        let subscription = cx.update(|cx| {
            let calls = calls.clone();
            cx.on_missing_glyphs(move |missing_glyphs, cx| {
                calls.set(calls.get() + 1);
                let slot = cx.missing_glyph_callback.clone();
                slot.invoke(missing_glyphs, cx);
            })
        });
        report(&cx, "first delivery");
        cx.run_until_parked();
        report(&cx, "second delivery");
        cx.run_until_parked();
        assert_eq!(calls.get(), 2);
        drop(subscription);
    }

    #[test]
    fn panicking_callback_slot_restores_its_state_before_resuming_unwind() {
        let cx = TestAppContext::single();
        let calls = Rc::new(Cell::new(0));
        let subscription = cx.update(|cx| {
            let calls = calls.clone();
            let mut panic_once = true;
            cx.on_missing_glyphs(move |_, _| {
                calls.set(calls.get() + 1);
                if std::mem::replace(&mut panic_once, false) {
                    panic!("injected missing-glyph callback panic");
                }
            })
        });
        let result = catch_unwind(AssertUnwindSafe(|| {
            cx.update(|cx| {
                let slot = cx.missing_glyph_callback.clone();
                slot.invoke(&[missing_glyph("panicking delivery")], cx);
            });
        }));
        let payload = result.expect_err("the callback panic must propagate");
        assert_eq!(
            payload.downcast_ref::<&str>(),
            Some(&"injected missing-glyph callback panic")
        );
        assert_eq!(calls.get(), 1);

        report(&cx, "delivery after unwind");
        cx.run_until_parked();
        assert_eq!(calls.get(), 2);
        drop(subscription);
    }

    #[test]
    fn callback_slot_preserves_replacement_during_unwind() {
        let cx = TestAppContext::single();
        let replacement: Rc<RefCell<Option<Subscription>>> = Rc::default();
        let observed = Rc::new(RefCell::new(Vec::new()));
        let first = cx.update(|cx| {
            let replacement = replacement.clone();
            let observed = observed.clone();
            cx.on_missing_glyphs(move |_, cx| {
                let observed = observed.clone();
                *replacement.borrow_mut() = Some(cx.on_missing_glyphs(move |missing_glyphs, _| {
                    observed.borrow_mut().extend_from_slice(missing_glyphs);
                }));
                panic!("injected panic after replacing missing-glyph callback");
            })
        });
        let result = catch_unwind(AssertUnwindSafe(|| {
            cx.update(|cx| {
                let slot = cx.missing_glyph_callback.clone();
                slot.invoke(&[missing_glyph("panicking delivery")], cx);
            });
        }));
        assert!(result.is_err());
        drop(first);
        report(&cx, "replacement after unwind");
        cx.run_until_parked();
        assert_eq!(
            observed.borrow().as_slice(),
            &[missing_glyph("replacement after unwind")]
        );
        drop(replacement.borrow_mut().take());
    }

    #[test]
    fn callback_slot_does_not_restore_a_subscription_dropped_during_unwind() {
        let cx = TestAppContext::single();
        let subscription: Rc<RefCell<Option<Subscription>>> = Rc::default();
        let calls = Rc::new(Cell::new(0));
        *subscription.borrow_mut() = Some(cx.update(|cx| {
            let subscription = subscription.clone();
            let calls = calls.clone();
            cx.on_missing_glyphs(move |_, _| {
                calls.set(calls.get() + 1);
                let current = subscription.borrow_mut().take();
                drop(current);
                panic!("injected panic after cancelling missing-glyph callback");
            })
        }));
        let result = catch_unwind(AssertUnwindSafe(|| {
            cx.update(|cx| {
                let slot = cx.missing_glyph_callback.clone();
                slot.invoke(&[missing_glyph("panicking delivery")], cx);
            });
        }));
        assert!(result.is_err());
        report(&cx, "after cancelled unwind");
        cx.run_until_parked();
        assert_eq!(calls.get(), 1);
        assert!(cx.read(|cx| cx.missing_glyph_callback.entry.borrow().is_none()));
    }

    #[test]
    fn replacing_a_callback_can_drop_its_subscription_from_its_destructor() {
        let cx = TestAppContext::single();
        let original: Rc<RefCell<Option<Subscription>>> = Rc::default();
        let drops = Rc::new(Cell::new(0));
        *original.borrow_mut() = Some(cx.update(|cx| {
            let original = original.clone();
            let drops = drops.clone();
            let on_drop = OnDrop(Some(Box::new(move || {
                drops.set(drops.get() + 1);
                let subscription = original.borrow_mut().take();
                drop(subscription);
            })));
            cx.on_missing_glyphs(move |_, _| {
                let _ = &on_drop;
            })
        }));

        let calls = Rc::new(Cell::new(0));
        let current = cx.update(|cx| {
            let calls = calls.clone();
            cx.on_missing_glyphs(move |_, _| calls.set(calls.get() + 1))
        });
        assert_eq!(drops.get(), 1);
        assert!(original.borrow().is_none());
        report(&cx, "after old callback destruction");
        cx.run_until_parked();
        assert_eq!(calls.get(), 1);
        drop(current);
    }

    #[test]
    fn callback_destructor_can_register_a_replacement_after_subscription_drop() {
        let cx = TestAppContext::single();
        let app = Rc::downgrade(&cx.app);
        let replacement: Rc<RefCell<Option<Subscription>>> = Rc::default();
        let calls = Rc::new(Cell::new(0));
        let first = cx.update(|cx| {
            let replacement = replacement.clone();
            let calls = calls.clone();
            let on_drop = OnDrop(Some(Box::new(move || {
                let app = app.upgrade().expect("the test still owns its App");
                *replacement.borrow_mut() = Some(
                    app.borrow_mut()
                        .update(|cx| cx.on_missing_glyphs(move |_, _| calls.set(calls.get() + 1))),
                );
            })));
            cx.on_missing_glyphs(move |_, _| {
                let _ = &on_drop;
            })
        });
        drop(first);
        report(&cx, "registered from destructor");
        cx.run_until_parked();
        assert_eq!(calls.get(), 1);
        drop(replacement.borrow_mut().take());
    }

    #[test]
    fn dropping_the_app_releases_detached_callbacks_and_pending_receiver_tasks() {
        for (has_pending_report, pump_while_borrowed) in
            [(false, false), (true, false), (true, true)]
        {
            let cx = TestAppContext::single();
            let dispatcher = cx.dispatcher.clone();
            let app = Rc::downgrade(&cx.app);
            let slot = cx.read(|cx| Rc::downgrade(&cx.missing_glyph_callback));
            let text_system = cx.read(|cx| cx.text_system().clone());
            let captured = Rc::new(());
            let callback_capture = Rc::downgrade(&captured);
            let calls = Rc::new(Cell::new(0));
            cx.update(|cx| {
                let calls = calls.clone();
                cx.on_missing_glyphs(move |_, _| {
                    let _ = &captured;
                    calls.set(calls.get() + 1);
                })
                .detach();
            });
            if has_pending_report {
                cx.update(|app| {
                    text_system
                        .report_missing_glyphs_in_test(vec![missing_glyph("pending at drop")]);
                    if pump_while_borrowed {
                        app.background_executor.run_until_parked();
                        assert_eq!(calls.get(), 0);
                    }
                });
            } else {
                cx.run_until_parked();
            }
            assert!(callback_capture.upgrade().is_some());
            drop(cx);
            dispatcher.run_until_parked();
            assert!(app.upgrade().is_none());
            assert!(slot.upgrade().is_none());
            assert!(callback_capture.upgrade().is_none());
            assert_eq!(calls.get(), 0);
            drop(text_system);
        }
    }
}
