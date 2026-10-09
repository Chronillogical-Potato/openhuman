---
description: >-
  Run a local model runtime yourself (Ollama, LM Studio, MLX, or any
  OpenAI-compatible server), pull your own models, and add the endpoint to
  OpenHuman as a provider. Includes checks and common failures.
icon: microchip
---

# Use OpenHuman with a local model

**Goal:** move some or all of OpenHuman's model work onto your own computer, so that the data used for those workloads never leaves the machine.

OpenHuman does not install a runtime, start it, or download models. You run the runtime, you pull the models, and OpenHuman calls the endpoint you give it. Local AI is **opt-in**; adding a local provider does not reroute anything until you point workloads at it.

For the config reference (provider strings, endpoint overrides, every workload field), see [Local AI (optional)](../features/model-routing/local-ai.md). This guide is the task-oriented version.

---

## Prerequisites

- A local runtime you install and run yourself:
  - [**Ollama**](https://ollama.com), default address `http://localhost:11434`.
  - [**LM Studio**](https://lmstudio.ai) with its local server enabled, default `http://localhost:1234/v1`.
  - MLX (`mlx_lm.server`), OMLX, or any other OpenAI-compatible server.
- Disk and RAM for the models you pick. A small chat model plus `bge-m3` for embeddings is a few GB on disk; 8 GB+ RAM is a sensible floor.

## Privacy implications

- Workloads you route locally run on-device. Nothing about that work is sent out.
- Anything you leave on the default route still goes through the OpenHuman [model router](../features/model-routing/README.md). Local AI only changes the workloads you move.
- Without [Privacy Mode](../features/privacy-mode.md), lightweight hints routed locally can fall back to the remote provider if the runtime is unreachable. Turn on `local_only` privacy mode if strict locality matters; then an unreachable runtime makes the request fail instead.

---

## Steps

### 1. Start the runtime

Install and launch the runtime so its server is running. For Ollama, confirm from a terminal:

```bash
curl http://localhost:11434/api/tags
```

A JSON list of models (even an empty one) means the server is reachable. For LM Studio or another OpenAI-compatible server, `curl http://localhost:1234/v1/models` (adjust the port) should return a model list.

### 2. Pull the models yourself

OpenHuman never pulls models. Pull every model you plan to route to before you configure it. For Ollama:

```bash
ollama pull gemma3:4b-it-qat   # chat, and vision from 4B up
ollama pull bge-m3             # memory embeddings (1024 dimensions)
```

In LM Studio, download the model in LM Studio and load it. For MLX or another server, start it with the model you want served.

See [Local models & bring your own key](../features/model-routing/local-and-byok-models.md) for which models can do chat, vision, and embeddings.

### 3. Add the endpoint as a provider

Open **Connections → LLM** and choose **Add a provider**:

- For Ollama, LM Studio, or OMLX, pick it under **Local runtimes**. The default endpoint is filled in; change it if your runtime listens elsewhere.
- For MLX or another server, choose **Add a custom provider** and enter its OpenAI-compatible endpoint (for example `http://127.0.0.1:8080/v1`).

When you save, OpenHuman asks the endpoint for its model list. If the runtime isn't reachable, the provider isn't saved; fix step 1 and try again.

### 4. Route workloads to it

Use **Custom routing** on the workloads you want local (chat, reasoning, vision, and so on) and pick a model the runtime reported. Only models you have pulled or loaded appear.

For memory embeddings, open **Connections → Embeddings** and choose Ollama with `bge-m3`, or set `embeddings_provider = "ollama:bge-m3"` in `config.toml`.

### 5. Test that it answers

Send a short message on a workload you routed locally, or use the model test in the LLM panel. A coherent reply means the path works end to end (endpoint reachable, model present, inference running). For Ollama you can also watch `ollama ps` while the request runs.

---

## Success checks

Local AI is working when:

- [ ] The local provider shows as connected under **Connections → LLM**, and its model list contains the models you pulled.
- [ ] A turn routed to the local provider returns a real reply.
- [ ] For embeddings: after the next memory sync, new summaries keep appearing in the **Memory** tab with the runtime running.

## Common failures

| What you see                                                    | Meaning                                                                     | Fix                                                                                                         |
| --------------------------------------------------------------- | --------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| Provider won't save; the endpoint is reported unreachable       | OpenHuman can't reach the runtime at that address                           | Start the runtime; check the port with `curl`; correct the endpoint                                         |
| The model you want isn't in the picker                          | The runtime doesn't have it                                                 | `ollama pull <model>`, or download and load it in LM Studio, then reopen the picker                         |
| A request fails saying the model is not found                   | The workload names a model the runtime no longer has                        | Pull it again, or route the workload to a model you have                                                    |
| Ollama answers but every model call fails                       | Ollama's model runner is broken                                             | Quit and relaunch Ollama, then retry                                                                        |
| Embeddings fail with a dimension error                          | The embedding model doesn't produce 1024-dimension vectors                  | Use `bge-m3`                                                                                                |
| Agent turns lose context on Ollama                              | Ollama's default context window is small                                    | Set `local_ai.num_ctx` (for example `8192`)                                                                 |
| Answers feel cloud-quality                                      | The runtime was unreachable and a lightweight hint fell back to remote      | Fix reachability; use `local_only` privacy mode if fallback is unacceptable                                 |

## Recovery

- **Back to cloud:** set the routed workloads back to the default route in **Custom routing**, or remove the local provider. No data is lost.
- **Stuck runtime:** quit the runtime fully and relaunch it. OpenHuman reconnects on the next request.
- **Disk pressure:** model pulls happen in your runtime, not in OpenHuman. Free space and pull again with the runtime's own command.

---

## Notes on what stays cloud anyway

Some workloads use the backend unless you configure something else: speech-to-text and web search go through the backend proxy, and text-to-speech uses the hosted voice unless you install Piper yourself and set `PIPER_BIN`. See [what stays in the cloud](../features/model-routing/local-ai.md#what-stays-in-the-cloud-by-default).

## See also

- [Local AI (optional)](../features/model-routing/local-ai.md): the config reference.
- [Keep sensitive data private](privacy-sensitive-data.md): the plain-language version of local vs external.
- [Automatic Model Routing](../features/model-routing/README.md): how tasks get matched to models.
