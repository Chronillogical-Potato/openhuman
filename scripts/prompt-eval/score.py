#!/usr/bin/env python3
"""Scorer for scripts/prompt-eval.sh. Reads only artifacts a run already wrote.

  score.py judge-prompt <cases.json> <case-id> <workspace>   -> judge prompt, or nothing
  score.py score <cases.json> <case-id> <workspace> <secs>    -> one JSON row
  score.py precondition <cases.json> <case-id> <output-file>  -> {"ok", ...}; exit 1 when it fails
  score.py selftest
"""
import glob
import json
import os
import re
import subprocess
import sys
import time

# Log lines the breaker middlewares emit when they halt a run
# (agent/tinyagents/middleware/{repeat_progress,repeated_failure}.rs).
HALT_LOG = re.compile(r"halted the run|halting run so the root cause|halting on first occurrence")
# Every breaker halt summary opens with "Stopping: " (tinyagents-harness
# no_progress/*.rs, agent/tinyagents/middleware/loop_guards.rs) and becomes the
# turn's text. `openhuman-core call` initialises no logging, so this, not the
# log, is the signal that works for CLI runs.
HALT_TEXT = re.compile(r"Stopping: (the |\d+ )")


def load_case(cases_path, case_id):
    doc = json.load(open(cases_path))
    return doc, next(c for c in doc["cases"] if c["id"] == case_id)


def rpc_value(path):
    """The value an `openhuman-core call` printed, unwrapped from {result, logs}."""
    try:
        text = open(path).read()
    except OSError:
        return None
    start = text.find("{")
    if start < 0:
        return text.strip() or None
    try:
        v = json.loads(text[start:])
    except ValueError:
        return text.strip()
    while isinstance(v, dict) and "result" in v and len(v) <= 2:
        v = v["result"]
    return v


PATHS = []


def transcripts(ws):
    """[(meta, [message lines])] for every session_raw transcript this case wrote.

    Hermetic runs read the whole fresh workspace. --real-workspace runs set
    PROMPT_EVAL_TRANSCRIPT_ROOT (~/.openhuman) and PROMPT_EVAL_SINCE, and only
    transcripts modified since the case started count."""
    root = os.environ.get("PROMPT_EVAL_TRANSCRIPT_ROOT") or ws
    since = float(os.environ.get("PROMPT_EVAL_SINCE") or 0)
    out = []
    for path in sorted(glob.glob(os.path.join(root, "**", "session_raw", "*.jsonl"), recursive=True)):
        if os.path.getmtime(path) < since:
            continue
        meta, lines = {}, []
        for raw in open(path, errors="replace"):
            raw = raw.strip()
            if not raw:
                continue
            try:
                obj = json.loads(raw)
            except ValueError:
                continue
            if "_meta" in obj:
                meta = obj["_meta"]  # last one wins: the header is rewritten as the run advances
            else:
                lines.append(obj)
        out.append((meta, lines))
        PATHS.append(path)
    return out


def tool_calls(lines):
    """Tool names called, in order. A `use_skill` call also counts as the packed
    tool it reaches (`{"skill", "tool", "args"}`), so forbidding a packed tool
    catches it either way."""
    out = []
    for m in lines:
        if m.get("role") != "assistant":
            continue
        for tc in m.get("tool_calls") or []:
            name = tc.get("name", "")
            out.append(name)
            if name == "use_skill":
                args = tc.get("arguments")
                if isinstance(args, str):
                    try:
                        args = json.loads(args)
                    except ValueError:
                        args = {}
                inner = (args or {}).get("tool") if isinstance(args, dict) else None
                if inner:
                    out.append(inner)
    return out


def max_consecutive(calls, tool):
    best = run = 0
    for name in calls:
        run = run + 1 if name == tool else 0
        best = max(best, run)
    return best


def verdict(text):
    """parse_close_verdict: the LAST standalone ACCEPT/REJECT token wins."""
    tokens = [t.upper() for t in re.split(r"[^A-Za-z]+", text or "")]
    found = [t for t in tokens if t in ("ACCEPT", "REJECT")]
    return found[-1] if found else "UNCLEAR"


def reply_text(value):
    if isinstance(value, dict):
        for key in ("assistant_text", "response", "text", "reply"):
            if isinstance(value.get(key), str):
                return value[key]
        return json.dumps(value)[:4000]
    return value or ""


def records(ts):
    out = []
    for _, lines in ts:
        for m in lines:
            if m.get("role") == "tool":
                status = "failed" if m.get("failure") else "ok"
                out.append(f"- [{status}] {str(m.get('content', ''))[:400]}")
    return "\n".join(out[-40:])


