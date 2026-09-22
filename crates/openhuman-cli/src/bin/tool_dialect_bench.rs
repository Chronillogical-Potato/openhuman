//! `tool-dialect-bench` — how well a local model calls tools under each text
//! dialect, and what each dialect costs in prompt tokens.
//!
//! The experiment behind `agent.tool_dispatcher = "python" | "typescript"`:
//! render the tool catalogue as function signatures instead of JSON schemas
//! or P-Format slots, and see whether a small code-trained model (an 8B
//! Ollama model) calls tools more reliably for fewer tokens.
//!
//! For every `dialect × model × task` the bench composes the system prompt
//! exactly as OpenHuman's `ToolsSection` + the dialect's protocol block would
//! (catalogue, then protocol), sends one user turn with **no** schemas on the
//! wire, reads the answer back through `tinytools_agent::parse_text` with the
//! same registry the harness builds, and records:
//!
//! * `usage.input_tokens` / `output_tokens` as the provider reports them
//!   (Ollama fills them from `prompt_eval_count` / `eval_count`);
//! * system-prompt bytes;
//! * whether a call was recovered at all, whether it named the expected tool,
//!   and whether its arguments matched (exactly, and as a superset);
//! * latency and the `CallSource` the call came through.
//!
//! Manual, network-touching, never run by CI:
//!
//! ```text
//! ollama pull qwen3:8b
//! cargo run -p openhuman-cli --bin tool-dialect-bench -- \
//!     --model qwen3:8b --dialects xml,pformat,python,typescript --trials 3
//! ```
//!
//! `OLLAMA_HOST` / `OPENHUMAN_LOCAL_INFERENCE_URL` pick the server, as for
//! the product.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use tinyinference_llm::message::Message;
use tinyinference_llm::model::{ChatModel, ModelRequest};
use tinytools::ToolSpec;
use tinytools_agent::dialect::{CodeDialect, CodeStyle, PFormatDialect, ToolDialect, XmlDialect};
use tinytools_agent::render::{render_code_catalogue, render_pformat_catalogue};
use tinytools_agent::{build_registry, parse_text, CallSource, PFormatRegistry, ParseOptions};

/// The role line every prompt starts with, so the dialect block is the only
/// thing that differs between runs.
const BASE_SYSTEM_PROMPT: &str = "You are a helpful assistant with tools. When the user's request \
     needs a tool, call it; do not describe what you would do instead of doing it. \
     Do not ask clarifying questions for these tasks.";

/// Default per-call ceiling: the answer is one call, but a thinking model
/// (qwen3) spends most of its budget in the reasoning channel first, and a
/// runaway 8B model can otherwise generate for minutes. `--max-output-tokens`
/// overrides it.
const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 1500;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Dialect {
    Xml,
    Pformat,
    Python,
    Typescript,
}

impl Dialect {
    fn parse(raw: &str) -> Result<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "xml" => Ok(Self::Xml),
            "pformat" => Ok(Self::Pformat),
            "python" => Ok(Self::Python),
            "typescript" => Ok(Self::Typescript),
            other => anyhow::bail!("unknown dialect {other:?} (xml|pformat|python|typescript)"),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Xml => "xml",
            Self::Pformat => "pformat",
            Self::Python => "python",
            Self::Typescript => "typescript",
        }
    }

    /// The tool section as OpenHuman renders it: catalogue first, then the
    /// dialect's protocol block. The XML dialect embeds its own catalogue in
    /// the block, so it gets no separate one.
    fn tool_section(self, specs: &[ToolSpec], registry: &PFormatRegistry) -> String {
        match self {
            Self::Xml => XmlDialect.prompt_instructions(specs),
            Self::Pformat => {
                let mut out = render_pformat_catalogue(specs);
                out.push('\n');
                out.push_str(&PFormatDialect::new(registry.clone()).prompt_instructions(specs));
                out
            }
            Self::Python | Self::Typescript => {
                let style = self.code_style();
                let mut out = render_code_catalogue(specs, style);
                out.push('\n');
                out.push_str(&CodeDialect::instructions(style));
                out
            }
        }
    }

    fn code_style(self) -> CodeStyle {
        match self {
            Self::Typescript => CodeStyle::TypeScript,
            _ => CodeStyle::Python,
        }
    }
}

