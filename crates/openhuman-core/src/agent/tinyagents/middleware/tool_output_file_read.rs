//! Which tool calls only read files the agent can read again.
//!
//! [`super::tool_output::ToolOutputMiddleware`] skips TinyJuice's LLM summary
//! for these. A summary of source code is a paraphrase, and an agent about to
//! edit a file needs its exact text: on live coding runs every summarized
//! `cat` of a source file was followed by `juice_retrieve` of the whole
//! original or a fresh `sed -n` read of the file. The summary bought nothing
//! and cost a model call plus up to TinyJuice's 8-second summary timeout,
//! which it hit on most attempts.
//!
//! The test is deliberately narrow: a file-reading tool, or a shell command
//! made only of `cd` and plain reads (`cat`, `head`, `tail`, `nl`, `sed`
//! without `-i`, `echo`/`printf` separators), with no redirects, heredocs,
//! substitutions or other programs. Anything else (a build, a test run, a
//! search) keeps the normal pipeline: its output is not a file on disk.

use serde_json::Value;

/// Tools whose whole job is returning a file's contents.
const FILE_READ_TOOLS: &[&str] = &["file_read", "read_file", "read", "view_file"];

/// Shell tools, by the names OpenHuman and common harness adapters use.
const SHELL_TOOLS: &[&str] = &["shell", "bash", "exec", "run_command", "terminal"];

/// Programs that print a file (`sed` only without in-place editing).
const READERS: &[&str] = &["cat", "head", "tail", "nl", "sed"];

/// Programs allowed beside the readers: navigation and separators.
const NEUTRAL: &[&str] = &["cd", "echo", "printf", "true", "pwd"];

/// Whether this call only prints files the agent can re-read (see the module
/// docs). `use_skill` is followed into the tool it wraps.
pub(crate) fn is_file_read_call(tool_name: &str, args: &Value) -> bool {
    let (name, args) = if tool_name == "use_skill" {
        match (args.get("tool").and_then(Value::as_str), args.get("args")) {
            (Some(inner), Some(inner_args)) => (inner, inner_args),
            _ => return false,
        }
    } else {
        (tool_name, args)
    };
    if FILE_READ_TOOLS.contains(&name) {
        return true;
    }
    if !SHELL_TOOLS.contains(&name) {
        return false;
    }
    args.get("command")
        .or_else(|| args.get("cmd"))
        .and_then(Value::as_str)
        .is_some_and(is_file_read_command)
}

/// Whether a shell command line only prints files: every segment (split on
/// `&&`, `||`, `;`, `|` and newlines) runs a reader or a neutral program, at
/// least one runs a reader, and nothing redirects, substitutes or feeds a
/// heredoc.
pub(crate) fn is_file_read_command(command: &str) -> bool {
    if command.contains(['>', '<', '`', '$']) {
        return false;
    }
    let mut reads = false;
    for segment in command
        .split(['\n', ';', '|', '&'])
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let mut words = segment.split_whitespace();
        let Some(program) = words.next() else {
            continue;
        };
        let program = program.rsplit('/').next().unwrap_or(program);
        if program == "sed"
            && segment
                .split_whitespace()
                .any(|w| w.starts_with("-i") || w == "--in-place" || w.starts_with("--in-place="))
        {
            return false;
        }
        if READERS.contains(&program) {
            reads = true;
        } else if !NEUTRAL.contains(&program) {
            return false;
        }
    }
    reads
}

#[cfg(test)]
#[path = "tool_output_file_read_tests.rs"]
mod tests;
