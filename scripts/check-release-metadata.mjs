// SPDX-License-Identifier: GPL-3.0-only

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));
const expectedVersion = process.argv[2];
if (expectedVersion === undefined || expectedVersion.length === 0) {
  throw new Error("usage: node scripts/check-release-metadata.mjs <version>");
}

const [manifest, packageJson, identifiers] = await Promise.all(
  ["extension/manifest.json", "extension/package.json", "identifiers.json"].map(
    async (name) =>
      JSON.parse(await readFile(path.join(repositoryRoot, name), "utf8")),
  ),
);

assert.equal(manifest.version, expectedVersion, "extension manifest version");
assert.equal(packageJson.version, expectedVersion, "extension package version");
assert.equal(packageJson.license, "GPL-3.0-only", "extension license");
assert.equal(manifest.manifest_version, 3, "extension manifest version family");
assert.equal(manifest.browser_specific_settings.gecko.strict_min_version, "156.0");
for (const key of ["applicationId", "nativeHostName", "extensionId"]) {
  assert.equal(typeof identifiers[key], "string", `${key} type`);
  assert.notEqual(identifiers[key].length, 0, `${key} value`);
}
