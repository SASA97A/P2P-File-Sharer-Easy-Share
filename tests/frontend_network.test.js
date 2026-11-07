import test from "node:test";
import assert from "node:assert/strict";

const elements = new Map();

function element(id) {
  const classes = new Set(["hidden"]);
  const el = {
    id,
    textContent: "",
    title: "",
    addEventListener: () => {},
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

const settingsRow = element("settingsNetworkField");
const settingsTitle = element("settingsNetworkTitle");
const settingsDesc = element("settingsNetworkDesc");
const settingsTrigger = element("settingsNetworkBtn");
const settingsAddress = element("settingsLocalAddress");
const pairField = element("pairNetworkField");
const names = [element("settingsNetworkName"), element("pairNetworkName")];
const ips = [element("settingsNetworkIp"), element("pairNetworkIp")];

const twoNetworks = {
  options: [
    { adapter: "Ethernet 2", ip: "10.140.75.88" },
    { adapter: "Wi-Fi", ip: "192.168.1.215" },
  ],
  selected: "Ethernet 2",
};

test("one network shows the plain address and hides the selectors", () => {
  renderNetworkOptions({ options: [twoNetworks.options[1]], selected: "Wi-Fi" });
  assert.equal(pairField.classList.contains("hidden"), true);
  assert.equal(settingsTrigger.classList.contains("hidden"), true);
  assert.equal(settingsAddress.classList.contains("hidden"), false);
  assert.equal(settingsTitle.textContent, "Your address");
  assert.equal(settingsDesc.textContent, "Other devices can use it to reach this one.");
  assert.equal(settingsRow.title, "");
});

test("selectors show the current network's adapter and address with two networks", () => {
  renderNetworkOptions(twoNetworks);
  assert.equal(pairField.classList.contains("hidden"), false);
  assert.equal(settingsTrigger.classList.contains("hidden"), false);
  assert.equal(settingsAddress.classList.contains("hidden"), true);
  assert.equal(settingsTitle.textContent, "Your network");
  assert.equal(settingsDesc.textContent, "Pick the network the other device is on.");
  assert.match(settingsRow.title, /^Sets the address/);
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
