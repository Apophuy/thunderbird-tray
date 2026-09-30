// SPDX-License-Identifier: GPL-3.0-only

import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import test from "node:test";

import {
  fullState,
  hello,
  helloAck,
  requestFullState,
} from "../dist/protocol.js";

const fixtureRoot = new URL("../../tests/fixtures/protocol/v1/", import.meta.url);

async function fixture(name) {
  return JSON.parse(await readFile(new URL(name, fixtureRoot), "utf8"));
}

const cases = [
  [
    "hello.json",
    hello({ extensionVersion: "0.1.0", thunderbirdVersion: "156.0.1" }),
  ],
  ["hello-ack.json", helloAck({ hostVersion: "0.1.0" })],
  [
    "full-state.json",
    fullState({
      totalUnread: 12,
      accounts: [
        { id: "account1", name: "Personal", unread: 7 },
        { id: "account2", name: "Work", unread: 5 },
      ],
    }),
  ],
  ["request-full-state.json", requestFullState()],
];

for (const [name, message] of cases) {
  test(`TypeScript serialization matches ${name}`, async () => {
    assert.deepEqual(JSON.parse(JSON.stringify(message)), await fixture(name));
  });
}
