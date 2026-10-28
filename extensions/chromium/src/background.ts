import { NativeBridge, type DecisionKind } from "./native";

export interface CookieEvent {
  id: string;
  ts_ms: number;
  action: "Set" | "Send" | "Js";
  party: "First" | "Third";
  scheme: string;
  host: string;
  name: string;
  value_redacted: boolean;
  attributes: {
    secure: boolean;
    http_only: boolean;
    same_site: string | null;
    max_age: number | null;
    domain: string | null;
    path: string | null;
  };
}

export function toCookieEvent(change: chrome.cookies.CookieChangeInfo): CookieEvent | null {
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
      path: cookie.path || null,
    },
  };
}

export function requestCookieEvents(details: {
  url: string;
  initiator?: string;
  requestHeaders?: Array<{ name: string; value?: string }>;
}): CookieEvent[] {
  const url = new URL(details.url);
  const initiatorHost = hostnameFromOrigin(details.initiator);
  const party = isSameSite(initiatorHost, url.hostname) ? "First" : "Third";
  const cookieHeader = details.requestHeaders?.find(
    (header) => header.name.toLowerCase() === "cookie",
  )?.value as string | undefined;

  if (!cookieHeader) return [];

  return cookieHeader.split(";").flatMap((part) => {
    const separator = part.indexOf("=");
    const name = (separator === -1 ? part : part.slice(0, separator)).trim();
    if (!name) return [];
    return [{
      id: crypto.randomUUID(),
      ts_ms: Date.now(),
      action: "Send" as const,
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
        path: null,
      },
    }];
  });
}

function hostnameFromOrigin(origin: string | undefined): string | null {
  if (!origin) return null;
  try {
    return new URL(origin).hostname;
  } catch {
    return null;
  }
}

export function isSameSite(initiatorHost: string | null, requestHost: string): boolean {
  if (!initiatorHost) return true;
  return approximateRegistrableDomain(initiatorHost)
    === approximateRegistrableDomain(requestHost);
}

function approximateRegistrableDomain(host: string): string {
  const labels = host.toLowerCase().replace(/\.$/, "").split(".");
  return labels.length >= 2 ? labels.slice(-2).join(".") : labels[0];
}

export function offlineDecision(mode: string): DecisionKind {
  return mode === "observe" ? "Allow" : "Deny";
}

export async function enforceDecision(
  event: CookieEvent,
  kind: DecisionKind,
  removeCookie: (details: chrome.cookies.CookieDetails) => Promise<unknown> = (details) =>
    chrome.cookies.remove(details),
): Promise<void> {
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
      const kind =
        response?.type === "decision"
          ? response.decision.kind
          : offlineDecision(bridge.mode);
      await enforceDecision(event, kind);
    })();
  });

  chrome.webRequest.onBeforeSendHeaders.addListener(
    (details) => {
      for (const event of requestCookieEvents(details)) {
        void (async () => {
          const response = await bridge.sendAsync({ type: "cookie_event", event });
          const kind =
            response?.type === "decision"
              ? response.decision.kind
              : offlineDecision(bridge.mode);
          await enforceDecision(event, kind);
        })();
      }
    },
    { urls: ["<all_urls>"] },
    ["requestHeaders", "extraHeaders"],
  );
}
