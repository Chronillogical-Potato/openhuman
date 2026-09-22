/**
 * Six "real life" assistant tasks, rewritten over a mock corpus that lives in
 * `fixtures/`. Nothing here touches a real mailbox, calendar or bank: the
 * corpus is fictional (Alex Rivera <alex.rivera@example.com>) and every domain
 * is `*.example`, so a run is reproducible and safe to commit.
 *
 * Each scenario is a prompt plus a *grader*. The grader is the part that makes
 * the number mean something — "the turn completed" is not completion, and a
 * model that writes a plausible-looking CSV full of invented rows has to score
 * zero on the rows it invented. Graders therefore check facts that are only
 * derivable from the fixtures.
 *
 * Grader contract: `grade(ctx) -> { checks: [{ id, ok, detail }] }` where
 * `ctx.read(relPath)` returns the file's text (or null), `ctx.exists(relPath)`,
 * and `ctx.transcript` is the final assistant message.
 */

import path from "node:path";

const TODAY = "2026-09-22"; // fixtures are anchored here; see README.md

// ---------------------------------------------------------------------------
// helpers shared by graders
// ---------------------------------------------------------------------------

const norm = (s) => (s || "").toLowerCase();

/** Case-insensitive "does the text mention all of these". */
function mentionsAll(text, needles) {
  const t = norm(text);
  return needles.filter((n) => !t.includes(norm(n)));
}

/** Parse a CSV into rows of trimmed cells. Tolerates quoted fields. */
function parseCsv(text) {
  const rows = [];
  let row = [];
  let cell = "";
  let quoted = false;
  for (let i = 0; i < text.length; i += 1) {
    const c = text[i];
    if (quoted) {
      if (c === '"') {
        if (text[i + 1] === '"') {
          cell += '"';
          i += 1;
        } else quoted = false;
      } else cell += c;
      continue;
    }
    if (c === '"') quoted = true;
    else if (c === ",") {
      row.push(cell.trim());
      cell = "";
    } else if (c === "\n") {
      row.push(cell.trim());
      if (row.some((x) => x !== "")) rows.push(row);
      row = [];
      cell = "";
    } else if (c !== "\r") cell += c;
  }
  row.push(cell.trim());
  if (row.some((x) => x !== "")) rows.push(row);
  return rows;
}

/** Pull every `$12.34` / `12.34 USD` style amount out of a string. */
function amounts(text) {
  return [...(text || "").matchAll(/\$?\s?(\d+(?:\.\d{2}))/g)].map((m) =>
    Number(m[1]),
  );
}

const check = (id, ok, detail = "") => ({ id, ok: Boolean(ok), detail });

// ---------------------------------------------------------------------------
// scenarios
// ---------------------------------------------------------------------------

