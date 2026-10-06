import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import test from "node:test";
import { fileURLToPath } from "node:url";

const repoRoot = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  "..",
  "..",
);

import { AREA_ENV, buildPlan } from "../ci/self-hosted/lanes-plan.mjs";
const workflow = fs.readFileSync(
  path.join(repoRoot, "scripts", "ci", "self-hosted", "lanes-plan.mjs"),
  "utf8",
);
const plan = buildPlan({
  profile: "ex63",
  areas: Object.fromEntries(Object.keys(AREA_ENV).map((key) => [key, true])),
  env: { CI_SCRATCH_DIR: "/scratch" },
});
const commands = plan.lanes.flatMap((lane) => lane.checks.map((check) => check.run)).join("\n");
const rustCoverage = fs.readFileSync(
  path.join(repoRoot, "scripts", "ci", "rust-coverage.sh"),
  "utf8",
);

test("CI lanes run the complete frontend suite", () => {
  assert.match(commands, /pnpm --filter openhuman-app test:coverage/);
  assert.doesNotMatch(commands, /vitest related|CHANGED_FILES|frontend-src/);
  assert.match(workflow, /test:coverage/);
});

test("CI lanes run the complete Rust suite", () => {
  assert.match(commands, /bash scripts\/ci\/rust-coverage\.sh/);
  assert.doesNotMatch(commands, /rust-core-src|rust-core-full/);
  assert.doesNotMatch(rustCoverage, /CHANGED_FILES|MAX_CHANGED_FILES/);

  for (const crate of [
    "openhuman",
    "openhuman-embed",
    "openhuman-rpc",
    "openhuman-tinyhumans",
    "openhuman-tui",
  ]) {
    assert.match(
      rustCoverage,
      new RegExp(`-p ${crate}(?: |\\n)`),
      `${crate} must remain in the complete Rust suite`,
    );
  }
});
