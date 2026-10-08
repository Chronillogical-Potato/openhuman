// A stateful mock of Composio's direct API (v3), enough for memory's connector
// sync: Gmail and GitHub connections, their fetch actions, and a DELETE.
//
// The core reaches it through `[composio] mode = "direct"` plus BOTH
// `OPENHUMAN_COMPOSIO_DIRECT_BASE_V3` and `..._V2` in a debug build (see
// scripts/life-scenarios/README.md, "Mock Composio"). Unlike life-scenarios'
// mock, items carry the version fields the sync keys edits on (Gmail
// `historyId`, GitHub `updated_at`), it answers the `after:` window query, and
// tests can edit an item, slow a fetch down, or drop a connection.
//
// All content is fictional (*.example, github.com/northwind-example).

import http from "node:http";

const ACCOUNT = "ca_memscen";

function freshState() {
  return {
    connections: {
      gmail: {
        id: `${ACCOUNT}-gmail`,
        status: "ACTIVE",
        toolkit: { slug: "gmail" },
      },
      github: {
        id: `${ACCOUNT}-github`,
        status: "ACTIVE",
        toolkit: { slug: "github" },
      },
    },
    gmail: [
      {
        id: "msg-001",
        historyId: "1001",
        sender: "Priya Raman <priya.raman@example.com>",
        subject: "Diwali travel",
        messageText:
          "Hi Jordan, I land in Pune on the 30th at 7pm. Can you pick me up? Priya",
        messageUrl: "https://mail.example/msg-001",
      },
      {
        id: "msg-002",
        historyId: "1002",
        sender: "Mateo Alvarez <mateo@alvarez-design.example>",
        subject: "Kestrel logo drafts",
        messageText:
          "Three logo drafts for Kestrel are attached; I prefer the second one.",
        messageUrl: "https://mail.example/msg-002",
      },
    ],
    github: [
      {
        id: 9001,
        number: 12,
        title: "Kestrel: launch checklist",
        body: "Track the launch checklist for the Kestrel app: store listing, privacy note, release notes.",
        html_url: "https://github.com/northwind-example/kestrel/issues/12",
        updated_at: "2026-10-01T09:00:00Z",
      },
      {
        id: 9002,
        number: 4,
        title: "Docs: travel policy page",
        body: "Add the hotel caps to the travel policy page.",
        html_url: "https://github.com/northwind-example/handbook/pull/4",
        updated_at: "2026-10-02T09:00:00Z",
      },
    ],
    fetchDelayMs: 0,
    requests: [],
  };
}

export async function startMockComposio({ port = 0 } = {}) {
  const ctx = freshState();

  const reply = (res, status, body) => {
    res.writeHead(status, { "content-type": "application/json" });
    res.end(JSON.stringify(body));
  };

  const actions = {
    GMAIL_GET_PROFILE: () => ({
      emailAddress: "jordan.lee@example.com",
      historyId: "2000",
    }),
    GMAIL_FETCH_EMAILS: () => ({ messages: ctx.gmail, nextPageToken: null }),
    GITHUB_GET_THE_AUTHENTICATED_USER: () => ({
      login: "jordanlee-example",
      id: 4242,
    }),
    GITHUB_SEARCH_ISSUES_AND_PULL_REQUESTS: () => ({
      items: ctx.github,
      total_count: ctx.github.length,
    }),
  };
  const slowActions = new Set([
    "GMAIL_FETCH_EMAILS",
    "GITHUB_SEARCH_ISSUES_AND_PULL_REQUESTS",
  ]);

  const server = http.createServer(async (req, res) => {
    let raw = "";
    for await (const chunk of req) raw += chunk;
    let body = {};
    try {
      body = raw ? JSON.parse(raw) : {};
    } catch {
      /* not JSON */
    }
    const url = new URL(req.url, "http://mock");
    ctx.requests.push({
      at: new Date().toISOString(),
      method: req.method,
      path: url.pathname,
      body,
    });

    // Connections.
    if (
      /\/(connected_accounts|connections)\/?$/.test(url.pathname) &&
      req.method === "GET"
    ) {
      const items = Object.values(ctx.connections);
      return reply(res, 200, { items, total_items: items.length });
    }
    const del = url.pathname.match(
      /\/(connected_accounts|connections)\/([^/]+)$/,
    );
    if (del && req.method === "DELETE") {
      for (const [k, c] of Object.entries(ctx.connections))
        if (c.id === decodeURIComponent(del[2])) delete ctx.connections[k];
      return reply(res, 200, { success: true });
    }
    // Execute.
    const exec =
      url.pathname.match(/\/tools\/execute\/([A-Z0-9_]+)$/) ||
      url.pathname.match(/\/actions\/([A-Z0-9_]+)\/execute$/);
    if (exec && req.method === "POST") {
      const action = exec[1];
      const fn = actions[action];
      if (!fn)
        return reply(res, 404, {
          successful: false,
          error: `mock: no ${action}`,
          data: null,
        });
      if (slowActions.has(action) && ctx.fetchDelayMs)
        await new Promise((r) => setTimeout(r, ctx.fetchDelayMs));
      return reply(res, 200, {
        successful: true,
        error: null,
        data: fn(body.arguments ?? {}),
      });
    }
    if (/\/(toolkits|tools)\/?$/.test(url.pathname))
      return reply(res, 200, { items: [], total_items: 0 });
    return reply(res, 404, {
      successful: false,
      error: `mock: no route ${req.method} ${url.pathname}`,
    });
  });

  await new Promise((resolve) => server.listen(port, "127.0.0.1", resolve));
  const { port: bound } = server.address();
  return {
    url: `http://127.0.0.1:${bound}`,
    ctx,
    connectionId: (toolkit) =>
      ctx.connections[toolkit]?.id ?? `${ACCOUNT}-${toolkit}`,
    /** Change an item's body and bump its version, as an edit upstream does. */
    edit(toolkit, id, text) {
      if (toolkit === "gmail") {
        const m = ctx.gmail.find((x) => x.id === id);
        m.messageText = text;
        m.historyId = String(Number(m.historyId) + 100);
      } else {
        const it = ctx.github.find((x) => x.id === id);
        it.body = text;
        it.updated_at = new Date().toISOString();
      }
    },
    /** Put a connection back (a reconnect). */
    reconnect(toolkit) {
      ctx.connections[toolkit] = {
        id: `${ACCOUNT}-${toolkit}`,
        status: "ACTIVE",
        toolkit: { slug: toolkit },
      };
    },
    setFetchDelay(ms) {
      ctx.fetchDelayMs = ms;
    },
    close: () => new Promise((r) => server.close(r)),
  };
}