def judge_prompt(doc, case, ws):
    if not case.get("judge"):
        return ""
    j = doc["judge"]
    rules = "\n".join(f"{i}. {r}" for i, r in enumerate(j["rules"], 1))
    recs = records(transcripts(ws)).strip() or "(no tool calls completed)"
    return (
        f"{j['preamble']}\n\nAnswer REJECT if any of these is true:\n{rules}\n\n"
        "Otherwise answer ACCEPT. Reply with the single word ACCEPT or REJECT.\n\n"
        f"<user_request>\n{case['message']}\n</user_request>\n\n<tool_records>\n{recs}\n</tool_records>\n\n"
        f"<reply>\n{reply_text(rpc_value(os.path.join(ws, 'result.json'))).strip()}\n</reply>"
    )


def score(doc, case, ws, secs, run=1):
    pre_path = os.path.join(ws, "precondition_result.json")
    pre = json.load(open(pre_path)) if os.path.exists(pre_path) else None
    ts = transcripts(ws)
    result = rpc_value(os.path.join(ws, "result.json"))
    failures = []

    # 1. Hard failure signals — cheap, no judge needed.
    signals = []
    log = ""
    for path in [os.path.join(ws, "core.log")] + glob.glob(os.path.join(ws, "**", "*.log"), recursive=True):
        try:
            log += open(path, errors="replace").read()
        except OSError:
            pass
    said = [str(m.get("content", "")) for _, lines in ts for m in lines if m.get("role") in ("assistant", "tool")]
    if HALT_LOG.search(log) or any(HALT_TEXT.search(t) for t in said + [reply_text(result)]):
        signals.append("breaker_halt")
    # One case = one user turn. More means the run resumed an earlier
    # conversation and is not independent of it.
    user_turns = max([sum(1 for m in lines if m.get("role") == "user") for _, lines in ts] or [0])
    if user_turns > 1:
        signals.append("contaminated")
    # Only tool results carry the envelope; the orchestrator's system prompt
    # *describes* it, so a whole-transcript grep flags every run.
    blob = json.dumps([m.get("content") for _, lines in ts for m in lines if m.get("role") == "tool"])
    if "[SUBAGENT_INCOMPLETE]" in blob:
        signals.append("SUBAGENT_INCOMPLETE")
    if isinstance(result, dict):
        for flag in ("trail_off", "capped"):
            if result.get(flag):
                signals.append(flag)
        if result.get("error"):
            signals.append("run_error")
    if result is None:
        signals.append("no_result")
    failures += [f"signal:{s}" for s in signals]

    # 2. Did it do the thing.
    all_calls = [c for _, lines in ts for c in tool_calls(lines)]
    checks = 0
    for tool in case.get("expect_calls", []):
        checks += 1
        if tool not in all_calls:
            failures.append(f"missing_call:{tool}")
    for tool in case.get("forbid_calls", []):
        checks += 1
        if tool in all_calls:
            failures.append(f"forbidden_call:{tool}")
    for tool, cap in case.get("max_consecutive", {}).items():
        checks += 1
        worst = max([max_consecutive(tool_calls(lines), tool) for _, lines in ts] or [0])
        if worst > cap:
            failures.append(f"repeat:{tool}x{worst}>{cap}")

    if case.get("reply_regex"):
        checks += 1
        if not re.search(case["reply_regex"], reply_text(result)):
            failures.append("reply_regex")

    # 3. Cost. ponytail: sums every transcript's _meta; if a parent's totals ever
    # include its children's, this double-counts — switch to root-only then.
    metas = [m for m, _ in ts if m]
    tok = {k: sum(int(m.get(k) or 0) for m in metas) for k in ("input_tokens", "output_tokens", "cached_input_tokens")}
    usd = sum(float(m.get("charged_amount_usd") or 0) for m in metas)
    models = sorted({m.get("model") for m in metas if m.get("model")} |
                    {l.get("model") for _, lines in ts for l in lines if l.get("model")})
    checks += 1
    if tok["input_tokens"] > case.get("max_input_tokens", 10**12):
        failures.append(f"input_tokens:{tok['input_tokens']}>{case['max_input_tokens']}")

    # 4. Judge.
    judged = None
    if case.get("judge"):
        checks += 1
        judged = verdict(reply_text(rpc_value(os.path.join(ws, "judge.json"))))
        if judged != "ACCEPT":
            failures.append(f"judge:{judged}")

    passed_checks = checks - len([f for f in failures if not f.startswith("signal:")])
    return {
        "ts": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        "case": case["id"],
        "run": run,
        "surface": case.get("surface", ""),
        "precondition": pre,
        "writes": case.get("writes", []),
        # The tree the measured binary was built from, and the scorer's own.
        "git_sha": os.environ.get("PROMPT_EVAL_BIN_SHA", ""),
        "eval_sha": subprocess.run(["git", "-C", os.path.dirname(os.path.abspath(__file__)), "rev-parse", "HEAD"],
                                   capture_output=True, text=True).stdout.strip(),
        "models": models,
        "pass": not failures,
        "score": 0.0 if signals else round(max(passed_checks, 0) / max(checks, 1), 3),
        "signals": signals,
        "failures": failures,
        "judge": judged,
        "calls": all_calls,
        "transcripts": len(ts),
        "user_turns": user_turns,
        "transcript_paths": sorted(set(PATHS)),
        **tok,
        "usd": usd,
        "seconds": secs,
        "workspace": ws,
    }


