//! [`TinyJevEvaluator`]: the `tinytools_jev::JevEvaluator` over
//! `tinyjevclient`, reaching Jev through the TinyHumans System One proxy.
//!
//! `tinytools-jev` decides *what* to ask — the options, the intent, the
//! family-stage wording — and this evaluator owns the wire: one `Choice`
//! over the options plus a `needs_tool` `Noul`, the credential, the
//! per-attempt timeout and retries the client applies, and a deadline of
//! its own so a slow answer becomes a fallback rather than a stalled turn.

use std::{collections::BTreeMap, time::Duration};

use serde_json::{json, Value};
use tinyjevclient::{
    Answer, Choice, Client, Error as JevError, EvaluationFailure, EvaluationRequest, Noul,
    NoulCriteria, Question,
};
use tinytools::RankError;
use tinytools_jev::{JevDecision, JevEvaluator, JevRequest};

/// Question id of the tool `Choice`.
const TOOL_QUESTION: &str = "tool";
/// Question id of the needs-a-tool `Noul`.
const NEEDS_TOOL_QUESTION: &str = "needs_tool";
/// Default deadline for one evaluation, on top of the client's own
/// per-attempt timeout and retries.
const DEFAULT_DEADLINE: Duration = Duration::from_secs(3);

/// Evaluates `tinytools_jev` requests against Jev through `tinyjevclient`.
#[derive(Debug, Clone)]
pub struct TinyJevEvaluator {
    client: Client,
    deadline: Duration,
}

impl TinyJevEvaluator {
    /// An evaluator over `client` with the default 3 s deadline.
    pub fn new(client: Client) -> Self {
        Self {
            client,
            deadline: DEFAULT_DEADLINE,
        }
    }

    /// Sets the per-evaluation deadline.
    pub fn with_deadline(mut self, deadline: Duration) -> Self {
        self.deadline = deadline;
        self
    }

    fn build(request: &JevRequest) -> EvaluationRequest {
        let mut state = json!({ "request": request.intent });
        if !request.recent_turns.is_empty() {
            if let Some(object) = state.as_object_mut() {
                object.insert(
                    "recent_user_turns".to_owned(),
                    Value::Array(
                        request
                            .recent_turns
                            .iter()
                            .map(|turn| Value::String(turn.clone()))
                            .collect(),
                    ),
                );
            }
        }
        let criteria: BTreeMap<String, Option<Value>> = request
            .options
            .iter()
            .map(|option| (option.key.clone(), Some(json!(option.description))))
            .collect();
        let instructions = request.instructions.clone().unwrap_or_else(|| {
            "Which tool accomplishes the user's `request`? Judge by what each tool does, \
             not by shared words. Pick `none` when no listed tool does it."
                .to_owned()
        });
        EvaluationRequest {
            state,
            model: request.model.clone(),
            questions: BTreeMap::from([
                (
                    TOOL_QUESTION.to_owned(),
                    Question::Choice(Choice {
                        instructions: json!(instructions),
                        criteria,
                    }),
                ),
                (
                    NEEDS_TOOL_QUESTION.to_owned(),
                    Question::Noul(Noul {
                        instructions: json!(
                            "Does fulfilling the user's `request` require calling a tool \
                             — an action or a lookup outside the assistant's own knowledge?"
                        ),
                        criteria: Some(NoulCriteria {
                            r#true: json!(
                                "The request asks for an action or for information that \
                                 must be fetched."
                            ),
                            r#false: json!(
                                "The request can be answered by replying, with no tool."
                            ),
                        }),
                    }),
                ),
            ]),
        }
    }
}

fn map_failure(failure: EvaluationFailure) -> RankError {
    match failure.error {
        JevError::InvalidRequest { reason } | JevError::InvalidConfig { reason } => {
            RankError::invalid_input(reason)
        }
        JevError::Timeout => RankError::Timeout,
        // `Display` on every variant is credential-free by the client's
        // contract; the transport source is dropped, not printed.
        other => RankError::backend(format!("{other} after {} attempt(s)", failure.attempts)),
    }
}

#[async_trait::async_trait]
impl JevEvaluator for TinyJevEvaluator {
    async fn evaluate(&self, request: &JevRequest) -> Result<JevDecision, RankError> {
        let wire = Self::build(request);
        let result = tokio::time::timeout(self.deadline, self.client.evaluate(&wire))
            .await
            .map_err(|_elapsed| RankError::Timeout)?
            .map_err(map_failure)?;
        let Some(Answer::Choice(choice)) = result.response.answers.get(TOOL_QUESTION) else {
            return Err(RankError::backend(
                "response has no choice answer for `tool`",
            ));
        };
        let needs_tool = match result.response.answers.get(NEEDS_TOOL_QUESTION) {
            Some(Answer::Noul(noul)) => Some(noul.noul),
            _ => None,
        };
        log::debug!(
            "[tool-search] jev evaluated {} option(s) (confidence={:.2} needs_tool={:?} latency_ms={} attempts={} input_tokens={:?})",
            request.options.len(),
            choice.confidence,
            needs_tool,
            result.latency.as_millis(),
            result.attempts,
            result.response.usage.input_tokens,
        );
        Ok(JevDecision {
            probabilities: choice.probabilities.clone(),
            choice_confidence: choice.confidence,
            needs_tool,
            input_tokens: result.response.usage.input_tokens,
            attempts: result.attempts,
        })
    }
}

#[cfg(test)]
#[path = "evaluator_tests.rs"]
mod tests;
