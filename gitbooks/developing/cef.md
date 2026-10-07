---
description: >-
  Historical design notes from when OpenHuman shipped its own Chromium (CEF)
  runtime. The shell now runs on stock Tauri (Wry); kept as background only.
icon: chrome
---

# Chromium Embedded Framework (historical)

{% hint style="warning" %}
**Historical.** The desktop shell no longer ships CEF. `crates/openhuman-app/` builds on upstream Tauri's native webview runtime (Wry: WKWebView / WebView2 / WebKitGTK), and `AGENTS.md` forbids restoring CEF or CDP-scanner assumptions. The native iMessage scanner remains separate because it reads `chat.db` directly. Everything below describes the retired runtime and is kept only as design background; the file paths it cites (`crates/openhuman-app/src/webview_accounts/`, `scripts/ensure-tauri-cli.sh`, `vendor/tauri-cef`) no longer exist.
{% endhint %}

For about a year OpenHuman did not run on the platform's built-in webview. It shipped its own **Chromium Embedded Framework (CEF) runtime** through a fork of `tauri-runtime`, and that single decision was load-bearing for almost every "OpenHuman knows what's happening in your tools" feature in the product.

This page explains why CEF was in the bundle, what the codebase used it for, and what replaced each piece. Nothing described below is in the shipped app.

## Why CEF instead of a stock webview

Stock Tauri uses each platform's native webview. WKWebView on macOS, WebView2 on Windows, WebKitGTK on Linux. Those work fine for rendering the OpenHuman app itself. They have one fatal limitation for our use case: **none of them expose Chrome DevTools Protocol (CDP)**.

CDP is the load-bearing primitive. Every "watch what's happening inside Slack / WhatsApp / Telegram / Discord / Meet" feature in OpenHuman talks to those embedded apps via CDP, not via injected JavaScript. CDP gives us:

- `Target.getTargets` to discover every page and service worker.
- `IndexedDB.requestDatabaseNames` / `requestDatabase` / `requestData` to walk a third-party app's local storage.
- `DOMSnapshot.captureSnapshot` for read-only DOM inspection that doesn't trip framework reactivity.
- `Runtime.evaluate` for ephemeral one-shot reads (a single fixed JSON serializer, never a persistent bridge).
- `Page.addScriptToEvaluateOnNewDocument` for the small number of cases where we genuinely need a renderer-side shim before page JS runs.

Stock webviews give none of that, which is why CEF was vendored.

**What replaced it.** Browser automation moved out of the shell entirely: a real Chrome driven over CDP from the Rust side, through the `tinycomputer` module, as a separate process the agent either launches or attaches to. See [Browser & Computer Control](../features/native-tools/browser-and-computer.md). The scanners that walked a third-party app's IndexedDB are gone; the one that survived, iMessage, never needed a browser because it reads `chat.db` directly.

The vendored runtime lived at `crates/openhuman-app/vendor/tauri-cef/` (forked from the upstream `tauri-cef` branch onto `tinyhumansai/tauri-cef:feat/cef-notification-intercept`, currently CEF 146.4.1). Every Tauri crate is patched at `crates/openhuman-app/Cargo.toml` via `[patch.crates-io]` to point at this fork. The vendored `cargo-tauri` CLI bundles Chromium correctly into `Contents/Frameworks/`; stock `@tauri-apps/cli` produces a broken bundle that panics in `cef::library_loader::LibraryLoader::new`. `scripts/ensure-tauri-cli.sh` (removed) reinstalled the vendored CLI whenever the fork is newer than the installed binary.

## What CEF was used for

### Embedded third-party webviews

Every connected provider that runs as a hosted web app gets its own child CEF webview:

- WhatsApp Web
- Telegram Web
- Slack
- Discord
- Google Meet
- LinkedIn
- Gmail
- Zoom
- WeChat
- Google Messages
- browserscan

Per-account storage is isolated to `{app_local_data_dir}/webview_accounts/{id}/`. Two Slack workspaces, two browser profiles. Code (removed): `crates/openhuman-app/src/webview_accounts/mod.rs`.

### CDP-driven scanners

Each provider had a **scanner module** in `crates/openhuman-app/src/` (all removed except `imessage_scanner/`). Every scanner holds a long-lived WebSocket to CEF's `--remote-debugging-port=19222` and ticks on a fixed schedule:

