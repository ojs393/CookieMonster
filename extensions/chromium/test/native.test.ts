import { afterEach, describe, expect, it, vi } from "vitest";

import { NativeBridge } from "../src/native";

afterEach(() => {
  vi.useRealTimers();
  vi.unstubAllGlobals();
});

describe("NativeBridge reconnect throttling", () => {
  it("queues sends without reconnecting until the backoff expires", () => {
    vi.useFakeTimers();
    const disconnectListeners: Array<() => void> = [];
    const connectNative = vi.fn(() => ({
      onMessage: { addListener: vi.fn() },
      onDisconnect: {
        addListener: (listener: () => void) => disconnectListeners.push(listener),
      },
      postMessage: vi.fn(),
    }));
    vi.stubGlobal("chrome", {
      runtime: {
        connectNative,
        lastError: undefined,
      },
    });

    const bridge = new NativeBridge();
    bridge.connect();
    disconnectListeners[0]();

    bridge.send({ type: "cookie_event", event: { id: "queued-1" } });
    bridge.send({ type: "cookie_event", event: { id: "queued-2" } });

    expect(connectNative).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(1_000);
    expect(connectNative).toHaveBeenCalledTimes(2);
  });
});

describe("NativeBridge message handling", () => {
  it("resolves sendAsync with decision and caches status mode", async () => {
    vi.useFakeTimers();
    const messageListeners: Array<(msg: unknown) => void> = [];
    const port = {
      onMessage: {
        addListener: (listener: (msg: unknown) => void) => messageListeners.push(listener),
      },
      onDisconnect: { addListener: vi.fn() },
      postMessage: vi.fn(),
    };
    vi.stubGlobal("chrome", {
      runtime: {
        connectNative: vi.fn(() => port),
        lastError: undefined,
      },
    });

    const bridge = new NativeBridge();
    const pending = bridge.sendAsync({ type: "get_status" });
    expect(port.postMessage).toHaveBeenCalled();
    messageListeners[0]({
      type: "status",
      mode: "cookie_monster",
      browser_shield: true,
      system_shield: false,
    });
    await expect(pending).resolves.toMatchObject({ type: "status", mode: "cookie_monster" });
    expect(bridge.mode).toBe("cookie_monster");
  });

  it("resolves null when disconnected mid-flight", async () => {
    const disconnectListeners: Array<() => void> = [];
    const port = {
      onMessage: { addListener: vi.fn() },
      onDisconnect: {
        addListener: (listener: () => void) => disconnectListeners.push(listener),
      },
      postMessage: vi.fn(),
    };
    vi.stubGlobal("chrome", {
      runtime: {
        connectNative: vi.fn(() => port),
        lastError: undefined,
      },
    });

    const bridge = new NativeBridge();
    const pending = bridge.sendAsync({ type: "cookie_event", event: { id: "x" } });
    disconnectListeners[0]();
    await expect(pending).resolves.toBeNull();
  });
});
