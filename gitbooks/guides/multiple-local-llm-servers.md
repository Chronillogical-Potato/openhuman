---
description: >-
  Route workloads to multiple self-hosted OpenAI-compatible LLM servers through
  a community gateway, with a Mac mini and Linux GPU PC example.
---

# Use multiple local LLM servers

If you already run models on several machines, you can expose them through one
OpenAI-compatible gateway and select a backend in each workload's model ID.
This guide uses [Model Router](https://github.com/tjm8874/model-router), an
independent, MIT-licensed community project. OpenHuman does not bundle, install,
or supervise it. Other compatible gateways can serve the same role.

The example puts OpenHuman Core, the router, and a small model on a **Mac mini
with 64 GB RAM**, and a larger model server on a **Linux GPU PC**:

```text
OpenHuman Core (Mac mini)
  └─ Model Router, 127.0.0.1:18080/v1
       ├─ MacMini:small-model → local model server, 127.0.0.1:8080/v1
       └─ LinuxGPU:large-model → SSH tunnel → Linux PC, 127.0.0.1:8000/v1
```

These labels and ports are examples. Choose models and context lengths that fit
your hardware, including KV-cache and concurrency headroom. The router neither
hosts models nor downloads weights. This setup changes inference destinations;
it does not relocate the Core's existing workspace, memory, or project files.
If you connect a UI from another machine, the router URL is relative to **Core**.

## 1. Start the model servers

Run a small model with an OpenAI-compatible server on the Mac mini, for example
MLX or LM Studio, at `http://127.0.0.1:8080/v1`. Run a larger model on the Linux PC
with a server such as vLLM at that PC's `http://127.0.0.1:8000/v1`.
Use the runtime's own installation and model-loading instructions; see
[the local-model guide](local-model.md) for the single-server setup.

On the Mac mini, create a tunnel to the Linux PC. Replace the example SSH host
with yours, and keep the tunnel running:

```sh
ssh -N -o ExitOnForwardFailure=yes -o ServerAliveInterval=30 \
  -L 127.0.0.1:18000:127.0.0.1:8000 user@linux-gpu.example
```

The forwarded Linux endpoint is now `http://127.0.0.1:18000/v1` on the Mac mini.
Keep authenticated remote inference behind SSH or HTTPS rather than sending
bearer keys over plain HTTP across the network.

## 2. Start the community gateway

Build Model Router following its
[README](https://github.com/tjm8874/model-router#build-and-run). Its
[example configuration](https://github.com/tjm8874/model-router/blob/main/examples/mac-mini-linux.toml)
contains the following backend mapping:

```toml
[server]
host = "127.0.0.1"
port = 18080

[backends.MacMini]
base_url = "http://127.0.0.1:8080/v1"
endpoints = ["chat/completions"]

[backends.LinuxGPU]
base_url = "http://127.0.0.1:18000/v1"
endpoints = ["chat/completions"]
# For an authenticated upstream, set the environment variable in the router:
# api_key_env = "LINUX_LLM_API_KEY"
```

Validate your configuration and start it:

```sh
model-router --config router.toml --check-config
model-router --config router.toml
curl http://127.0.0.1:18080/v1/models
```

Use the binary's actual path if it is not on `PATH`. The configuration check does
not contact upstream servers. The model catalogue prefixes upstream IDs with the
backend name: `org/model:4bit` becomes `MacMini:org/model:4bit`. Colons and slashes
inside the upstream model ID are preserved. Check `router_errors` as well as
`data`; one reachable backend does not mean every backend is ready.

## 3. Route OpenHuman workloads

Merge these settings into the active Core user's `config.toml`, preserving your
other settings. Replace the placeholder model IDs with exact IDs from the gateway
catalogue. Keep workload keys at the top level, **before** `[local_ai]`:

```toml
chat_provider = "local-openai:MacMini:your-small-model-id"
memory_provider = "local-openai:MacMini:your-small-model-id"
learning_provider = "local-openai:MacMini:your-small-model-id"
reasoning_provider = "local-openai:LinuxGPU:your-large-model-id"
agentic_provider = "local-openai:LinuxGPU:your-large-model-id"
coding_provider = "local-openai:LinuxGPU:your-large-model-id"

[local_ai]
runtime_enabled = true
opt_in_confirmed = true
provider = "lm_studio"
base_url = "http://127.0.0.1:18080/v1"
chat_model_id = "MacMini:your-small-model-id"
vision_model_id = ""
```

`lm_studio` is the existing local OpenAI-compatible settings identifier; this
example's endpoint is Model Router, not an LM Studio installation requirement.
The `local-openai:` prefix selects OpenHuman's compatible provider. Core passes
`MacMini:your-small-model-id` to the router, which removes `MacMini:` and forwards
`your-small-model-id` to the named upstream.

`OPENHUMAN_LOCAL_INFERENCE_URL` or `LOCAL_OPENAI_URL` in Core's environment can
override the configured URL. Update or unset them when switching endpoints; see
[Local AI](../features/model-routing/local-ai.md#supported-runtimes).

Only assign vision to a vision-capable model and register its capabilities and
context window in OpenHuman. This text-only example leaves vision and embeddings
unassigned. Embedding services and background memory engines have separate
configuration/version constraints; adding a gateway does not automatically
reroute them. Model Router can forward `/v1/embeddings` and `/v1/decide` to
compatible services but does not translate APIs or start those services.

## 4. Verify and maintain the routes

Test a short chat using the local model, then a reasoning or agentic workload
using the Linux model. Confirm the backend/model in the router's logs and the
responses in OpenHuman. Model discovery and `/health` alone are not inference
proof. Test streaming and tool calls with models that support them.

Run the router under a per-user LaunchAgent or systemd user service if you need
continuous availability. Supervise the SSH tunnel separately. Validate the router
configuration and restart it after changes; upstream connection failures return
errors, with no automatic retry or fallback in this gateway.

The gateway listens only on loopback and has no inbound authentication. It keeps
upstream keys separate from Core's incoming Authorization header. A loopback
gateway can still forward prompts to another machine: OpenHuman's `local_only`
provider classification does not inspect the gateway's upstream configuration.
Review those destinations yourself; do not infer on-device-only inference from
the `local-openai:` prefix. Unassigned workloads retain their existing routes.
