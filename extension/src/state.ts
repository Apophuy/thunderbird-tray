// SPDX-License-Identifier: GPL-3.0-only

import type { FullStatePayload } from "./protocol.js";

const MAX_UNREAD_COUNT = 0xffff_ffff;

export interface AccountSummary {
  id: string;
  name: string;
}

export interface FolderSummary {
  id: string;
  accountId?: string;
  specialUse: string[];
}

export interface FolderInfo {
  unreadMessageCount?: number;
}

export interface ThunderbirdStateApi {
  accounts: {
    list(includeSubFolders?: boolean): Promise<AccountSummary[]>;
  };
  folders: {
    query(queryInfo: { specialUse: string[] }): Promise<FolderSummary[]>;
    getFolderInfo(folderId: string): Promise<FolderInfo>;
  };
}

export async function collectInboxState(
  api: ThunderbirdStateApi,
): Promise<FullStatePayload> {
  const [accounts, inboxes] = await Promise.all([
    api.accounts.list(false),
    api.folders.query({ specialUse: ["inbox"] }),
  ]);
  const unreadByAccount = new Map(accounts.map((account) => [account.id, 0]));

  const folderCounts = await Promise.all(
    inboxes.map(async (folder) => ({
      accountId: folder.accountId,
      unread: normalizeUnreadCount(
        (await api.folders.getFolderInfo(folder.id)).unreadMessageCount,
      ),
    })),
  );

  for (const { accountId, unread } of folderCounts) {
    if (accountId === undefined || !unreadByAccount.has(accountId)) {
      continue;
    }
    unreadByAccount.set(
      accountId,
      addUnread(unreadByAccount.get(accountId) ?? 0, unread),
    );
  }

  const accountStates = accounts.map((account) => ({
    id: account.id,
    name: account.name,
    unread: unreadByAccount.get(account.id) ?? 0,
  }));
  const totalUnread = accountStates.reduce(
    (total, account) => addUnread(total, account.unread),
    0,
  );

  return { totalUnread, accounts: accountStates };
}

function normalizeUnreadCount(value: number | undefined): number {
  if (value === undefined) {
    return 0;
  }
  if (!Number.isSafeInteger(value) || value < 0 || value > MAX_UNREAD_COUNT) {
    throw new Error(`invalid Thunderbird unread count: ${String(value)}`);
  }
  return value;
}

function addUnread(left: number, right: number): number {
  const total = left + right;
  if (!Number.isSafeInteger(total) || total > MAX_UNREAD_COUNT) {
    throw new Error("aggregate unread count exceeds protocol limit");
  }
  return total;
}