/// One thing we ask the model to do, and the call that would do it.
struct Task {
    prompt: &'static str,
    tool: &'static str,
    args: Value,
}

fn tool(name: &str, description: &str, parameters: Value) -> ToolSpec {
    ToolSpec {
        name: name.to_string(),
        description: description.to_string(),
        parameters,
    }
}

/// Ten tools with the shapes real OpenHuman tools have: required and optional
/// parameters, integers, booleans, enums, lists, and one nested object.
fn fixture_tools() -> Vec<ToolSpec> {
    vec![
        tool(
            "read_file",
            "Read a text file from the workspace.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Path relative to the workspace root"},
                    "limit": {"type": "integer", "description": "Maximum number of lines to return"}
                },
                "required": ["path"]
            }),
        ),
        tool(
            "write_file",
            "Create or overwrite a text file in the workspace.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"}
                },
                "required": ["path", "content"]
            }),
        ),
        tool(
            "list_dir",
            "List the entries of a directory.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Directory to list; defaults to the workspace root"}
                }
            }),
        ),
        tool(
            "search_files",
            "Search file contents for a string.",
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "glob": {"type": "string", "description": "Restrict to files matching this glob"},
                    "max_results": {"type": "integer"}
                },
                "required": ["query"]
            }),
        ),
        tool(
            "shell",
            "Run a shell command and return its output.",
            json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"},
                    "background": {"type": "boolean", "description": "Run detached and return immediately"},
                    "timeout_secs": {"type": "number"}
                },
                "required": ["command"]
            }),
        ),
        tool(
            "get_weather",
            "Get the current weather for a location.",
            json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string", "description": "City name"},
                    "unit": {"type": "string", "enum": ["metric", "imperial"]}
                },
                "required": ["location"]
            }),
        ),
        tool(
            "send_email",
            "Send an email.",
            json!({
                "type": "object",
                "properties": {
                    "to": {"type": "string"},
                    "subject": {"type": "string"},
                    "body": {"type": "string"},
                    "cc": {"type": "array", "items": {"type": "string"}}
                },
                "required": ["to", "subject", "body"]
            }),
        ),
        tool(
            "calendar_create",
            "Create a calendar event.",
            json!({
                "type": "object",
                "properties": {
                    "title": {"type": "string"},
                    "start": {"type": "string", "description": "ISO 8601 start time"},
                    "duration_minutes": {"type": "integer"},
                    "attendees": {"type": "array", "items": {"type": "string"}}
                },
                "required": ["title", "start"]
            }),
        ),
        tool(
            "memory_recall",
            "Search long-term memory for relevant notes.",
            json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "limit": {"type": "integer"}
                },
                "required": ["query"]
            }),
        ),
        tool(
            "http_fetch",
            "Fetch a URL.",
            json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string"},
                    "method": {"type": "string", "enum": ["GET", "POST"]},
                    "headers": {"type": "object", "description": "Extra request headers"}
                },
                "required": ["url"]
            }),
        ),
    ]
}

