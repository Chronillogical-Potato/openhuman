#!/usr/bin/env node
// Fold the per-target tinyanalyzer JSON reports written by
// scripts/dep-audit/run.sh into one Markdown report.
//
//   node scripts/dep-audit/report.mjs --reports target/dep-audit \
//        [--out target/dep-audit/REPORT.md] [--top 15] [--json summary.json]
//
// The report has four sections per the run.sh header: unused declared
// dependencies, crates resolved at several versions, the heaviest direct
// dependencies (by exclusive transitive crate count), and cross-repository
// version drift for crates that several targets depend on directly.
//
// Every unused-dependency flag is re-checked here with a textual scan of the
// package's own sources, because tinyanalyzer's check does not see crate names
// inside attributes (`#[derive(thiserror::Error)]`, `#[serde(...)]`). A flag
// whose crate name *does* appear somewhere in the package is reported as
// "attribute/macro use" instead of "remove", so nothing is silently hidden but
// the reader knows which rows are real.

import fs from "node:fs";
import path from "node:path";
import { execFileSync } from "node:child_process";

const args = parseArgs(process.argv.slice(2));
const reportsDir = path.resolve(args.reports ?? "target/dep-audit");
const outFile = args.out ? path.resolve(args.out) : null;
const jsonOut = args.json ? path.resolve(args.json) : null;
const top = Number(args.top ?? 15);
const toolVersion = args["tinyanalyzer-version"] ?? "tinyanalyzer";

const targets = readTargets(path.join(reportsDir, "targets.tsv"));
const reports = [];
for (const target of targets) {
  const file = path.join(reportsDir, `${target.name}.json`);
  if (!fs.existsSync(file)) continue;
  const data = JSON.parse(fs.readFileSync(file, "utf8"));
  reports.push({ target, data });
}
if (reports.length === 0) {
  console.error(`dep-audit/report: no <target>.json reports in ${reportsDir}`);
  process.exit(1);
}

const summary = {
  generated_at: new Date().toISOString(),
  tinyanalyzer: toolVersion,
  targets: reports.map(({ target, data }) => ({
    name: target.name,
    path: target.path,
    commit: target.sha,
    remote: target.remote,
    packages: data.dependencies.packages.length,
    external_packages: data.dependencies.external_packages,
    max_depth: data.dependencies.max_depth,
    direct: data.dependencies.packages.filter((p) => p.is_direct).length,
    unused: unusedFor(target, data),
    duplicates: data.dependencies.duplicates
      .map((d) => ({ name: d.name, versions: d.versions }))
      .sort((a, b) => b.versions.length - a.versions.length || a.name.localeCompare(b.name)),
    heavy: heavyFor(data, top),
  })),
};
summary.drift = driftAcross(reports);

const md = renderMarkdown(summary, top);
if (outFile) {
  fs.mkdirSync(path.dirname(outFile), { recursive: true });
  fs.writeFileSync(outFile, md);
} else {
  process.stdout.write(md);
}
if (jsonOut) {
  fs.mkdirSync(path.dirname(jsonOut), { recursive: true });
  fs.writeFileSync(jsonOut, JSON.stringify(summary, null, 2) + "\n");
}

// ---------------------------------------------------------------------------

function parseArgs(argv) {
  const out = {};
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (!a.startsWith("--")) continue;
    const key = a.slice(2);
    const next = argv[i + 1];
    if (next !== undefined && !next.startsWith("--")) {
      out[key] = next;
      i += 1;
    } else {
      out[key] = true;
    }
  }
  return out;
}

function readTargets(file) {
  if (!fs.existsSync(file)) {
    console.error(`dep-audit/report: missing ${file}; run scripts/dep-audit/run.sh first`);
    process.exit(1);
  }
  return fs
    .readFileSync(file, "utf8")
    .split("\n")
    .filter(Boolean)
    .map((line) => {
      const [name, p, sha, remote] = line.split("\t");
      return { name, path: p, sha, remote: remote ?? "" };
    });
}

