import test from "node:test";
import assert from "node:assert/strict";

const elements = new Map();

function button(id) {
  const listeners = {};
  const attrs = {};
  const el = {
    id,
    innerHTML: "",
    title: "",
    setAttribute: (key, value) => {
      attrs[key] = value;
    },
    getAttribute: (key) => attrs[key],
    addEventListener: (type, fn) => {
      listeners[type] = fn;
    },
    click: () => listeners.click(),
  };
  elements.set(id, el);
  return el;
}

globalThis.document = { getElementById: (id) => elements.get(id) || null };

const { setupTitlebar } = await import("../src/script/titlebar.js");

const minimizeBtn = button("windowMinimizeBtn");
const maximizeBtn = button("windowMaximizeBtn");
const closeBtn = button("windowCloseBtn");

function fakeWindow() {
  const calls = [];
  const win = {
    calls,
    maximized: false,
    minimize: async () => calls.push("minimize"),
    toggleMaximize: async () => calls.push("toggleMaximize"),
    close: async () => calls.push("close"),
    isMaximized: async () => win.maximized,
    onResized: async (fn) => {
      win.resize = fn;
    },
  };
  return win;
}

test("window controls call the matching window actions", async () => {
  const win = fakeWindow();
  await setupTitlebar(win);

  await minimizeBtn.click();
  await maximizeBtn.click();
  await closeBtn.click();

  assert.deepEqual(win.calls, ["minimize", "toggleMaximize", "close"]);
});

test("maximize button switches to restore while the window is maximized", async () => {
  const win = fakeWindow();
  await setupTitlebar(win);
  assert.equal(maximizeBtn.getAttribute("aria-label"), "Maximize");
  const maximizeIcon = maximizeBtn.innerHTML;

  win.maximized = true;
  await win.resize();
  assert.equal(maximizeBtn.getAttribute("aria-label"), "Restore");
  assert.equal(maximizeBtn.title, "Restore");
  assert.notEqual(maximizeBtn.innerHTML, maximizeIcon);

  win.maximized = false;
  await win.resize();
  assert.equal(maximizeBtn.getAttribute("aria-label"), "Maximize");
  assert.equal(maximizeBtn.innerHTML, maximizeIcon);
});

test("a failing window action does not throw out of the click handler", async () => {
  const win = fakeWindow();
  win.minimize = async () => {
    throw new Error("denied");
  };
  const originalError = console.error;
  console.error = () => {};
  try {
    await setupTitlebar(win);
    await assert.doesNotReject(minimizeBtn.click());
  } finally {
    console.error = originalError;
  }
});

test("without a native window the setup does nothing", async () => {
  await assert.doesNotReject(setupTitlebar(null));
});
