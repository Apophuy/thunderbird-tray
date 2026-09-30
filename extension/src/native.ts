// SPDX-License-Identifier: GPL-3.0-only

import {
  decodeHostMessage,
  fullState,
  hello,
  type FullStatePayload,
  type HelloPayload,
  type ProtocolMessage,
} from "./protocol.js";

const INITIAL_RECONNECT_DELAY_MS = 250;
const MAX_RECONNECT_DELAY_MS = 30_000;

interface ListenerEvent<Arguments extends unknown[]> {
  addListener(listener: (...arguments_: Arguments) => void): void;
}

export interface NativePort {
  postMessage(message: ProtocolMessage): void;
  disconnect(): void;
  onMessage: ListenerEvent<[message: unknown]>;
  onDisconnect: ListenerEvent<[port: NativePort]>;
}

export interface NativeClientOptions {
  connect(): NativePort;
  collectState(): Promise<FullStatePayload>;
  disconnectError(): string | undefined;
  hello: HelloPayload;
  schedule(callback: () => void, delayMilliseconds: number): unknown;
  logger: {
    info(message: string, details?: unknown): void;
    warn(message: string, error?: unknown): void;
    error(message: string, error?: unknown): void;
  };
}

export function reconnectDelay(attempt: number): number {
  const exponent = Math.min(Math.max(0, attempt), 30);
  return Math.min(
    INITIAL_RECONNECT_DELAY_MS * 2 ** exponent,
    MAX_RECONNECT_DELAY_MS,
  );
}

export class NativeClient {
  readonly #options: NativeClientOptions;
  #started = false;
  #port: NativePort | undefined;
  #handshakeComplete = false;
  #reconnectAttempt = 0;
  #stateRequested = false;
  #statePushRunning = false;

  constructor(options: NativeClientOptions) {
    this.#options = options;
  }

  start(): void {
    if (this.#started) {
      return;
    }
    this.#started = true;
    this.#connect();
  }

  requestStatePush(): void {
    this.#stateRequested = true;
    void this.#drainStatePushes();
  }

  #connect(): void {
    let port: NativePort;
    try {
      port = this.#options.connect();
    } catch (error) {
      this.#options.logger.warn("native host connection failed", error);
      this.#scheduleReconnect();
      return;
    }

    this.#port = port;
    this.#handshakeComplete = false;
    port.onMessage.addListener((message) => this.#handleMessage(port, message));
    port.onDisconnect.addListener(() => this.#handleDisconnect(port));

    try {
      port.postMessage(hello(this.#options.hello));
    } catch (error) {
      this.#options.logger.warn("could not send hello to native host", error);
      this.#handleDisconnect(port);
    }
  }

  #handleMessage(port: NativePort, value: unknown): void {
    if (this.#port !== port) {
      return;
    }

    let message;
    try {
      message = decodeHostMessage(value);
    } catch (error) {
      this.#options.logger.warn("invalid message from native host", error);
      return;
    }

    if (message === undefined) {
      this.#options.logger.warn("unknown message type from native host");
      return;
    }

    if (message.type === "helloAck") {
      this.#handshakeComplete = true;
      this.#reconnectAttempt = 0;
      this.#options.logger.info("native messaging handshake completed");
      this.requestStatePush();
    } else if (message.type === "requestFullState") {
      this.requestStatePush();
    }
  }

  #handleDisconnect(port: NativePort): void {
    if (this.#port !== port) {
      return;
    }
    this.#port = undefined;
    this.#handshakeComplete = false;
    this.#stateRequested = false;
    const error = this.#options.disconnectError();
    if (error !== undefined) {
      this.#options.logger.warn("native host disconnected", error);
    }
    this.#scheduleReconnect();
  }

  #scheduleReconnect(): void {
    const delay = reconnectDelay(this.#reconnectAttempt);
    this.#reconnectAttempt += 1;
    this.#options.schedule(() => this.#connect(), delay);
  }

  async #drainStatePushes(): Promise<void> {
    if (
      this.#statePushRunning ||
      !this.#stateRequested ||
      !this.#handshakeComplete ||
      this.#port === undefined
    ) {
      return;
    }

    this.#statePushRunning = true;
    try {
      while (
        this.#stateRequested &&
        this.#handshakeComplete &&
        this.#port !== undefined
      ) {
        this.#stateRequested = false;
        const port: NativePort = this.#port;
        const state = await this.#options.collectState();
        if (this.#port === port && this.#handshakeComplete) {
          port.postMessage(fullState(state));
          this.#options.logger.info("sent Thunderbird Inbox unread state", {
            totalUnread: state.totalUnread,
            accountCount: state.accounts.length,
          });
        }
      }
    } catch (error) {
      this.#options.logger.error("could not send Thunderbird unread state", error);
    } finally {
      this.#statePushRunning = false;
      if (this.#stateRequested) {
        void this.#drainStatePushes();
      }
    }
  }
}