/** Directory of a workspace member from its cargo package id. */
function packageDir(data, packageName) {
  const pkg = data.dependencies.packages.find(
    (p) => p.name === packageName && (p.is_workspace_member || p.is_root_package),
  );
  if (!pkg) return null;
  const m = /^path\+file:\/\/(.+?)(#.*)?$/.exec(pkg.id);
  return m ? m[1] : null;
}

/**
 * Directories whose `*.rs` files belong to a package: its own directory plus
 * the parent directory of every explicit `path = "..."` target in its
 * Cargo.toml (`[[test]]`, `[[example]]`, `[[bench]]`, `[[bin]]`, `[lib]`).
 * OpenHuman's root crate declares its integration tests as
 * `path = "../../tests/<name>.rs"`, which a scan of the crate dir alone misses.
 */
function packageSourceDirs(dir) {
  const dirs = new Set([dir]);
  const manifest = path.join(dir, "Cargo.toml");
  if (!fs.existsSync(manifest)) return [...dirs];
  const toml = fs.readFileSync(manifest, "utf8");
  let section = "";
  for (const raw of toml.split("\n")) {
    const line = raw.trim();
    const head = /^\[\[?([a-zA-Z.-]+)\]?\]/.exec(line);
    if (head) {
      section = head[1];
      continue;
    }
    if (!/^(test|example|bench|bin|lib)$/.test(section)) continue;
    const m = /^path\s*=\s*"([^"]+)"/.exec(line);
    if (m) dirs.add(path.dirname(path.resolve(dir, m[1])));
  }
  return [...dirs];
}

/**
 * Textual re-check of an unused flag over the package's sources.
 *
 * Returns:
 *   "path"  — the crate is referenced the way Rust code references a crate
 *             (`foo::`, `use foo`, `extern crate foo`, `#[foo`, `foo!`), which
 *             tinyanalyzer misses only when it sits inside an attribute or a
 *             macro body. Treat as used.
 *   "word"  — the bare name occurs (comment, string, doc) but never as a path.
 *   "none"  — nothing at all.
 */
function textualUse(dir, dep) {
  if (!dir || !fs.existsSync(dir)) return "unknown";
  const ident = dep.replace(/-/g, "_");
  const dirs = packageSourceDirs(dir);
  const pathRe = `(\\b${ident}::|\\buse\\s+${ident}\\b|extern\\s+crate\\s+${ident}\\b|#\\[${ident}\\b|\\b${ident}!)`;
  if (grepAny(dirs, pathRe)) return "path";
  if (grepAny(dirs, `\\b${ident}\\b`)) return "word";
  return "none";
}

function grepAny(dirs, pattern) {
  try {
    const out = execFileSync(
      "grep",
      ["-rlE", pattern, "--include=*.rs", "--exclude-dir=target", "--exclude-dir=vendor", ...dirs],
      { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] },
    );
    return out.trim().length > 0;
  } catch (err) {
    // grep exits 1 when nothing matched; anything else is a real failure we
    // would rather surface as "used" than as a false removal.
    return err.status !== 1;
  }
}

function unusedFor(target, data) {
  const rows = [];
  const seen = new Set();
  for (const u of data.dependencies.unused) {
    const key = `${u.package}\u0000${u.dependency}`;
    // The same dependency declared as both normal and dev shows up twice.
    const kinds = data.dependencies.unused
      .filter((v) => v.package === u.package && v.dependency === u.dependency)
      .map((v) => v.kind);
    if (seen.has(key)) continue;
    seen.add(key);
    const dir = packageDir(data, u.package);
    const evidence = textualUse(dir, u.dependency);
    const pkg = data.dependencies.packages.find((p) => p.name === u.dependency);
    rows.push({
      package: u.package,
      dependency: u.dependency,
      kinds: [...new Set(kinds)],
      version: pkg?.version ?? null,
      exclusive_count: pkg?.exclusive_count ?? null,
      evidence,
      verdict: evidence === "path" ? "keep" : "remove",
    });
  }
  return rows.sort(
    (a, b) =>
      (a.verdict === "remove" ? 0 : 1) - (b.verdict === "remove" ? 0 : 1) ||
      (a.evidence === "none" ? 0 : 1) - (b.evidence === "none" ? 0 : 1) ||
      (b.exclusive_count ?? 0) - (a.exclusive_count ?? 0) ||
      a.package.localeCompare(b.package) ||
      a.dependency.localeCompare(b.dependency),
  );
}

