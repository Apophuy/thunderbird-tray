// SPDX-License-Identifier: GPL-3.0-only

import { access, chmod, mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { fileURLToPath } from "node:url";

const repositoryRoot = fileURLToPath(new URL("..", import.meta.url));

export async function generateNativeManifest({ binary, output }) {
  const binaryPath = path.resolve(binary);
  const outputPath = path.resolve(output);
  await access(binaryPath);

  const identifiers = JSON.parse(
    await readFile(path.join(repositoryRoot, "identifiers.json"), "utf8"),
  );
  const template = JSON.parse(
    await readFile(
      path.join(
        repositoryRoot,
        "packaging/native-messaging/manifest.template.json",
      ),
      "utf8",
    ),
  );

  template.name = identifiers.nativeHostName;
  template.path = binaryPath;
  template.allowed_extensions = [identifiers.extensionId];

  await mkdir(path.dirname(outputPath), { recursive: true });
  await writeFile(outputPath, `${JSON.stringify(template, null, 2)}\n`, {
    mode: 0o600,
  });
  await chmod(outputPath, 0o600);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [binary, output] = process.argv.slice(2);
  if (binary === undefined || output === undefined) {
    console.error(
      "usage: node scripts/generate-native-manifest.mjs <binary> <output>",
    );
    process.exitCode = 2;
  } else {
    await generateNativeManifest({ binary, output });
  }
}
