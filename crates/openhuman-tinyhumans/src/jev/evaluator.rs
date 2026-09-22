//! [`SystemOneEvaluator`]: the `tinytools_jev::JevEvaluator` that carries a
//! ranking decision over the wire to TypeSafe's System One endpoint through
//! the TinyHumans backend proxy.
//!
//! `tinytools-jev` owns the *decision* (retrieve, shortlist, ask, decode) and
//! hands the host one provider-neutral [`JevRequest`] per evaluation; this
//! type owns the *transport*: the `tinyjevclient` HTTP client, the
//! credential, the deadline and the retry policy. It translates the request
//! into one System One evaluation — a `Choice` over the options and a `Noul`
//! asking whether a tool is needed at all — and the answer back into a
//! [`JevDecision`].

use std::{collections::BTreeMap, time::Duration};

use serde_json::{Value, json};
use tinyjevclient::{
    Answer, Choice, Client, ClientConfig, Error as JevError, EvaluationFailure,
    EvaluationRequest, Noul, NoulCriteria, Question,
};
use tinytools::RankError;
use tinytools_jev::{JevDecision, JevEvaluator, JevRequest};

/// Question id of the option `Choice`.
const TOOL_QUESTION: &str = "tool";
/// Question id of the needs-a-tool `Noul`.
const NEEDS_TOOL_QUESTION: &str = "needs_tool";
/// The default wording when the ranker does not set
/// [`JevRequest::instructions`]: which *tool* accomplishes the request.
const DEFAULT_INSTRUCTIONS: &str = "Which tool accomplishes the user's `request`? Judge by what \
                                    each tool does, not by shared words. Pick `none` when no \
                                    listed tool does it.";

/// A System One evaluator over one built `tinyjevclient` client.
#[derive(Clone)]
pub struct SystemOneEvaluator {
    client: Client,
    /// Wall-clock cap on one evaluation, retries included. A tool search sits
    /// inside a model's turn; a slow decision is worse than a BM25 fallback.
    timeout: Duration,
}

impl std::fmt::Debug for SystemOneEvaluator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemOneEvaluator")
            .field("timeout", &self.timeout)
            .finish_non_exhaustive()
    }
}

impl SystemOneEvaluator {
    /// The deadline every evaluation runs under unless
    /// [`with_timeout`](Self::with_timeout) changes it.
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(3);

    /// An evaluator over a client built from `client_config`.
    ///
    /// # Errors
    ///
    /// Returns the client's configuration error (empty key, bad base URL,
    /// zero timeout) as [`RankError::InvalidInput`].
    pub fn from_config(client_config: ClientConfig) -> Result<Self, RankError> {
        let client = Client::new(client_config).map_err(|error| RankError::InvalidInput {
            reason: error.to_string(),
        })?;
        Ok(Self {
            client,
            timeout: Self::DEFAULT_TIMEOUT,
        })
    }

    /// Replace the per-evaluation deadline.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    fn build_request(request: &JevRequest) -> Result<EvaluationRequest, RankError> {
        let mut criteria: BTreeMap<String, Option<Value>> = BTreeMap::new();
        for option in &request.options {
            if criteria
                .insert(option.key.clone(), Some(json!(option.description)))
                .is_some()
            {
                return Err(RankError::InvalidInput {
                    reason: format!("duplicate option key `{}`", option.key),
                });
            }
        }
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
        let instructions = request
            .instructions
            .as_deref()
            .unwrap_or(DEFAULT_INSTRUCTIONS);
        let questions = BTreeMap::from([
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
                        r#false: json!("The request can be answered by replying, with no tool."),
                    }),
                }),
            ),
        ]);
        Ok(EvaluationRequest {
            state,
            model: request.model.clone(),
            questions,
        })
    }
}

#[async_trait::async_trait]
impl JevEvaluator for SystemOneEvaluator {
    async fn evaluate(&self, request: &JevRequest) -> Result<JevDecision, RankError> {
        let wire = Self::build_request(request)?;
        let evaluated = tokio::time::timeout(self.timeout, self.client.evaluate(&wire))
            .await
            .map_err(|_elapsed| RankError::Timeout)?;
        let result = evaluated.map_err(map_failure)?;
        let Some(Answer::Choice(choice)) = result.response.answers.get(TOOL_QUESTION) else {
            return Err(RankError::Backend {
                reason: format!("response has no choice answer for `{TOOL_QUESTION}`"),
            });
        };
        let needs_tool = match result.response.answers.get(NEEDS_TOOL_QUESTION) {
            Some(Answer::Noul(noul)) => Some(noul.noul),
            _ => None,
        };
        log::debug!(
            "[tool-search] system one answered (options={} confidence={:.2} needs_tool={:?} attempts={} latency_ms={} request_id={:?})",
            request.options.len(),
            choice.confidence,
            needs_tool,
            result.attempts,
            result.latency.as_millis(),
            result.request_id,
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

fn map_failure(failure: EvaluationFailure) -> RankError {
    match failure.error {
        JevError::InvalidRequest { reason } | JevError::InvalidConfig { reason } => {
            RankError::InvalidInput { reason }
        }
        JevError::Timeout => RankError::Timeout,
        other => RankError::Backend {
            // `Display` on every variant is credential-free by the client's
            // contract; the transport source is dropped, not printed.
            reason: format!("{other} after {} attempt(s)", failure.attempts),
        },
    }
}

#[cfg(test)]
#[path = "evaluator_tests.rs"]
mod tests;