function heavyFor(data, n) {
  return data.dependencies.packages
    .filter((p) => p.is_direct && !p.is_workspace_member && !p.is_root_package)
    .map((p) => ({
      name: p.name,
      version: p.version,
      kinds: p.kinds,
      exclusive_count: p.exclusive_count,
      transitive_count: p.transitive_count,
      source_bytes: p.source_bytes,
      features: p.features,
    }))
    .sort((a, b) => b.exclusive_count - a.exclusive_count || b.source_bytes - a.source_bytes)
    .slice(0, n);
}

/**
 * For every crate that is a *direct* dependency of two or more targets, the
 * resolved version in each. Only rows where the versions differ are kept.
 */
function driftAcross(reports) {
  const byCrate = new Map();
  for (const { target, data } of reports) {
    for (const p of data.dependencies.packages) {
      if (!p.is_direct || p.is_workspace_member || p.is_root_package) continue;
      if (!byCrate.has(p.name)) byCrate.set(p.name, new Map());
      const m = byCrate.get(p.name);
      if (!m.has(target.name)) m.set(target.name, new Set());
      m.get(target.name).add(p.version);
    }
  }
  const rows = [];
  for (const [name, perTarget] of byCrate) {
    if (perTarget.size < 2) continue;
    const versions = new Set([...perTarget.values()].flatMap((s) => [...s]));
    if (versions.size < 2) continue;
    rows.push({
      name,
      targets: [...perTarget.entries()]
        .map(([t, vs]) => ({ target: t, versions: [...vs].sort(semverish) }))
        .sort((a, b) => a.target.localeCompare(b.target)),
      versions: [...versions].sort(semverish),
    });
  }
  return rows.sort((a, b) => b.versions.length - a.versions.length || a.name.localeCompare(b.name));
}

function semverish(a, b) {
  const pa = a.split(/[.+-]/).map((x) => (Number.isNaN(Number(x)) ? x : Number(x)));
  const pb = b.split(/[.+-]/).map((x) => (Number.isNaN(Number(x)) ? x : Number(x)));
  for (let i = 0; i < Math.max(pa.length, pb.length); i += 1) {
    if (pa[i] === pb[i]) continue;
    if (pa[i] === undefined) return -1;
    if (pb[i] === undefined) return 1;
    return pa[i] < pb[i] ? -1 : 1;
  }
  return 0;
}

function mib(bytes) {
  return `${(bytes / 1048576).toFixed(1)} MiB`;
}

function code(s) {
  return `\`${s}\``;
}

