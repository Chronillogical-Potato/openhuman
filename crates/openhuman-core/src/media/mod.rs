//! Media generation.
//!
//! - [`generation`] — the `media_generate_*` agent tools (image/video via GMI,
//!   proxied through the TinyHumans backend)
//!
//! Gated by the `media` feature at the family root (`pub mod media;` in
//! `crates/openhuman-core/src/lib.rs`). It is a **surface-only** gate: media
//! generation is backend-proxied over the shared `IntegrationClient`/`reqwest`,
//! so no exclusive dependency is shed. No controller/store/subscriber is
//! tagged `DomainGroup::Media` — this family is agent-tools-only.

pub mod generation;
