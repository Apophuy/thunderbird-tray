// SPDX-License-Identifier: GPL-3.0-only

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

const manifestUrl = new URL("../manifest.json", import.meta.url);

test("the extension skeleton targets Thunderbird 156 MV3", async () => {
  const manifest = JSON.parse(await readFile(manifestUrl, "utf8"));

  assert.equal(manifest.manifest_version, 3);
  assert.equal(manifest.browser_specific_settings.gecko.strict_min_version, "156.0");
  assert.deepEqual(manifest.permissions, []);
  assert.deepEqual(manifest.background, {
    scripts: ["background.js"],
    type: "module",
  });
  assert.equal(manifest.browser_specific_settings.gecko.id, undefined);
});