function renderMarkdown(summary, n) {
  const lines = [];
  const push = (...xs) => lines.push(...xs);

  push(
    `# Dependency audit`,
    ``,
    `Generated ${summary.generated_at} by ${code("scripts/dep-audit/run.sh")} (${summary.tinyanalyzer}).`,
    `Re-run with ${code("pnpm dep:audit")}; see ${code("scripts/dep-audit/README.md")} for how to act on each section.`,
    ``,
    `## Targets`,
    ``,
    `| Target | Path | Commit | Direct deps | Crates in graph | Unused flags | Duplicate versions |`,
    `| --- | --- | --- | ---: | ---: | ---: | ---: |`,
  );
  for (const t of summary.targets) {
    push(
      `| ${t.name} | ${code(t.path)} | ${code(t.commit.slice(0, 10))} | ${t.direct} | ${t.external_packages} | ${t.unused.length} | ${t.duplicates.length} |`,
    );
  }

  // --- Unused --------------------------------------------------------------
  push(``, `## 1. Declared dependencies no source file names`, ``);
  push(
    `Verdict ${code("remove")}: nothing in the package's ${code("*.rs")} files (including ${code("[[test]]")}/${code("[[example]]")} targets declared by path) references the crate as ${code("crate::…")}, ${code("use crate")}, ${code("#[crate…")} or ${code("crate!")}. Delete the line from ${code("Cargo.toml")} and build; if it was an optional dependency, drop the ${code("dep:")} feature too. "name only" means the word occurs in a comment or string, which is not a use.`,
    `Verdict ${code("keep")}: tinyanalyzer saw no ${code("use")}/path, but one exists inside an attribute or macro body (${code("#[tokio::test]")}, ${code("#[derive(thiserror::Error)]")}) — a false positive of the tool, listed so the count is honest.`,
    `Crates listed in ${code("[dependencies].ignore_unused")} of ${code("scripts/dep-audit/tinyanalyzer.toml")} are not reported at all.`,
    ``,
  );
  const unusedRows = summary.targets.flatMap((t) => t.unused.map((u) => ({ target: t.name, ...u })));
  if (unusedRows.length === 0) {
    push(`_None._`);
  } else {
    push(
      `| Target | Package | Dependency | Kind | Resolved | Exclusive crates | Verdict |`,
      `| --- | --- | --- | --- | --- | ---: | --- |`,
    );
    for (const u of unusedRows) {
      push(
        `| ${u.target} | ${u.package} | ${code(u.dependency)} | ${u.kinds.join(", ")} | ${u.version ?? "—"} | ${u.exclusive_count ?? "—"} | ${u.verdict === "remove" ? (u.evidence === "word" ? "**remove** (name only)" : "**remove**") : "keep (attribute/macro path)"} |`,
      );
    }
  }

  // --- Duplicates ----------------------------------------------------------
  push(``, `## 2. Crates resolved at more than one version`, ``);
  push(
    `Each version is compiled and linked separately. The ${code("root")} row is the one that costs the shipped build; submodule rows show where a pin should move so the root can unify.`,
    `Use ${code("cargo tree -i <crate>@<version>")} in that target to find who pins the older one.`,
    ``,
  );
  for (const t of summary.targets) {
    if (t.duplicates.length === 0) continue;
    push(`### ${t.name} — ${t.duplicates.length} crate(s)`, ``);
    push(`| Crate | Versions |`, `| --- | --- |`);
    for (const d of t.duplicates) {
      push(`| ${code(d.name)} | ${d.versions.map(code).join(", ")} |`);
    }
    push(``);
  }
  const dupIndex = new Map();
  for (const t of summary.targets) {
    for (const d of t.duplicates) {
      if (!dupIndex.has(d.name)) dupIndex.set(d.name, []);
      dupIndex.get(d.name).push(t.name);
    }
  }
  const widespread = [...dupIndex.entries()].filter(([, ts]) => ts.length >= 3).sort((a, b) => b[1].length - a[1].length);
  if (widespread.length) {
    push(`### Duplicated in three or more targets`, ``, `| Crate | Targets |`, `| --- | --- |`);
    for (const [name, ts] of widespread) push(`| ${code(name)} | ${ts.length}: ${ts.join(", ")} |`);
    push(``);
  }

  // --- Heavy ---------------------------------------------------------------
  push(`## 3. Heaviest direct dependencies (top ${n} per target)`, ``);
  push(
    `${code("Exclusive")} is how many crates leave the graph if this dependency is dropped — the honest cost. ${code("Reaches")} is the transitive count, most of which something else pulls in anyway. ${code("Source")} is checked-out source size, not binary size.`,
    `A high exclusive count usually means default features pulling in a subtree you do not use: try ${code("default-features = false")} plus the two or three features needed.`,
    ``,
  );
  for (const t of summary.targets) {
    if (t.heavy.length === 0) continue;
    push(`### ${t.name}`, ``);
    push(`| Direct dependency | Version | Kind | Exclusive | Reaches | Source |`, `| --- | --- | --- | ---: | ---: | ---: |`);
    for (const h of t.heavy) {
      push(
        `| ${code(h.name)} | ${h.version} | ${h.kinds.join(", ")} | ${h.exclusive_count} | ${h.transitive_count} | ${mib(h.source_bytes)} |`,
      );
    }
    push(``);
  }

  // --- Drift ---------------------------------------------------------------
  push(`## 4. Version drift across repositories`, ``);
  push(
    `Crates that two or more targets depend on *directly* but resolve to different versions. When the root workspace ${code("[patch]")}-es a submodule in, both versions end up in the root build (section 2), so aligning the submodule's requirement with the root's removes a duplicate for free.`,
    ``,
  );
  if (summary.drift.length === 0) {
    push(`_None._`);
  } else {
    push(`| Crate | Versions | Per target |`, `| --- | --- | --- |`);
    for (const d of summary.drift) {
      const per = d.targets.map((t) => `${t.target}: ${t.versions.join("/")}`).join("; ");
      push(`| ${code(d.name)} | ${d.versions.map(code).join(", ")} | ${per} |`);
    }
  }
  push(``);
  return lines.join("\n");
}
