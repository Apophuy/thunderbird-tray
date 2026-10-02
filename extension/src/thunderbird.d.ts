// SPDX-License-Identifier: GPL-3.0-only

interface ThunderbirdEvent<Arguments extends unknown[]> {
  addListener(listener: (...arguments_: Arguments) => void): void;
}

interface ThunderbirdNativePort {
  postMessage(message: unknown): void;
  disconnect(): void;
  onMessage: ThunderbirdEvent<[message: unknown]>;
  onDisconnect: ThunderbirdEvent<[port: ThunderbirdNativePort]>;
}

interface ThunderbirdAccount {
  id: string;
  name: string;
}

interface ThunderbirdFolder {
  id: string;
  accountId?: string;
  specialUse: string[];
}

interface ThunderbirdFolderInfo {
  unreadMessageCount?: number;
}

declare const messenger: {
  runtime: {
    connectNative(application: string): ThunderbirdNativePort;
    getBrowserInfo(): Promise<{ version: string }>;
    getManifest(): { version: string };
    lastError?: { message?: string };
  };
  accounts: {
    list(includeSubFolders?: boolean): Promise<ThunderbirdAccount[]>;
    onCreated: ThunderbirdEvent<[accountId: string, account: ThunderbirdAccount]>;
    onDeleted: ThunderbirdEvent<[accountId: string]>;
    onUpdated: ThunderbirdEvent<[
      accountId: string,
      changedValues: Record<string, unknown>,
    ]>;
  };
  folders: {
    query(queryInfo?: {
      specialUse?: string[];
    }): Promise<ThunderbirdFolder[]>;
    getFolderInfo(folderId: string): Promise<ThunderbirdFolderInfo>;
    onCreated: ThunderbirdEvent<[folder: ThunderbirdFolder]>;
    onDeleted: ThunderbirdEvent<[folder: ThunderbirdFolder]>;
    onFolderInfoChanged: ThunderbirdEvent<[
      folder: ThunderbirdFolder,
      folderInfo: ThunderbirdFolderInfo,
    ]>;
    onUpdated: ThunderbirdEvent<[
      originalFolder: ThunderbirdFolder,
      updatedFolder: ThunderbirdFolder,
    ]>;
  };
};
