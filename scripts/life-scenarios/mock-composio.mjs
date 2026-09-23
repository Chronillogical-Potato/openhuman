/**
 * A mock Composio server for the life-scenario benchmark.
 *
 * The real Composio is an OAuth broker in front of Gmail, Google Calendar and
 * a few hundred other SaaS tools. A benchmark cannot use it: it needs live
 * consumer accounts, it costs real quota, and nothing about it is
 * reproducible. But the scenarios are *about* mail and calendar, so stubbing
 * the tools out would benchmark the wrong thing.
 *
 * So this serves Composio's wire shape over the same fixture corpus the file
 * tools see. `GMAIL_FETCH_EMAILS` returns the fixture mailbox parsed into
 * Gmail's message shape; `GOOGLECALENDAR_EVENTS_LIST` returns the fixture
 * calendar in Google's event shape; the write actions record what the agent
 * tried to do into `outbox.json` so a grader can assert on it instead of an
 * email actually being sent.
 *
 * The core is pointed here with `OPENHUMAN_COMPOSIO_DIRECT_BASE_V3`.
 *
 * Every response is derived from the fixtures — no network, no secrets, and
 * the recorded writes are the point rather than a side effect.
 */

import fs from "node:fs";
import http from "node:http";
import path from "node:path";

const CONNECTED_ACCOUNT_ID = "ca_life_scenarios_0001";
const USER_ID = "life-scenarios-user";

// ---------------------------------------------------------------------------
// fixture -> provider shapes
// ---------------------------------------------------------------------------

/** Parse one `Key: value` header block + body out of a fixture .txt message. */
function parseMessage(text, file) {
  const idx = text.indexOf("\n\n");
  const headText = idx >= 0 ? text.slice(0, idx) : text;
  const body = idx >= 0 ? text.slice(idx + 2) : "";
  const headers = {};
  for (const line of headText.split("\n")) {
    const m = /^([A-Za-z-]+):\s*(.*)$/.exec(line.trim());
    if (m) headers[m[1].toLowerCase()] = m[2];
  }
  const date = headers.date ? new Date(headers.date) : null;
  return {
    messageId: file.replace(/\W+/g, "-"),
    threadId: file.replace(/\W+/g, "-"),
    sender: headers.from || "",
    to: headers.to || "",
    subject: headers.subject || "",
    messageTimestamp: date && !Number.isNaN(+date) ? date.toISOString() : null,
    labelIds: ["INBOX"],
    preview: { body: body.slice(0, 200) },
    messageText: body,
  };
}

function loadMailbox(fixtureRoot) {
  const dir = path.join(fixtureRoot, "mailbox");
  if (!fs.existsSync(dir)) return [];
  return fs
    .readdirSync(dir)
    .filter((f) => f.endsWith(".txt"))
    .sort()
    .map((f) => parseMessage(fs.readFileSync(path.join(dir, f), "utf8"), f));
}

function loadCalendar(fixtureRoot) {
  const p = path.join(fixtureRoot, "calendar", "calendar.json");
  if (!fs.existsSync(p)) return [];
  const doc = JSON.parse(fs.readFileSync(p, "utf8"));
  return (doc.events || []).map((e) => ({
    id: e.id,
    summary: e.title,
    start: { dateTime: e.start, timeZone: doc.timezone },
    end: { dateTime: e.end, timeZone: doc.timezone },
    organizer: { email: e.organizer },
    attendees: (e.attendees || []).map((a) => ({ email: a })),
    status: "confirmed",
    htmlLink: `https://calendar.example/event/${e.id}`,
  }));
}

// ---------------------------------------------------------------------------
// actions
// ---------------------------------------------------------------------------

/**
 * Each handler returns Composio's `{ successful, data, error }` envelope.
 * `ctx.record(action, args)` appends to the outbox so writes are observable.
 */
const ACTIONS = {
  GMAIL_FETCH_EMAILS(args, ctx) {
    const max = Number(args.max_results || args.maxResults || 25);
    const q = String(args.query || args.q || "").toLowerCase();
    let messages = ctx.mailbox;
    if (q) {
      messages = messages.filter((m) =>
        `${m.subject} ${m.sender} ${m.messageText}`.toLowerCase().includes(q),
      );
    }
    return {
      successful: true,
      data: {
        messages: messages.slice(0, max),
        resultSizeEstimate: messages.length,
        nextPageToken: null,
      },
      error: null,
    };
  },

  GMAIL_FETCH_MESSAGE_BY_MESSAGE_ID(args, ctx) {
    const id = String(args.message_id || args.messageId || "");
    const msg = ctx.mailbox.find((m) => m.messageId === id);
    return msg
      ? { successful: true, data: msg, error: null }
      : { successful: false, data: null, error: `no message ${id}` };
  },

  GMAIL_SEND_EMAIL(args, ctx) {
    ctx.record("GMAIL_SEND_EMAIL", args);
    return {
      successful: true,
      data: { id: `sent-${ctx.outbox.length}`, threadId: `sent-${ctx.outbox.length}`, labelIds: ["SENT"] },
      error: null,
    };
  },

  GMAIL_CREATE_EMAIL_DRAFT(args, ctx) {
    ctx.record("GMAIL_CREATE_EMAIL_DRAFT", args);
    return {
      successful: true,
      data: { id: `draft-${ctx.outbox.length}`, message: { id: `draft-${ctx.outbox.length}` } },
      error: null,
    };
  },

  GOOGLECALENDAR_EVENTS_LIST(args, ctx) {
    const timeMin = args.timeMin || args.time_min;
    const timeMax = args.timeMax || args.time_max;
    let items = ctx.calendar;
    if (timeMin) items = items.filter((e) => new Date(e.end.dateTime) >= new Date(timeMin));
    if (timeMax) items = items.filter((e) => new Date(e.start.dateTime) <= new Date(timeMax));
    return { successful: true, data: { items, nextPageToken: null }, error: null };
  },

  GOOGLECALENDAR_FIND_EVENT(args, ctx) {
    const q = String(args.query || args.q || "").toLowerCase();
    const items = q
      ? ctx.calendar.filter((e) => e.summary.toLowerCase().includes(q))
      : ctx.calendar;
    return { successful: true, data: { items }, error: null };
  },

  GOOGLECALENDAR_UPDATE_EVENT(args, ctx) {
    ctx.record("GOOGLECALENDAR_UPDATE_EVENT", args);
    const id = String(args.event_id || args.eventId || "");
    const ev = ctx.calendar.find((e) => e.id === id);
    if (!ev) return { successful: false, data: null, error: `no event ${id}` };
    if (args.start_datetime || args.start) ev.start.dateTime = args.start_datetime || args.start;
    if (args.end_datetime || args.end) ev.end.dateTime = args.end_datetime || args.end;
    return { successful: true, data: ev, error: null };
  },
};

