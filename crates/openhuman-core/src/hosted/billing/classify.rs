//! OpenHuman billing-state classification used by UI and telemetry policy.

/// Return whether a provider message represents deterministic exhausted-budget
/// user state rather than a product defect.
pub fn is_budget_exhausted_message(message: &str) -> bool {
    const PHRASES: &[&str] = &[
        "insufficient budget",
        "budget exceeded",
        "add credits",
        "insufficient balance",
        "no remaining credits",
        "credit balance is too low",
    ];
    let lower = message.to_ascii_lowercase();
    PHRASES.iter().any(|phrase| lower.contains(phrase))
}

#[cfg(test)]
#[path = "classify_tests.rs"]
mod tests;
