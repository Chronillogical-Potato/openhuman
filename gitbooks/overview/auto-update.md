---
description: >-
  How OpenHuman updates itself: one signed manifest, a bounded download retry,
  and a shell update that carries the core with it.
icon: arrows-rotate
---

# Auto-update

OpenHuman updates the whole app in one step. The core is linked into the desktop shell rather than shipped beside it, so there is no separate core update to think about: a shell update upgrades both.

## What you see

When a newer build is available, the app checks for it and downloads it in the background. The prompt appears once the download is ready, then moves through **ready to install**, **installing** and **restarting**. If you are already current you see **up to date**, and a failure says so rather than silently retrying forever.

Installing shuts the in-process core down first, under the same restart lock the app uses elsewhere, so replacing the bundle never races a core still holding file handles. On macOS that is what keeps a `.app` replacement clean.

## Where updates come from

A single endpoint, a manifest published with every release:

```text
https://github.com/tinyhumansai/openhuman/releases/latest/download/latest.json
```

Every artifact is signed, and the public key is compiled into the app. A build whose signature does not verify is refused. That is the whole trust story: no update server to trust, no second channel.

## Retry policy, and what is deliberately not retried

Downloads get **three attempts** (one try plus two retries) with a linear backoff of two seconds times the attempt number.

Only transient network failures are retried. Signature failures, filesystem errors and "no artifact for this target" are **not**, and that asymmetry is the point: re-downloading cannot fix a bad signature, and looping on a verification failure would both waste the user's bandwidth and mask tampering. The policy lives in `crates/openhuman-app/src/app_update.rs`, where `classify` and `is_transient_updater_err` are the two functions that decide.

## Periodic checks

The core runs the schedule, under `[update]` in `config.toml`:

| Key | Default | What it does |
| --- | --- | --- |
| `enabled` | `true` | Turn periodic checks off entirely. |
| `interval_minutes` | `60` | How often to check. The runtime floor is 10 minutes. |
| `restart_strategy` | `self_replace` | `self_replace` restarts in place after staging; `supervisor` stages and leaves the restart to whatever is managing the process. |
| `rpc_mutations_enabled` | `true` | Whether a bearer-authenticated RPC client may call the mutating update methods at all. |

`supervisor` is the setting for a headless or containerised core: something else owns the process lifecycle, and the update should not decide to restart on its own.

## Headless and self-hosted cores

A core you run yourself (`openhuman-core serve`) checks and stages through the `update` namespace rather than the desktop prompt, which is why `rpc_mutations_enabled` exists as a separate switch. The desktop commands that used to update a core binary separately (`check_core_update`, `apply_core_update`) are kept as explicit no-ops for frontend compatibility and do nothing.

## See also

- [Platform & Availability](../features/platform.md): what ships on each OS.
- [Release Policy](../developing/release-policy.md): how a release is cut, and the OAuth minimum-version gate.
- [Cloud Deploy](../features/cloud-deploy.md): running the core somewhere else.
