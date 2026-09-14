//! Keep remote pages from replacing the main desktop UI.
use tauri::{plugin::TauriPlugin, Manager};
use tauri_plugin_opener::OpenerExt;
use url::Url;

fn is_external(url: &Url, dev_url: Option<&Url>) -> bool {
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }
    if url.host_str() == Some("tauri.localhost") && url.port().is_none() {
        return false;
    }
    !dev_url.is_some_and(|dev| dev.origin() == url.origin())
}

pub fn init() -> TauriPlugin<crate::AppRuntime> {
    tauri::plugin::Builder::new("external-navigation")
        .on_navigation(|webview, url| {
            let dev_url = if cfg!(debug_assertions) {
                webview.config().build.dev_url.as_ref()
            } else {
                None
            };
            if webview.label() != "main" || !is_external(url, dev_url) {
                return true;
            }
            log::debug!("[external-navigation] handing main-window navigation to OS");
            let app = webview.app_handle().clone();
            let target = url.to_string();
            tauri::async_runtime::spawn_blocking(move || {
                if app.opener().open_url(target, None::<&str>).is_err() {
                    log::warn!("[external-navigation] OS opener failed; retained main window");
                }
            });
            false
        })
        .build()
}

#[cfg(test)]
#[path = "external_navigation_tests.rs"]
mod tests;
