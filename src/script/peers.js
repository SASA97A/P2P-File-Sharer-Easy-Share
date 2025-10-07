// Import UI rendering function
import { renderPeers } from "./ui.js";

// Array to store discovered peers
let discoveredPeers = [];
// Currently selected peer for file transfer
let selectedPeer = null;

/**
 * Returns all discovered peers
 * @returns {Array} Array of peer objects
 */
export function getPeers() {
  return discoveredPeers;
}

/**
 * Returns the currently selected peer
 * @returns {Object|null} Selected peer or null
 */
export function getSelectedPeer() {
  return selectedPeer;
}

/**
 * Sets the selected peer for file transfer
 * @param {Object} peer - Peer object to select
 */
export function setSelectedPeer(peer) {
  selectedPeer = peer;
}

// Listen for peer discovery events from main process
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

// Listen for peer lost events from main process
window.api.onPeerLost((peer) => {
  const key = getPeerKey(peer);
  discoveredPeers = discoveredPeers.filter((p) => getPeerKey(p) !== key);
  console.log("Lost:", key);
  renderPeers(discoveredPeers, setSelectedPeer);
});

/**
 * Adds a manually specified peer by IP address
 * @param {string} host - IP address of peer
 */
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

/**
 * Creates a unique key for a peer based on host:port
 * @param {Object} peer - Peer object
 * @returns {string} Unique key
 */
function getPeerKey(peer) {
  return `${peer.host}:${peer.port}`;
}
