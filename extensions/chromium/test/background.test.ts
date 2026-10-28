import { describe, expect, it, vi } from "vitest";

import {
  enforceDecision,
  isSameSite,
  offlineDecision,
  requestCookieEvents,
  toCookieEvent,
} from "../src/background";

describe("toCookieEvent", () => {
  it("maps a Chrome cookie change to the core CookieEvent JSON shape", () => {
    vi.spyOn(Date, "now").mockReturnValue(1_700_000_000_000);

    const event = toCookieEvent({
      removed: false,
      cause: "explicit",
      cookie: {
        domain: ".example.com",
        hostOnly: false,
        httpOnly: true,
        name: "session",
        path: "/account",
        sameSite: "lax",
        secure: true,
        session: false,
        storeId: "0",
        value: "must-not-leak",
        expirationDate: 1_800_000_000,
      },
    });

    expect(event).not.toBeNull();
    expect(event).toMatchObject({
      ts_ms: 1_700_000_000_000,
      action: "Set",
      party: "First",
      scheme: "https",
      host: "example.com",
      name: "session",
      value_redacted: true,
      attributes: {
        secure: true,
        http_only: true,
        same_site: "lax",
        max_age: null,
        domain: ".example.com",
        path: "/account",
      },
    });
    expect(event?.id).toEqual(expect.any(String));
    expect(JSON.stringify(event)).not.toContain("must-not-leak");
  });

  it("skips cookie removals instead of reporting them as Set", () => {
    const event = toCookieEvent({
      removed: true,
      cause: "explicit",
      cookie: {
        domain: ".example.com",
        hostOnly: false,
        httpOnly: true,
        name: "session",
        path: "/",
        sameSite: "lax",
        secure: true,
        session: true,
        storeId: "0",
        value: "",
      },
    });

    expect(event).toBeNull();
  });
});

describe("requestCookieEvents", () => {
  it("parses Cookie header into Send events without values", () => {
    const events = requestCookieEvents({
      url: "https://cdn.tracker.test/pixel",
      initiator: "https://www.example.com",
      requestHeaders: [{ name: "Cookie", value: "a=1; b=secret; =bad" }],
    });

    expect(events).toHaveLength(2);
    expect(events[0]).toMatchObject({
      action: "Send",
      party: "Third",
      host: "cdn.tracker.test",
      name: "a",
      value_redacted: true,
    });
    expect(events[1].name).toBe("b");
    expect(JSON.stringify(events)).not.toContain("secret");
  });

  it("returns empty when Cookie header missing", () => {
    expect(
      requestCookieEvents({
        url: "https://example.com/",
        requestHeaders: [],
      }),
    ).toEqual([]);
  });

  it("marks same-site initiator as First", () => {
    const events = requestCookieEvents({
      url: "https://cdn.example.com/x",
      initiator: "https://www.example.com",
      requestHeaders: [{ name: "cookie", value: "sid=1" }],
    });
    expect(events[0].party).toBe("First");
  });
});

describe("isSameSite", () => {
  it("treats sibling subdomains as first-party", () => {
    expect(isSameSite("www.example.com", "cdn.example.com")).toBe(true);
  });

  it("treats hosts on different registrable domains as third-party", () => {
    expect(isSameSite("www.example.com", "cdn.tracker.test")).toBe(false);
  });

  it("treats missing initiator as first-party", () => {
    expect(isSameSite(null, "example.com")).toBe(true);
  });
});

describe("offlineDecision", () => {
  it("fails open in observe and closed otherwise", () => {
    expect(offlineDecision("observe")).toBe("Allow");
    expect(offlineDecision("moderate")).toBe("Deny");
    expect(offlineDecision("cookie_monster")).toBe("Deny");
  });
});

describe("enforceDecision", () => {
  it("removes cookies on Deny for Set events", async () => {
    const removeCookie = vi.fn(async () => undefined);
    await enforceDecision(
      {
        id: "1",
        ts_ms: 1,
        action: "Set",
        party: "First",
        scheme: "https",
        host: "example.com",
        name: "_ga",
        value_redacted: true,
        attributes: {
          secure: true,
          http_only: false,
          same_site: null,
          max_age: null,
          domain: null,
          path: "/",
        },
      },
      "Deny",
      removeCookie,
    );
    expect(removeCookie).toHaveBeenCalledWith({
      url: "https://example.com/",
      name: "_ga",
    });
  });

  it("does nothing on Allow", async () => {
    const removeCookie = vi.fn(async () => undefined);
    await enforceDecision(
      {
        id: "1",
        ts_ms: 1,
        action: "Set",
        party: "First",
        scheme: "https",
        host: "example.com",
        name: "session",
        value_redacted: true,
        attributes: {
          secure: true,
          http_only: false,
          same_site: null,
          max_age: null,
          domain: null,
          path: "/",
        },
      },
      "Allow",
      removeCookie,
    );
    expect(removeCookie).not.toHaveBeenCalled();
  });
});