| Scanner             | Cadence                         | What it does                                                         |
| ------------------- | ------------------------------- | -------------------------------------------------------------------- |
| `whatsapp_scanner`  | 2s DOM tick + 30s full IDB walk | Reads message stores, pulls media metadata                           |
| `telegram_scanner`  | Same                            | Plus QR-login hand-off to native Telegram Desktop                    |
| `slack_scanner`     | 30s IDB walk                    | Pure IDB - no DOM scrape needed                                      |
| `discord_scanner`   | Periodic                        | Channel + DM state via CDP                                           |
| `meet_scanner`      | Periodic                        | Live captions + participant state during calls                       |
| `wechat_scanner`    | Periodic                        | WeChat Web chat list + active conversation DOM scrape via CDP        |
| `gmessages_scanner` | Periodic                        | Google Messages Web read-only IndexedDB walk                         |
| `imessage_scanner`  | Periodic                        | **No webview.** Reads `~/Library/Messages/chat.db` directly on macOS |

Each scan emitted `webview:event` payloads from the shell, so scanning continued whether the UI window was open or backgrounded. (The native iMessage scanner's old `openhuman.memory_doc_ingest` RPC call no longer exists in the core.)

### Google Meet mascot camera

The flashiest CEF trick, and the one that went with it. The Meet agent did not just _attend_ a meeting, it **broadcast** itself as a camera. That worked because CEF let us:

1. Inject a tiny bridge (`camera_bridge.js`) via `Page.addScriptToEvaluateOnNewDocument` before any Meet code runs.
2. Override `navigator.mediaDevices.getUserMedia` so it returns a `MediaStream` from a hidden 640×480 canvas instead of a real camera.
3. Render the mascot SVG on that canvas, swapping mood states (idle, thinking, talking) via `window.__openhumanSetMood(...)` driven from Rust over CDP.

There's also a build-time path that rasterizes the mascot SVG to Y4M and uses CEF's native `--use-file-for-fake-video-capture` flag, a fully native fake-camera source with no JS at all.

Code (removed): `crates/openhuman-app/src/meet_video/`.

### Native notification interception

The fork at `feat/cef-notification-intercept` adds renderer-side shims for `Notification.permission`, `Notification.requestPermission()`, and `navigator.permissions.query({name: "notifications"})`. These now install in the real `tauri-runtime-cef` path on every runtime code path, so when Slack checks if it can show notifications, the answer is consistent with what CEF's permission callbacks already granted.

This was the bulk of the (since removed) `docs/TAURI_CEF_FINDINGS_AND_CHANGES.md`. It's why Slack stops asking the same permission five times in a session.

## The "no new JS injection" rule

The rule is documented in [`CLAUDE.md`](https://github.com/tinyhumansai/openhuman/blob/main/AGENTS.md): **migrated providers load with zero injected JavaScript**. All scraping happens natively over CDP from the scanner side.

This matters because anything host-controlled that runs inside a third-party origin is an attack-surface liability. A persistent JS bridge inside Slack is one Slack update away from breaking, and one mistake away from leaking the bridge to attacker-controlled JS. CDP from outside the renderer is strictly better.

| Provider    | Migrated?     | What loads at startup            |
| ----------- | ------------- | -------------------------------- |
| WhatsApp    | Yes           | Zero JS                          |
| Telegram    | Yes           | Zero JS                          |
| Slack       | Yes           | Zero JS                          |
| Discord     | Yes           | Zero JS                          |
| browserscan | Yes           | Zero JS                          |
| Gmail       | grandfathered | Legacy `runtime.js` bridge       |
| LinkedIn    | grandfathered | Legacy `LINKEDIN_RECIPE_JS`      |
| Google Meet | grandfathered | Camera + audio + caption bridges |

Legacy injection should shrink, never grow. New providers go straight onto the CDP-only path.

## Removed sections

This page used to carry a CEF prewarm note, Windows and Linux startup-triage
runbooks, a plugin-audit table, and a "where this could evolve" design section.
They are dropped rather than kept: the runbooks told a maintainer to collect
logs and set environment variables that no longer exist, and the design section
has been overtaken. The one idea in it that did happen, browser automation as a
first-class agent tool, is
[Browser & Computer Control](../features/native-tools/browser-and-computer.md).

## See also

- [`AGENTS.md`](https://github.com/tinyhumansai/openhuman/blob/main/AGENTS.md): the canonical rule that no JavaScript injection is added to child webviews, and that CEF and CDP-scanner assumptions are not restored.
