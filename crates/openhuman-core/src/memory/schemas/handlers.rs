//! Thin JSON-RPC handlers: parse params, load config, delegate to the ops,
//! and persist config for the setters.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Map, Value};

use crate::config::Config;
use crate::core::all::ControllerFuture;
use crate::core::Outcome;
use crate::memory::error::{MemoryError, MemoryResult};
use crate::memory::explore::{self, ExploreParams, ItemsGetParams};
use crate::memory::lifecycle::views::{self, JobsRunParams, PackPreviewParams, PolicySetParams};
use crate::memory::types::{
    EmptyParams, EngineSetParams, FetchParams, ForgetParams, ImportStartParams, ImportStateView,
    ItemsListParams, LearnParams, RecallParams, SourceAddedView, SourceRemovedView,
    SourcesAddParams, SourcesListView, SourcesRemoveParams, SourcesSyncParams, SourcesSyncView,
};
use crate::memory::{backfill, brain, engine, import, ops, sources};

fn parse<T: DeserializeOwned>(params: Map<String, Value>) -> Result<T, String> {
    serde_json::from_value(Value::Object(params))
        .map_err(|error| MemoryError::invalid(format!("invalid params: {error}")).into())
}

fn to_json<T: Serialize>(value: T) -> Result<Value, String> {
    Outcome::new(value, Vec::new()).into_cli_compatible_json()
}

fn finish<T: Serialize>(result: MemoryResult<T>) -> Result<Value, String> {
    to_json(result.map_err(String::from)?)
}

async fn load() -> Result<Config, String> {
    crate::config::rpc::load_config_with_timeout().await
}

async fn save(config: &Config) -> MemoryResult<()> {
    config
        .save()
        .await
        .map_err(|error| MemoryError::Engine(format!("saving config failed: {error:#}")))
}

pub(super) fn engines_list(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        parse::<EmptyParams>(params)?;
        to_json(ops::engines_list(&load().await?))
    })
}

pub(super) fn engine_get(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        parse::<EmptyParams>(params)?;
        to_json(ops::engine_get(&load().await?).await)
    })
}

pub(super) fn engine_set(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let params = parse::<EngineSetParams>(params)?;
        let mut config = load().await?;
        let result = async {
            ops::apply_engine_set(&mut config, &params)?;
            save(&config).await
        }
        .await;
        result.map_err(String::from)?;
        engine::invalidate();
        to_json(ops::engine_get(&config).await)
    })
}

pub(super) fn recall(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let params = parse::<RecallParams>(params)?;
        finish(ops::recall(&load().await?, params).await)
    })
}

pub(super) fn fetch(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let params = parse::<FetchParams>(params)?;
        finish(ops::fetch(&load().await?, params).await)
    })
}

pub(super) fn learn(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let params = parse::<LearnParams>(params)?;
        finish(ops::learn(&load().await?, params, None).await)
    })
}

pub(super) fn forget(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let params = parse::<ForgetParams>(params)?;
        finish(ops::forget(&load().await?, params).await)
    })
}

pub(super) fn items_list(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let params = parse::<ItemsListParams>(params)?;
        finish(ops::items_list(&load().await?, params).await)
    })
}

pub(super) fn explore(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let params = parse::<ExploreParams>(params)?;
        finish(explore::explore(&load().await?, params).await)
    })
}

pub(super) fn items_get(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let params = parse::<ItemsGetParams>(params)?;
        finish(explore::items_get(&load().await?, params).await)
    })
}

pub(super) fn policy_get(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        parse::<EmptyParams>(params)?;
        to_json(views::policy_view(&load().await?))
    })
}

pub(super) fn policy_set(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let params = parse::<PolicySetParams>(params)?;
        let mut config = load().await?;
        views::apply_policy_set(&mut config, &params).map_err(String::from)?;
        save(&config).await.map_err(String::from)?;
        to_json(views::policy_view(&config))
    })
}

pub(super) fn pack_preview(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let params = parse::<PackPreviewParams>(params)?;
        finish(views::pack_preview(&load().await?, params).await)
    })
}

pub(super) fn agents_list(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        parse::<EmptyParams>(params)?;
        finish(views::agents_list(&load().await?).await)
    })
}

pub(super) fn conversations_backfill_status(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        parse::<EmptyParams>(params)?;
        finish(backfill::status(&load().await?).await)
    })
}

pub(super) fn conversations_backfill_start(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let params = parse::<backfill::BackfillStartParams>(params)?;
        finish(backfill::start(&load().await?, params).await)
    })
}

pub(super) fn brain_sources(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        parse::<EmptyParams>(params)?;
        finish(brain::sources(&load().await?).await)
    })
}

pub(super) fn brain_search(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let params = parse::<brain::BrainSearchParams>(params)?;
        finish(brain::search(&load().await?, params).await)
    })
}

pub(super) fn brain_ingest(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let params = parse::<brain::BrainIngestParams>(params)?;
        finish(brain::ingest(&load().await?, params).await)
    })
}

pub(super) fn brain_forget(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let params = parse::<brain::BrainForgetParams>(params)?;
        finish(brain::forget(&load().await?, params).await)
    })
}

pub(super) fn jobs_list(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        parse::<EmptyParams>(params)?;
        to_json(views::jobs_list(&load().await?).await)
    })
}

pub(super) fn jobs_run(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let params = parse::<JobsRunParams>(params)?;
        finish(views::jobs_run(&load().await?, params).await)
    })
}

pub(super) fn sources_list(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        parse::<EmptyParams>(params)?;
        to_json(SourcesListView {
            sources: sources::list(&load().await?),
        })
    })
}

pub(super) fn sources_add(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let params = parse::<SourcesAddParams>(params)?;
        let mut config = load().await?;
        let source = sources::apply_add(&mut config, &params).map_err(String::from)?;
        save(&config).await.map_err(String::from)?;
        to_json(SourceAddedView {
            source: sources::view(&source, None),
        })
    })
}

pub(super) fn sources_remove(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let params = parse::<SourcesRemoveParams>(params)?;
        let mut config = load().await?;
        let Some(removed) = sources::apply_remove(&mut config, &params.id) else {
            return to_json(SourceRemovedView { removed: false });
        };
        save(&config).await.map_err(String::from)?;
        sources::state::remove(&config.workspace_dir, &removed.id);
        if params.forget_items.unwrap_or(false) {
            sources::forget_items(&config, &removed.id)
                .await
                .map_err(String::from)?;
        }
        to_json(SourceRemovedView { removed: true })
    })
}

pub(super) fn sources_sync(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let params = parse::<SourcesSyncParams>(params)?;
        let config = load().await?;
        let started = sources::start_sync(&config, params.id.as_deref()).map_err(String::from)?;
        to_json(SourcesSyncView { started })
    })
}

pub(super) fn import_scan(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        parse::<EmptyParams>(params)?;
        finish(import::scan(&load().await?).await)
    })
}

pub(super) fn import_start(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        let params = parse::<ImportStartParams>(params)?;
        let config = load().await?;
        finish(
            import::start(&config, params.consent)
                .await
                .map(|state| ImportStateView { state }),
        )
    })
}

pub(super) fn import_status(params: Map<String, Value>) -> ControllerFuture {
    Box::pin(async move {
        parse::<EmptyParams>(params)?;
        let config = load().await?;
        to_json(json!(ImportStateView {
            state: import::status(&config)
        }))
    })
}
