//! `tool-search-bench` — how well does each `tool_search` ranker pick the
//! right tool for an intent?
//!
//! The catalogue is the real thing: every tool the orchestrator session
//! registers (built exactly as a session builds it, with a temp workspace),
//! plus the recorded Composio catalogues under `tests/fixtures/composio_*.json`
//! (≈1,000 actions across nine toolkits) as the deferred per-action tools a
//! signed-in workspace would synthesise. The intents are
//! `tests/fixtures/tool_search/intents.jsonl`: hand-written requests, each
//! labelled with the tool it should reach (or `none` for a request no tool
//! should answer).
//!
//! Three rankers are measured on the same rows:
//!
//! - `bm25` — `tinytools::Bm25Ranker`, what the harness answers with when
//!   nothing else is installed.
//! - `overlap` — `tinyagents_harness::tool::select::rank_tools_by_prompt`,
//!   the verb-gated token overlap the Composio sub-agent narrows with today.
//! - `jev` — `tinytools_jev::JevRanker`: BM25 retrieves the top 20, one Jev
//!   `Choice` decides. Needs a key: `OPENHUMAN_BACKEND_API_KEY` (through the
//!   TinyHumans proxy, `BACKEND_URL` to override the base) or
//!   `TYPESAFE_API_KEY` (TypeSafe directly). Skipped without one.
//!
//! Reported per ranker: top-1 and top-3 accuracy over the labelled rows,
//! recall@20 of the BM25 retriever (the ceiling Jev can reach), the needless
//! rate (a `none` row answered with a hit), p50/p95 latency, input tokens and
//! USD from the provider's `usage`, and a per-family confusion table.
//!
//! ```text
//! cargo run -p openhuman-cli --bin tool-search-bench -- --ranker all
//! cargo run -p openhuman-cli --bin tool-search-bench -- --dump-catalogue
//! ```

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tinytools::{Bm25Ranker, RankCandidate, RankContext, ToolRanker};

/// jev-1.13 list price for input tokens, USD per million; output is free.
const JEV_USD_PER_MILLION_INPUT: f64 = 0.042;
/// Toolkits with a recorded catalogue under `tests/fixtures/`.
const FIXTURE_TOOLKITS: &[&str] = &[
    "gmail",
    "slack",
    "github",
    "notion",
    "googledrive",
    "googlesheets",
    "reddit",
    "facebook",
    "instagram",
];

