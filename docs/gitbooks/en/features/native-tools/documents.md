---
description: >-
  Write .docx and .pptx files, and read PDF, Word, PowerPoint and Excel back
  in, with every limit stated up front.
icon: file-lines
---

# Documents

Two tools write files a person can open, and the ingest path reads the common office formats back in. Both run in a loadable module rather than in the app binary, which is why the writers' dependencies are in no OpenHuman build.

## Writing

| Tool | Produces |
| --- | --- |
| `generate_document` | A `.docx` from a structured spec: title, author, headings, paragraphs, bullet lists. |
| `generate_presentation` | A `.pptx` from slides: titles, bullets, images. |

The spec is validated before anything is written, and the limits are part of the contract rather than advice:

| `.docx` | Limit |
| --- | --- |
| Sections | 128 |
| Paragraphs per section | 200 |
| Bullets per section | 200 |
| Characters per title, author or heading | 2,000 |
| Characters per paragraph | 20,000 |
| Characters in total | 2,000,000 |

| `.pptx` | Limit |
| --- | --- |
| Slides | 64 |
| Bullets per slide | 32 |
| Characters per text field | 2,000 |
| Images per slide | 6 |
| Images per deck | 8 |
| Bytes per image | 5 MiB |

The aggregate character cap is the load-bearing one: sections times paragraphs times characters per paragraph would otherwise allow a spec that no reader could open.

The finished file arrives as an [artifact](../chat.md), so it shows up in the thread's files panel with download and reveal-in-folder.

## Reading

Attach a file, or point a [memory source](../memory.md) at one, and the ingest path extracts text with per-section provenance (a page number for a PDF, the part path for an Office file):

| Format | Notes |
| --- | --- |
| PDF | Up to 4,096 source pages. A page with no text layer is kept and flagged as a scan candidate, which is a suggestion for vision, not a claim that OCR is needed. |
| DOCX, PPTX, XLSX | Slide and sheet order comes from the file's own relationship manifest, so an unreferenced slide is excluded rather than guessed at by filename. Spreadsheets read shared and inline strings and use cached cell values; formulas are never evaluated. |

Bounds: 64 MiB in, 256 result sections, 200,000 bytes of text out, and the result says whether it was truncated. Text is never cut mid-character. Rendering PDF pages to images is opt-in per page, at most 8 pages per call and 2,048 pixels on the longest edge.

Nothing is written to disk during extraction, no office application is launched, and there is no network OCR.

## Where it runs

Document work happens in the `tinydocs` module: a signed native library downloaded on first use and verified against a SHA-256 pinned in the compiled registry. The consequence worth knowing is that the document writers and the PDF reader are **not** in OpenHuman's dependency graph in any build configuration, and with the `documents` feature off the two tools are simply absent from the agent's list rather than present and failing, while a PDF attachment degrades to a file reference instead of extracted text.

## See also

- [Image & Video Generation](media-generation.md): the other thing that produces a file.
- [Memory](../memory.md): where a read document ends up if you add it as a source.
- [Loadable modules](../../developing/loadable-modules.md): how a module is pinned and loaded.
