// SPDX-License-Identifier: GPL-3.0-only

import { NATIVE_HOST_NAME } from "./generated/identifiers.js";
import { NativeClient } from "./native.js";
import { collectInboxState } from "./state.js";

let client: NativeClient | undefined;

function requestStateIfInbox(folder: ThunderbirdFolder): void {
  if (folder.specialUse.includes("inbox")) {
    client?.requestStatePush();
  }
}

messenger.folders.onFolderInfoChanged.addListener((folder) => {
  requestStateIfInbox(folder);
});
messenger.folders.onCreated.addListener(requestStateIfInbox);
messenger.folders.onDeleted.addListener(requestStateIfInbox);
messenger.folders.onUpdated.addListener((originalFolder, updatedFolder) => {
  if (
    originalFolder.specialUse.includes("inbox") ||
    updatedFolder.specialUse.includes("inbox")
  ) {
    client?.requestStatePush();
  }
});
messenger.accounts.onCreated.addListener(() => client?.requestStatePush());
messenger.accounts.onDeleted.addListener(() => client?.requestStatePush());
messenger.accounts.onUpdated.addListener(() => client?.requestStatePush());

async function start(): Promise<void> {
  const browserInfo = await messenger.runtime.getBrowserInfo();
  client = new NativeClient({
    connect: () => messenger.runtime.connectNative(NATIVE_HOST_NAME),
    collectState: () => collectInboxState(messenger),
    disconnectError: () => messenger.runtime.lastError?.message,
    hello: {
      extensionVersion: messenger.runtime.getManifest().version,
      thunderbirdVersion: browserInfo.version,
    },
    schedule: (callback, delayMilliseconds) =>
      globalThis.setTimeout(callback, delayMilliseconds),
    logger: console,
  });
  client.start();
}

void start().catch((error: unknown) => {
  console.error("could not start thunderbird-tray extension", error);
});
