/**
 * Behaviour of the life-scenario mock search backend
 * (`scripts/life-scenarios/mock-search.mjs`).
 *
 * The rig exists because `web_search_tool` posts to the hosted backend, which
 * the benchmark has no session for — so the tool 401'd on every call and the
 * agent burned its iteration budget routing around it by hand. These lock in
 * the two properties that make the substitute honest: it ranks a corpus of
 * real URLs the agent must still fetch for itself, and it returns nothing at
 * all for a query the corpus does not cover.
 */

import assert from "node:assert/strict";
import test, { after, before } from "node:test";

import {
  DEFAULT_INDEX_PATH,
  startMockSearch,
} from "../life-scenarios/mock-search.mjs";

let server;

before(async () => {
  server = await startMockSearch({ indexPath: DEFAULT_INDEX_PATH, port: 0 });
});

after(async () => {
  await server?.close();
});

/** Post a query the way `WebSearchTool` does. */
async function search(objective, maxResults = 5) {
  const res = await fetch(`${server.url}/agent-integrations/parallel/search`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      objective,
      searchQueries: [objective],
      mode: "fast",
      excerpts: { maxResults, maxCharsPerResult: 500 },
    }),
  });
  assert.equal(res.status, 200);
  const envelope = await res.json();
  // `integrations/client/errors.rs::parse_envelope` deserializes
  // `{ success, data, error }` and returns `data`. A bare payload fails as
  // `missing field 'success'` — and fails late enough to look like a broken
  // tool rather than an empty result, so the agent abandons the task.
  assert.equal(
    envelope.success,
    true,
    "every response must carry the backend envelope",
  );
  return envelope.data;
}

test("the corpus loads past the fixture's _comment key", () => {
  assert.ok(
    server.ctx.documents.length > 0,
    "documents should come from the `documents` array, not the whole object",
  );
});

test("a baggage query ranks delta.com pages first", async () => {
  const body = await search(
    "Delta Air Lines Basic Economy carry-on baggage allowance transatlantic",
  );
  assert.ok(body.results.length > 0, "the corpus covers this query");
  assert.match(body.results[0].url, /(^|\.)delta\.com/);
});

test("a checked-bag fee query surfaces the fee page", async () => {
  const body = await search("Delta checked bag fee first second bag Europe");
  const urls = body.results.map((r) => r.url);
  assert.ok(
    urls.some((u) => /excess-overweight-baggage/.test(u)),
    `expected the fee page among ${JSON.stringify(urls)}`,
  );
});

test("an off-topic query returns nothing rather than the least-bad rows", async () => {
  // A confidently wrong result set is worse than an empty one: the agent
  // spends fetches on it, which is the failure mode this rig removes.
  for (const query of [
    "how do I bake sourdough bread at home",
    "completely unrelated zebra topic",
  ]) {
    const body = await search(query);
    assert.deepEqual(body.results, [], `"${query}" should match nothing`);
  }
});

test("results carry live URLs, so retrieval stays real", async () => {
  const body = await search("Delta baggage overview allowance");
  for (const item of body.results) {
    assert.match(item.url, /^https:\/\//, "every hit must be fetchable");
    assert.ok(item.title, "every hit must carry a title");
  }
});

test("maxResults is honoured and clamped", async () => {
  const body = await search("Delta baggage allowance fees carry-on checked", 2);
  assert.ok(body.results.length <= 2);
});

test("the search is recorded for post-mortems", async () => {
  const before = server.ctx.searches.length;
  await search("Delta baggage overview");
  assert.equal(server.ctx.searches.length, before + 1);
  assert.ok(server.ctx.searches.at(-1).queries.length > 0);
});

test("the cost is reported as zero, because it was", async () => {
  const body = await search("Delta baggage overview");
  assert.equal(body.costUsd, 0);
});

test("the unauthenticated backend calls get benign stubs", async () => {
  // Answered so the core's log shows the run's own behaviour rather than one
  // fixed auth failure repeated every turn. Enveloped like everything else.
  const usage = await fetch(`${server.url}/teams/me/usage`);
  assert.equal(usage.status, 200);
  assert.equal((await usage.json()).success, true);

  const toolkits = await fetch(
    `${server.url}/agent-integrations/composio/toolkits`,
  );
  assert.equal(toolkits.status, 200);
  const body = await toolkits.json();
  assert.equal(body.success, true);
  // Empty on purpose: Composio is served by mock-composio.mjs over the direct
  // base, and a competing list here would shadow it.
  assert.deepEqual(body.data.items, []);
});

test("an unhandled route 404s and is recorded", async () => {
  const res = await fetch(`${server.url}/agent-integrations/something-new`);
  assert.equal(res.status, 404);
  assert.ok(
    server.ctx.requests.some(
      (r) => r.path === "/agent-integrations/something-new",
    ),
    "a capability that starts reaching for the backend must be visible",
  );
});
