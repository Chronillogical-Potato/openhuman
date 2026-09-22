//! [`TinyHumansJevRanker`]: a `ToolRanker` that resolves the process's
//! TinyHumans credential on every search and ranks through Jev.

use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    sync::Mutex,
};

use std::{future::Future, pin::Pin, sync::Arc};

use async_trait::async_trait;
use openhuman_core::api::config::effective_backend_api_url;
use openhuman_core::api::headers::build_backend_client;
use openhuman_core::api::transport::TransportProfile;
use openhuman_core::config::Config;
use openhuman_core::security::credentials::session_support::resolve_backend_credential;
use serde_json::{json, Value};
use tinytools::{RankCandidate, RankContext, RankError, RankHit, ToolRanker};
use tinytools_jev::{JevDecision, JevEvaluator, JevRanker, JevRankerConfig, JevRequest};

const SYSTEM_ONE_PATH: &str = "agent-integrations/openrouter/systemone";

/// How the ranker reads the config a search runs under. The default is the
/// core's own read path (the embedder's config when one is bound, else the
/// process-global load); a test hands in a fixed one.
pub type ConfigLoader =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<Config, String>> + Send>> + Send + Sync>;

/// A [`JevRanker`] bound to whichever credential and backend the process has
/// at search time.
pub struct TinyHumansJevRanker {
    config: JevRankerConfig,
    load_config: ConfigLoader,
    cached: Mutex<Option<Cached>>,
}

struct Cached {
    fingerprint: u64,
    ranker: JevRanker,
}

/// OpenHuman's System One transport. `tinytools-jev` deliberately keeps this
/// policy at the host boundary, where backend headers and credentials belong.
struct TinyHumansJevEvaluator {
    client: reqwest::Client,
    base_url: String,
    credential: String,
}

impl std::fmt::Debug for TinyHumansJevEvaluator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TinyHumansJevEvaluator")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl JevEvaluator for TinyHumansJevEvaluator {
    async fn evaluate(&self, request: &JevRequest) -> Result<JevDecision, RankError> {
        let response = self
            .client
            .post(format!(
                "{}/{SYSTEM_ONE_PATH}",
                self.base_url.trim_end_matches('/')
            ))
            .bearer_auth(&self.credential)
            .json(&system_one_request(request))
            .send()
            .await
            .map_err(|error| RankError::Backend {
                reason: format!("Jev request failed: {error}"),
            })?
            .error_for_status()
            .map_err(|error| RankError::Backend {
                reason: format!("Jev request was rejected: {error}"),
            })?;
        let value: Value = response.json().await.map_err(|error| RankError::Backend {
            reason: format!("Jev response could not be decoded: {error}"),
        })?;
        system_one_decision(value)
    }
}

fn system_one_request(request: &JevRequest) -> Value {
    let criteria = request
        .options
        .iter()
        .map(|option| {
            (
                option.key.clone(),
                Value::String(option.description.clone()),
            )
        })
        .collect::<serde_json::Map<_, _>>();
    let instructions = request.instructions.clone().unwrap_or_else(|| {
        "Which tool accomplishes the user's `request`? Judge by what each tool does, not by shared words. Pick `none` when no listed tool does it.".into()
    });
    json!({
        "state": {
            "request": request.intent,
            "recent_user_turns": request.recent_turns,
        },
        "model": request.model,
        "questions": {
            "tool": {
                "type": "choice",
                "instructions": instructions,
                "criteria": criteria,
            },
            "needs_tool": {
                "type": "noul",
                "instructions": "Does fulfilling the user's `request` require calling a tool — an action or a lookup outside the assistant's own knowledge?",
                "criteria": {
                    "true": "The request asks for an action or for information that must be fetched.",
                    "false": "The request can be answered by replying, with no tool.",
                },
            },
        },
    })
}

fn system_one_decision(value: Value) -> Result<JevDecision, RankError> {
    let answers = value
        .get("answers")
        .and_then(Value::as_object)
        .ok_or_else(|| RankError::Backend {
            reason: "Jev response has no answers object".into(),
        })?;
    let tool = answers.get("tool").ok_or_else(|| RankError::Backend {
        reason: "Jev response has no tool answer".into(),
    })?;
    let probabilities =
        serde_json::from_value(tool.get("probabilities").cloned().ok_or_else(|| {
            RankError::Backend {
                reason: "Jev tool answer has no probabilities".into(),
            }
        })?)
        .map_err(|error| RankError::Backend {
            reason: format!("Jev tool probabilities are invalid: {error}"),
        })?;
    let choice_confidence = tool
        .get("confidence")
        .and_then(Value::as_f64)
        .ok_or_else(|| RankError::Backend {
            reason: "Jev tool answer has no confidence".into(),
        })?;
    Ok(JevDecision {
        probabilities,
        choice_confidence,
        needs_tool: answers
            .get("needs_tool")
            .and_then(|answer| answer.get("noul"))
            .and_then(Value::as_f64),
        input_tokens: value
            .get("usage")
            .and_then(|usage| usage.get("input_tokens"))
            .and_then(Value::as_u64),
        attempts: 1,
    })
}