#[derive(Debug, Clone, Deserialize)]
struct IntentRow {
    intent: String,
    /// The tool that should answer, or `"none"`.
    expected: String,
    #[serde(default)]
    family: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CatalogueEntry {
    name: String,
    family: Option<String>,
    description: String,
    #[serde(skip)]
    parameters: serde_json::Value,
}

impl CatalogueEntry {
    fn candidate(&self) -> RankCandidate {
        let mut summary = String::with_capacity(self.description.len() + 64);
        summary.push_str(&self.name);
        summary.push(' ');
        summary.push_str(&self.name.replace('_', " "));
        summary.push(' ');
        summary.push_str(&self.description);
        if let Some(props) = self
            .parameters
            .get("properties")
            .and_then(|v| v.as_object())
        {
            for key in props.keys() {
                summary.push(' ');
                summary.push_str(key);
            }
        }
        RankCandidate {
            key: self.name.clone(),
            family: self.family.clone(),
            summary,
        }
    }
}

#[derive(Debug, Default, Serialize)]
struct RankerReport {
    ranker: String,
    rows: usize,
    labelled: usize,
    top1: usize,
    top3: usize,
    /// Labelled rows whose expected tool BM25 retrieved in its top 20.
    recall_at_20: usize,
    none_rows: usize,
    needless: usize,
    errors: usize,
    latency_ms: Vec<u64>,
    input_tokens: u64,
    usd: f64,
    /// `expected family -> top-1 family -> count`, labelled rows only.
    confusion: BTreeMap<String, BTreeMap<String, usize>>,
    misses: Vec<Miss>,
}

#[derive(Debug, Clone, Serialize)]
struct Miss {
    intent: String,
    expected: String,
    got: Vec<String>,
}

impl RankerReport {
    fn percentile(&self, p: f64) -> u64 {
        if self.latency_ms.is_empty() {
            return 0;
        }
        let mut sorted = self.latency_ms.clone();
        sorted.sort_unstable();
        let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
        sorted[idx.min(sorted.len() - 1)]
    }
}

struct Args {
    ranker: String,
    intents: PathBuf,
    dump_catalogue: bool,
    json_out: Option<PathBuf>,
    top_k: usize,
    retrieval_k: usize,
    misses: bool,
}

fn parse_args() -> Args {
    let mut args = Args {
        ranker: "all".to_string(),
        intents: repo_root().join("tests/fixtures/tool_search/intents.jsonl"),
        dump_catalogue: false,
        json_out: None,
        top_k: 3,
        retrieval_k: 20,
        misses: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--ranker" => args.ranker = it.next().unwrap_or_default(),
            "--intents" => args.intents = PathBuf::from(it.next().unwrap_or_default()),
            "--dump-catalogue" => args.dump_catalogue = true,
            "--json" => args.json_out = Some(PathBuf::from(it.next().unwrap_or_default())),
            "--top-k" => args.top_k = it.next().and_then(|v| v.parse().ok()).unwrap_or(3),
            "--retrieval-k" => {
                args.retrieval_k = it.next().and_then(|v| v.parse().ok()).unwrap_or(20)
            }
            "--misses" => args.misses = true,
            "-h" | "--help" => {
                eprintln!(
                    "usage: tool-search-bench [--ranker all|bm25|overlap|jev] [--intents FILE] \
                     [--dump-catalogue] [--json OUT] [--top-k N] [--retrieval-k N] [--misses]"
                );
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    args
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("OPENHUMAN_REPOSITORY_ROOT"))
}

/// The orchestrator's registered tools, built the way a session builds them.
fn core_catalogue() -> Result<Vec<CatalogueEntry>> {
    use openhuman_core::agent::harness::AgentDefinitionRegistry;
    use openhuman_core::agent::OpenHumanSessionHost;

    let _ = AgentDefinitionRegistry::init_global_builtins();
    let tmp = tempfile::TempDir::new()?;
    let config = openhuman_core::config::Config {
        workspace_dir: tmp.path().join("workspace"),
        action_dir: tmp.path().join("workspace"),
        config_path: tmp.path().join("config.toml"),
        ..openhuman_core::config::Config::default()
    };
    std::fs::create_dir_all(&config.workspace_dir)?;
    let host = OpenHumanSessionHost::from_config_for_agent(&config, "orchestrator")
        .context("build orchestrator session")?;
    let mut out = Vec::new();
    for spec in host.tool_specs() {
        // Pack members carry their pack as the family; everything else is a
        // core tool with no family, like the registry sees it.
        let family = openhuman_core::tools::toolpacks::pack_for_tool(&spec.name)
            .map(|pack| pack.id.to_string());
        out.push(CatalogueEntry {
            name: spec.name.clone(),
            family,
            description: spec.description.clone(),
            parameters: spec.parameters.clone(),
        });
    }
    Ok(out)
}

/// The recorded Composio catalogues, one deferred action per entry.
fn composio_catalogue() -> Result<Vec<CatalogueEntry>> {
    let mut out = Vec::new();
    for toolkit in FIXTURE_TOOLKITS {
        let path = repo_root().join(format!("tests/fixtures/composio_{toolkit}.json"));
        let raw = std::fs::read_to_string(&path).with_context(|| format!("read {path:?}"))?;
        let parsed: serde_json::Value = serde_json::from_str(&raw)?;
        let tools = parsed
            .pointer("/result/result/tools")
            .and_then(|t| t.as_array())
            .with_context(|| format!("missing /result/result/tools in {path:?}"))?;
        for tool in tools {
            let name = tool
                .pointer("/function/name")
                .and_then(|n| n.as_str())
                .unwrap_or_default()
                .to_string();
            if name.is_empty() {
                continue;
            }
            out.push(CatalogueEntry {
                name,
                family: Some((*toolkit).to_string()),
                description: tool
                    .pointer("/function/description")
                    .and_then(|d| d.as_str())
                    .unwrap_or_default()
                    .to_string(),
                parameters: tool
                    .pointer("/function/parameters")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({"type": "object"})),
            });
        }
    }
    Ok(out)
}

fn load_intents(path: &PathBuf) -> Result<Vec<IntentRow>> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("read {path:?}"))?;
    let mut rows = Vec::new();
    for (i, line) in raw.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let row: IntentRow =
            serde_json::from_str(line).with_context(|| format!("{path:?}:{} bad row", i + 1))?;
        rows.push(row);
    }
    Ok(rows)
}

