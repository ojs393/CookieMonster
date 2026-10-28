// src/native.ts
var HOST_NAME = "com.cookiemonster.host";
var SAFARI_APP_ID = "com.cookiemonster.safari";
var BUFFER_LIMIT = 100;
var RECONNECT_DELAY_MS = 1e3;
function safariBuild() {
  return true;
}
function safariRuntime() {
  const g = globalThis;
  return g.browser ?? g.chrome ?? null;
}
var NativeBridge = class {
  port = null;
  queue = [];
  awaitingResponse = false;
  reconnectTimer = null;
  mode = "observe";
  connect() {
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
          const response = message;
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
  send(request) {
    void this.sendAsync(request);
  }
  sendAsync(request) {
    if (safariBuild()) {
      return this.sendSafari(request);
    }
    return new Promise((resolve) => {
      this.enqueue({ request, resolve });
      if (!this.port) this.connect();
      this.drain();
    });
  }
  async sendSafari(request) {
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
  enqueue(pending) {
    if (this.queue.length === BUFFER_LIMIT) {
      const dropAt = this.awaitingResponse ? 1 : 0;
      const dropped = this.queue.splice(dropAt, 1)[0];
      dropped?.resolve(null);
    }
    this.queue.push(pending);
  }
  drain() {
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
  scheduleReconnect() {
    if (this.reconnectTimer) return;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, RECONNECT_DELAY_MS);
  }
};

// src/background.ts
function toCookieEvent(change) {
  if (change.removed) return null;
  const { cookie } = change;
  return {
    id: crypto.randomUUID(),
    ts_ms: Date.now(),
    action: "Set",
    party: "First",
    scheme: cookie.secure ? "https" : "http",
    host: cookie.domain.replace(/^\./, ""),
    name: cookie.name,
    value_redacted: true,
    attributes: {
      secure: cookie.secure,
      http_only: cookie.httpOnly,
      same_site: cookie.sameSite === "unspecified" ? null : cookie.sameSite,
      max_age: null,
      domain: cookie.domain || null,
      path: cookie.path || null
    }
  };
}
function requestCookieEvents(details) {
  const url = new URL(details.url);
  const initiatorHost = hostnameFromOrigin(details.initiator);
  const party = isSameSite(initiatorHost, url.hostname) ? "First" : "Third";
  const cookieHeader = details.requestHeaders?.find(
    (header) => header.name.toLowerCase() === "cookie"
  )?.value;
  if (!cookieHeader) return [];
  return cookieHeader.split(";").flatMap((part) => {
    const separator = part.indexOf("=");
    const name = (separator === -1 ? part : part.slice(0, separator)).trim();
    if (!name) return [];
    return [{
      id: crypto.randomUUID(),
      ts_ms: Date.now(),
      action: "Send",
      party,
      scheme: url.protocol.replace(":", ""),
      host: url.hostname,
      name,
      value_redacted: true,
      attributes: {
        secure: url.protocol === "https:",
        http_only: false,
        same_site: null,
        max_age: null,
        domain: null,
        path: null
      }
    }];
  });
}
function hostnameFromOrigin(origin) {
  if (!origin) return null;
  try {
    return new URL(origin).hostname;
  } catch {
    return null;
  }
}
function isSameSite(initiatorHost, requestHost) {
  if (!initiatorHost) return true;
  return approximateRegistrableDomain(initiatorHost) === approximateRegistrableDomain(requestHost);
}
function approximateRegistrableDomain(host) {
  const labels = host.toLowerCase().replace(/\.$/, "").split(".");
  return labels.length >= 2 ? labels.slice(-2).join(".") : labels[0];
}
function offlineDecision(mode) {
  return mode === "observe" ? "Allow" : "Deny";
}
async function enforceDecision(event, kind, removeCookie = (details) => chrome.cookies.remove(details)) {
  if (kind !== "Deny" && kind !== "Strip") return;
  if (event.action !== "Set" && event.action !== "Send") return;
  const url = `${event.scheme}://${event.host}${event.attributes.path ?? "/"}`;
  try {
    await removeCookie({ url, name: event.name });
  } catch {
  }
}
if (typeof chrome !== "undefined" && chrome.cookies && chrome.webRequest) {
  const bridge = new NativeBridge();
  bridge.connect();
  chrome.cookies.onChanged.addListener((change) => {
    const event = toCookieEvent(change);
    if (!event) return;
    void (async () => {
      const response = await bridge.sendAsync({ type: "cookie_event", event });
      const kind = response?.type === "decision" ? response.decision.kind : offlineDecision(bridge.mode);
      await enforceDecision(event, kind);
    })();
  });
  chrome.webRequest.onBeforeSendHeaders.addListener(
    (details) => {
      for (const event of requestCookieEvents(details)) {
        void (async () => {
          const response = await bridge.sendAsync({ type: "cookie_event", event });
          const kind = response?.type === "decision" ? response.decision.kind : offlineDecision(bridge.mode);
          await enforceDecision(event, kind);
        })();
      }
    },
    { urls: ["<all_urls>"] },
    ["requestHeaders", "extraHeaders"]
  );
}
export {
  enforceDecision,
  isSameSite,
  offlineDecision,
  requestCookieEvents,
  toCookieEvent
};
