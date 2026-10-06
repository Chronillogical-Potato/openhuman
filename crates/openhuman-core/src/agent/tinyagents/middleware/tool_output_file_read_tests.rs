use super::*;
use serde_json::json;

#[test]
fn plain_reads_of_files_are_file_reads() {
    for command in [
        "cat vm/vmExprFunction.go",
        "cd /app && cat vm/vmExprFunction.go",
        "cd /app && cat -n parser/lexer.go",
        "cd /app && sed -n '120,300p' vm/vmExprFunction.go",
        "cd /app && cat parser/Makefile && echo \"=== parser.go.y ===\" && cat parser/parser.go.y",
        "cd /app && sed -n '125,150p' ast/expr.go && echo \"=== vmExprFunction.go ===\" && cat vm/vmExprFunction.go",
        "head -50 README.md; tail -20 CHANGELOG.md",
        "cat -n src/lib.rs | sed -n '1,80p'",
    ] {
        assert!(is_file_read_command(command), "{command}");
    }
}

#[test]
fn anything_that_runs_writes_or_searches_is_not() {
    for command in [
        "go test ./vm/...",
        "cd /app && git diff",
        "grep -rn FuncExpr .",
        "cat foo.go > bar.go",
        "sed -i 's/a/b/' foo.go",
        "sed --in-place 's/a/b/' foo.go",
        "sed --in-place=.bak -n 's/old/new/p' file",
        "cat <<'EOF' > x.go\npackage x\nEOF",
        "cat $(git ls-files)",
        "cat README.md && printf '%s\\n' \"$BUILD_LOG\"",
        "cd /app && cat a.go && go build ./...",
        "echo hello",
        "cd /app",
    ] {
        assert!(!is_file_read_command(command), "{command}");
    }
}

#[test]
fn file_read_tools_and_wrapped_calls_are_recognised() {
    assert!(is_file_read_call("file_read", &json!({"path": "a.rs"})));
    assert!(is_file_read_call("shell", &json!({"command": "cat a.rs"})));
    assert!(!is_file_read_call(
        "shell",
        &json!({"command": "cargo test"})
    ));
    assert!(!is_file_read_call(
        "web_fetch",
        &json!({"url": "https://example.com"})
    ));
    assert!(is_file_read_call(
        "use_skill",
        &json!({"tool": "shell", "args": {"command": "cat a.rs"}})
    ));
    assert!(!is_file_read_call("use_skill", &json!({"tool": "shell"})));
}
