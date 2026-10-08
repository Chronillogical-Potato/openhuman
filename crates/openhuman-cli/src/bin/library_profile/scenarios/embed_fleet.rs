//! `embed-fleet`: N distinct agents on one `openhuman_embed::Runtime`.
//!
//! The `fleet` scenario measures N copies of one session host under one agent
//! id. This one measures what a library host actually runs: one runtime, then
//! `Runtime::agent` per tenant, each with its own id, home and context, and
//! each driving its own turns through `Agent::turn`.
//!
//! Reported on top of the shared `fleet` fields:
//! - `marginal_rss_kib_per_agent`: `baseline → constructed`, the standing cost
//!   of an idle agent.
//! - `loaded_marginal_rss_kib_per_agent`: `baseline → loaded`, after every
//!   agent has run its turns.
//!
//! Env knobs are the `fleet` ones (`OPENHUMAN_PROFILE_AGENTS`, `_TURNS`,
//! `_MOCK_LATENCY_MS`, `_TARGET_AGENTS`, `_RAM_BUDGET_MIB`).

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use openhuman_core::inference::provider::factory::test_provider_override;
use openhuman_core::platform::proc_metrics;
use openhuman_tinyhumans::embed::{Access, Agent, AgentSpec, Provider, Runtime, Workspace};

use super::fleet::{env_u64, env_usize, latency_summary, raise_fd_limit};
use crate::harness::{fixture, measure, FleetBudget, ProfileResult, Recorder};
use crate::mock::LatencyMock;

const DEFAULT_AGENTS: usize = 100;
const DEFAULT_TURNS: usize = 3;
const DEFAULT_TARGET_AGENTS: u64 = 1000;
const DEFAULT_RAM_BUDGET_MIB: u64 = 2048;
const IDLE_WINDOW: Duration = Duration::from_secs(10);
/// A routed provider keeps the turn off the managed backend; the installed
/// mock model answers before any request is sent.
const MOCK_ROUTE: &str = "http://127.0.0.1:9/v1";

/// The id of the `index`th fleet agent.
fn agent_id(index: usize) -> String {
    format!("agent-{index}")
}

#[derive(Default)]
struct EmbedFleetMetrics {
    agents_built: usize,
    baseline_rss_kib: u64,
    constructed_rss_kib: u64,
    loaded_rss_kib: u64,
    idle_cpu_ms: u64,
    latency_ms: Vec<u128>,
}

fn build_agents(runtime: &Runtime, n: usize, rec: &Recorder) -> Result<Vec<Agent>> {
    let stride = (n / 10).max(1);
    let mut agents = Vec::with_capacity(n);
    for index in 0..n {
        match runtime.agent(AgentSpec::new(agent_id(index)).access(Access::readonly())) {
            Ok(agent) => agents.push(agent),
            Err(err) => {
                eprintln!(
                    "[library-profile] embed-fleet: construction failed at agent {} — {err}",
                    index + 1
                );
                rec.checkpoint(format!("construction-failed-{}", agents.len()))?;
                break;
            }
        }
        let built = index + 1;
        if built % stride == 0 || built == n {
            rec.checkpoint(format!("built-{built}"))?;
        }
    }
    Ok(agents)
}

async fn drive(agents: &[Agent], turns: usize) -> Vec<u128> {
    let mut handles = Vec::with_capacity(agents.len());
    for (index, agent) in agents.iter().cloned().enumerate() {
        let stagger = Duration::from_millis(((index as u64) * 10).min(2000));
        handles.push(tokio::spawn(async move {
            tokio::time::sleep(stagger).await;
            let session = format!("fleet-{}", agent.id());
            let mut latencies = Vec::with_capacity(turns);
            for _ in 0..turns {
                let started = Instant::now();
                let outcome = agent
                    .turn("Give me a one-line status update.")
                    .session(&session)
                    .send()
                    .await;
                let elapsed = started.elapsed().as_millis();
                match outcome {
                    Ok(outcome) if !outcome.reply.trim().is_empty() => latencies.push(elapsed),
                    Ok(_) => eprintln!(
                        "[library-profile] embed-fleet: empty reply agent={}",
                        agent.id()
                    ),
                    Err(err) => eprintln!(
                        "[library-profile] embed-fleet: turn error agent={} — {err}",
                        agent.id()
                    ),
                }
            }
            latencies
        }));
    }
    let mut all = Vec::new();
    for handle in handles {
        match handle.await {
            Ok(mut latencies) => all.append(&mut latencies),
            Err(err) => eprintln!("[library-profile] embed-fleet: task join error — {err}"),
        }
    }
    all
}