const TOOLKITS = [
  { slug: "gmail", name: "Gmail", description: "Read and send mail" },
  { slug: "googlecalendar", name: "Google Calendar", description: "Read and edit events" },
];

const TOOLS = Object.keys(ACTIONS).map((slug) => ({
  slug,
  name: slug,
  description: `Mock ${slug}`,
  toolkit: { slug: slug.startsWith("GMAIL") ? "gmail" : "googlecalendar" },
  input_parameters: { type: "object", properties: {}, additionalProperties: true },
}));

// ---------------------------------------------------------------------------
// server
// ---------------------------------------------------------------------------

export function startMockComposio({ fixtureRoot, outboxPath, port = 0 }) {
  const ctx = {
    mailbox: loadMailbox(fixtureRoot),
    calendar: loadCalendar(fixtureRoot),
    outbox: [],
    requests: [],
    record(action, args) {
      ctx.outbox.push({ at: new Date().toISOString(), action, args });
      if (outboxPath)
        fs.writeFileSync(outboxPath, JSON.stringify(ctx.outbox, null, 2));
    },
  };

  const json = (res, code, body) => {
    const payload = JSON.stringify(body);
    res.writeHead(code, {
      "content-type": "application/json",
      "content-length": Buffer.byteLength(payload),
    });
    res.end(payload);
  };

  const server = http.createServer((req, res) => {
    const url = new URL(req.url, "http://mock");
    const chunks = [];
    req.on("data", (c) => chunks.push(c));
    req.on("end", () => {
      const raw = Buffer.concat(chunks).toString("utf8");
      let body = {};
      if (raw) {
        try {
          body = JSON.parse(raw);
        } catch {
          body = {};
        }
      }
      ctx.requests.push({ method: req.method, path: url.pathname });

      const p = url.pathname.replace(/\/+$/, "");

      // Connected accounts / connections.
      if (/\/(connected_accounts|connections)$/.test(p) && req.method === "GET") {
        return json(res, 200, {
          items: TOOLKITS.map((t) => ({
            id: `${CONNECTED_ACCOUNT_ID}-${t.slug}`,
            status: "ACTIVE",
            toolkit: { slug: t.slug },
            user_id: USER_ID,
            auth_config: { id: `ac_${t.slug}` },
          })),
          total_items: TOOLKITS.length,
        });
      }

      if (/\/toolkits$/.test(p) && req.method === "GET")
        return json(res, 200, { items: TOOLKITS, total_items: TOOLKITS.length });

      if (/\/tools$/.test(p) && req.method === "GET")
        return json(res, 200, { items: TOOLS, total_items: TOOLS.length });

      // Execute: .../tools/execute/<ACTION>  or  .../actions/<ACTION>/execute
      const exec =
        /\/tools\/execute\/([A-Z0-9_]+)$/.exec(p) ||
        /\/actions\/([A-Z0-9_]+)\/execute$/.exec(p);
      if (exec && req.method === "POST") {
        const action = exec[1];
        const handler = ACTIONS[action];
        if (!handler)
          return json(res, 404, {
            successful: false,
            data: null,
            error: `mock composio has no action ${action}`,
          });
        const args = body.arguments || body.input || body.params || body || {};
        try {
          return json(res, 200, handler(args, ctx));
        } catch (e) {
          return json(res, 500, { successful: false, data: null, error: String(e.message) });
        }
      }

      // Anything else: answer in the envelope rather than a bare 404 so the
      // core's error classification sees a Composio-shaped failure.
      return json(res, 404, {
        successful: false,
        data: null,
        error: `mock composio: unhandled ${req.method} ${p}`,
      });
    });
  });

  return new Promise((resolve) => {
    server.listen(port, "127.0.0.1", () => {
      const actual = server.address().port;
      resolve({
        url: `http://127.0.0.1:${actual}`,
        port: actual,
        ctx,
        close: () => new Promise((r) => server.close(r)),
      });
    });
  });
}