export const SCENARIOS = [
  // -------------------------------------------------------------------------
  {
    id: "calendar-buffer",
    title: "Smart calendar & meeting buffer",
    capabilities: ["file_read", "file_write", "temporal reasoning"],
    // 7 events on 2026-09-23. Three of them start less than 15 minutes after
    // the previous one ends: ev-02 (0 min), ev-05 (0 min), ev-06 (10 min).
    prompt: `Today is ${TODAY}. My calendar for tomorrow is the JSON file at \`calendar/calendar.json\` (relative to your working directory).

Find every pair of consecutive meetings on 2026-09-23 where the second meeting starts less than 15 minutes after the first one ends. Ignore events that have no attendee other than me.

For each one, decide whether to push the second meeting back so there is at least a 15 minute gap, and draft a short rescheduling email to that meeting's organizer asking for the adjustment.

Write your result to \`out/meeting_buffer_plan.json\` with exactly this shape:

{
  "conflicts": [
    { "event_id": "...", "title": "...", "gap_minutes": 0, "proposed_start": "2026-09-23T09:45:00-04:00", "organizer": "..." }
  ],
  "emails": [
    { "to": "...", "subject": "...", "body": "..." }
  ]
}

Do not invent meetings that are not in the file.`,
    grade(ctx) {
      const raw = ctx.read("out/meeting_buffer_plan.json");
      if (!raw) return { checks: [check("output_exists", false, "file missing")] };
      let doc;
      try {
        doc = JSON.parse(raw);
      } catch (e) {
        return {
          checks: [
            check("output_exists", true),
            check("valid_json", false, String(e.message).slice(0, 120)),
          ],
        };
      }
      const conflicts = Array.isArray(doc.conflicts) ? doc.conflicts : [];
      const ids = new Set(conflicts.map((c) => String(c.event_id || "").trim()));
      const emails = Array.isArray(doc.emails) ? doc.emails : [];
      const expected = ["ev-02", "ev-05", "ev-06"];
      const missing = expected.filter((e) => !ids.has(e));
      const spurious = [...ids].filter((i) => !expected.includes(i));
      // ev-06 already has a 10 minute gap; getting that one right is the test
      // of "less than 15" rather than "zero".
      const gapOk = conflicts.every((c) => {
        if (c.event_id === "ev-06") return Number(c.gap_minutes) === 10;
        if (c.event_id === "ev-02" || c.event_id === "ev-05")
          return Number(c.gap_minutes) === 0;
        return true;
      });
      const organizers = new Set(
        conflicts.map((c) => norm(c.organizer)).filter(Boolean),
      );
      return {
        checks: [
          check("output_exists", true),
          check("valid_json", true),
          check(
            "found_all_three_conflicts",
            missing.length === 0,
            missing.length ? `missing ${missing.join(",")}` : "",
          ),
          check(
            "no_hallucinated_conflicts",
            spurious.length === 0,
            spurious.length ? `extra ${spurious.join(",")}` : "",
          ),
          check("gap_minutes_correct", gapOk),
          check(
            "skipped_solo_event",
            !ids.has("ev-07"),
            ids.has("ev-07") ? "flagged the solo focus block" : "",
          ),
          check(
            "drafted_an_email_per_conflict",
            emails.length >= Math.min(3, conflicts.length) && emails.length > 0,
            `${emails.length} emails for ${conflicts.length} conflicts`,
          ),
          check(
            "emails_address_real_organizers",
            emails.length > 0 &&
              emails.every((e) =>
                [...organizers].some((o) => norm(e.to).includes(o)),
              ),
          ),
        ],
      };
    },
  },

  // -------------------------------------------------------------------------
  {
    id: "subscription-scan",
    title: "Subscription & recurring expense scanner",
    capabilities: ["file_read", "grep", "file_write", "parsing"],
    prompt: `Today is ${TODAY}. The directory \`mailbox/\` (relative to your working directory) holds my recent receipt and bank-notification emails as plain text files.

Read them and identify every genuine recurring subscription. A one-off purchase, a usage-based utility bill, and a phishing email are not subscriptions. A bank card alert that duplicates a merchant receipt you already counted is not a second subscription.

Write \`out/subscriptions.csv\` with exactly this header and one row per subscription:

service,cost,frequency,last_billed,price_increase

- \`cost\` is the most recent amount as a plain number, e.g. 15.99
- \`frequency\` is \`monthly\` or \`yearly\`
- \`last_billed\` is ISO \`YYYY-MM-DD\`
- \`price_increase\` is \`yes\` if an earlier receipt in the mailbox shows a lower price for the same service, otherwise \`no\`

Then write a one-paragraph summary of what you flagged to \`out/subscriptions_notes.md\`.`,
    grade(ctx) {
      const raw = ctx.read("out/subscriptions.csv");
      if (!raw) return { checks: [check("output_exists", false, "file missing")] };
      const rows = parseCsv(raw);
      const header = (rows[0] || []).map((h) => norm(h));
      const body = rows.slice(1);
      const byService = new Map();
      for (const r of body) {
        if (!r[0]) continue;
        byService.set(norm(r[0]).replace(/[^a-z]/g, ""), r);
      }
      const get = (name) => byService.get(name.replace(/[^a-z]/g, ""));
      const streamflix = get("streamflix");
      const clouddrive = get("clouddrivepro") || get("clouddrive");
      const newsdaily = get("newsdaily");
      const gympass = get("gympass");
      const found = [streamflix, clouddrive, newsdaily, gympass].filter(Boolean);
      const distractors = ["mealbox", "metroelectric", "premiumvault", "firstharbor"];
      const leaked = distractors.filter((d) =>
        [...byService.keys()].some((k) => k.includes(d.replace(/[^a-z]/g, ""))),
      );
      const col = (row, name) => {
        const i = header.indexOf(name);
        return i >= 0 && row ? row[i] : undefined;
      };
      return {
        checks: [
          check("output_exists", true),
          check(
            "header_exact",
            header.join(",") ===
              "service,cost,frequency,last_billed,price_increase",
            header.join(","),
          ),
          check(
            "all_four_subscriptions",
            found.length === 4,
            `found ${found.length}/4`,
          ),
          check(
            "no_distractors",
            leaked.length === 0,
            leaked.length ? `counted ${leaked.join(",")}` : "",
          ),
          check("row_count_is_four", body.length === 4, `${body.length} rows`),
          check(
            "streamflix_cost_1599",
            streamflix && Number(col(streamflix, "cost")) === 15.99,
          ),
          check(
            "streamflix_price_increase_yes",
            streamflix && norm(col(streamflix, "price_increase")).startsWith("y"),
          ),
          check(
            "gympass_price_increase_no",
            gympass && norm(col(gympass, "price_increase")).startsWith("n"),
          ),
          check(
            "newsdaily_yearly",
            newsdaily && norm(col(newsdaily, "frequency")).includes("year"),
          ),
          check(
            "clouddrive_last_billed",
            clouddrive && String(col(clouddrive, "last_billed")).includes("2026-09-12"),
          ),
          check("notes_written", ctx.exists("out/subscriptions_notes.md")),
        ],
      };
    },
  },

  // -------------------------------------------------------------------------
  {
    id: "baggage-policy",
    title: "Airline baggage policy summariser (live web)",
    capabilities: ["web_search", "web_fetch", "file_write"],
    // The only scenario that leaves the sandbox. Airline baggage rules are
    // public, non-personal, and change often enough that a model answering
    // from memory is detectable: the grader demands a cited source URL.
    prompt: `I am flying Skyward Atlantic (a real airline you should look up as "Delta Air Lines" — use Delta's own published policy) from JFK to LHR in Basic Economy.

Search the web for Delta Air Lines' current published baggage policy and read the official delta.com pages. Extract the actual allowance for Basic Economy on a transatlantic route: carry-on dimensions and any weight limit, personal item allowance, checked bag allowance, and the fee for the first and second checked bag.

Write \`out/delta_baggage_guide.md\` as a short markdown guide. It must contain:
- a table of the limits with exact numbers and units
- a "Fees" section
- a "Sources" section listing the delta.com URLs you actually fetched, one per line

If the live pages do not state something, say so explicitly rather than filling it in from memory.`,
    grade(ctx) {
      const doc = ctx.read("out/delta_baggage_guide.md");
      if (!doc) return { checks: [check("output_exists", false, "file missing")] };
      const urls = [...doc.matchAll(/https?:\/\/[^\s)\]]+/g)].map((m) => m[0]);
      const deltaUrls = urls.filter((u) => /(^|\/\/|\.)delta\.com/i.test(u));
      const hasDims = /\b22\s*(x|×)\s*14\s*(x|×)\s*9\b/i.test(doc) ||
        /\b\d{2}\s*(x|×)\s*\d{2}\s*(x|×)\s*\d{1,2}\s*(in|inch|cm)/i.test(doc);
      return {
        checks: [
          check("output_exists", true),
          check("has_table", /\|.*\|/.test(doc)),
          check("has_fees_section", /##\s*fees/i.test(doc)),
          check("has_sources_section", /##\s*sources/i.test(doc)),
          check(
            "cites_delta_com",
            deltaUrls.length > 0,
            `${urls.length} urls, ${deltaUrls.length} on delta.com`,
          ),
          check("states_carryon_dimensions", hasDims),
          check(
            "mentions_personal_item",
            /personal item/i.test(doc),
          ),
          check("mentions_basic_economy", /basic economy/i.test(doc)),
          check(
            "mentions_checked_allowance",
            /checked/i.test(doc) && /(fee|\$|free|included)/i.test(doc),
          ),
        ],
      };
    },
  },

  // -------------------------------------------------------------------------
  {
    id: "meal-plan",
    title: "Weekly meal plan & deduplicated grocery list",
    capabilities: ["generation", "constraint satisfaction", "file_write"],
    prompt: `Create a 5-day dinner meal plan for a family of 4 that is Mediterranean-diet friendly and keeps active prep time under 30 minutes per night. Reuse shared ingredients across nights so nothing is bought for a single meal — spinach, feta and olive oil should each appear in at least two dinners.

Write \`out/meal_plan.md\`. Use one \`## Day N — <dish>\` heading per night, and under each give an \`Active prep: N minutes\` line, an ingredient list with quantities scaled for 4 people, and brief steps.

Then write \`out/grocery_list.txt\`: a single consolidated shopping checklist for all five dinners. Group it under supermarket aisle headings written as \`[Aisle name]\` on their own line, with one \`- item — quantity\` per line underneath. Every ingredient must appear exactly once, with the quantities summed across all five nights.`,
    grade(ctx) {
      const plan = ctx.read("out/meal_plan.md");
      const list = ctx.read("out/grocery_list.txt");
      if (!plan || !list)
        return {
          checks: [
            check("plan_exists", Boolean(plan)),
            check("list_exists", Boolean(list)),
          ],
        };
      const days = [...plan.matchAll(/^##\s*Day\s*(\d)/gim)].map((m) => m[1]);
      const prepTimes = [...plan.matchAll(/active prep:\s*(\d+)/gi)].map((m) =>
        Number(m[1]),
      );
      const aisles = [...list.matchAll(/^\s*\[([^\]]+)\]\s*$/gm)].map((m) =>
        m[1].trim(),
      );
      const items = [...list.matchAll(/^\s*[-*]\s*(.+?)(?:\s+[—–-]\s+|:)/gm)].map(
        (m) => norm(m[1]).replace(/[^a-z ]/g, "").trim(),
      );
      const dupes = items.filter((it, i) => items.indexOf(it) !== i);
      const shared = ["spinach", "feta", "olive oil"];
      const sharedCounts = shared.map((ing) => ({
        ing,
        n: (plan.match(new RegExp(ing, "gi")) || []).length,
      }));
      return {
        checks: [
          check("plan_exists", true),
          check("list_exists", true),
          check("five_days", new Set(days).size === 5, `days: ${days.join(",")}`),
          check(
            "prep_times_stated",
            prepTimes.length >= 5,
            `${prepTimes.length} stated`,
          ),
          check(
            "prep_under_30",
            prepTimes.length > 0 && prepTimes.every((p) => p <= 30),
            `max ${Math.max(0, ...prepTimes)}`,
          ),
          check(
            "aisle_headings",
            aisles.length >= 3,
            `aisles: ${aisles.join(" / ")}`,
          ),
          check("items_parsed", items.length >= 12, `${items.length} items`),
          check(
            "no_duplicate_items",
            dupes.length === 0,
            dupes.length ? `dupes: ${[...new Set(dupes)].join(", ")}` : "",
          ),
          check(
            "shared_ingredients_reused",
            sharedCounts.every((s) => s.n >= 2),
            sharedCounts.map((s) => `${s.ing}=${s.n}`).join(" "),
          ),
          check(
            "quantities_present",
            (list.match(/\d/g) || []).length >= 10,
          ),
        ],
      };
    },
  },

  // -------------------------------------------------------------------------
  {
    id: "trip-itinerary",
    title: "Multi-source trip itinerary & logistics",
    capabilities: [
      "file_read",
      "pdf extraction",
      "web_fetch",
      "synthesis",
      "file_write",
    ],
    // The hardest one: three sources in three formats (text email, binary PDF,
    // live web) plus a routing/ordering judgement, into one HTML artifact.
    prompt: `Today is ${TODAY}. Build a complete 3-day Tokyo itinerary from my own documents.

Sources, relative to your working directory:
- \`mailbox/2026-09-18-flight.txt\` — my flight confirmation
- \`documents/hotel_booking_tokyo.pdf\` — my hotel booking (a real PDF; extract the text from it, do not guess)

Do this:
1. Read both and establish the exact dates I am actually in Tokyo, the hotel name, its address, and its nearest station.
2. Look up the typical mid-October weather for Tokyo and say where you got it. Put outdoor activities on the better days and indoor ones (museums, markets, galleries) on the wetter or colder days.
3. Plan each day as a sequence of venues that are geographically sensible from the hotel, and give an estimated walking or train time between consecutive venues.
4. Write \`out/tokyo_trip_master.html\` — a single self-contained HTML page with a section per day, a table of venues with arrival times and transit times, and an "Emergency information" section with Japan's emergency numbers and the address and phone number of my hotel.

Only use dates when I am actually in Tokyo. Do not schedule anything for a travel day I spend in the air.`,
    grade(ctx) {
      const html = ctx.read("out/tokyo_trip_master.html");
      if (!html) return { checks: [check("output_exists", false, "file missing")] };
      const t = norm(html);
      const missingFacts = mentionsAll(html, [
        "Sakura View Hotel",
        "Nishi-Shinjuku",
        "+81 3-5555-0142",
        "QK7F2P",
      ]);
      // In Tokyo 15–18 Oct; 14 Oct is spent in the air.
      const mentions14 = /(oct(ober)?\s*14|14\s*oct|2026-10-14)/i.test(html);
      const dayCount = (html.match(/<h2[^>]*>/gi) || []).length;
      return {
        checks: [
          check("output_exists", true),
          check("is_html", /<html/i.test(html) && /<\/html>/i.test(html)),
          check(
            "read_the_pdf",
            missingFacts.filter((f) => f !== "QK7F2P").length === 0,
            missingFacts.length ? `missing: ${missingFacts.join(", ")}` : "",
          ),
          check("has_flight_confirmation", /QK7F2P/i.test(html)),
          check(
            "correct_stay_dates",
            /15/.test(html) && /18/.test(html) && /oct/i.test(html),
          ),
          check(
            "excludes_travel_day",
            !mentions14,
            mentions14 ? "scheduled something on 14 Oct (in the air)" : "",
          ),
          check("three_day_sections", dayCount >= 3, `${dayCount} <h2>`),
          check("has_venue_table", /<table/i.test(html)),
          check(
            "has_transit_times",
            /(\d+\s*(min|minute))/i.test(html) &&
              /(walk|train|metro|subway|line)/i.test(html),
          ),
          check(
            "emergency_numbers",
            t.includes("110") && t.includes("119"),
          ),
          check("weather_sourced", /(weather|climate)/i.test(html) && /https?:\/\//.test(html)),
        ],
      };
    },
  },

  // -------------------------------------------------------------------------
  {
    id: "fact-check-publish",
    title: "Fact-check, reformat and lint a draft",
    capabilities: ["file_read", "web_search", "transform", "verification"],
    prompt: `Today is ${TODAY}. My draft article is at \`drafts/draft.md\` (relative to your working directory).

1. Read it and list every factual claim and every hyperlink.
2. Verify the claims against current sources on the web. The remote-work statistic in the opening is several years old — replace it with a current, verified figure and cite the source inline.
3. Check every hyperlink and every image path. Image paths are relative to \`drafts/\`; at least one asset referenced by the draft does not exist on disk, and at least one hyperlink does not resolve. Find them.
4. Produce two versions:
   - \`out/dist/newsletter.html\` — a self-contained HTML email template (inline styles, table-based layout, no external CSS)
   - \`out/dist/post.md\` — a markdown post keeping the YAML front matter, with the corrected stats and links
5. Write \`out/dist/lint_report.md\` listing every broken link and missing image path you found, each with the exact path or URL and why it failed. Do not ship a broken reference into either output.`,
    grade(ctx) {
      const html = ctx.read("out/dist/newsletter.html");
      const md = ctx.read("out/dist/post.md");
      const lint = ctx.read("out/dist/lint_report.md");
      const checks = [
        check("newsletter_exists", Boolean(html)),
        check("post_exists", Boolean(md)),
        check("lint_report_exists", Boolean(lint)),
      ];
      if (!html || !md || !lint) return { checks };
      const brokenImg = "handoff-latency-chart.png";
      const brokenLink = "example.com/blog/async-standups-2019";
      checks.push(
        check("newsletter_is_html", /<html/i.test(html) || /<table/i.test(html)),
        check(
          "newsletter_inline_styles",
          /style\s*=\s*"/i.test(html) && !/<link[^>]+stylesheet/i.test(html),
        ),
        check("post_keeps_front_matter", /^---[\s\S]*?title:/m.test(md)),
        check(
          "lint_names_missing_image",
          norm(lint).includes(brokenImg),
          "expected the missing chart asset",
        ),
        check(
          "lint_names_broken_link",
          norm(lint).includes(norm(brokenLink)) ||
            norm(lint).includes("async-standups"),
        ),
        check(
          "missing_image_not_shipped",
          !norm(html).includes(brokenImg) && !norm(md).includes(brokenImg),
        ),
        check(
          "broken_link_not_shipped",
          !norm(html).includes(norm(brokenLink)) &&
            !norm(md).includes(norm(brokenLink)),
        ),
        check(
          "stat_updated",
          !/as of 2021/i.test(md),
          "the 2021 remote-work stat survived unchanged",
        ),
        check(
          "stat_has_citation",
          /\[[^\]]+\]\(https?:\/\/[^)]+\)/.test(md),
        ),
      );
      return { checks };
    },
  },
];

export function scenarioById(id) {
  const s = SCENARIOS.find((x) => x.id === id);
  if (!s) throw new Error(`unknown scenario '${id}'`);
  return s;
}

export const FIXTURES_DIR = path.join(
  path.dirname(new URL(import.meta.url).pathname),
  "fixtures",
);