impl std::fmt::Debug for TinyHumansJevRanker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TinyHumansJevRanker")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl Default for TinyHumansJevRanker {
    fn default() -> Self {
        Self::new()
    }
}

impl TinyHumansJevRanker {
    /// A ranker with `tinytools-jev`'s defaults: BM25 retrieval to 20, one
    /// Jev decision, a 3 s deadline.
    pub fn new() -> Self {
        Self::with_config(JevRankerConfig::new())
    }

    /// A ranker with an explicit `tinytools-jev` configuration.
    pub fn with_config(config: JevRankerConfig) -> Self {
        Self {
            config,
            load_config: Arc::new(|| {
                Box::pin(openhuman_core::config::ops::load_config_with_timeout())
            }),
            cached: Mutex::new(None),
        }
    }

    /// Reads the config through `loader` instead of the core's read path.
    pub fn with_config_loader(mut self, loader: ConfigLoader) -> Self {
        self.load_config = loader;
        self
    }

    /// The `JevRanker` for the current credential and backend, built or
    /// reused.
    ///
    /// Reads the config and credential fresh each time: the cost is a config
    /// load, which a `tool_search` (one model round trip plus a network call)
    /// dwarfs, and the benefit is that sign-in, sign-out and a backend URL
    /// change are all honoured by the next search.
    async fn current(&self) -> Result<JevRanker, RankError> {
        let config = (self.load_config)()
            .await
            .map_err(|error| RankError::Backend {
                reason: format!("config unavailable: {error}"),
            })?;
        let credential = resolve_backend_credential(&config).map_err(|reason| {
            // The message names what is missing, never a secret.
            RankError::Backend {
                reason: format!("no TinyHumans credential ({reason})"),
            }
        })?;
        let base_url = effective_backend_api_url(&config.api_url);
        let fingerprint = fingerprint(credential.secret(), &base_url);

        let mut cached = self
            .cached
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(entry) = cached
            .as_ref()
            .filter(|entry| entry.fingerprint == fingerprint)
        {
            return Ok(entry.ranker.clone());
        }
        let evaluator = TinyHumansJevEvaluator {
            client: build_backend_client(TransportProfile::Integrations).map_err(|error| {
                RankError::Backend {
                    reason: format!("Jev client is unavailable: {error}"),
                }
            })?,
            base_url: base_url.clone(),
            credential: credential.into_secret(),
        };
        let ranker = JevRanker::new(Arc::new(evaluator), self.config.clone());
        log::info!(
            "[tool-search] jev ranker bound to backend {} ({})",
            openhuman_core::util::redact::redact_url_for_log(&base_url),
            if fingerprint_changed(cached.as_ref(), fingerprint) {
                "credential or backend changed"
            } else {
                "first search"
            }
        );
        *cached = Some(Cached {
            fingerprint,
            ranker: ranker.clone(),
        });
        Ok(ranker)
    }
}

fn fingerprint(secret: &str, base_url: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    secret.hash(&mut hasher);
    base_url.hash(&mut hasher);
    hasher.finish()
}

fn fingerprint_changed(cached: Option<&Cached>, fingerprint: u64) -> bool {
    cached.is_some_and(|entry| entry.fingerprint != fingerprint)
}

#[async_trait::async_trait]
impl ToolRanker for TinyHumansJevRanker {
    fn kind(&self) -> &'static str {
        JevRanker::KIND
    }

    async fn rank(
        &self,
        intent: &str,
        context: &RankContext,
        candidates: &[RankCandidate],
        limit: usize,
    ) -> Result<Vec<RankHit>, RankError> {
        let ranker = self.current().await?;
        let ranking = ranker
            .rank_detailed(intent, context, candidates, limit)
            .await?;
        log::debug!(
            "[tool-search] jev ranked {} of {} shortlisted (choice_confidence={:.2} needs_tool={:?} none={:.2} latency_ms={} attempts={} input_tokens={:?})",
            ranking.hits.len(),
            ranking.shortlisted,
            ranking.choice_confidence,
            ranking.needs_tool,
            ranking.none_probability,
            ranking.latency.as_millis(),
            ranking.attempts,
            ranking.input_tokens,
        );
        Ok(ranking.hits)
    }
}

#[cfg(test)]
#[path = "ranker_tests.rs"]
mod tests;
