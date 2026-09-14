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
mod tests {
    use super::*;

    #[test]
    fn remote_navigation_is_external_but_app_origins_are_preserved() {
        let dev = Url::parse("http://localhost:1420").unwrap();
        for href in [
            "https://github.com/example/repo/pull/1",
            "http://localhost:8000",
            "https://tauri.localhost.example.com",
        ] {
            assert!(
                is_external(&Url::parse(href).unwrap(), Some(&dev)),
                "{href}"
            );
        }
        for href in [
            "tauri://localhost/#/chat",
            "http://tauri.localhost/#/chat",
            "https://tauri.localhost/#/chat",
            "http://localhost:1420/#/chat",
        ] {
            assert!(
                !is_external(&Url::parse(href).unwrap(), Some(&dev)),
                "{href}"
            );
        }
        assert!(is_external(&dev, None));
    }
}
