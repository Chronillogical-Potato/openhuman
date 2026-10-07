//! Moving memory from the legacy CortexDB tree into the per-user tree.
//!
//! Memory written before layout v3 lives under `app:tinymemory/…`; layout v3
//! keeps each person's memory below `user:<id>`. Items are moved by being
//! stored again ([`copy`]) rather than by copying events, because an item's
//! namespace is part of its identity and its placement changes
//! ([`map`]). Progress is saved after every page ([`state`]), so the move
//! resumes where it stopped and repeats at most one page.
//!
//! This module holds the parts that need no engine binding: detecting legacy
//! memory, placing items, and the copy-and-verify pass over a given pair of
//! engines. Binding the two engines, the switch to the per-user tree, the
//! catch-up pass, cleanup and the RPCs build on it.

pub mod cleanup;
pub mod copy;
pub mod map;
pub mod state;

pub use cleanup::cleanup;
pub use copy::{copy, legacy_present, Engines};
pub use map::{FlowPlacement, Placement};
pub use state::{MigrationState, Phase};
