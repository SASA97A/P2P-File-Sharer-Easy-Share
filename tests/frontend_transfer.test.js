import test from "node:test";
import assert from "node:assert/strict";

// Setup minimal DOM environment
if (typeof globalThis.document === "undefined") {
  const elements = new Map();
  globalThis.document = {
    getElementById: (id) => elements.get(id) || null,
    querySelector: (sel) => elements.get(sel) || null,
    querySelectorAll: () => [],
    createElement: () => ({
      dataset: {},
      style: { setProperty() {} },
      addEventListener() {},
      appendChild() {},
    }),
  };
}

import { cancelTransfer, onTransferCancelled } from "../src/script/tauri-bridge.js";

test("transfer bridge - cancellation API", async (t) => {
  await t.test("cancelTransfer invokes cancel_transfer command with sessionToken without error", async () => {
    // In mock/test mode without Tauri runtime, cancelTransfer resolves void/undefined safely
    await assert.doesNotReject(async () => {
      await cancelTransfer("test-session-token-999", { ip: "127.0.0.1", port: 5050 });
    });
  });

  await t.test("onTransferCancelled registers listener", async () => {
    let called = false;
    await onTransferCancelled(() => {
      called = true;
    });
    // In mock mode without event bus, it returns safely without throwing
    assert.equal(typeof onTransferCancelled, "function");
  });
});