/// Twenty requests, each answerable by exactly one call. Optional
/// arguments are only expected when the prompt names their value.
fn fixture_tasks() -> Vec<Task> {
    vec![
        Task { prompt: "Show me the file README.md.", tool: "read_file", args: json!({"path": "README.md"}) },
        Task { prompt: "Read the first 20 lines of src/main.rs.", tool: "read_file", args: json!({"path": "src/main.rs", "limit": 20}) },
        Task { prompt: "Create a file called notes.txt containing exactly: hello world", tool: "write_file", args: json!({"path": "notes.txt", "content": "hello world"}) },
        Task { prompt: "What files are in the workspace root?", tool: "list_dir", args: json!({}) },
        Task { prompt: "List the contents of the docs directory.", tool: "list_dir", args: json!({"path": "docs"}) },
        Task { prompt: "Find every occurrence of the word TODO in the code.", tool: "search_files", args: json!({"query": "TODO"}) },
        Task { prompt: "Search for the string 'unwrap()' but only in Rust files (*.rs), at most 5 results.", tool: "search_files", args: json!({"query": "unwrap()", "glob": "*.rs", "max_results": 5}) },
        Task { prompt: "Run the command `cargo test`.", tool: "shell", args: json!({"command": "cargo test"}) },
        Task { prompt: "Start the dev server with `npm run dev` in the background.", tool: "shell", args: json!({"command": "npm run dev", "background": true}) },
        Task { prompt: "What's the weather in London?", tool: "get_weather", args: json!({"location": "London"}) },
        Task { prompt: "What's the weather in Chicago, in imperial units?", tool: "get_weather", args: json!({"location": "Chicago", "unit": "imperial"}) },
        Task { prompt: "Email bob@example.com with the subject 'Lunch' and the body 'Are you free at noon?'", tool: "send_email", args: json!({"to": "bob@example.com", "subject": "Lunch", "body": "Are you free at noon?"}) },
        Task { prompt: "Send an email to ana@example.com, subject 'Report', body 'Attached is the report.', and cc carl@example.com.", tool: "send_email", args: json!({"to": "ana@example.com", "subject": "Report", "body": "Attached is the report.", "cc": ["carl@example.com"]}) },
        Task { prompt: "Put a 'Standup' meeting on my calendar starting 2026-10-01T09:00:00.", tool: "calendar_create", args: json!({"title": "Standup", "start": "2026-10-01T09:00:00"}) },
        Task { prompt: "Schedule 'Design review' at 2026-10-02T14:00:00 for 45 minutes with dana@example.com and eli@example.com.", tool: "calendar_create", args: json!({"title": "Design review", "start": "2026-10-02T14:00:00", "duration_minutes": 45, "attendees": ["dana@example.com", "eli@example.com"]}) },
        Task { prompt: "What do you remember about my dog's vet?", tool: "memory_recall", args: json!({"query": "dog's vet"}) },
        Task { prompt: "Recall up to 3 notes about the Berlin trip.", tool: "memory_recall", args: json!({"query": "Berlin trip", "limit": 3}) },
        Task { prompt: "Fetch https://example.com/status", tool: "http_fetch", args: json!({"url": "https://example.com/status"}) },
        Task { prompt: "POST to https://api.example.com/ping", tool: "http_fetch", args: json!({"url": "https://api.example.com/ping", "method": "POST"}) },
        Task { prompt: "Read config.toml, but only the first 5 lines.", tool: "read_file", args: json!({"path": "config.toml", "limit": 5}) },
    ]
}

#[derive(Debug, Clone, Serialize)]
struct Row {
    model: String,
    dialect: &'static str,
    trial: usize,
    task: usize,
    expected_tool: &'static str,
    system_prompt_bytes: usize,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    latency_ms: u128,
    recovered: bool,
    name_ok: bool,
    args_exact: bool,
    args_superset: bool,
    source: Option<String>,
    parsed_tool: Option<String>,
    parsed_args: Option<Value>,
    raw_text: String,
    error: Option<String>,
}

#[derive(Debug, Default)]
struct Summary {
    n: usize,
    input_tokens: u64,
    input_samples: u64,
    output_tokens: u64,
    output_samples: u64,
    system_prompt_bytes: usize,
    latency_ms: u128,
    recovered: usize,
    name_ok: usize,
    args_exact: usize,
    args_superset: usize,
    errors: usize,
    sources: BTreeMap<String, usize>,
}

