//! Managed Python runtime for Python-backed integrations.
//!
//! [`bootstrap`] is the client for the `tinyruntime` module: it asks for an
//! interpreter and adapts the answer. Discovery, selection, download, and
//! install all live in the module now.

pub mod bootstrap;

pub use bootstrap::{PythonBootstrap, PythonSource, ResolvedPython};
