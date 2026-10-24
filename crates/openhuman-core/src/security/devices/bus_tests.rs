use super::*;
use crate::security::devices::lock_unpoisoned;
use std::sync::{Mutex, PoisonError};

#[test]
fn poisoned_device_state_lock_is_recovered_for_the_next_frame() {
    let state = Mutex::new(vec!["paired"]);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = state.lock().unwrap_or_else(PoisonError::into_inner);
        panic!("simulated frame panic");
    }));
    assert!(panic.is_err());

    assert_eq!(&*lock_unpoisoned(&state), &["paired"]);
}
