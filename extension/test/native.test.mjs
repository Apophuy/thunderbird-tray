// SPDX-License-Identifier: GPL-3.0-only

import assert from "node:assert/strict";
import test from "node:test";

import { NativeClient, reconnectDelay } from "../dist/native.js";
import { helloAck, requestFullState } from "../dist/protocol.js";

class FakeEvent {
  listeners = [];

  addListener(listener) {
    this.listeners.push(listener);
  }

  emit(...arguments_) {
    for (const listener of this.listeners) {
      listener(...arguments_);
    }
  }
}

class FakePort {
  messages = [];
  onMessage = new FakeEvent();
  onDisconnect = new FakeEvent();

  postMessage(message) {
    this.messages.push(message);
  }

  disconnect() {
    this.onDisconnect.emit(this);
  }
}

async function settle() {
  await new Promise((resolve) => setImmediate(resolve));
}

test("fake host completes the extension handshake and receives initial state", async () => {
  const port = new FakePort();
  const information = [];
  const client = new NativeClient({
    connect: () => port,
    collectState: async () => ({
      totalUnread: 2,
      accounts: [{ id: "a", name: "A", unread: 2 }],
    }),
    disconnectError: () => undefined,
    hello: { extensionVersion: "0.1.0", thunderbirdVersion: "156.0.1" },
    schedule() {},
    logger: {
      info(message, details) { information.push({ message, details }); },
      warn() {},
      error() {},
    },
  });

  client.start();
  assert.deepEqual(port.messages, [
    {
      protocol: 1,
      type: "hello",
      payload: {
        extensionVersion: "0.1.0",
        thunderbirdVersion: "156.0.1",
      },
    },
  ]);

  port.onMessage.emit(helloAck({ hostVersion: "0.1.0" }));
  await settle();
  assert.deepEqual(port.messages[1], {
    protocol: 1,
    type: "fullState",
    payload: {
      totalUnread: 2,
      accounts: [{ id: "a", name: "A", unread: 2 }],
    },
  });
  assert.deepEqual(information, [
    { message: "native messaging handshake completed", details: undefined },
    {
      message: "sent Thunderbird Inbox unread state",
      details: { totalUnread: 2, accountCount: 1 },
    },
  ]);
});

test("requestFullState asks the connected extension for another snapshot", async () => {
  const port = new FakePort();
  let collections = 0;
  const client = new NativeClient({
    connect: () => port,
    collectState: async () => ({ totalUnread: ++collections, accounts: [] }),
    disconnectError: () => undefined,
    hello: { extensionVersion: "0.1.0", thunderbirdVersion: "156.0.1" },
    schedule() {},
    logger: { info() {}, warn() {}, error() {} },
  });
  client.start();
  port.onMessage.emit(helloAck({ hostVersion: "0.1.0" }));
  await settle();

  port.onMessage.emit(requestFullState());
  await settle();
  assert.equal(port.messages.at(-1).payload.totalUnread, 2);
});

test("disconnect schedules a bounded reconnect", () => {
  const firstPort = new FakePort();
  const secondPort = new FakePort();
  const ports = [firstPort, secondPort];
  const scheduled = [];
  const warnings = [];
  const client = new NativeClient({
    connect: () => ports.shift(),
    collectState: async () => ({ totalUnread: 0, accounts: [] }),
    disconnectError: () => "Native host has exited.",
    hello: { extensionVersion: "0.1.0", thunderbirdVersion: "156.0.1" },
    schedule: (callback, delay) => scheduled.push({ callback, delay }),
    logger: {
      info() {},
      warn(message, error) { warnings.push({ message, error }); },
      error() {},
    },
  });

  client.start();
  firstPort.disconnect();

  assert.equal(scheduled[0].delay, 250);
  assert.deepEqual(warnings, [
    { message: "native host disconnected", error: "Native host has exited." },
  ]);
  scheduled[0].callback();
  assert.equal(secondPort.messages[0].type, "hello");
  assert.equal(reconnectDelay(100), 30_000);
});

test("reconnect repeats the handshake and sends a fresh complete state", async () => {
  const firstPort = new FakePort();
  const secondPort = new FakePort();
  const ports = [firstPort, secondPort];
  const scheduled = [];
  let collections = 0;
  const client = new NativeClient({
    connect: () => ports.shift(),
    collectState: async () => ({
      totalUnread: ++collections,
      accounts: [],
    }),
    disconnectError: () => undefined,
    hello: { extensionVersion: "0.1.0", thunderbirdVersion: "156.0.1" },
    schedule: (callback, delay) => scheduled.push({ callback, delay }),
    logger: { info() {}, warn() {}, error() {} },
  });

  client.start();
  firstPort.onMessage.emit(helloAck({ hostVersion: "0.1.0" }));
  await settle();
  assert.equal(firstPort.messages.at(-1).payload.totalUnread, 1);

  firstPort.disconnect();
  assert.equal(scheduled[0].delay, 250);
  scheduled[0].callback();
  assert.equal(secondPort.messages[0].type, "hello");

  firstPort.onMessage.emit(requestFullState());
  await settle();
  assert.equal(firstPort.messages.length, 2);

  secondPort.onMessage.emit(helloAck({ hostVersion: "0.1.0" }));
  await settle();
  assert.equal(secondPort.messages.at(-1).type, "fullState");
  assert.equal(secondPort.messages.at(-1).payload.totalUnread, 2);
});
