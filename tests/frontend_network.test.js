import test from "node:test";
import assert from "node:assert/strict";

const elements = new Map();

function element(id) {
  const listeners = {};
  const classes = new Set(["hidden"]);
  const el = {
    id,
    value: "",
    children: [],
    classList: {
      contains: (cls) => classes.has(cls),
      toggle: (cls, on) => (on ? classes.add(cls) : classes.delete(cls)),
      add: (cls) => classes.add(cls),
      remove: (cls) => classes.delete(cls),
    },
    replaceChildren(...kids) {
      el.children = kids;
    },
    addEventListener(type, fn) {
      listeners[type] = fn;
    },
    fire: (type) => listeners[type](),
  };
  elements.set(id, el);
  return el;
}

globalThis.document = {
  getElementById: (id) => elements.get(id) || null,
  createElement: () => ({ value: "", textContent: "" }),
  querySelectorAll: () => [],
};

const { renderNetworkOptions, setupNetworkSelector } = await import("../src/script/network.js");

const settingsField = element("settingsNetworkField");
const settingsSelect = element("settingsNetworkSelect");
const pairField = element("pairNetworkField");
const pairSelect = element("pairNetworkSelect");

const twoNetworks = {
  options: [
    { adapter: "Ethernet 2", ip: "10.140.75.88" },
    { adapter: "Wi-Fi", ip: "192.168.1.215" },
  ],
  selected: "Ethernet 2",
};

test("selectors stay hidden when there is one network", () => {
  renderNetworkOptions({ options: [twoNetworks.options[1]], selected: "Wi-Fi" });
  assert.equal(settingsField.classList.contains("hidden"), true);
  assert.equal(pairField.classList.contains("hidden"), true);
});

test("selectors show every network and the current one with two networks", () => {
  renderNetworkOptions(twoNetworks);
  for (const [field, select] of [[settingsField, settingsSelect], [pairField, pairSelect]]) {
    assert.equal(field.classList.contains("hidden"), false);
    assert.deepEqual(
      select.children.map((o) => [o.value, o.textContent]),
      [
        ["Ethernet 2", "Ethernet 2 · 10.140.75.88"],
        ["Wi-Fi", "Wi-Fi · 192.168.1.215"],
      ]
    );
    assert.equal(select.value, "Ethernet 2");
  }
});

test("changing the network updates both selectors and reports the new pairing info", async () => {
  let pairing = null;
  await setupNetworkSelector((info) => {
    pairing = info;
  });

  settingsSelect.value = settingsSelect.children[1].value;
  await settingsSelect.fire("change");

  assert.equal(settingsSelect.value, "Wi-Fi");
  assert.equal(pairSelect.value, "Wi-Fi");
  assert.equal(pairing.ip, "192.168.1.215");
});
