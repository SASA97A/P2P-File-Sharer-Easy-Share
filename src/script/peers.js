import { renderPeers } from "./ui.js";

let discoveredPeers = [];
let selectedPeer = null;

export function getPeers() {
  return discoveredPeers;
}
export function getSelectedPeer() {
  return selectedPeer;
}
export function setSelectedPeer(peer) {
  selectedPeer = peer;
}

// Handle Bonjour / UDP discovered peer
window.api.onPeerFound((peer) => {
  const key = getPeerKey(peer);
  const existingIndex = discoveredPeers.findIndex((p) => getPeerKey(p) === key);

  if (existingIndex !== -1) {
    discoveredPeers[existingIndex] = {
      ...discoveredPeers[existingIndex],
      ...peer,
    };
  } else {
    console.log("Found:", key);
    discoveredPeers.push(peer);
  }
  renderPeers(discoveredPeers, setSelectedPeer);
});

// Handle peer lost
window.api.onPeerLost((peer) => {
  const key = getPeerKey(peer);
  discoveredPeers = discoveredPeers.filter((p) => getPeerKey(p) !== key);
  console.log("Lost:", key);
  renderPeers(discoveredPeers, setSelectedPeer);
});

// Manual peer add
export function addManualPeer(host) {
  if (!host) return;
  const peer = { name: "Manual", host, port: 5050, osType: "Unknown" };
  const key = getPeerKey(peer);

  const existingIndex = discoveredPeers.findIndex((p) => getPeerKey(p) === key);
  if (existingIndex !== -1) {
    discoveredPeers[existingIndex] = {
      ...discoveredPeers[existingIndex],
      ...peer,
    };
  } else {
    discoveredPeers.push(peer);
  }
  renderPeers(discoveredPeers, setSelectedPeer);
}

function getPeerKey(peer) {
  return `${peer.host}:${peer.port}`;
}
