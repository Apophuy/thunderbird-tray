// SPDX-License-Identifier: GPL-3.0-only

import assert from "node:assert/strict";
import test from "node:test";

import { collectInboxState } from "../dist/state.js";

test("collectInboxState aggregates only queried Inbox folders per account", async () => {
  const queryCalls = [];
  const api = {
    accounts: {
      async list(includeSubFolders) {
        assert.equal(includeSubFolders, false);
        return [
          { id: "account1", name: "Personal" },
          { id: "account2", name: "Work" },
          { id: "account3", name: "Empty" },
        ];
      },
    },
    folders: {
      async query(queryInfo) {
        queryCalls.push(queryInfo);
        return [
          { id: "inbox1", accountId: "account1", specialUse: ["inbox"] },
          { id: "inbox2", accountId: "account2", specialUse: ["inbox"] },
          { id: "inbox3", accountId: "account2", specialUse: ["inbox"] },
        ];
      },
      async getFolderInfo(folderId) {
        return {
          unreadMessageCount: { inbox1: 4, inbox2: 2, inbox3: 3 }[folderId],
        };
      },
    },
  };

  assert.deepEqual(await collectInboxState(api), {
    totalUnread: 9,
    accounts: [
      { id: "account1", name: "Personal", unread: 4 },
      { id: "account2", name: "Work", unread: 5 },
      { id: "account3", name: "Empty", unread: 0 },
    ],
  });
  assert.deepEqual(queryCalls, [{ specialUse: ["inbox"] }]);
});

test("collectInboxState treats missing folder counts as zero", async () => {
  const api = {
    accounts: { async list() { return [{ id: "a", name: "A" }]; } },
    folders: {
      async query() {
        return [{ id: "inbox", accountId: "a", specialUse: ["inbox"] }];
      },
      async getFolderInfo() { return {}; },
    },
  };

  assert.deepEqual(await collectInboxState(api), {
    totalUnread: 0,
    accounts: [{ id: "a", name: "A", unread: 0 }],
  });
});

test("collectInboxState rejects counts outside protocol u32 range", async () => {
  const api = {
    accounts: { async list() { return [{ id: "a", name: "A" }]; } },
    folders: {
      async query() {
        return [{ id: "inbox", accountId: "a", specialUse: ["inbox"] }];
      },
      async getFolderInfo() { return { unreadMessageCount: 0x1_0000_0000 }; },
    },
  };

  await assert.rejects(() => collectInboxState(api), /invalid Thunderbird unread count/);
});
