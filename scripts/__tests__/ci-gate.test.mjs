import assert from "node:assert/strict";
import { test } from "node:test";

import { SOURCES, decide, latestPrRun } from "../ci/ci-gate.mjs";

const [EX63, HOSTED, LITE] = SOURCES;

function entry(source, id, status, conclusion, job) {
  return {
    source,
    run: { id, status, conclusion, html_url: `https://runs/${id}` },
    job: job === undefined ? null : { ...job, html_url: `https://jobs/${id}` },
  };
}

test("the EX63 lanes passing wins and cancels the hosted runs still going", () => {
  const verdict = decide([
    entry(EX63, 1, "completed", "success", {
      status: "completed",
      conclusion: "success",
    }),
    entry(HOSTED, 2, "completed", "success", {
      status: "completed",
      conclusion: "skipped",
    }),
    entry(LITE, 3, "in_progress", null, null),
  ]);
  assert.equal(verdict.state, "success");
  assert.equal(verdict.description, "CI Fast passed");
  assert.equal(verdict.targetUrl, "https://jobs/1");
  assert.deepEqual(verdict.cancel, [{ id: 3, name: "CI Lite" }]);
});

test("CI Lite passing first also passes the gate, and never cancels the EX63", () => {
  const verdict = decide([
    entry(EX63, 1, "in_progress", null, {
      status: "in_progress",
      conclusion: null,
    }),
    entry(LITE, 3, "completed", "success", {
      status: "completed",
      conclusion: "success",
    }),
  ]);
  assert.equal(verdict.state, "success");
  assert.equal(verdict.description, "CI Lite passed");
  assert.deepEqual(verdict.cancel, []);
});

test("a skipped decisive job is not a pass: an outsider's CI Fast run is pending on the rest", () => {
  const verdict = decide([
    entry(EX63, 1, "completed", "success", {
      status: "completed",
      conclusion: "skipped",
    }),
    entry(HOSTED, 2, "in_progress", null, {
      status: "queued",
      conclusion: null,
    }),
    entry(LITE, 3, "queued", null, null),
  ]);
  assert.equal(verdict.state, "pending");
  assert.equal(verdict.description, "Waiting on CI Fast (hosted), CI Lite");
});

test("an EX63 failure waits for CI Lite instead of failing the gate early", () => {
  const verdict = decide([
    entry(EX63, 1, "completed", "failure", {
      status: "completed",
      conclusion: "failure",
    }),
    entry(LITE, 3, "in_progress", null, null),
  ]);
  assert.equal(verdict.state, "pending");
});

test("everything finished with no pass fails and points at the failed job", () => {
  const verdict = decide([
    entry(EX63, 1, "completed", "failure", {
      status: "completed",
      conclusion: "failure",
    }),
    entry(HOSTED, 2, "completed", "success", {
      status: "completed",
      conclusion: "skipped",
    }),
    entry(LITE, 3, "completed", "failure", {
      status: "completed",
      conclusion: "failure",
    }),
  ]);
  assert.equal(verdict.state, "failure");
  assert.equal(verdict.description, "CI Fast: failure; CI Lite: failure");
  assert.equal(verdict.targetUrl, "https://jobs/1");
});

test("finished flows whose decisive jobs never ran fail rather than pass", () => {
  const verdict = decide([
    entry(EX63, 1, "completed", "cancelled", null),
    entry(LITE, 3, "completed", "cancelled", {
      status: "completed",
      conclusion: "skipped",
    }),
  ]);
  assert.equal(verdict.state, "failure");
  assert.equal(verdict.description, "No CI flow ran its checks");
});

test("no run yet is pending", () => {
  assert.equal(decide([]).state, "pending");
});

test("latestPrRun ignores push runs and picks the newest PR run", () => {
  const run = latestPrRun([
    { id: 1, event: "pull_request", created_at: "2026-09-23T10:00:00Z" },
    { id: 2, event: "push", created_at: "2026-09-23T12:00:00Z" },
    { id: 3, event: "pull_request_target", created_at: "2026-09-23T11:00:00Z" },
  ]);
  assert.equal(run.id, 3);
  assert.equal(latestPrRun([{ id: 2, event: "push", created_at: "x" }]), null);
});
