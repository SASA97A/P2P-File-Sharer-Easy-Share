import test from "node:test";
import assert from "node:assert/strict";

import { formatSpeed, formatEta } from "../src/script/ui.js";
import {
  pauseTransfer,
  resumeTransfer,
  onTransferPaused,
  onTransferResumed,
} from "../src/script/tauri-bridge.js";

test("formatSpeed formats bytes per second into readable strings", () => {
  assert.equal(formatSpeed(0), "0 B/s");
  assert.equal(formatSpeed(null), "0 B/s");
  assert.equal(formatSpeed(undefined), "0 B/s");
  assert.equal(formatSpeed(512), "512 B/s");
  assert.equal(formatSpeed(1024), "1.0 KB/s");
  assert.equal(formatSpeed(1024 * 1024 * 5.4), "5.4 MB/s");
  assert.equal(formatSpeed(1024 * 1024 * 1024 * 1.5), "1.5 GB/s");
});

test("formatEta formats remaining seconds into human friendly text", () => {
  assert.equal(formatEta(null), "--");
  assert.equal(formatEta(undefined), "--");
  assert.equal(formatEta(0), "0s");
  assert.equal(formatEta(45), "45s remaining");
  assert.equal(formatEta(95), "1m 35s remaining");
  assert.equal(formatEta(120), "2m remaining");
  assert.equal(formatEta(3665), "1h 1m remaining");
});

test("tauri bridge pause and resume transfer calls", async (t) => {
  await t.test("pauseTransfer returns true in mock environment", async () => {
    const result = await pauseTransfer("test-session-123");
    assert.equal(result, true);
  });

  await t.test("resumeTransfer returns true in mock environment", async () => {
    const result = await resumeTransfer("test-session-123");
    assert.equal(result, true);
  });

  await t.test("onTransferPaused registers a subscriber", async () => {
    let unlisten = await onTransferPaused(() => {});
    assert.equal(typeof unlisten, "function");
  });

  await t.test("onTransferResumed registers a subscriber", async () => {
    let unlisten = await onTransferResumed(() => {});
    assert.equal(typeof unlisten, "function");
  });
});

test("updateReceiveProgress includes formatted speed and ETA in toast DOM", async () => {
  const { updateReceiveProgress } = await import("../src/script/ui.js");

  // Minimal DOM container mock
  const container = {
    id: "toast-container",
    appendChild(child) {
      this.child = child;
    },
  };

  const statusEl = {
    id: "myDeviceStatus",
    textContent: "",
  };

  globalThis.document = {
    getElementById: (id) => {
      if (id === "toast-container") return container;
      if (id === "myDeviceStatus") return statusEl;
      return null;
    },
    createElement: (tag) => ({
      tagName: tag.toUpperCase(),
      id: "",
      className: "",
      innerHTML: "",
      isConnected: true,
    }),
  };

  updateReceiveProgress({
    file_name: "archive.zip",
    percent: 65,
    speed_bps: 1024 * 1024 * 8.5,
    eta_seconds: 14,
  });

  assert.equal(statusEl.textContent, "Receiving (65%)");
  assert.ok(container.child);
  assert.ok(container.child.innerHTML.includes("8.5 MB/s"));
  assert.ok(container.child.innerHTML.includes("14s remaining"));
});