fn per_agent(from_kib: u64, to_kib: u64, agents: usize) -> Option<f64> {
    (agents > 0).then(|| (to_kib as f64 - from_kib as f64) / agents as f64)
}

pub async fn run() -> Result<ProfileResult> {
    let agents_requested = env_usize("OPENHUMAN_PROFILE_AGENTS", DEFAULT_AGENTS);
    let turns = env_usize("OPENHUMAN_PROFILE_TURNS", DEFAULT_TURNS);
    let target_agents = env_u64("OPENHUMAN_PROFILE_TARGET_AGENTS", DEFAULT_TARGET_AGENTS);
    let ram_budget_mib = env_u64("OPENHUMAN_PROFILE_RAM_BUDGET_MIB", DEFAULT_RAM_BUDGET_MIB);

    raise_fd_limit();

    let fixture = fixture()?;
    let mock = LatencyMock::from_env("Fleet agent: nothing needs your attention.");
    let _provider = test_provider_override::install_model(mock.clone());
    let runtime = Arc::new(
        Runtime::builder()
            .config(fixture.config.clone())
            .workspace(Workspace::Ephemeral)
            .provider(Provider::openai_compatible(MOCK_ROUTE, "sk-profile").model("profile-mock"))
            .build()
            .await
            .context("build the embed runtime")?,
    );
    eprintln!(
        "[library-profile] embed-fleet: agents={agents_requested} turns={turns} \
         target={target_agents} budget_mib={ram_budget_mib}"
    );

    let metrics = Arc::new(Mutex::new(EmbedFleetMetrics::default()));
    let metrics_for_workload = Arc::clone(&metrics);
    let runtime_for_workload = Arc::clone(&runtime);

    let mut result = measure(
        "embed-fleet",
        agents_requested,
        Some(turns),
        move |rec| async move {
            rec.checkpoint("baseline")?;
            let baseline_rss = proc_metrics::sample_self()?.rss_kib;

            let agents = build_agents(&runtime_for_workload, agents_requested, &rec)?;
            rec.checkpoint("constructed")?;
            let constructed_rss = proc_metrics::sample_self()?.rss_kib;
            eprintln!(
                "[library-profile] embed-fleet: built {}/{agents_requested} agents; \
                 baseline_rss={baseline_rss} constructed_rss={constructed_rss}",
                agents.len()
            );

            rec.checkpoint("idle-start")?;
            let idle_start = proc_metrics::sample_self()?;
            tokio::time::sleep(IDLE_WINDOW).await;
            rec.checkpoint("idle-end")?;
            let idle_end = proc_metrics::sample_self()?;
            let idle_cpu_ms = (idle_end.cpu_user_ms + idle_end.cpu_system_ms)
                .saturating_sub(idle_start.cpu_user_ms + idle_start.cpu_system_ms);

            let latencies = drive(&agents, turns).await;
            tokio::time::sleep(Duration::from_millis(500)).await;
            rec.checkpoint("loaded")?;
            let loaded_rss = proc_metrics::sample_self()?.rss_kib;

            let mut m = metrics_for_workload.lock().expect("metrics lock");
            m.agents_built = agents.len();
            m.baseline_rss_kib = baseline_rss;
            m.constructed_rss_kib = constructed_rss;
            m.loaded_rss_kib = loaded_rss;
            m.idle_cpu_ms = idle_cpu_ms;
            m.latency_ms = latencies;
            drop(m);
            drop(agents);
            Ok(())
        },
    )
    .await?;

    let metrics = Arc::try_unwrap(metrics)
        .map(|m| m.into_inner().expect("metrics lock"))
        .unwrap_or_default();
    let agents_built = metrics.agents_built;
    let marginal = per_agent(
        metrics.baseline_rss_kib,
        metrics.constructed_rss_kib,
        agents_built,
    );
    let loaded = per_agent(
        metrics.baseline_rss_kib,
        metrics.loaded_rss_kib,
        agents_built,
    );
    let base_mib = metrics.baseline_rss_kib as f64 / 1024.0;
    let marginal_mib = loaded.or(marginal).unwrap_or(0.0) / 1024.0;
    let projected = base_mib + marginal_mib * target_agents as f64;

    result.agents = Some(agents_requested);
    result.agents_built = Some(agents_built);
    result.marginal_rss_kib_per_agent = marginal;
    result.loaded_marginal_rss_kib_per_agent = loaded;
    result.idle_cpu_ms = Some(metrics.idle_cpu_ms);
    result.turn_latency_ms = latency_summary(metrics.latency_ms);
    result.budget = Some(FleetBudget {
        target_agents,
        ram_budget_mib,
        projected_rss_mib_at_target: projected,
        fits: projected <= ram_budget_mib as f64,
    });
    drop(runtime);
    Ok(result)
}
