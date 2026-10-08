// D. Connectors through a mock Composio (LOCAL ONLY).
//
// Gmail and GitHub are added as `composio` memory sources and synced from
// mock-composio.mjs; the checks read what memory filed.

import { sleep, waitFor } from "./lib.mjs";

const byToolkit = (items, toolkit) =>
  items.filter((h) => (h.meta?.tags ?? []).includes(toolkit));

export const connectors = {
  id: "D-connectors",
  title:
    "Connectors: filing, edits replace, disconnect-with-clear, disconnect mid-sync, reconnect, repo split, sender actor",
  engines: ["local"],
  needsComposio: true,
  async run(ctx) {
    const { check, composio } = ctx;
    // The sender only reaches the engine with observed_actor on.
    await ctx.setConfigToml("memory", { observed_actor: true });
    await ctx.restartCore();

    const composioItems = () => ctx.listAll({ sources: ["composio"] });
    const added = {};
    for (const toolkit of ["gmail", "github"]) {
      const r = await ctx.tryRpc("openhuman.memory_sources_add", {
        kind: "composio",
        target: toolkit,
        label: `memscen ${toolkit}`,
      });
      check(
        `D0-add-${toolkit}`,
        r.ok,
        `a ${toolkit} source can be added`,
        r.ok ? r.value : r.error,
      );
      if (r.ok) added[toolkit] = r.value?.source?.id;
    }
    await ctx.rpc("openhuman.memory_sources_sync", {});
    const synced = await waitFor(
      async () => {
        const items = await composioItems();
        return byToolkit(items, "gmail").length >= 2 &&
          byToolkit(items, "github").length >= 2
          ? items
          : null;
      },
      { timeoutMs: 180_000, intervalMs: 2000, what: "connector items synced" },
    ).catch((e) => {
      check(
        "D1-synced",
        false,
        "both toolkits' items are synced",
        e.message,
        "high",
      );
      return null;
    });
    if (!synced) return;
    ctx.ledger.addAll(synced.map((h) => h.id));

    // D1: filed under source:<app>; GitHub split by repository.
    const gmail = byToolkit(synced, "gmail");
    const github = byToolkit(synced, "github");
    check(
      "D1-gmail-node",
      gmail.every((h) =>
        String(h.meta?.namespace ?? "").startsWith("source:gmail"),
      ),
      "Gmail items sit under source:gmail",
      gmail.map((h) => h.meta?.namespace),
    );
    check(
      "D1-github-node",
      github.every((h) =>
        String(h.meta?.namespace ?? "").startsWith("source:github"),
      ),
      "GitHub items sit under source:github",
      github.map((h) => h.meta?.namespace),
    );
    check(
      "D6-github-by-repo",
      github.some((h) =>
        String(h.meta?.namespace ?? "").includes("northwind-example--kestrel"),
      ),
      "GitHub items are split by repository ({owner}--{repo}) by default",
      github.map((h) => h.meta?.namespace),
    );

    // D7: the Gmail sender is the observed actor.
    const priya = gmail.find((h) => (h.text ?? "").includes("Pune"));
    check(
      "D7-sender-actor",
      priya?.meta?.observed_actor?.id === "user:priya.raman@example.com",
      "the Gmail sender is stored as observed_actor (user:<address>)",
      priya?.meta?.observed_actor ?? null,
    );

    // D2: an edit upstream replaces the old version, no duplicate.
    composio.edit(
      "gmail",
      "msg-001",
      "Hi Jordan, change of plan: I land in Pune on the 31st at 9am. Priya",
    );
    await ctx.rpc("openhuman.memory_sources_sync", { id: added.gmail });
    const edited = await waitFor(
      async () => {
        const g = byToolkit(await composioItems(), "gmail");
        return g.some((h) => (h.text ?? "").includes("31st")) &&
          !g.some((h) => (h.text ?? "").includes("30th at 7pm"))
          ? g
          : null;
      },
      {
        timeoutMs: 120_000,
        intervalMs: 2000,
        what: "edited Gmail item synced",
      },
    ).catch(() => null);
    if (!edited)
      check("D2-edit-synced", false, "the edited message is synced", null);
    else {
      ctx.ledger.addAll(edited.map((h) => h.id));
      const stale = edited.filter((h) =>
        (h.text ?? "").includes("30th at 7pm"),
      );
      check(
        "D2-edit-replaces",
        stale.length === 0,
        "the old version is gone after the edit (no duplicate)",
        { versions: edited.map((h) => (h.text ?? "").slice(0, 50)) },
        "high",
      );
    }

    // D3: disconnect Gmail with clear: only Gmail's items go.
    const del = await ctx.tryRpc("openhuman.composio_delete_connection", {
      connection_id: composio.connectionId("gmail"),
      clear_memory: true,
    });
    if (
      !del.ok &&
      /not available over the direct route/i.test(JSON.stringify(del.error))
    ) {
      ctx.note(
        "D3-D4-untestable",
        "disconnect-with-clear is not available over Composio's direct route (the mock's mode), so D3 and D4 could not run here; they need the backend route",
      );
      for (const id of Object.values(added))
        if (id)
          await ctx.tryRpc("openhuman.memory_sources_remove", {
            id,
            forget_items: true,
          });
      await ctx.setConfigToml("memory", { observed_actor: false });
      await ctx.restartCore();
      return;
    }
    check(
      "D3-disconnect",
      del.ok,
      "the Gmail connection deletes with clear_memory",
      del.ok ? del.value : del.error,
    );
    await sleep(5000);
    const afterClear = await composioItems();
    check(
      "D3-gmail-cleared",
      byToolkit(afterClear, "gmail").length === 0,
      "Gmail's items are forgotten",
      byToolkit(afterClear, "gmail").map((h) => h.id),
      "high",
    );
    check(
      "D3-github-kept",
      byToolkit(afterClear, "github").length >= 2,
      "GitHub's items are untouched",
      byToolkit(afterClear, "github").length,
      "high",
    );

    // D4: disconnect GitHub while its sync is in flight: nothing left behind.
    composio.edit(
      "github",
      9001,
      "Kestrel launch checklist, updated: add the accessibility review.",
    );
    composio.setFetchDelay(8000);
    await ctx.rpc("openhuman.memory_sources_sync", { id: added.github });
    await sleep(2000);
    await ctx.tryRpc("openhuman.composio_delete_connection", {
      connection_id: composio.connectionId("github"),
      clear_memory: true,
    });
    composio.setFetchDelay(0);
    await sleep(20_000);
    const leftover = byToolkit(await composioItems(), "github");
    check(
      "D4-mid-sync-disconnect",
      leftover.length === 0,
      "a disconnect during a sync leaves no GitHub items behind",
      leftover.map((h) => (h.text ?? "").slice(0, 50)),
      "high",
    );

    // D5: reconnect: syncing works again.
    composio.reconnect("gmail");
    await ctx.rpc("openhuman.memory_sources_sync", { id: added.gmail });
    const back = await waitFor(
      async () => {
        const g = byToolkit(await composioItems(), "gmail");
        return g.length >= 2 ? g : null;
      },
      {
        timeoutMs: 120_000,
        intervalMs: 2000,
        what: "Gmail synced after reconnect",
      },
    ).catch(() => null);
    check(
      "D5-reconnect-syncs",
      !!back,
      "after a reconnect, Gmail syncs again",
      back?.length ?? 0,
    );
    if (back) ctx.ledger.addAll(back.map((h) => h.id));

    for (const id of Object.values(added))
      if (id)
        await ctx.tryRpc("openhuman.memory_sources_remove", {
          id,
          forget_items: true,
        });
    await ctx.setConfigToml("memory", { observed_actor: false });
    await ctx.restartCore();
  },
};
