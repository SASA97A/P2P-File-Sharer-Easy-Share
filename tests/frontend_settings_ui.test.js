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
      contains(cls) { return this._classes.has(cls); },
      toggle(cls, val) {
        const on = val === undefined ? !this._classes.has(cls) : val;
        if (on) this._classes.add(cls);
        else this._classes.delete(cls);
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

import { setupSettings, openSettingsView, closeSettingsView } from "../src/script/settings.js";
import { getMyDeviceInfo, getDownloadDir } from "../src/script/tauri-bridge.js";

test("settings UI controller", async (t) => {
  // Initialize mock DOM elements
  const view = createMockElement("settingsView");
  view.classList.add("hidden");
  const openBtn = createMockElement("openSettingsBtn", "button");
  const nameInput = createMockElement("settingsDeviceNameInput", "input");
  const saveNameBtn = createMockElement("saveDeviceNameBtn", "button");
  const dirDisplay = createMockElement("settingsDownloadDirDisplay", "span");
  const changeDirBtn = createMockElement("changeDownloadDirBtn", "button");
  const openDirBtn = createMockElement("openDownloadFolderBtn", "button");
  const sidebarOpenDirBtn = createMockElement("openDownloadsBtn", "button");
  const myDeviceNameBadge = createMockElement("myDeviceName", "span");
  const speedSelect = createMockElement("settingsSpeedLimitSelect", "select");

  const minToggle = createMockElement("settingsMinimizeToTrayToggle", "input");
  const notifToggle = createMockElement("settingsNotificationsToggle", "input");

  setupSettings();

  await t.test("openSettingsView populates inputs with current configuration", async () => {
    const { getSpeedLimit, setTraySettings } = await import("../src/script/tauri-bridge.js");
    await setTraySettings({ minimizeToTray: true, notificationsEnabled: true });
    const currentLimit = await getSpeedLimit();
    await openSettingsView();
    assert.equal(view.classList._classes.has("hidden"), false);
    assert.ok(nameInput.value.length > 0);
    assert.ok(dirDisplay.textContent.length > 0);
    assert.equal(speedSelect.value, String(currentLimit));
    assert.equal(minToggle.checked, true);
    assert.equal(notifToggle.checked, true);
  });

  await t.test("changing minimize to tray toggle invokes setTraySettings", async () => {
    const { getTraySettings } = await import("../src/script/tauri-bridge.js");
    minToggle.checked = false;
    minToggle.dispatchEvent({ type: "change", target: { checked: false } });
    await new Promise((r) => setTimeout(r, 10));

    const current = await getTraySettings();
    assert.equal(current.minimizeToTray, false);
  });

  await t.test("changing desktop notifications toggle invokes setTraySettings", async () => {
    const { getTraySettings } = await import("../src/script/tauri-bridge.js");
    notifToggle.checked = false;
    notifToggle.dispatchEvent({ type: "change", target: { checked: false } });
    await new Promise((r) => setTimeout(r, 10));

    const current = await getTraySettings();
    assert.equal(current.notificationsEnabled, false);
  });

  await t.test("changing speed limit dropdown invokes setSpeedLimit", async () => {
    const { getSpeedLimit } = await import("../src/script/tauri-bridge.js");
    speedSelect.value = "10485760";
    speedSelect.dispatchEvent({ type: "change" });
    await new Promise((r) => setTimeout(r, 10));

    const current = await getSpeedLimit();
    assert.equal(current, 10485760);
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

  await t.test("closeSettingsView hides the settings page", () => {
    closeSettingsView();
    assert.equal(view.classList._classes.has("hidden"), true);
  });
});
