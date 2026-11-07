import test from "node:test";
import assert from "node:assert/strict";

const elements = new Map();

function element(id) {
  const classes = new Set(["hidden"]);
  const el = {
    id,
    textContent: "",
    title: "",
    classList: {
      contains: (cls) => classes.has(cls),
      toggle: (cls, on) => (on ? classes.add(cls) : classes.delete(cls)),
      add: (cls) => classes.add(cls),
      remove: (cls) => classes.delete(cls),
    },
  };
  elements.set(id, el);
  return el;
}

globalThis.document = {
  getElementById: (id) => elements.get(id) || null,
  querySelectorAll: () => [],
  addEventListener: () => {},
};
globalThis.window = { addEventListener: () => {} };

const { renderNetworkOptions, selectNetwork, setupNetworkSelector } = await import(
  "../src/script/network.js"
);

const fields = [element("settingsNetworkField"), element("pairNetworkField")];
const names = [element("settingsNetworkName"), element("pairNetworkName")];
const ips = [element("settingsNetworkIp"), element("pairNetworkIp")];

const twoNetworks = {
  options: [
    { adapter: "Ethernet 2", ip: "10.140.75.88" },
    { adapter: "Wi-Fi", ip: "192.168.1.215" },
  ],
  selected: "Ethernet 2",
};

test("selectors stay hidden when there is one network", () => {
  renderNetworkOptions({ options: [twoNetworks.options[1]], selected: "Wi-Fi" });
  fields.forEach((field) => assert.equal(field.classList.contains("hidden"), true));
});

test("selectors show the current network's adapter and address with two networks", () => {
  renderNetworkOptions(twoNetworks);
  fields.forEach((field) => assert.equal(field.classList.contains("hidden"), false));
  names.forEach((name) => {
    assert.equal(name.textContent, "Ethernet 2");
    assert.equal(name.title, "Ethernet 2");
  });
  ips.forEach((ip) => assert.equal(ip.textContent, "10.140.75.88"));
});

test("changing the network updates both selectors and reports the new pairing info", async () => {
  let pairing = null;
  await setupNetworkSelector((info) => {
    pairing = info;
  });

  await selectNetwork("Wi-Fi");

  names.forEach((name) => assert.equal(name.textContent, "Wi-Fi"));
  ips.forEach((ip) => assert.equal(ip.textContent, "192.168.1.215"));
  assert.equal(pairing.ip, "192.168.1.215");
});
