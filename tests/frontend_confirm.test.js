import test from "node:test";
import assert from "node:assert/strict";

const elements = new Map();
const windowListeners = {};

function mockElement(id) {
  const listeners = {};
  const classes = new Set();
  const el = {
    id,
    textContent: "",
    innerHTML: "",
    classList: {
      add: (c) => classes.add(c),
      remove: (c) => classes.delete(c),
      contains: (c) => classes.has(c),
    },
    setAttribute() {},
    addEventListener(type, fn) {
      (listeners[type] ||= []).push(fn);
    },
    removeEventListener(type, fn) {
      listeners[type] = (listeners[type] || []).filter((f) => f !== fn);
    },
    click() {
      (listeners.click || []).slice().forEach((fn) => fn({ target: el }));
    },
    focus() {
      globalThis.document.activeElement = el;
    },
    contains: (other) => other === el,
  };
  elements.set(id, el);
  return el;
}

function pressKey(key, shiftKey = false) {
  const event = { key, shiftKey, prevented: false, preventDefault() { this.prevented = true; } };
  (windowListeners.keydown || []).slice().forEach((fn) => fn(event));
  return event;
}

globalThis.document = {
  activeElement: null,
  getElementById: (id) => elements.get(id) || null,
  querySelector: () => null,
  querySelectorAll: () => [],
  addEventListener: () => {},
};
globalThis.window = {
  addEventListener(type, fn) {
    (windowListeners[type] ||= []).push(fn);
  },
  removeEventListener(type, fn) {
    windowListeners[type] = (windowListeners[type] || []).filter((f) => f !== fn);
  },
};

const { showConfirm } = await import("../src/script/ui.js");

function setup() {
  elements.clear();
  const modal = mockElement("confirm-modal");
  modal.classList.add("hidden");
  const dialog = mockElement("confirmDialog");
  modal.contains = (other) => [modal, dialog, cancel, ok].includes(other);
  const cancel = mockElement("confirmCancelBtn");
  const ok = mockElement("confirmOkBtn");
  const title = mockElement("confirmTitle");
  const desc = mockElement("confirmDesc");
  const icon = mockElement("confirmIcon");
  const consent = mockElement("consent-modal");
  consent.classList.add("hidden");
  const trigger = mockElement("trigger");
  trigger.focus();
  return { modal, dialog, cancel, ok, title, desc, icon, consent, trigger };
}

const OPTIONS = { title: "Clear <b>?", desc: "Gone.", confirmLabel: "Clear", icon: "trash" };

test("showConfirm fills the dialog and focuses Cancel", () => {
  const ui = setup();
  const answer = showConfirm(OPTIONS);
  assert.equal(ui.modal.classList.contains("hidden"), false);
  assert.equal(ui.title.textContent, "Clear <b>?");
  assert.equal(ui.desc.textContent, "Gone.");
  assert.equal(ui.ok.textContent, "Clear");
  assert.match(ui.icon.innerHTML, /<svg/);
  assert.equal(document.activeElement, ui.cancel);
  ui.cancel.click();
  return answer;
});

test("the confirm button resolves true, hides the dialog and restores focus", async () => {
  const ui = setup();
  const answer = showConfirm(OPTIONS);
  ui.ok.click();
  assert.equal(await answer, true);
  assert.equal(ui.modal.classList.contains("hidden"), true);
  assert.equal(document.activeElement, ui.trigger);
});

test("Cancel, Escape and a backdrop click all resolve false", async () => {
  let ui = setup();
  let answer = showConfirm(OPTIONS);
  ui.cancel.click();
  assert.equal(await answer, false);

  ui = setup();
  answer = showConfirm(OPTIONS);
  pressKey("Escape");
  assert.equal(await answer, false);

  ui = setup();
  answer = showConfirm(OPTIONS);
  ui.modal.click();
  assert.equal(await answer, false);
  assert.equal(ui.modal.classList.contains("hidden"), true);
});

test("Tab stays inside the dialog", async () => {
  const ui = setup();
  const answer = showConfirm(OPTIONS);

  assert.equal(pressKey("Tab").prevented, true);
  assert.equal(document.activeElement, ui.ok);
  pressKey("Tab");
  assert.equal(document.activeElement, ui.cancel);
  pressKey("Tab", true);
  assert.equal(document.activeElement, ui.ok);

  // Focus that fell out of the dialog is pulled back in
  ui.trigger.focus();
  pressKey("Tab");
  assert.equal(document.activeElement, ui.cancel);

  ui.cancel.click();
  await answer;
});

test("keys belong to the consent dialog while it is open on top", async () => {
  const ui = setup();
  const answer = showConfirm(OPTIONS);
  ui.consent.classList.remove("hidden");

  pressKey("Escape");
  assert.equal(ui.modal.classList.contains("hidden"), false);
  assert.equal(pressKey("Tab").prevented, false);

  ui.consent.classList.add("hidden");
  pressKey("Escape");
  assert.equal(await answer, false);
});

test("a second request cancels the open one", async () => {
  const ui = setup();
  const first = showConfirm(OPTIONS);
  const second = showConfirm(OPTIONS);
  assert.equal(await first, false);
  ui.ok.click();
  assert.equal(await second, true);
});

test("a missing dialog never counts as a yes", async () => {
  elements.clear();
  assert.equal(await showConfirm(OPTIONS), false);
});
