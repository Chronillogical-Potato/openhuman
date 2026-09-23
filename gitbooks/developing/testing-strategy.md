---
description: How OpenHuman tests its product - Vitest, cargo test, WDIO E2E. Where each test goes.
icon: vial
---

# Testing Strategy

How OpenHuman tests its product. Source of truth for "where does my test go?". Companion to [`TEST-COVERAGE-MATRIX.md`](../../docs/TEST-COVERAGE-MATRIX.md).

---

## Layers

| Layer                | Where it lives                                                                                                                                        | What it tests                                                                                                                                   | Driver                                                                                                        |
| -------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| **Rust unit**        | Sibling `<module>_tests.rs` files beside the module under `crates/openhuman-core/src/<domain>/`, or a `tests/` subdir under a domain (e.g. `crates/openhuman-core/src/channels/tests/`); inline `#[cfg(test)] mod` blocks and files named `tests.rs`/`test.rs` fail `pnpm rust:layout` | Pure domain logic, schemas, RPC handler shape, in-memory state machines                                                                         | `cargo test`                                                                                                  |
| **Rust integration** | `tests/*.rs` at repo root, each an explicit `[[test]]` target in `crates/openhuman-cli/Cargo.toml` (`autotests = false`); `tests/raw_coverage/*.rs` is globbed by `build.rs` into the single `raw_coverage_all` target | Full domain wiring with real Tokio runtime, mock external services, JSON-RPC end-to-end (`tests/json_rpc_e2e.rs`), domain × domain interactions | `pnpm test:rust` (which calls `bash scripts/test-rust-with-mock.sh`)                                          |
| **Vitest unit**      | Co-located as `*.test.ts(x)` next to source under `app/src/**`, or under `app/src/**/__tests__/`                                                      | React components, hooks, store slices, pure utilities, service-layer adapters                                                                   | `pnpm test:unit`                                                                                              |
| **WDIO E2E**         | `app/test/e2e/specs/*.spec.ts`                                                                                                                        | Full desktop flow: UI → Tauri → in-process core → JSON-RPC; user-visible behaviour                                                              | All platforms: Linux CI drives the Wry-based debug build (macOS/Windows desktop E2E is disabled until a native driver lands, #5485). See [E2E Testing](e2e-testing.md). |
| **Manual smoke**     | [`docs/RELEASE-MANUAL-SMOKE.md`](../../docs/RELEASE-MANUAL-SMOKE.md)                                                                                  | OS-level surfaces drivers cannot assert: TCC permission prompts, Gatekeeper, code signing, DMG install, OS-native toasts                        | Human at release-cut, signed off in release PR                                                                |

---

## Decision tree - where does my test go?

```text
Is the change behind the JSON-RPC boundary (in `crates/openhuman-core/src/`, `crates/openhuman-rpc/`, or `crates/openhuman-embed/`)?
├─ YES - does it cross domains or talk to external services?
│   ├─ YES → Rust integration (tests/*.rs)
│   └─ NO  → Rust unit (next to source)
└─ NO - change is in `app/`
    ├─ Is it a pure function, hook, slice, or component in isolation?
    │   └─ YES → Vitest unit (*.test.tsx co-located)
    └─ Is it user-visible AND it crosses UI ⇄ Tauri ⇄ embedded core ⇄ JSON-RPC?
        ├─ YES → WDIO E2E (app/test/e2e/specs/*.spec.ts)
        └─ Is it OS-level (TCC, Gatekeeper, install, OS toasts)?
            └─ YES → Manual smoke checklist
```

If a change touches more than one of these, write a test in **each** layer it touches. Don't substitute one for another.

---

## Failure-path requirement

Every feature leaf in the coverage matrix must have **at least one failure / edge** assertion in addition to the happy path. Examples:

- File-write tool: happy = wrote bytes; failure = path-restriction denial.
- OAuth flow: happy = token issued; edge = expired refresh token recovery.
- Memory store: happy = stored + recalled; edge = forget-then-recall returns nothing.

A spec that asserts only the happy path is incomplete.

---

## Mock policy

- **No real network in unit / integration / E2E.** Use the shared mock backend (`scripts/mock-api-core.mjs`, `scripts/mock-api-server.mjs`, `app/test/e2e/mock-server.ts`).
- Admin endpoints for tests: `GET /__admin/health`, `POST /__admin/reset`, `POST /__admin/behavior`, `GET /__admin/requests`.
- **External services** (Telegram, Slack, Gmail, Notion, Ollama, OpenAI, etc.) are stubbed at the mock backend level; tests assert the request shape via `getRequestLog()`.
- The only acceptable exception is a documented release-cut manual smoke step.

---

## Determinism rules

- No wall-clock waits, use `waitForApp`, `waitForAppReady`, `waitForWebView` helpers, or explicit element-readiness predicates.
- No shared filesystem state, every E2E spec runs inside an isolated `OPENHUMAN_WORKSPACE` (created/cleaned by `app/scripts/e2e-run-spec.sh`).
- No order-dependent specs, each spec must pass when run alone.
- No reliance on absolute coordinates or animation timing.
- Prefer synthesizing keyboard input via `browser.execute(...)` over `browser.keys()` (see `command-palette.spec.ts` for the pattern).

---

## What the existing harness gives you

- **Mock backend bootstrapping**: `startMockServer` / `stopMockServer` in `app/test/e2e/mock-server.ts`.
- **Auth shortcut**: `triggerAuthDeepLink` / `triggerAuthDeepLinkBypass` in `helpers/deep-link-helpers.ts` skips real OAuth.
- **Element helpers**: `clickNativeButton`, `waitForWebView`, `clickToggle` in `helpers/element-helpers.ts`, use these instead of raw `XCUIElementType*` selectors.
- **Shared flows**: `completeOnboardingIfVisible`, `navigateViaHash`, `navigateToSkills`, `walkOnboarding` in `helpers/shared-flows.ts`.
- **Core RPC from spec**: `callOpenhumanRpc` in `helpers/core-rpc.ts`, drives the embedded core directly when a UI step would be brittle.
- **Platform guards**: `isTauriDriver`, `isMac2`, `supportsExecuteScript` in `helpers/platform.ts` (the first two are legacy shims — everything runs on the Appium Chromium driver now).
- **Artifact capture on failure**: `captureFailureArtifacts` runs from `wdio.conf.ts`, screenshots + DOM dumps land under `app/test/e2e/artifacts/`.

---

## Naming + structure conventions

- WDIO specs: `<feature-area>-flow.spec.ts` for end-to-end product flows; `<feature>.spec.ts` for narrower surfaces.
- Vitest co-location: prefer `Component.tsx` + `Component.test.tsx` siblings; use `__tests__/` only when grouping multiple related tests.
- Rust integration tests: snake_case file matching the surface, `<feature>_e2e.rs` for JSON-RPC-driven flows, `<feature>_integration.rs` for cross-domain.
- Each `describe` / `mod tests` block maps to a feature-list ID range, link the matrix row in a comment if the mapping is non-obvious.

---

## Pre-merge gates

Run before opening a PR. CI runs the same set, but local runs are faster:

```bash
# Rust core
cargo fmt --check
cargo check --manifest-path Cargo.toml   # covers openhuman-core, openhuman-embed, openhuman-rpc, openhuman-tui
cargo clippy --manifest-path Cargo.toml -- -D warnings
cargo test --manifest-path Cargo.toml

# Tauri shell
cargo check --manifest-path crates/openhuman-app/Cargo.toml

# Frontend
pnpm typecheck
pnpm lint
pnpm format:check
pnpm test:unit

# Rust integration with mock backend
pnpm test:rust

# E2E (slow - run when behaviour changes user-visibly)
pnpm test:e2e:build
bash app/scripts/e2e-run-spec.sh test/e2e/specs/<your-spec>.spec.ts <id>
```

---

## Not driver-automatable - manual smoke required

Some surfaces cannot be driven by WDIO / Appium because they cross OS-level trust boundaries or hardware paths. The complete checklist + sign-off block lives in [`docs/RELEASE-MANUAL-SMOKE.md`](../../docs/RELEASE-MANUAL-SMOKE.md), that file is the source of truth for what must be verified per release. Examples of what it covers:

- macOS TCC permission prompts (Accessibility, Input Monitoring, Microphone)
- Gatekeeper signature validation on first launch
- Code-sign integrity (`codesign --verify --deep --strict`)
- DMG install / drag-to-Applications flow
- Auto-update download + relaunch
- OS-native notification toasts on Linux (no display server visible to the driver beyond Xvfb)

If a feature has no automated coverage AND is not on the manual smoke list, treat it as untested, open a coverage gap.

---

## Coverage matrix as the contract

Every feature leaf in the [coverage matrix](../../docs/TEST-COVERAGE-MATRIX.md) maps to:

1. A test path or paths, **or**
2. A justified `🚫` with a manual-smoke entry.

When you add / remove / rename a feature, **update the matrix row in the same PR**. CI will guard this contract once #965 lands.

---

## A red Rust suite locally is usually not a defect

Most local Rust failures in this repo are environmental, and the environments
fail in ways that look exactly like product defects. Work down this list before
concluding anything — each step is one command, and every one of them has caught
a wrong conclusion that was already on its way into an issue.

**Every prerequisite below carries the failure you get without it.** A step with
its failure attached is self-auditing — anyone can check in thirty seconds
whether it is still true, and a stale one announces itself. A bare list of steps
cannot be audited at all, which is how it rots into folklore. *If you cannot
produce the failure for an item, it does not belong here.*

**A note that tells you the cause is not evidence of the cause.** A line in a
doc or a memory saying "these fail locally, it is provisioning" is a
*pre-supplied conclusion*. It terminates the investigation before it starts,
which is worse than no note at all — one such note sent a worker looking for a
provisioning gap when the real cause was a product defect
(`agent definition \`harness\` was not found`, openhuman#6487). Use the steps,
not the folklore.

### 1. Did anything compile?

`cargo` exits **101** for at least three states that are identical from the
shell: nothing compiled, a test asserted false, or the process aborted.

```bash
grep -c "^test result" run.log     # 0 = nothing ran
```

Zero `test result` lines means the question is *compilation or abort*, never
"which test failed". Then read the **first** error line:

- `failed to load source for dependency <x>` / `No such file or directory` →
  a submodule is not checked out.
- `fatal runtime error: stack overflow` with `signal: 6, SIGABRT` and no
  `test result` line at all → the process aborted; see step 3.

### 2. Are the submodules actually there — at every depth?

Some crates depend on submodules **nested inside other submodules**.
`openhuman-tinyhumans` is the clearest case: it needs `vendor/tinyhumans-sdk`
(top level) **and** `vendor/tinyagents/vendor/tinytools` (nested). A plain
`git submodule update --init` fetches the first and silently not the second, so
the obvious command leaves you with a compile failure that reads as a test
failure.

```bash
git submodule update --init
git -C vendor/tinyagents submodule update --init --recursive
```

*Falsified:* deinitialising `vendor/tinyagents/vendor/tinytools` and re-running
`cargo test -p openhuman-tinyhumans` gives, with **no** `test result` line:

```
error: failed to get `tinytools` as a dependency of package `openhuman-cli v0.63.31`
  failed to load source for dependency `tinytools`
  No such file or directory (os error 2)
```

**Check content, never status.** `git submodule status` prints `-` only for
*unregistered* modules, and registration happens **before** checkout — so a
mid-flight init reads as ready while every working tree is still empty.
Observed mid-`update --init`: **13 `+`, 3 ` `, zero `-`** while `du -sh vendor`
was **120K** and `ls vendor/tinychannels` was empty.

```bash
du -sh vendor                      # 120K = empty; a real checkout is GBs
ls vendor/<module> | wc -l         # 0 = empty, whatever status says
```

### 3. Is your feature set the one CI uses?

`cargo test -p openhuman --lib` on default features is a **materially smaller
product** than CI builds, and the difference changes test *outcomes*, not just
which tests exist.

The mechanism is worth understanding because it is invisible in the output: a
feature flag can gate the *registration* of a tool, a test's expected side is a
static allowlist while its actual side is computed by **introspecting the built
binary**, and so the flag silently moves the actual. Nothing is stubbed, nothing
is skipped, and the failure names a product concept —
*"agents that carry tools but whose prompt names none of them"* — with no
mention of a feature anywhere.

*Falsified, from the two runs side by side:* on `0f1ecc9d2`, `openhuman --lib`
fails **5** tests on default and **3** under CI's set; the two that differ are
pure profile artefacts
(`every_prompt_names_at_least_one_tool_it_can_call` needs `runtime-node`,
`the_withheld_block_renders_for_a_renamed_session_with_a_filter` needs
`documents`). Reproduce CI exactly before asserting anything:

```bash
RUST_MIN_STACK=67108864 cargo test -p openhuman --lib \
  --features "$(bash scripts/ci/product-features.sh)"
```

Tracked as openhuman#6512; per-profile detail in openhuman#6486.

**`RUST_MIN_STACK` is per-suite, not universal.**

*Falsified for `openhuman-tinyhumans`:* removing it and re-running still gives
`183 passed; 0 failed`. It is **not** a requirement there.

*Reported for `openhuman --lib`:* `web_chat` aborts with
`fatal runtime error: stack overflow` without it, which is why `ci-lite.yml`
exports it. Not reproduced here — treat as the reason the CI command above
carries it, and re-verify before relying on it elsewhere.

Two crates, one variable, opposite answers. That is the argument against a
single blanket recipe: do not add a prerequisite by reflex. One that is not
needed teaches contributors the docs are unreliable, and it is precisely how the
misleading note above came to exist — someone's defensively-accumulated setup,
written down as necessity, never falsified.

### 4. Only now suspect a defect — and make the silence speak

If it compiled, the submodules are present and the feature set matches CI, then
a failure may be real. The trap at this stage is a layer that *correctly*
discards detail: a sanitizer, a scrubber, a redactor. The symptom is a generic
message where a specific one was produced.

Patch the discarding line to print what it drops, run once, and read the
underlying error. On openhuman#6487 a sanitized
`hosted agent invocation was rejected by policy` was hiding
`Validation("agent definition \`harness\` was not found")` — which named the
defect outright. **Reading seven links of a chain correctly is not the same as
watching it fire.**

### Why this order

The cheap checks are first because a wrong answer early is invisible later: a
suite that never compiled reports the same exit code as one that failed, and a
plausible-but-wrong explanation gets *confirmed* rather than tested, because
confirming it succeeds. Knowing that features matter is not enough — the rule is
not applied at the moment of asserting. Running CI's literal command is.

---

## When in doubt

- Push the test as low in the layer stack as possible (Rust unit > Rust integration > Vitest > WDIO). Lower layers are faster, more deterministic, and cheaper to run.
- WDIO is for behaviours that genuinely cross UI ⇄ Tauri ⇄ embedded core ⇄ JSON-RPC. Don't drive a unit-testable concern through WDIO just because the UI exists.
- A failing happy path is a regression. A missing failure-path test is a gap. Both are bugs.
