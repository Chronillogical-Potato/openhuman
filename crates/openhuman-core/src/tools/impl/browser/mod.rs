//! Browser tools backed by the loadable TinyBrowser module.

#[allow(clippy::module_inception)]
#[cfg(feature = "modules")]
mod browser;
#[cfg(feature = "modules")]
mod browser_open;
mod image_info;
mod security;

#[cfg(feature = "modules")]
pub use browser::BrowserTool;
#[cfg(feature = "modules")]
pub use browser_open::BrowserOpenTool;
pub use image_info::ImageInfoTool;