def precondition(case, out_path):
    pre = case["precondition"]
    try:
        text = open(out_path, errors="replace").read()
    except OSError:
        text = ""
    ok = bool(text.strip())
    if ok and pre.get("expect_regex"):
        ok = re.search(pre["expect_regex"], text) is not None
    if ok and pre.get("expect_not_regex"):
        ok = re.search(pre["expect_not_regex"], text) is None
    return {"what": pre.get("what", ""), "method": pre["method"], "ok": ok, "evidence": text.strip()[:600]}


def selftest():
    assert verdict("I think... ACCEPT") == "ACCEPT"
    assert verdict("ACCEPT? no. REJECT") == "REJECT"
    assert verdict("UNACCEPTABLE") == "UNCLEAR"
    assert max_consecutive(["a", "b", "b", "a", "b", "b", "b"], "b") == 3
    assert tool_calls([{"role": "assistant", "tool_calls": [
        {"name": "use_skill", "arguments": json.dumps({"skill": "s", "tool": "skill_registry_install"})}]}]) == [
        "use_skill", "skill_registry_install"]
    import tempfile
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
        f.write('{"result":{"connections":[{"toolkit":"gmail","status":"ACTIVE"}]}}')
    assert precondition({"precondition": {"method": "m", "expect_regex": "(?is)gmail.{0,200}active"}}, f.name)["ok"]
    assert not precondition({"precondition": {"method": "m", "expect_not_regex": '"toolkit"'}}, f.name)["ok"]
    assert not precondition({"precondition": {"method": "m"}}, f.name + ".missing")["ok"]
    import tempfile as _t
    d = _t.mkdtemp()
    os.makedirs(os.path.join(d, "w", "session_raw"))
    with open(os.path.join(d, "w", "session_raw", "a.jsonl"), "w") as f:
        f.write(json.dumps({"role": "system", "content": "explains [SUBAGENT_INCOMPLETE] envelopes"}) + "\n")
        f.write(json.dumps({"role": "assistant", "content": "done"}) + "\n")
    open(os.path.join(d, "result.json"), "w").write('{"result": "ok"}')
    case = {"id": "t", "message": "m"}
    assert "SUBAGENT_INCOMPLETE" not in score({}, case, d, 0)["signals"], "system prompt must not trip the signal"
    with open(os.path.join(d, "w", "session_raw", "a.jsonl"), "a") as f:
        f.write(json.dumps({"role": "tool", "content": "[SUBAGENT_INCOMPLETE] the x sub-agent stopped"}) + "\n")
    assert "SUBAGENT_INCOMPLETE" in score({}, case, d, 0)["signals"]
    assert HALT_TEXT.search("Stopping: the same successful tool-call batch was issued 3 times")
    assert not HALT_TEXT.search("Stopping by the store later")
    assert HALT_LOG.search("[tinyagents::mw] crate successful-repeat tracker halted the run")
    print("score.py selftest ok")


if __name__ == "__main__":
    cmd = sys.argv[1]
    if cmd == "selftest":
        selftest()
    elif cmd == "judge-prompt":
        doc, case = load_case(sys.argv[2], sys.argv[3])
        print(judge_prompt(doc, case, sys.argv[4]))
    elif cmd == "precondition":
        doc, case = load_case(sys.argv[2], sys.argv[3])
        r = precondition(case, sys.argv[4])
        print(json.dumps(r))
        sys.exit(0 if r["ok"] else 1)
    elif cmd == "score":
        doc, case = load_case(sys.argv[2], sys.argv[3])
        run = int(sys.argv[6]) if len(sys.argv) > 6 else 1
        print(json.dumps(score(doc, case, sys.argv[4], float(sys.argv[5]), run)))
