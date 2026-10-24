//! Process credential request, mutation, clearing, and teardown operations.

use crate::config::Config;
use crate::core::runtime::DispatchCtx;
use crate::core::Outcome;
use serde_json::Value;

pub(crate) const LOG_PREFIX: &str = "[credentials][set-credential]";
pub(crate) static CREDENTIAL_MUTATION_LOCK: tokio::sync::Mutex<()> =
    tokio::sync::Mutex::const_new(());

mod clear;
mod request;
mod set;
mod teardown;

pub use clear::{clear_credential, clear_session, store_session};
pub use request::SetCredentialRequest;
pub use set::set_credential;
pub(crate) use teardown::{clear_session_credential, process_credential_refusal};
pub use teardown::PENDING_BACKEND_VALIDATION_FIELD;

#[cfg(test)]
#[path = "../credential_tests.rs"]
mod tests;
