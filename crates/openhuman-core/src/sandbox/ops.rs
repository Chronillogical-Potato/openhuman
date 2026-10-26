//! Sandbox backend operations split between host policy and execution adapters.

mod exec;
mod policy;

pub use exec::*;
pub use policy::*;

#[cfg(test)]
#[path = "ops_tests.rs"]
mod tests;
