//! Loadable native modules, as far as a host configures them.

/// Point module installs at artifacts bundled with the host (an app's
/// resources directory) instead of downloading them. Set once, before boot;
/// `Err` returns the path when one was already set.
pub use openhuman_core::modules::ops::set_bundled_releases_dir;

/// The browser-control module.
pub mod browser {
    /// Its registry id, for a host that checks whether it is installed.
    pub use openhuman_core::modules::browser::MODULE_ID;
}
