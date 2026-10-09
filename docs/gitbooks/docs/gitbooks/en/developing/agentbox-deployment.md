---
description: >-
  The removed AgentBox container deployment surface, kept as the record of
  what it was and which test pins its absence.
icon: box-archive
---

# AgentBox marketplace deployment (historical)

This page describes a container surface that no longer exists in
`openhuman-core`. A regression test,
`agentbox_run_and_jobs_paths_are_no_longer_public` in
`crates/openhuman-rpc/src/server/auth_tests.rs`, asserts that `/run` and
`/jobs/{job_id}` are gone and pins the reason in a comment: the AgentBox
marketplace surface moved out of this codebase. `OPENHUMAN_AGENTBOX_MODE`
has no remaining reader anywhere in `crates/openhuman-core/src` today, even
though the Dockerfile and `.env.example` still mention it; those references
are themselves stale and due for cleanup.

If you need to run OpenHuman on GMI Cloud's AgentBox marketplace today, this
is not the current integration path; check with the team that owns the
GMI Cloud deployment for where that surface lives now. The rest of this page
is kept as a historical record of how the integration worked before removal,
in case you are debugging an older build that still has it.

## Container contract (as it worked before removal)

When `OPENHUMAN_AGENTBOX_MODE=1`, the core HTTP server exposes:

- `POST /run`: accept work, return `202 { "job_id": "<uuid>" }`. Body shape:
  `{ "payload": { "message": "<string>", "thread_id": "<optional string>" } }`.
- `GET /jobs/{job_id}`: return `{ "status": "pending|running|completed|failed", "result": ..., "error": ... }`.
- `GET /health`: liveness.

Both `/run` and `/jobs/*` are unauthenticated at the container boundary;
AgentBox's edge handles auth before traffic reaches us.

## The 4-step register wizard

In the AgentBox console:

1. **Basic Info**: name `OpenHuman`, description, listing identity.
2. **Infrastructure**: Docker image source (push tagged builds to your
   chosen registry, see "Image push" below), compute tier, region. Enable the
   "GMI MaaS" toggle so the platform injects `GMI_MAAS_BASE_URL` and
   `GMI_MAAS_API_KEY` at runtime.
3. **Env Variables**: set:
   - `OPENHUMAN_AGENTBOX_MODE=1`
   - (optional) `OPENHUMAN_AGENTBOX_JOB_TIMEOUT_SECS` (default 600)
   - `GMI_MODELS` to the marketplace-approved model id (e.g.
     `deepseek-ai/DeepSeek-V4-Pro`).
   - `OPENHUMAN_WORKSPACE` to a writable container path (e.g. `/home/openhuman/.openhuman`).
   - `RUST_LOG=info` (or `debug` while shaking out the first deploy).
4. **Review & Register**: confirm and test from the console panel.

> ⚠️ The platform API key is shown ONCE on the registration confirmation
> screen. Save it to your secrets manager immediately. It is NOT recoverable
> from the console after that.

## Image push

Build and push from `main` using the existing `Dockerfile`:

```bash
docker build -t <registry>/openhuman-core:<tag> .
docker push <registry>/openhuman-core:<tag>
```

First deploy takes 10 to 25 minutes to reach `running`; later deploys are faster.

## Long-running requests

AgentBox treats requests >2 min as long-running. OpenHuman handles this with
**polling** per AgentBox's documented pattern: the agent runtime is invoked
inside the worker task, capped by `OPENHUMAN_AGENTBOX_JOB_TIMEOUT_SECS`
(default 10 minutes). No streaming.

Polling clients should:

1. `POST /run` and capture `job_id`.
2. `GET /jobs/{job_id}` every 1 to 3 seconds.
3. Stop when `status` is `completed` or `failed`.
4. Note: terminal jobs are retained for 1 hour after completion, then
   garbage-collected. Long pauses between poll and read may return `404`.

## Local smoke test

```bash
OPENHUMAN_AGENTBOX_MODE=1 \
GMI_MAAS_BASE_URL=https://api.gmi-serving.com \
GMI_MAAS_API_KEY=sk-... \
GMI_MODELS=deepseek-ai/DeepSeek-V4-Pro \
./target/debug/openhuman-core serve &

curl -X POST http://127.0.0.1:7788/run \
  -H 'content-type: application/json' \
  -d '{"payload":{"message":"hello"}}'

# Then poll the returned job_id:
curl http://127.0.0.1:7788/jobs/<job_id>
```

## Troubleshooting

- `404 job not found` after a successful submit: retention window (1h) has
  elapsed, or the container restarted (in-memory store is not durable in v1).
- `status: "failed"`, `error: "agentbox: agent runtime bridge not wired"`:
  the production invoker stub from before Task 9 landed; rebuild against a
  current `main`.
- `status: "failed"`, `error: "job timeout after Ns"`: the agent invocation
  exceeded `OPENHUMAN_AGENTBOX_JOB_TIMEOUT_SECS`. Bump the env var on the
  next deploy.
- `[agentbox::gmi] not registering GMI MaaS provider: missing/blank: GMI_MAAS_API_KEY`:
  the platform did not inject the key. Re-check the wizard's "MaaS
  integration toggle" in Step 2.
- `[agentbox::gmi] current-thread runtime detected, skipping provider registration`:
  the core was booted in a single-threaded tokio runtime. Use the standard
  `serve` subcommand, which spawns a multi-thread runtime.
