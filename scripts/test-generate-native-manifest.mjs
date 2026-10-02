// SPDX-License-Identifier: GPL-3.0-only

import assert from "node:assert/strict";
import { chmod, mkdtemp, readFile, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

import { generateNativeManifest } from "./generate-native-manifest.mjs";

test("native manifest uses centralized identifiers and an absolute binary path", async () => {
  const directory = await mkdtemp(path.join(tmpdir(), "thunderbird-tray-manifest-"));
  const binary = path.join(directory, "thunderbird-tray");
  const output = path.join(directory, "host.json");
  await writeFile(binary, "test");
  await chmod(binary, 0o700);

  await generateNativeManifest({ binary, output });
  const manifest = JSON.parse(await readFile(output, "utf8"));

  assert.equal(manifest.name, "io.github.apophuy.thunderbird_tray");
  assert.equal(manifest.path, binary);
  assert.equal(manifest.type, "stdio");
  assert.deepEqual(manifest.allowed_extensions, [
    "{6fd82ebe-fdb5-40f5-b272-04585e8ba661}",
  ]);
  assert.equal((await stat(output)).mode & 0o777, 0o600);
});