use openhuman_core::agent::tinyagents::discovery::OverlapRanker;

#[cfg(feature = "jev")]
fn jev_ranker(retrieval_k: usize) -> Option<(Arc<dyn ToolRanker>, Arc<tinytools_jev::JevRanker>)> {
    use tinytools_jev::{ClientConfig, JevRanker, JevRankerConfig};
    let client = if let Ok(key) = std::env::var("OPENHUMAN_BACKEND_API_KEY") {
        let mut client = ClientConfig::tinyhumans_openrouter(key);
        if let Ok(base) = std::env::var("BACKEND_URL") {
            if !base.trim().is_empty() {
                client.base_url = base.trim().trim_end_matches('/').to_string();
            }
        }
        client
    } else if let Ok(key) = std::env::var("TYPESAFE_API_KEY") {
        ClientConfig::new(key)
    } else {
        return None;
    };
    let ranker = JevRanker::from_config(
        client,
        JevRankerConfig::new()
            .with_retrieval_k(retrieval_k)
            .with_timeout(Duration::from_secs(15)),
    )
    .ok()?;
    let ranker = Arc::new(ranker);
    Some((ranker.clone() as Arc<dyn ToolRanker>, ranker))
}

#[cfg(not(feature = "jev"))]
fn jev_ranker(_retrieval_k: usize) -> Option<(Arc<dyn ToolRanker>, Arc<()>)> {
    None
}

