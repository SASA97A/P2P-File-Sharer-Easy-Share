import test from "node:test";
import assert from "node:assert/strict";

// Setup minimal document global
if (typeof globalThis.document === "undefined") {
  globalThis.document = {
    getElementById: () => null,
    querySelector: () => null,
    querySelectorAll: () => [],
    createElement: () => ({
      dataset: {},
      style: { setProperty() {} },
      addEventListener() {},
      appendChild() {},
    }),
  };
}

import {
  getPeers,
  upsertPeer,
  setPeerList,
  getSelectedPeer,
  setSelectedPeer,
  getPeerKey,
} from "../src/script/peers.js";

test("frontend peers module - deduplication across interfaces (Wi-Fi and Ethernet)", async (t) => {
  setPeerList([]);

  await t.test("upsertPeer updates existing peer card when same device is announced with a different IP", () => {
    setPeerList([]);

    const wifiPeer = {
      device_name: "Alice-Workstation",
      device_type: "desktop",
      os: "windows",
      ip: "192.168.1.150",
      port: 5050,
      pairing_pin: "123456",
    };

    const ethernetPeer = {
      device_name: "Alice-Workstation",
      device_type: "desktop",
      os: "windows",
      ip: "10.0.0.150",
      port: 5050,
      pairing_pin: "123456",
    };

    // 1. Initial discovery over Wi-Fi
    upsertPeer(wifiPeer);
    let peers = getPeers();
    assert.equal(peers.length, 1, "Should have 1 peer initially");
    assert.equal(peers[0].ip, "192.168.1.150");

    // 2. Subsequent discovery of same device over Ethernet
    upsertPeer(ethernetPeer);
    peers = getPeers();
    assert.equal(peers.length, 1, "Must NOT create duplicate cards for the same device");
    assert.equal(peers[0].ip, "10.0.0.150", "Must update to the newer/Ethernet IP address");
  });

  await t.test("setPeerList deduplicates list containing same device on multiple IPs", () => {
    setPeerList([]);

    const inputList = [
      {
        device_name: "Bob-Laptop",
        device_type: "laptop",
        os: "macos",
        ip: "192.168.1.100",
        port: 5050,
      },
      {
        device_name: "Bob-Laptop",
        device_type: "laptop",
        os: "macos",
        ip: "192.168.1.101",
        port: 5050,
      },
      {
        device_name: "Charlie-Phone",
        device_type: "mobile",
        os: "android",
        ip: "192.168.1.102",
        port: 5050,
      },
    ];

    setPeerList(inputList);
    const peers = getPeers();
    assert.equal(peers.length, 2, "Must deduplicate Bob-Laptop so only 2 total devices exist");
    assert.equal(peers[0].device_name, "Bob-Laptop");
    assert.equal(peers[1].device_name, "Charlie-Phone");
  });

  await t.test("preserves active selection when device IP updates", () => {
    setPeerList([]);

    const wifiPeer = {
      device_name: "Dana-Server",
      device_type: "desktop",
      os: "linux",
      ip: "192.168.1.200",
      port: 5050,
    };

    upsertPeer(wifiPeer);
    setSelectedPeer(wifiPeer);
    assert.ok(getSelectedPeer(), "Peer should be selected");

    const ethernetPeer = {
      device_name: "Dana-Server",
      device_type: "desktop",
      os: "linux",
      ip: "10.0.0.200",
      port: 5050,
    };

    upsertPeer(ethernetPeer);
    const selected = getSelectedPeer();
    assert.ok(selected, "Selection must be preserved after IP change");
    assert.equal(selected.ip, "10.0.0.200", "Selected peer must have updated IP");
  });
});
