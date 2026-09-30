// SPDX-License-Identifier: GPL-3.0-only

export const PROTOCOL_VERSION = 1 as const;

export interface HelloPayload {
  extensionVersion: string;
  thunderbirdVersion: string;
}

export interface HelloMessage {
  protocol: typeof PROTOCOL_VERSION;
  type: "hello";
  payload: HelloPayload;
}

export interface HelloAckPayload {
  hostVersion: string;
}

export interface HelloAckMessage {
  protocol: typeof PROTOCOL_VERSION;
  type: "helloAck";
  payload: HelloAckPayload;
}

export interface AccountState {
  id: string;
  name: string;
  unread: number;
}

export interface FullStatePayload {
  totalUnread: number;
  accounts: AccountState[];
}

export interface FullStateMessage {
  protocol: typeof PROTOCOL_VERSION;
  type: "fullState";
  payload: FullStatePayload;
}

export interface RequestFullStateMessage {
  protocol: typeof PROTOCOL_VERSION;
  type: "requestFullState";
  payload: Record<string, never>;
}

export type ProtocolMessage =
  | HelloMessage
  | HelloAckMessage
  | FullStateMessage
  | RequestFullStateMessage;

export function hello(payload: HelloPayload): HelloMessage {
  return { protocol: PROTOCOL_VERSION, type: "hello", payload };
}

export function helloAck(payload: HelloAckPayload): HelloAckMessage {
  return { protocol: PROTOCOL_VERSION, type: "helloAck", payload };
}

export function fullState(payload: FullStatePayload): FullStateMessage {
  return { protocol: PROTOCOL_VERSION, type: "fullState", payload };
}

export function requestFullState(): RequestFullStateMessage {
  return {
    protocol: PROTOCOL_VERSION,
    type: "requestFullState",
    payload: {},
  };
}
