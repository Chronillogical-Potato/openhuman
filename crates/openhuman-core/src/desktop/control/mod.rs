//! Local opt-in desktop automation connection and agent tools.

mod confirmation;
mod ops;
mod schemas;
pub mod tools;

pub use confirmation::{confirm, pending};
pub(crate) use ops::approvals_disabled_for;
pub use ops::listener_is_loopback;
pub(crate) use ops::set_listener_is_loopback;
pub use ops::{enabled, probe, set_enabled, status, DesktopProbe, DesktopStatus};
pub use schemas::all_registered_controllers;