impl Summary {
    fn add(&mut self, row: &Row) {
        self.n += 1;
        if let Some(tokens) = row.input_tokens {
            self.input_tokens += tokens;
            self.input_samples += 1;
        }
        if let Some(tokens) = row.output_tokens {
            self.output_tokens += tokens;
            self.output_samples += 1;
        }
        self.system_prompt_bytes = row.system_prompt_bytes;
        self.latency_ms += row.latency_ms;
        self.recovered += usize::from(row.recovered);
        self.name_ok += usize::from(row.name_ok);
        self.args_exact += usize::from(row.args_exact);
        self.args_superset += usize::from(row.args_superset);
        self.errors += usize::from(row.error.is_some());
        if let Some(source) = &row.source {
            *self.sources.entry(source.clone()).or_default() += 1;
        }
    }

    fn pct(&self, count: usize) -> String {
        if self.n == 0 {
            return "-".into();
        }
        format!("{:.0}%", 100.0 * count as f64 / self.n as f64)
    }

    fn avg(total: u64, samples: u64) -> String {
        if samples == 0 {
            return "-".into();
        }
        format!("{}", total / samples)
    }
}

struct Args {
    models: Vec<String>,
    dialects: Vec<Dialect>,
    trials: usize,
    json: bool,
    verbose: bool,
    tasks: Option<Vec<usize>>,
    max_output_tokens: u32,
}

fn parse_args() -> Result<Args> {
    let mut models = vec!["qwen3:8b".to_string()];
    let mut dialects = vec![
        Dialect::Xml,
        Dialect::Pformat,
        Dialect::Python,
        Dialect::Typescript,
    ];
    let mut trials = 1usize;
    let mut json = false;
    let mut verbose = false;
    let mut tasks = None;
    let mut max_output_tokens = DEFAULT_MAX_OUTPUT_TOKENS;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--model" | "--models" => {
                models = it
                    .next()
                    .context("--model needs a value")?
                    .split(',')
                    .map(|m| m.trim().to_string())
                    .filter(|m| !m.is_empty())
                    .collect();
            }
            "--dialects" => {
                dialects = it
                    .next()
                    .context("--dialects needs a value")?
                    .split(',')
                    .map(Dialect::parse)
                    .collect::<Result<Vec<_>>>()?;
            }
            "--trials" => {
                trials = it
                    .next()
                    .context("--trials needs a value")?
                    .parse()
                    .context("--trials value")?;
            }
            "--tasks" => {
                tasks = Some(
                    it.next()
                        .context("--tasks needs a value")?
                        .split(',')
                        .map(|t| t.trim().parse::<usize>().context("--tasks index"))
                        .collect::<Result<Vec<_>>>()?,
                );
            }
            "--max-output-tokens" => {
                max_output_tokens = it
                    .next()
                    .context("--max-output-tokens needs a value")?
                    .parse()
                    .context("--max-output-tokens value")?;
            }
            "--json" => json = true,
            "--verbose" | "-v" => verbose = true,
            "--help" | "-h" => {
                eprintln!(
                    "tool-dialect-bench [--model m1,m2] [--dialects xml,pformat,python,typescript] \
                     [--trials N] [--tasks 0,3,5] [--max-output-tokens N] [--json] [--verbose]"
                );
                std::process::exit(0);
            }
            other => anyhow::bail!("unknown argument: {other}"),
        }
    }
    Ok(Args {
        models,
        dialects,
        trials,
        json,
        verbose,
        tasks,
        max_output_tokens,
    })
}

/// `expected ⊆ parsed`, comparing values after light normalisation (a
/// number the model quoted still counts).
fn is_superset(parsed: &Value, expected: &Value) -> bool {
    let (Some(parsed), Some(expected)) = (parsed.as_object(), expected.as_object()) else {
        return false;
    };
    expected
        .iter()
        .all(|(key, want)| parsed.get(key).is_some_and(|got| loosely_equal(got, want)))
}

