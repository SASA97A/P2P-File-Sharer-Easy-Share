import test from "node:test";
import assert from "node:assert/strict";

// Setup mock DOM for settings testing
const domElements = new Map();

function createMockElement(id, tag = "div") {
  const listeners = {};
  const attrs = {};
  const el = {
    id,
    tagName: tag.toUpperCase(),
    value: "",
    textContent: "",
    title: "",
    classList: {
      _classes: new Set(),
      add(cls) { this._classes.add(cls); },
      remove(cls) { this._classes.delete(cls); },
      contains(cls) { return this._classes.contains(cls); },
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
  querySelectorAll: () => [],
  addEventListener: () => {},
};

import { setupSettings, openSettingsModal, closeSettingsModal } from "../src/script/settings.js";
import { getMyDeviceInfo, getDownloadDir } from "../src/script/tauri-bridge.js";

test("settings UI controller", async (t) => {
  // Initialize mock DOM elements
  const modal = createMockElement("settings-modal");
  modal.classList.add("hidden");
  const openBtn = createMockElement("openSettingsBtn", "button");
  const closeBtn = createMockElement("closeSettingsBtn", "button");
  const nameInput = createMockElement("settingsDeviceNameInput", "input");
  const saveNameBtn = createMockElement("saveDeviceNameBtn", "button");
  const dirDisplay = createMockElement("settingsDownloadDirDisplay", "span");
  const changeDirBtn = createMockElement("changeDownloadDirBtn", "button");
  const openDirBtn = createMockElement("openDownloadFolderBtn", "button");
  const headerOpenDirBtn = createMockElement("headerOpenDownloadsBtn", "button");
  const myDeviceNameBadge = createMockElement("myDeviceName", "span");

  setupSettings();

  await t.test("openSettingsModal populates inputs with current configuration", async () => {
    await openSettingsModal();
    assert.equal(modal.classList._classes.has("hidden"), false);
    assert.ok(nameInput.value.length > 0);
    assert.ok(dirDisplay.textContent.length > 0);
  });

  await t.test("save device name updates device and badge", async () => {
    nameInput.value = "New-Awesome-PC";
    saveNameBtn.dispatchEvent({ type: "click" });
    // Allow async handlers to execute
    await new Promise((r) => setTimeout(r, 10));

    const updatedInfo = await getMyDeviceInfo();
    assert.equal(updatedInfo.device_name, "New-Awesome-PC");
    assert.equal(myDeviceNameBadge.textContent, "New-Awesome-PC");
  });

  await t.test("closeSettingsModal adds hidden class", () => {
    closeSettingsModal();
    assert.equal(modal.classList._classes.has("hidden"), true);
  });
});
