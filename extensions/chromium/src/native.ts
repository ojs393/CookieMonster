const HOST_NAME = "com.cookiemonster.host";
/** Safari containing-app bundle id used with sendNativeMessage. */
const SAFARI_APP_ID = "com.cookiemonster.safari";
const BUFFER_LIMIT = 100;
const RECONNECT_DELAY_MS = 1_000;

declare const COOKIE_MONSTER_SAFARI: boolean | undefined;

function safariBuild(): boolean {
  return typeof COOKIE_MONSTER_SAFARI !== "undefined" && COOKIE_MONSTER_SAFARI === true;
}

type RuntimeLike = {
  sendNativeMessage?: (
    application: string,
    message: unknown,
  ) => Promise<NativeResponse>;
};

function safariRuntime(): RuntimeLike | null {
  const g = globalThis as { browser?: RuntimeLike; chrome?: RuntimeLike };
  return g.browser ?? g.chrome ?? null;
}

export type DecisionKind = "Allow" | "Deny" | "Strip";

export interface NativeRequest {
  type: "cookie_event" | "get_status" | "set_mode";
  event?: unknown;
  mode?: string;
}

export interface DecisionResponse {
  type: "decision";
  decision: { kind: DecisionKind; reason: string };
}

export interface StatusResponse {
  type: "status";
  mode: "observe" | "moderate" | "cookie_monster";
  browser_shield: boolean;
  system_shield: boolean;
}

export type NativeResponse = DecisionResponse | StatusResponse | { type: "error"; message: string };

type Pending = {
  request: NativeRequest;
  resolve: (value: NativeResponse | null) => void;
};

export class NativeBridge {
  private port: chrome.runtime.Port | null = null;
  private readonly queue: Pending[] = [];
  private awaitingResponse = false;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  mode: "observe" | "moderate" | "cookie_monster" = "observe";

  connect(): void {
    if (safariBuild()) {
      void this.sendAsync({ type: "get_status" });
      return;
    }
    if (this.port || this.reconnectTimer) return;

    try {
      const port = chrome.runtime.connectNative(HOST_NAME);
      this.port = port;
      port.onMessage.addListener((message) => {
        if (this.port !== port || !this.awaitingResponse) return;
        const pending = this.queue.shift();
        this.awaitingResponse = false;
        if (pending) {
          const response = message as NativeResponse;
          if (response?.type === "status" && response.mode) {
            this.mode = response.mode;
          }
          pending.resolve(response);
        }
        this.drain();
      });
      port.onDisconnect.addListener(() => {
        void chrome.runtime.lastError;
        if (this.port === port) {
          this.port = null;
          this.awaitingResponse = false;
          const pending = this.queue.shift();
          pending?.resolve(null);
        }
        this.scheduleReconnect();
      });
      this.drain();
      void this.sendAsync({ type: "get_status" });
    } catch {
      this.port = null;
      this.scheduleReconnect();
    }
  }

  send(request: NativeRequest): void {
    void this.sendAsync(request);
  }

  sendAsync(request: NativeRequest): Promise<NativeResponse | null> {
    if (safariBuild()) {
      return this.sendSafari(request);
    }
    return new Promise((resolve) => {
      this.enqueue({ request, resolve });
      if (!this.port) this.connect();
      this.drain();
    });
  }

  private async sendSafari(request: NativeRequest): Promise<NativeResponse | null> {
    const runtime = safariRuntime();
    if (!runtime?.sendNativeMessage) return null;
    try {
      const response = await runtime.sendNativeMessage(SAFARI_APP_ID, request);
      if (response?.type === "status" && response.mode) {
        this.mode = response.mode;
      }
      return response ?? null;
    } catch {
      return null;
    }
  }

  private enqueue(pending: Pending): void {
    if (this.queue.length === BUFFER_LIMIT) {
      const dropAt = this.awaitingResponse ? 1 : 0;
      const dropped = this.queue.splice(dropAt, 1)[0];
      dropped?.resolve(null);
    }
    this.queue.push(pending);
  }

  private drain(): void {
    if (!this.port || this.awaitingResponse || this.queue.length === 0) return;

    try {
      this.port.postMessage(this.queue[0].request);
      this.awaitingResponse = true;
    } catch {
      this.port = null;
      this.awaitingResponse = false;
      const pending = this.queue.shift();
      pending?.resolve(null);
      this.scheduleReconnect();
    }
  }

  private scheduleReconnect(): void {
    if (this.reconnectTimer) return;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, RECONNECT_DELAY_MS);
  }
}
