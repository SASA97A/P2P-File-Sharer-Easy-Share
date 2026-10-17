import test from "node:test";
import assert from "node:assert/strict";

const domElements = new Map();

function createMockElement(id, tag = "div") {
  const listeners = {};
  const attrs = {};
  const children = [];
  const el = {
    id,
    tagName: tag.toUpperCase(),
    value: "",
    textContent: "",
    innerHTML: "",
    title: "",
    dataset: {},
    classList: {
      _classes: new Set(),
      add(cls) { this._classes.add(cls); },
      remove(cls) { this._classes.delete(cls); },
      contains(cls) { return this._classes.has(cls); },
      toggle(cls, val) {
        if (val !== undefined) {
          if (val) this.add(cls);
          else this.remove(cls);
        } else {
          if (this.contains(cls)) this.remove(cls);
          else this.add(cls);
        }
      },
    },
    setAttribute(k, v) { attrs[k] = String(v); },
    getAttribute(k) { return attrs[k] || null; },
    addEventListener(evt, fn) {
      if (!listeners[evt]) listeners[evt] = [];
      listeners[evt].push(fn);
    },
    dispatchEvent(evt) {
      const fns = listeners[evt.type] || [];
      fns.forEach((fn) => fn(evt));
    },
    appendChild(child) { children.push(child); },
    removeChild(child) {
      const idx = children.indexOf(child);
      if (idx !== -1) children.splice(idx, 1);
    },
    querySelectorAll(selector) {
      return [];
    },
    focus() {},
  };
  domElements.set(id, el);
  return el;
}

globalThis.document = {
  getElementById: (id) => domElements.get(id) || null,
  querySelector: (sel) => {
    if (sel.startsWith("#")) return domElements.get(sel.slice(1)) || null;
    return null;
  },
  querySelectorAll: (sel) => {
    if (sel === ".history-filter-btn") {
      return [
        domElements.get("filterAllBtn"),
        domElements.get("filterSentBtn"),
        domElements.get("filterReceivedBtn"),
      ].filter(Boolean);
    }
    return [];
  },
  addEventListener: () => {},
};

globalThis.confirm = () => true;

import {
  setupHistory,
  openHistoryModal,
  closeHistoryModal,
  renderHistoryList,
  setActiveFilter,
  setSearchQuery,
} from "../src/script/history.js";

test("transfer history UI controller", async (t) => {
  const modal = createMockElement("history-modal");
  modal.classList.add("hidden");
  const openBtn = createMockElement("openHistoryBtn", "button");
  const closeBtn = createMockElement("closeHistoryBtn", "button");
  const searchInput = createMockElement("historySearchInput", "input");
  const filterAllBtn = createMockElement("filterAllBtn", "button");
  filterAllBtn.dataset.filter = "all";
  const filterSentBtn = createMockElement("filterSentBtn", "button");
  filterSentBtn.dataset.filter = "sent";
  const filterReceivedBtn = createMockElement("filterReceivedBtn", "button");
  filterReceivedBtn.dataset.filter = "received";
  const clearBtn = createMockElement("clearHistoryBtn", "button");
  const historyList = createMockElement("historyList");
  const emptyState = createMockElement("historyEmptyState");
  emptyState.classList.add("hidden");

  setupHistory();

  await t.test("openHistoryModal unhides modal and loads history", async () => {
    await openHistoryModal();
    assert.equal(modal.classList.contains("hidden"), false);
  });

  await t.test("closeHistoryModal hides modal", async () => {
    closeHistoryModal();
    assert.equal(modal.classList.contains("hidden"), true);
  });

  await t.test("renderHistoryList handles empty records cleanly", async () => {
    renderHistoryList([]);
    assert.equal(emptyState.classList.contains("hidden"), false);
  });

  await t.test("renderHistoryList renders record cards", async () => {
    const mockRecords = [
      {
        id: "r1",
        session_token: "tok1",
        timestamp: Date.now(),
        direction: "received",
        peer_name: "Pixel 7",
        peer_ip: "192.168.1.5",
        status: "completed",
        files: [{ name: "photo.jpg", path: "C:\\Downloads\\photo.jpg", size: 2048 }],
        total_bytes: 2048,
        error_message: null,
      },
      {
        id: "r2",
        session_token: "tok2",
        timestamp: Date.now() - 5000,
        direction: "sent",
        peer_name: "MacBook",
        peer_ip: "192.168.1.10",
        status: "completed",
        files: [{ name: "notes.txt", path: "C:\\Downloads\\notes.txt", size: 512 }],
        total_bytes: 512,
        error_message: null,
      },
    ];

    renderHistoryList(mockRecords);
    assert.equal(emptyState.classList.contains("hidden"), true);
    assert.ok(historyList.innerHTML.includes("Pixel 7"));
    assert.ok(historyList.innerHTML.includes("MacBook"));
    assert.ok(historyList.innerHTML.includes("photo.jpg"));
  });

  await t.test("filtering by tab works", async () => {
    setActiveFilter("sent");
    assert.equal(filterSentBtn.classList.contains("active"), true);
    assert.equal(filterAllBtn.classList.contains("active"), false);
  });
});
