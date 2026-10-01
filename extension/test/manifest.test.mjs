// SPDX-License-Identifier: GPL-3.0-only

import assert from "node:assert/strict";
import { readFile, stat } from "node:fs/promises";
import test from "node:test";

const manifestUrl = new URL("../dist/manifest.json", import.meta.url);
const identifiersUrl = new URL("../../identifiers.json", import.meta.url);

test("the extension skeleton targets Thunderbird 156 MV3", async () => {
  const manifest = JSON.parse(await readFile(manifestUrl, "utf8"));
  const identifiers = JSON.parse(await readFile(identifiersUrl, "utf8"));

  assert.equal(manifest.manifest_version, 3);
  assert.equal(manifest.browser_specific_settings.gecko.strict_min_version, "156.0");
  assert.deepEqual(manifest.permissions, ["accountsRead", "nativeMessaging"]);
  assert.deepEqual(manifest.background, {
    scripts: ["background.js"],
    type: "module",
  });
  assert.equal(manifest.browser_specific_settings.gecko.id, identifiers.extensionId);
  assert.deepEqual(manifest.icons, {
    "16": "icons/icon-16.png",
    "32": "icons/icon-32.png",
    "48": "icons/icon-48.png",
    "64": "icons/icon-64.png",
    "96": "icons/icon-96.png",
    "128": "icons/icon-128.png",
  });
  for (const path of Object.values(manifest.icons)) {
    assert.ok((await stat(new URL(`../dist/${path}`, import.meta.url))).size > 0);
  }
  assert.deepEqual(
    await readFile(new URL("../dist/icons/icon-128.png", import.meta.url)),
    await readFile(
      new URL(
        "../../assets/hicolor/128x128/apps/io.github.apophuy.thunderbird-tray.png",
        import.meta.url,
      ),
    ),
  );
});
