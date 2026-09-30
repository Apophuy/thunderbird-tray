// SPDX-License-Identifier: GPL-3.0-only

import { copyFile, mkdir, rm } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const extensionRoot = fileURLToPath(new URL("..", import.meta.url));
const outputDirectory = fileURLToPath(new URL("../dist", import.meta.url));
const compiler = fileURLToPath(
  new URL("../node_modules/typescript/bin/tsc", import.meta.url),
);

await rm(outputDirectory, { recursive: true, force: true });

const result = spawnSync(
  process.execPath,
  [compiler, "--project", "tsconfig.json"],
  { cwd: extensionRoot, stdio: "inherit" },
);

if (result.error) {
  throw result.error;
}

if (result.status !== 0) {
  process.exitCode = result.status ?? 1;
} else {
  await mkdir(outputDirectory, { recursive: true });
  await copyFile(
    fileURLToPath(new URL("../manifest.json", import.meta.url)),
    fileURLToPath(new URL("../dist/manifest.json", import.meta.url)),
  );
}
