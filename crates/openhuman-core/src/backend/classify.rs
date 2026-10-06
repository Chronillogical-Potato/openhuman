//! Shared provider error classification helpers.

/// Whether a provider error body represents exhausted budget or credits.
pub fn is_budget_exhausted_message(message: &str) -> bool {
    tinyinference_providers::is_budget_exhausted_message(message)
}