fn loosely_equal(got: &Value, want: &Value) -> bool {
    if got == want {
        return true;
    }
    match (got, want) {
        (Value::String(g), Value::Number(w)) => g.trim() == w.to_string(),
        (Value::Number(g), Value::String(w)) => g.to_string() == w.trim(),
        (Value::String(g), Value::Bool(w)) => g.trim().eq_ignore_ascii_case(&w.to_string()),
        (Value::String(g), Value::String(w)) => g.trim() == w.trim(),
        _ => false,
    }
}

fn is_exact(parsed: &Value, expected: &Value) -> bool {
    let (Some(parsed), Some(expected)) = (parsed.as_object(), expected.as_object()) else {
        return false;
    };
    parsed.len() == expected.len()
        && is_superset(
            &Value::Object(parsed.clone()),
            &Value::Object(expected.clone()),
        )
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = env_logger::try_init();
    let args = parse_args()?;
    let specs = fixture_tools();
    let all_tasks = fixture_tasks();
    let tasks: Vec<(usize, &Task)> = match &args.tasks {
        Some(indices) => indices
            .iter()
            .map(|&i| {
                all_tasks
                    .get(i)
                    .map(|t| (i, t))
                    .context("task index out of range")
            })
            .collect::<Result<Vec<_>>>()?,
        None => all_tasks.iter().enumerate().collect(),
    };
    let registry = build_registry(
        specs
            .iter()
            .map(|spec| (spec.name.clone(), spec.parameters.clone())),
    );
    let known: Vec<String> = specs.iter().map(|spec| spec.name.clone()).collect();
    let config = openhuman_core::config::Config::default();

    let mut rows: Vec<Row> = Vec::new();
    let mut summaries: BTreeMap<(String, Dialect), Summary> = BTreeMap::new();

    for model_name in &args.models {
        let provider = format!("ollama:{model_name}");
        let (model, model_id): (Arc<dyn ChatModel<()>>, String) =
            openhuman_core::inference::provider::create_chat_model_from_string_with_model_id(
                "chat", &provider, &config, 0.0,
            )
            .with_context(|| format!("building model {provider}"))?;
        eprintln!("== model {model_id} ({provider})");

        for &dialect in &args.dialects {
            let section = dialect.tool_section(&specs, &registry);
            let system = format!("{BASE_SYSTEM_PROMPT}\n\n{section}");
            eprintln!(
                "-- dialect {} (system prompt {} bytes, ~{} tokens at 4 chars/token)",
                dialect.as_str(),
                system.len(),
                system.len() / 4
            );
            if args.verbose {
                eprintln!("{system}\n");
            }
            for trial in 0..args.trials {
                for &(task_index, task) in &tasks {
                    let request = ModelRequest::new(vec![
                        Message::system(system.clone()),
                        Message::user(task.prompt),
                    ])
                    .with_model(model_id.clone())
                    .with_temperature(0.0)
                    .with_max_tokens(args.max_output_tokens);
                    let started = Instant::now();
                    let outcome =
                        tokio::time::timeout(Duration::from_secs(180), model.invoke(&(), request))
                            .await;
                    let latency_ms = started.elapsed().as_millis();
                    let mut row = Row {
                        model: model_id.clone(),
                        dialect: dialect.as_str(),
                        trial,
                        task: task_index,
                        expected_tool: task.tool,
                        system_prompt_bytes: system.len(),
                        input_tokens: None,
                        output_tokens: None,
                        latency_ms,
                        recovered: false,
                        name_ok: false,
                        args_exact: false,
                        args_superset: false,
                        source: None,
                        parsed_tool: None,
                        parsed_args: None,
                        raw_text: String::new(),
                        error: None,
                    };
                    match outcome {
                        Err(_) => row.error = Some("timeout".into()),
                        // An empty answer that used the whole budget is the
                        // model thinking past the cap, not a dialect failure.
                        Ok(Ok(response))
                            if response.text().trim().is_empty()
                                && response.usage.is_some_and(|u| {
                                    u.output_tokens >= u64::from(args.max_output_tokens)
                                }) =>
                        {
                            row.input_tokens = response.usage.map(|u| u.input_tokens);
                            row.output_tokens = response.usage.map(|u| u.output_tokens);
                            row.error = Some("output cap reached with no visible text".into());
                        }
                        Ok(Err(error)) => row.error = Some(error.to_string()),
                        Ok(Ok(response)) => {
                            row.input_tokens = response.usage.map(|u| u.input_tokens);
                            row.output_tokens = response.usage.map(|u| u.output_tokens);
                            let text = response.text();
                            row.raw_text = text.clone();
                            // The same recovery the harness runs: known tools
                            // for name repair, the registry for positional
                            // and code-style bodies.
                            let options = ParseOptions::new()
                                .with_known_tools(&known)
                                .with_registry(&registry);
                            let (_narrative, calls) = parse_text(&text, &options).into_parts();
                            if let Some(call) = calls.first() {
                                row.recovered = true;
                                row.name_ok = call.name == task.tool;
                                row.args_exact =
                                    row.name_ok && is_exact(&call.arguments, &task.args);
                                row.args_superset =
                                    row.name_ok && is_superset(&call.arguments, &task.args);
                                row.source = Some(source_name(call.source).to_string());
                                row.parsed_tool = Some(call.name.clone());
                                row.parsed_args = Some(call.arguments.clone());
                            }
                        }
                    }
                    let mark = if row.args_exact {
                        "ok "
                    } else if row.args_superset {
                        "sup"
                    } else if row.name_ok {
                        "arg"
                    } else if row.recovered {
                        "tool"
                    } else if row.error.is_some() {
                        "ERR"
                    } else {
                        "none"
                    };
                    eprintln!(
                        "   [{mark:>4}] task {task_index:>2} {:<16} in={:<5} out={:<4} {}ms{}",
                        task.tool,
                        row.input_tokens.map_or("-".to_string(), |t| t.to_string()),
                        row.output_tokens.map_or("-".to_string(), |t| t.to_string()),
                        row.latency_ms,
                        if args.verbose || !row.args_superset {
                            format!("\n         raw: {}", one_line(&row.raw_text))
                        } else {
                            String::new()
                        }
                    );
                    summaries
                        .entry((model_id.clone(), dialect))
                        .or_default()
                        .add(&row);
                    rows.push(row);
                }
            }
        }
    }

    println!();
    println!(
        "| model | dialect | n | sys bytes | in tok | out tok | recovered | tool | args ⊇ | args = | avg ms | errors | sources |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    for ((model, dialect), s) in &summaries {
        let sources: Vec<String> = s.sources.iter().map(|(k, v)| format!("{k}:{v}")).collect();
        println!(
            "| {model} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
            dialect.as_str(),
            s.n,
            s.system_prompt_bytes,
            Summary::avg(s.input_tokens, s.input_samples),
            Summary::avg(s.output_tokens, s.output_samples),
            s.pct(s.recovered),
            s.pct(s.name_ok),
            s.pct(s.args_superset),
            s.pct(s.args_exact),
            if s.n == 0 {
                0
            } else {
                s.latency_ms / s.n as u128
            },
            s.errors,
            sources.join(" ")
        );
    }
    if args.json {
        println!();
        println!("{}", serde_json::to_string_pretty(&rows)?);
    }
    Ok(())
}

fn source_name(source: CallSource) -> &'static str {
    match source {
        CallSource::Native => "native",
        CallSource::TaggedJson => "tagged_json",
        CallSource::InvokeXml => "invoke_xml",
        CallSource::Sentinel => "sentinel",
        CallSource::Harmony => "harmony",
        CallSource::Mistral => "mistral",
        CallSource::Glm => "glm",
        CallSource::BareJson => "bare_json",
        CallSource::PFormat => "pformat",
        CallSource::Code => "code",
        _ => "other",
    }
}

fn one_line(text: &str) -> String {
    let flat: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() > 240 {
        let clipped: String = flat.chars().take(240).collect();
        format!("{clipped}…")
    } else {
        flat
    }
}
