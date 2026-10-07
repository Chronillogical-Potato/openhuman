import assert from "node:assert/strict";
import net from "node:net";
import test from "node:test";

import {
  devConnectUrl,
  freePort,
  isLoopback,
  parseArgs,
  runStamp,
  waitFor,
} from "../debug/web-ui-lib.mjs";

test("parseArgs defaults to an interactive, signed-in run that builds the core", () => {
  assert.deepEqual(parseArgs([]), {
    script: null,
    headed: false,
    keep: false,
    core: null,
    build: true,
    signIn: true,
    help: false,
  });
});

test("parseArgs reads every flag", () => {
  const opts = parseArgs([
    "--script",
    "s.mjs",
    "--core",
    "bin/core",
    "--headed",
    "--keep",
    "--no-build",
    "--no-sign-in",
  ]);
  assert.equal(opts.script, "s.mjs");
  assert.equal(opts.core, "bin/core");
  assert.equal(opts.headed, true);
  assert.equal(opts.keep, true);
  assert.equal(opts.build, false);
  assert.equal(opts.signIn, false);
  assert.equal(parseArgs(["-h"]).help, true);
});

test("parseArgs rejects unknown flags and missing values", () => {
  assert.throws(() => parseArgs(["--bogus"]), /unknown option: --bogus/);
  assert.throws(() => parseArgs(["--script"]), /--script requires a value/);
  assert.throws(() => parseArgs(["--core", "--keep"]), /--core requires a value/);
});

test("devConnectUrl carries the RPC URL and bearer in the fragment", () => {
  const url = new URL(devConnectUrl("http://localhost:1420", "http://127.0.0.1:7788/rpc", "tok"));
  assert.equal(url.pathname, "/__dev-connect");
  assert.equal(url.search, "");
  const fragment = new URLSearchParams(url.hash.slice(1));
  assert.equal(fragment.get("rpcUrl"), "http://127.0.0.1:7788/rpc");
  assert.equal(fragment.get("token"), "tok");
});

test("runStamp is a sortable local timestamp", () => {
  assert.equal(runStamp(new Date(2026, 9, 7, 9, 5, 3)), "20261007-090503");
});

test("freePort returns a port that can be bound", async () => {
  const port = await freePort();
  await new Promise((resolve, reject) => {
    const srv = net.createServer().listen(port, "127.0.0.1", () => srv.close(resolve));
    srv.on("error", reject);
  });
});

test("waitFor resolves once the probe passes and times out otherwise", async () => {
  let calls = 0;
  await waitFor(async () => ++calls >= 3, { timeoutMs: 1000, intervalMs: 1, what: "x" });
  assert.equal(calls, 3);
  await assert.rejects(
    waitFor(async () => false, { timeoutMs: 20, intervalMs: 5, what: "the thing" }),
    /timed out waiting for the thing/,
  );
});

test("isLoopback admits only local hosts and inline URLs", () => {
  assert.equal(isLoopback("http://localhost:1420/src/main.tsx"), true);
  assert.equal(isLoopback("http://127.0.0.1:7788/rpc"), true);
  assert.equal(isLoopback("ws://127.0.0.1:7788/socket.io/"), true);
  assert.equal(isLoopback("data:image/png;base64,AAAA"), true);
  assert.equal(isLoopback("https://panel.tinyhumans.ai/api/track"), false);
  assert.equal(isLoopback("https://raw.githubusercontent.com/x"), false);
});
