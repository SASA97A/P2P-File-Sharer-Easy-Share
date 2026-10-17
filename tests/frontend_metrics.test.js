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

test("updateReceiveProgress renders overall percent, speed and ETA in the receive card", async () => {
  const { updateReceiveProgress, setReceiveContext, formatEtaShort } = await import("../src/script/ui.js");

  assert.equal(formatEtaShort(14), "14s");
  assert.equal(formatEtaShort(95), "1m 35s");
  assert.equal(formatEtaShort(null), "—");

  // Minimal receive card mock: role slots resolved through querySelector
  const slots = new Map();
  const makeSlot = () => ({
    textContent: "",
    innerHTML: "",
    style: {},
    attrs: {},
    setAttribute(k, v) { this.attrs[k] = String(v); },
    addEventListener() {},
  });
  const card = {
    id: "receiveCard",
    dataset: {},
    innerHTML: "",
    classList: { _c: new Set(["hidden"]), add(c) { this._c.add(c); }, remove(c) { this._c.delete(c); }, contains(c) { return this._c.has(c); } },
    querySelector(sel) {
      const m = sel.match(/data-role="([^"]+)"/);
      if (!m) return null;
      if (!slots.has(m[1])) slots.set(m[1], makeSlot());
      return slots.get(m[1]);
    },
  };
  const badge = { textContent: "", classList: { add() {}, remove() {} } };

  globalThis.document = {
    getElementById: (id) => {
      if (id === "receiveCard") return card;
      if (id === "navSendProgress") return badge;
      return null;
    },
    querySelectorAll: () => [],
  };

  setReceiveContext({ senderName: "Pixel-8-Pro", fileCount: 2, totalBytes: 2000, saveDir: "C:\Downloads" });

  updateReceiveProgress({
    session_token: "tok",
    file_id: "a",
    file_name: "archive.zip",
    percent: 65,
    received_bytes: 1000,
    total_bytes: 1500,
    speed_bps: 1024 * 1024 * 8.5,
    eta_seconds: 14,
  });

  assert.equal(card.classList.contains("hidden"), false);
  assert.equal(card.dataset.sessionToken, "tok");
  assert.equal(slots.get("title").textContent, "Receiving from Pixel-8-Pro");
  assert.equal(slots.get("percent").textContent, "50", "overall percent uses the batch total");
  assert.equal(slots.get("speed").textContent, "8.5 MB/s");
  assert.equal(slots.get("eta").textContent, "14s");
  assert.equal(slots.get("fill").style.width, "50%");
  assert.equal(badge.textContent, "50%");
});
