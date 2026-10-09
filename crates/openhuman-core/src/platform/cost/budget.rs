//! Checking a model call against the configured budgets
//! (`[[cost.budgets]]`, [`BudgetPolicy`]).
//!
//! [`evaluate`] is a pure function of the policies, the ledger records of the
//! period and the call about to be made, so every rule is testable without a
//! ledger. For each policy it works out which bucket the call falls in — all
//! calls (`global`), or this call's thread, agent, model, provider or user
//! agent — sums that bucket's spend and tokens over the period, and compares
//! them with the limits:
//!
//! - at or over a limit, a `refuse` policy refuses the call and a `warn`
//!   policy reports it;
//! - at or over `warn_fraction` of a limit, the policy reports a warning.
//!
//! A call that does not carry the policy's attribute (no thread outside a
//! chat, say) is outside that policy. Records from before attribution count
//! only towards `global` and `model` budgets.

use chrono::{DateTime, Datelike, TimeZone, Utc};

use crate::config::{BudgetAction, BudgetPeriod, BudgetPolicy, BudgetScope};

use super::types::{CostRecord, UsageScope};

/// The call being checked: its model and attribution.
#[derive(Debug, Clone, Copy)]
pub struct CallUnderCheck<'a> {
    pub model: &'a str,
    pub scope: &'a UsageScope,
}

/// One policy that is at or near its limit for this call.
#[derive(Debug, Clone, PartialEq)]
pub struct BudgetHit {
    pub policy: String,
    /// The bucket the call fell in (`*` for a global budget).
    pub bucket: String,
    pub spent_usd: f64,
    pub max_usd: Option<f64>,
    pub tokens: u64,
    pub max_tokens: Option<u64>,
    /// At or over a limit (otherwise only past the warn fraction).
    pub exceeded: bool,
    pub action: BudgetAction,
}

impl BudgetHit {
    /// The refusal text. Starts with `BUDGET_EXCEEDED:` so callers and the
    /// error classifier can recognise it.
    pub fn refusal(&self) -> String {
        let usd = self
            .max_usd
            .map(|max| format!(" ${:.4} of ${max:.2}", self.spent_usd))
            .unwrap_or_default();
        let tokens = self
            .max_tokens
            .map(|max| format!(" {} of {max} tokens", self.tokens))
            .unwrap_or_default();
        format!(
            "BUDGET_EXCEEDED: budget `{}` reached for {}:{usd}{tokens}",
            self.policy, self.bucket
        )
    }
}

/// What the budgets say about a call.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BudgetVerdict {
    /// Every policy at or near its limit.
    pub hits: Vec<BudgetHit>,
}

impl BudgetVerdict {
    /// The first hit that refuses the call, if any.
    pub fn refusal(&self) -> Option<&BudgetHit> {
        self.hits
            .iter()
            .find(|hit| hit.exceeded && hit.action == BudgetAction::Refuse)
    }
}

/// When `period` began, as of `now`.
pub fn period_start(period: BudgetPeriod, now: DateTime<Utc>) -> DateTime<Utc> {
    let day = Utc
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
        .single()
        .unwrap_or(now);
    match period {
        BudgetPeriod::Day => day,
        BudgetPeriod::Month => Utc
            .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
            .single()
            .unwrap_or(day),
    }
}

/// The earliest period start any of `policies` needs, or `None` with none.
pub fn earliest_start(policies: &[BudgetPolicy], now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    policies.iter().map(|p| period_start(p.period, now)).min()
}

fn bucket_of<'a>(scope: BudgetScope, model: &'a str, usage: &'a UsageScope) -> Option<&'a str> {
    match scope {
        BudgetScope::Global => Some("*"),
        BudgetScope::Model => Some(model),
        BudgetScope::Thread => usage.thread_id.as_deref(),
        BudgetScope::Agent => usage.agent_id.as_deref(),
        BudgetScope::Provider => usage.provider.as_deref(),
        BudgetScope::SessionAgent => usage.session_agent.as_deref(),
    }
}

/// Check `call` against `policies`, given the ledger `records` (at least
/// those since [`earliest_start`]).
pub fn evaluate(
    policies: &[BudgetPolicy],
    records: &[CostRecord],
    call: CallUnderCheck<'_>,
    now: DateTime<Utc>,
) -> BudgetVerdict {
    let mut verdict = BudgetVerdict::default();
    for (index, policy) in policies.iter().enumerate() {
        if policy.max_usd.is_none() && policy.max_tokens.is_none() {
            continue;
        }
        let Some(bucket) = bucket_of(policy.scope, call.model, call.scope) else {
            continue;
        };
        if policy.matches.as_deref().is_some_and(|want| want != bucket) {
            continue;
        }
        let start = period_start(policy.period, now);
        let (mut spent_usd, mut tokens) = (0.0, 0u64);
        for record in records {
            let usage = &record.usage;
            if usage.timestamp < start || usage.timestamp > now {
                continue;
            }
            if bucket_of(policy.scope, &usage.model, &usage.scope) != Some(bucket) {
                continue;
            }
            spent_usd += usage.cost_usd;
            // Saturating: a corrupt record must not wrap a total back under
            // its limit.
            tokens = tokens.saturating_add(usage.input_tokens.saturating_add(usage.output_tokens));
        }
        let over = |spent: f64, max: Option<f64>, fraction: f64| {
            max.is_some_and(|max| max >= 0.0 && spent >= max * fraction)
        };
        let warn_fraction = policy.warn_fraction.clamp(0.0, 1.0);
        // Token limits compare as integers; only the warn threshold, a
        // fraction of the limit, is computed in floating point.
        let exceeded = over(spent_usd, policy.max_usd, 1.0)
            || policy.max_tokens.is_some_and(|max| tokens >= max);
        let warning = over(spent_usd, policy.max_usd, warn_fraction)
            || policy.max_tokens.is_some_and(|max| {
                tokens >= max || (tokens as f64) >= (max as f64) * warn_fraction
            });
        if exceeded || warning {
            verdict.hits.push(BudgetHit {
                policy: policy
                    .name
                    .clone()
                    .unwrap_or_else(|| format!("budget #{}", index + 1)),
                bucket: bucket.to_string(),
                spent_usd,
                max_usd: policy.max_usd,
                tokens,
                max_tokens: policy.max_tokens,
                exceeded,
                action: policy.action,
            });
        }
    }
    verdict
}

/// Check a call against `policies` using `tracker`'s ledger, as of `now`.
/// With no policies nothing is read and the verdict is empty.
pub fn check_call(
    policies: &[BudgetPolicy],
    tracker: &super::tracker::CostTracker,
    call: CallUnderCheck<'_>,
    now: DateTime<Utc>,
) -> anyhow::Result<BudgetVerdict> {
    let Some(start) = earliest_start(policies, now) else {
        return Ok(BudgetVerdict::default());
    };
    let records = tracker.records_between(start, now)?;
    Ok(evaluate(policies, &records, call, now))
}

#[cfg(test)]
#[path = "budget_tests.rs"]
mod tests;
