import assert from "node:assert/strict";
import { test } from "node:test";
import { checkManifest } from "../ci/check-security-module-dependencies.mjs";

test("allows the security bus and the documented core crypto bridge", () => {
  assert.deepEqual(
    checkManifest(
      '[dependencies]\ntinysecurity-bus = "0.17"\ntinysecurity-crypto = "0.17"',
      "crates/openhuman-core/Cargo.toml",
    ),
    [],
  );
});

test("rejects every other TinySecurity crate from core and host manifests", () => {
  for (const filename of ["crates/openhuman-core/Cargo.toml", "crates/openhuman-rpc/Cargo.toml"]) {
    assert.equal(
      checkManifest('[dependencies]\ntinysecurity-crypto = "0.17"', filename).length,
      1,
      filename,
    );
  }
});

test("rejects the engine crate from target-specific and patch tables", () => {
  assert.equal(
    checkManifest(
      '[target."cfg(unix)".dependencies]\ncrypto = { package = "tinysecurity-crypto", version = "0.17" }\n[patch.crates-io]\ntinysecurity = "0.17"',
      "crates/openhuman-app/Cargo.toml",
    ).length,
    2,
  );
});