fn family_of<'a>(catalogue: &'a [CatalogueEntry], name: &str) -> &'a str {
    catalogue
        .iter()
        .find(|e| e.name == name)
        .and_then(|e| e.family.as_deref())
        .unwrap_or("core")
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = parse_args();
    let mut catalogue = core_catalogue()?;
    let core_count = catalogue.len();
    catalogue.extend(composio_catalogue()?);
    eprintln!(
        "catalogue: {} tools ({} core, {} composio actions)",
        catalogue.len(),
        core_count,
        catalogue.len() - core_count
    );
    if args.dump_catalogue {
        for entry in &catalogue {
            println!(
                "{}\t{}\t{}",
                entry.name,
                entry.family.as_deref().unwrap_or("-"),
                entry.description.chars().take(100).collect::<String>()
            );
        }
        return Ok(());
    }
    let names: std::collections::HashSet<&str> =
        catalogue.iter().map(|e| e.name.as_str()).collect();
    let rows = load_intents(&args.intents)?;
    for row in &rows {
        if row.expected != "none" && !names.contains(row.expected.as_str()) {
            anyhow::bail!(
                "intent {:?} expects `{}`, which is not in the catalogue",
                row.intent,
                row.expected
            );
        }
    }
    let candidates: Vec<RankCandidate> = catalogue.iter().map(CatalogueEntry::candidate).collect();

    let mut rankers: Vec<(String, Arc<dyn ToolRanker>)> = Vec::new();
    let want = |k: &str| args.ranker == "all" || args.ranker == k;
    if want("bm25") {
        rankers.push(("bm25".into(), Arc::new(Bm25Ranker)));
    }
    if want("overlap") {
        rankers.push(("overlap".into(), Arc::new(OverlapRanker)));
    }
    if want("jev") {
        match jev_ranker(args.retrieval_k) {
            Some((ranker, _)) => rankers.push(("jev".into(), ranker)),
            None => eprintln!(
                "jev: skipped (set OPENHUMAN_BACKEND_API_KEY or TYPESAFE_API_KEY; build with the `jev` feature)"
            ),
        }
    }

    let mut reports = Vec::new();
    for (kind, ranker) in &rankers {
        let mut report = RankerReport {
            ranker: kind.clone(),
            rows: rows.len(),
            ..RankerReport::default()
        };
        for row in &rows {
            let started = Instant::now();
            let result = ranker
                .rank(&row.intent, &RankContext::empty(), &candidates, args.top_k)
                .await;
            report
                .latency_ms
                .push(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX));
            let hits = match result {
                Ok(hits) => hits,
                Err(error) => {
                    report.errors += 1;
                    eprintln!("{kind}: {:?} -> {error}", row.intent);
                    Vec::new()
                }
            };
            let got: Vec<String> = hits.iter().map(|h| h.key.clone()).collect();
            if row.expected == "none" {
                report.none_rows += 1;
                if !got.is_empty() {
                    report.needless += 1;
                }
                continue;
            }
            report.labelled += 1;
            if got.first().map(String::as_str) == Some(row.expected.as_str()) {
                report.top1 += 1;
            }
            if got.iter().any(|g| g == &row.expected) {
                report.top3 += 1;
            } else if args.misses {
                report.misses.push(Miss {
                    intent: row.intent.clone(),
                    expected: row.expected.clone(),
                    got: got.clone(),
                });
            }
            let retrieved = Bm25Ranker::rank_sync(&candidates, &row.intent, args.retrieval_k);
            if retrieved.iter().any(|h| h.key == row.expected) {
                report.recall_at_20 += 1;
            }
            let expected_family = row
                .family
                .clone()
                .unwrap_or_else(|| family_of(&catalogue, &row.expected).to_string());
            let got_family = got
                .first()
                .map(|g| family_of(&catalogue, g).to_string())
                .unwrap_or_else(|| "(none)".to_string());
            *report
                .confusion
                .entry(expected_family)
                .or_default()
                .entry(got_family)
                .or_default() += 1;
        }
        reports.push(report);
    }

    #[cfg(feature = "jev")]
    if let Some(report) = reports.iter_mut().find(|r| r.ranker == "jev") {
        // Tokens and cost: one detailed pass over the labelled rows so the
        // number is the provider's own `usage`, not an estimate.
        if let Some((_, detailed)) = jev_ranker(args.retrieval_k) {
            let mut tokens = 0_u64;
            let mut counted = 0_u64;
            for row in rows.iter().take(25) {
                if let Ok(ranking) = detailed
                    .rank_detailed(&row.intent, &RankContext::empty(), &candidates, args.top_k)
                    .await
                {
                    if let Some(t) = ranking.input_tokens {
                        tokens += t;
                        counted += 1;
                    }
                }
            }
            if counted > 0 {
                let per_search = tokens as f64 / counted as f64;
                report.input_tokens = per_search.round() as u64;
                report.usd = per_search * JEV_USD_PER_MILLION_INPUT / 1_000_000.0;
            }
        }
    }

    println!("| ranker | rows | top-1 | top-3 | recall@{} (bm25) | needless (of {}) | errors | p50 ms | p95 ms | tokens/search | USD/search |",
        args.retrieval_k,
        reports.first().map_or(0, |r| r.none_rows));
    println!("|---|---|---|---|---|---|---|---|---|---|---|");
    for r in &reports {
        let pct = |n: usize, d: usize| {
            if d == 0 {
                "n/a".to_string()
            } else {
                format!("{:.1}%", 100.0 * n as f64 / d as f64)
            }
        };
        println!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            r.ranker,
            r.rows,
            pct(r.top1, r.labelled),
            pct(r.top3, r.labelled),
            pct(r.recall_at_20, r.labelled),
            r.needless,
            r.errors,
            r.percentile(0.5),
            r.percentile(0.95),
            if r.input_tokens == 0 {
                "-".to_string()
            } else {
                r.input_tokens.to_string()
            },
            if r.usd == 0.0 {
                "-".to_string()
            } else {
                format!("${:.5}", r.usd)
            },
        );
    }
    for r in &reports {
        println!(
            "\n### {} — top-1 family confusion (expected → got)",
            r.ranker
        );
        for (expected, gots) in &r.confusion {
            let line: Vec<String> = gots.iter().map(|(g, n)| format!("{g}:{n}")).collect();
            println!("- {expected}: {}", line.join(", "));
        }
        if args.misses && !r.misses.is_empty() {
            println!("\n### {} — misses", r.ranker);
            for m in &r.misses {
                println!("- {:?} expected `{}` got {:?}", m.intent, m.expected, m.got);
            }
        }
    }
    if let Some(out) = &args.json_out {
        std::fs::write(out, serde_json::to_string_pretty(&reports)?)?;
        eprintln!("wrote {out:?}");
    }
    Ok(())
}
