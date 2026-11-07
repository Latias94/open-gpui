use super::*;

#[::std::prelude::v1::test]
fn accessibility_enabled_is_independent_of_activation() {
    for force_disabled in [false, true] {
        for active in [false, true] {
            let state = Arc::new(AtomicU64::new(0));
            set_requested_active(&state, active);
            let mut a11y = A11y::new(state, force_disabled, WindowId::from(19));
            a11y.sync_active_flag();
            assert_eq!(a11y.is_enabled(), !force_disabled);
            assert_eq!(a11y.is_active(), !force_disabled && active);
        }
    }
}
