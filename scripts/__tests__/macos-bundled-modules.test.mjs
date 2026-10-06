// The macOS bundled-module signer and its check, end to end on a staged
// marker-layout bundle. Ad-hoc signing (`-`) needs no certificate, so this runs
// on any Mac. An ad-hoc signature has no Developer ID and no secure timestamp,
// so `check` reports exactly those two; every other problem it can report is
// asserted absent, or present after tampering, by name.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "..", "..");
const SCRIPT = join(ROOT, "scripts/release/macos-bundled-modules.sh");
const ENTITLEMENTS = join(
  ROOT,
  "crates/openhuman-app/entitlements.sidecar.plist",
);
const sha = (path) =>
  createHash("sha256").update(readFileSync(path)).digest("hex");
const run = (...args) =>
  spawnSync("bash", [SCRIPT, ...args], { encoding: "utf8" });

/** The first architecture of a universal (fat) Mach-O, or the file if thin. */
function firstSlice(bytes) {
  if (bytes.readUInt32BE(0) !== 0xcafebabe) return bytes; // fat_header magic
  // fat_arch[0]: cputype, cpusubtype, offset, size, align (big-endian u32s).
  const offset = bytes.readUInt32BE(8 + 8);
  const size = bytes.readUInt32BE(8 + 12);
  return bytes.subarray(offset, offset + size);
}

/** An app whose bundle holds one module in the macOS layout: library, allowlist, marker. */
function stagedApp() {
  const app = join(
    mkdtempSync(join(tmpdir(), "openhuman-sign-")),
    "OpenHuman.app",
  );
  const modules = join(app, "Contents/Resources/bundled-modules");
  const dir = join(modules, "demo/1.0.0/macos-15-arm64");
  mkdirSync(dir, { recursive: true });
  const library = join(dir, "libdemo.dylib");
  // Any thin Mach-O will do. Thinning matters: in a universal binary the bytes
  // between slices are padding no signature hashes. The first slice is cut out
  // here rather than with `lipo`, which needs the Xcode command-line tools.
  writeFileSync(library, firstSlice(readFileSync("/usr/bin/true")));
  writeFileSync(
    join(dir, "modules.toml"),
    `"libdemo.dylib" = "${sha(library)}"\n`,
  );
  writeFileSync(
    join(dir, "demo-1.0.0-macos-15-arm64.tar.gz.sha256"),
    `${"a".repeat(64)}\n`,
  );
  return { app, modules, dir, library };
}

test(
  "signing re-pins modules.toml, and check verifies both",
  {
    skip: process.platform !== "darwin",
  },
  () => {
    const { app, modules, dir, library } = stagedApp();
    const unsigned = sha(library);
    const marker = readFileSync(
      join(dir, "demo-1.0.0-macos-15-arm64.tar.gz.sha256"),
      "utf8",
    );

    const signed = run("sign", app, ENTITLEMENTS, "-");
    assert.equal(signed.status, 0, signed.stderr);
    assert.notEqual(sha(library), unsigned, "codesign rewrites the library");
    assert.equal(
      readFileSync(join(dir, "modules.toml"), "utf8"),
      `"libdemo.dylib" = "${sha(library)}"\n`,
      "the allowlist names the signed file",
    );
    assert.equal(
      readFileSync(
        join(dir, "demo-1.0.0-macos-15-arm64.tar.gz.sha256"),
        "utf8",
      ),
      marker,
    );

    const clean = run("check", modules);
    assert.equal(clean.status, 1, "ad-hoc is not Developer ID");
    assert.equal(
      clean.stdout,
      "[sign-check] UNSIGNED for notarization: no-developer-id no-timestamp: demo/1.0.0/macos-15-arm64/libdemo.dylib\n",
    );

    // One changed byte in the second page. In a thin Mach-O every page before
    // the signature is hashed; the fixture is asserted rather than assumed, so a
    // byte that missed the hashed region fails here, not as a check bug.
    const bytes = readFileSync(library);
    const offset = 4096 + 16;
    assert.ok(
      bytes.length > 3 * 4096,
      "fixture is large enough to have a hashed second page",
    );
    assert.equal(
      spawnSync("codesign", ["--verify", "--strict", library]).status,
      0,
    );
    bytes[offset] ^= 0xff;
    writeFileSync(library, bytes);
    assert.match(
      spawnSync("file", ["-b", library], { encoding: "utf8" }).stdout,
      /^Mach-O/,
    );
    assert.notEqual(
      spawnSync("codesign", ["--verify", "--strict", library]).status,
      0,
      "the tampered byte is inside the hashed code",
    );
    const tampered = run("check", modules);
    assert.equal(tampered.status, 1);
    assert.match(tampered.stdout, /invalid-signature/);
    assert.match(tampered.stdout, /modules\.toml does not match its file/);
  },
);

test(
  "signing refuses a bundle that still ships a module archive",
  {
    skip: process.platform !== "darwin",
  },
  () => {
    const { app, dir } = stagedApp();
    writeFileSync(
      join(dir, "demo-1.0.0-macos-15-arm64.tar.gz"),
      "pinned bytes",
    );
    const result = run("sign", app, ENTITLEMENTS, "-");
    assert.equal(result.status, 1);
    assert.match(result.stderr, /module archives in the macOS bundle/);
  },
);

test("check fails when the bundle has no modules directory", () => {
  const result = run("check", join(tmpdir(), "openhuman-no-such-bundle"));
  assert.equal(result.status, 1);
  assert.match(result.stderr, /no bundled modules/);
});
