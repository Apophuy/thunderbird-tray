// SPDX-License-Identifier: GPL-3.0-only

import { mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const extensionRoot = fileURLToPath(new URL("..", import.meta.url));
const outputDirectory = fileURLToPath(new URL("../dist", import.meta.url));
const generatedDirectory = fileURLToPath(
  new URL("../src/generated", import.meta.url),
);
const compiler = fileURLToPath(
  new URL("../node_modules/typescript/bin/tsc", import.meta.url),
);
const identifiersPath = fileURLToPath(
  new URL("../../identifiers.json", import.meta.url),
);
const manifestPath = fileURLToPath(new URL("../manifest.json", import.meta.url));

await rm(outputDirectory, { recursive: true, force: true });
await rm(generatedDirectory, { recursive: true, force: true });
await mkdir(generatedDirectory, { recursive: true });

const identifiers = JSON.parse(await readFile(identifiersPath, "utf8"));
for (const key of ["nativeHostName", "extensionId"]) {
  if (typeof identifiers[key] !== "string" || identifiers[key].length === 0) {
    throw new Error(`identifiers.json is missing ${key}`);
  }
}

await writeFile(
  fileURLToPath(new URL("../src/generated/identifiers.ts", import.meta.url)),
  `// SPDX-License-Identifier: GPL-3.0-only\n\nexport const NATIVE_HOST_NAME = ${JSON.stringify(identifiers.nativeHostName)} as const;\n`,
);

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
  const manifest = JSON.parse(await readFile(manifestPath, "utf8"));
  manifest.browser_specific_settings.gecko.id = identifiers.extensionId;
  await writeFile(
    fileURLToPath(new URL("../dist/manifest.json", import.meta.url)),
    `${JSON.stringify(manifest, null, 2)}\n`,
  );
}
