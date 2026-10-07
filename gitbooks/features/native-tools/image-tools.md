---
description: >-
  The reading side of images: attachments the model can see natively, local
  image metadata, and which model handles vision.
icon: image
---

# Image Tools

Generating images is [Image & Video Generation](media-generation.md). This page is the other direction: getting an image *into* a turn so the model can look at it.

## Attaching an image

Attach a file in the composer, or point the agent at a path. What happens next depends on the model and the route, not on a setting:

- If the model's facts say it accepts that modality **and** the transport supports it, the image goes up natively as image content.
- If not, it degrades rather than failing: a bounded readout, or a reference to the file with its path and metadata, so the agent can still reason about it and tell you what it cannot see.

Uploads are preserved in the acting workspace across a restart, and the ordered references survive in the transcript, so a later turn can still refer to "the second screenshot".

## `image_info`

One tool, read-only: it reads a local image's metadata (format, dimensions, size) and can return its bytes as base64 text for a model that wants them inline. It is the tool to reach for when the question is "what *is* this file", and it does not need a vision model.

## Which model sees it

Vision is a workload with its own provider slot, so you can route it independently of chat:

```toml
vision_provider = "…"   # a provider id, or leave unset to use the chat provider
```

If a configured vision model turns out to be chat-only, the call returns a named error rather than silently substituting another model. There is deliberately no default local vision model: a local setup has to name one. [Local models and BYOK](../model-routing/local-and-byok-models.md) lists the ones worth trying.

## Delegation

A turn that needs to look at several images, or to work through one carefully, is usually handed to the built-in vision sub-agent rather than done inline, so the main conversation does not carry every pixel of context. Delegation happens on the agent's own judgement; you do not have to ask for it.

## See also

- [Image & Video Generation](media-generation.md): making images and video rather than reading them.
- [Chat](../chat.md): where attachments live in the UI.
- [Automatic Model Routing](../model-routing/README.md): how the vision slot is resolved.
