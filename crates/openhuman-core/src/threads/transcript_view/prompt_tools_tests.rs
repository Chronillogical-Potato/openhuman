//! Prompt-guided (text-mode) tool rounds in the display projection — see
//! `prompt_tools.rs`.

use super::project::project_records;
use super::types::DisplayItem;
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tinyagents_session::transcript::{self, read_transcript_display};

fn write_raw(workspace: &Path, stem: &str, thread_id: &str, body: &[&str]) -> PathBuf {
    let path = transcript::resolve_keyed_transcript_path(workspace, stem).expect("resolve");
    let mut buf = format!(
        r#"{{"_meta":{{"version":1,"agent":"orchestrator","dispatcher":"xml","created":"2026-09-24T00:00:00Z","updated":"2026-09-24T00:00:10Z","turn_count":1,"input_tokens":1,"output_tokens":1,"cached_input_tokens":0,"charged_amount_usd":0.0,"thread_id":"{thread_id}"}}}}"#
    );
    buf.push('\n');
    for line in body {
        buf.push_str(line);
        buf.push('\n');
    }
    std::fs::write(&path, buf).expect("write raw transcript");
    path
}

/// A native turn: the calling line carries its own `tool_calls`.
fn native_turn_body() -> Vec<&'static str> {
    vec![
        r#"{"role":"user","content":"What's the weather in NYC?","request_id":"req-1"}"#,
        r#"{"role":"assistant","content":"Let me check.","provider":"anthropic","model":"claude-x","usage":{"input":10,"output":5,"cached_input":0,"cost_usd":0.001},"ts":"2026-07-21T09:00:01Z","tool_calls":[{"id":"call-1","name":"get_weather","arguments":"{\"city\":\"NYC\"}"}],"iteration":1,"request_id":"req-1"}"#,
        r#"{"role":"tool","content":"72F and sunny","id":"call-1","request_id":"req-1"}"#,
        r#"{"role":"assistant","content":"It's 72F and sunny in NYC.","provider":"anthropic","model":"claude-x","usage":{"input":20,"output":8,"cached_input":0,"cost_usd":0.002},"ts":"2026-07-21T09:00:02Z","iteration":2,"request_id":"req-1"}"#,
    ]
}

/// A prompt-guided (text-mode) turn, as `session_raw` actually records it: the
/// calling lines carry no `tool_calls`, each round's results are ONE
/// `[Tool results]` user line, and the turn's calls are stamped once on the
/// final answer line.
fn prompt_guided_turn_body(result_ids: bool) -> Vec<String> {
    let block = |id: &str, body: &str| {
        if result_ids {
            format!("<tool_result id=\\\"{id}\\\">\\n{body}\\n</tool_result>")
        } else {
            format!("<tool_result>\\n{body}\\n</tool_result>")
        }
    };
    vec![
        r#"{"role":"user","content":"Check the config, then search.","request_id":"req-p"}"#.to_string(),
        r#"{"role":"assistant","content":"Let me read the config first.","request_id":"req-p"}"#.to_string(),
        format!(
            r#"{{"role":"user","content":"[Tool results]\n{}","request_id":"req-p"}}"#,
            block("call-read", "port = 8080")
        ),
        r#"{"role":"assistant","content":"Now I will search for the setting.","request_id":"req-p"}"#.to_string(),
        format!(
            r#"{{"role":"user","content":"[Tool results]\n{}","request_id":"req-p"}}"#,
            block("call-grep", "config.toml:3: setting = on")
        ),
        r#"{"role":"assistant","content":"The setting is on.","provider":"openhuman","model":"m","usage":{"input":1,"output":1,"cached_input":0,"cost_usd":0.0},"tool_calls":[{"id":"call-read","name":"file_read","arguments":"{\"path\":\"/etc/config.toml\"}"},{"id":"call-grep","name":"grep","arguments":"{\"pattern\":\"setting\"}"}],"iteration":3,"ts":"2026-09-24T05:20:52Z","request_id":"req-p"}"#.to_string(),
    ]
}

/// What a reader sees of a projected turn, in order.
fn outline(items: &[DisplayItem]) -> Vec<String> {
    items
        .iter()
        .filter_map(|item| match item {
            DisplayItem::UserMessage { content, .. } => Some(format!("user:{content}")),
            DisplayItem::AssistantMessage {
                content, interim, ..
            } => Some(format!(
                "{}:{content}",
                if *interim { "narration" } else { "answer" }
            )),
            DisplayItem::ToolCall {
                call_id,
                name,
                status,
                result,
                ..
            } => Some(format!(
                "tool:{call_id}:{name}:{status:?}:{}",
                result.as_deref().unwrap_or("-")
            )),
            _ => None,
        })
        .collect()
}

/// The prompt-guided turn used to project inverted: both narrations as final
/// answers, the answer as an interim step with every call after it, and each
/// `[Tool results]` line as something the user said. A reopened thread showed
/// the answer twice with its tools below it.
#[test]
fn prompt_guided_turn_projects_like_a_native_one() {
    let dir = TempDir::new().unwrap();
    let body = prompt_guided_turn_body(true);
    let body: Vec<&str> = body.iter().map(String::as_str).collect();
    let path = write_raw(dir.path(), "300_orchestrator", "thr_p", &body);
    let display = read_transcript_display(&path).unwrap();

    assert_eq!(
        outline(&project_records(&display.records)),
        vec![
            "user:Check the config, then search.".to_string(),
            "narration:Let me read the config first.".to_string(),
            "tool:call-read:file_read:Success:port = 8080".to_string(),
            "narration:Now I will search for the setting.".to_string(),
            "tool:call-grep:grep:Success:config.toml:3: setting = on".to_string(),
            "answer:The setting is on.".to_string(),
        ]
    );
}

#[test]
fn prompt_guided_results_without_ids_pair_in_issue_order() {
    let dir = TempDir::new().unwrap();
    let body = prompt_guided_turn_body(false);
    let body: Vec<&str> = body.iter().map(String::as_str).collect();
    let path = write_raw(dir.path(), "301_orchestrator", "thr_q", &body);
    let display = read_transcript_display(&path).unwrap();

    let outline = outline(&project_records(&display.records));
    assert_eq!(outline[2], "tool:call-read:file_read:Success:port = 8080");
    assert_eq!(
        outline[4],
        "tool:call-grep:grep:Success:config.toml:3: setting = on"
    );
    assert_eq!(outline[5], "answer:The setting is on.");
}

/// A native turn records its calls on the line that made them; the answer
/// line's calls are only skipped when they were already placed.
#[test]
fn native_turn_projection_is_unchanged() {
    let dir = TempDir::new().unwrap();
    let path = write_raw(dir.path(), "302_orchestrator", "thr_n", &native_turn_body());
    let display = read_transcript_display(&path).unwrap();
    assert_eq!(
        outline(&project_records(&display.records)),
        vec![
            "user:What's the weather in NYC?".to_string(),
            "narration:Let me check.".to_string(),
            "tool:call-1:get_weather:Success:72F and sunny".to_string(),
            "answer:It's 72F and sunny in NYC.".to_string(),
        ]
    );
}
