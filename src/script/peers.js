/**
 * Easy Share - Peers Module
 * Manages discovered and manual network peer state with persistent selection
 */

import { renderPeers, showToast } from "./ui.js";
import { addManualPeer as bridgeAddManualPeer } from "./tauri-bridge.js";

// Array to store discovered and manual peers
let discoveredPeers = [];
// Currently selected peer for file transfer
let selectedPeer = null;

/**
 * Returns a unique string key for a peer based on ip:port
 * @param {Object} peer
 * @returns {string}
 */
export function getPeerKey(peer) {
  if (!peer) return "";
  const host = peer.ip || peer.host || "unknown";
  const port = peer.port || 5050;
  return `${host}:${port}`;
}

/**
 * Returns all discovered peers
 * @returns {Array<Object>}
 */
export function getPeers() {
  return [...discoveredPeers];
}

/**
 * Returns the currently selected peer or null
 * @returns {Object|null}
 */
export function getSelectedPeer() {
  return selectedPeer;
}

/**
 * Sets or clears the selected peer and refreshes UI
 * @param {Object|null} peer
 */
export function setSelectedPeer(peer) {
  selectedPeer = peer;
  renderPeers(discoveredPeers, handlePeerSelect, selectedPeer);
}

/**
 * Handles user clicking a peer card to toggle selection
 * @param {Object} peer
 */
export function handlePeerSelect(peer) {
  if (selectedPeer && getPeerKey(selectedPeer) === getPeerKey(peer)) {
    // Toggle off selection if clicked again
    selectedPeer = null;
  } else {
    selectedPeer = peer;
  }
  renderPeers(discoveredPeers, handlePeerSelect, selectedPeer);
}

/**
 * Replaces entire peer list while preserving active selection
 * @param {Array<Object>} peers
 */
export function setPeerList(peers) {
  discoveredPeers = Array.isArray(peers) ? [...peers] : [];

  // Preserve selection if the selected peer still exists in the new list
  if (selectedPeer) {
    const matching = discoveredPeers.find(
      (p) => getPeerKey(p) === getPeerKey(selectedPeer)
    );
    selectedPeer = matching || null;
  }

  renderPeers(discoveredPeers, handlePeerSelect, selectedPeer);
}

/**
 * Adds or updates a single peer in the list while preserving active selection
 * @param {Object} peer
 */
export function upsertPeer(peer) {
  if (!peer) return;

  const key = getPeerKey(peer);
  const existingIdx = discoveredPeers.findIndex((p) => getPeerKey(p) === key);

  if (existingIdx !== -1) {
    discoveredPeers[existingIdx] = {
      ...discoveredPeers[existingIdx],
      ...peer,
    };
    if (selectedPeer && getPeerKey(selectedPeer) === key) {
      selectedPeer = discoveredPeers[existingIdx];
    }
  } else {
    discoveredPeers.push(peer);
    if (selectedPeer && getPeerKey(selectedPeer) === key) {
      selectedPeer = peer;
    }
  }

  renderPeers(discoveredPeers, handlePeerSelect, selectedPeer);
}

/**
 * Removes a lost peer from the list while preserving active selection
 * @param {Object} peer
 */
export function removePeer(peer) {
  if (!peer) return;

  const key = getPeerKey(peer);
  discoveredPeers = discoveredPeers.filter((p) => getPeerKey(p) !== key);

  if (selectedPeer && getPeerKey(selectedPeer) === key) {
    selectedPeer = null;
  }

  renderPeers(discoveredPeers, handlePeerSelect, selectedPeer);
}

/**
 * Adds a manual peer by IP/port, clears the input, and updates peer radar
 * @param {string} hostInput
 * @param {number|null} portInput
 */
export async function addManualPeerInput(hostInput, portInput = null) {
  const host = (hostInput || "").trim();
  if (!host) {
    showToast("Please enter a valid IP address", "error");
    return;
  }

  try {
    let port = portInput;
    let ip = host;

    // Handle "ip:port" syntax in host input
    if (host.includes(":")) {
      const parts = host.split(":");
      ip = parts[0];
      const parsedPort = parseInt(parts[1], 10);
      if (!isNaN(parsedPort) && parsedPort > 0) {
        port = parsedPort;
      }
    }

    const peer = await bridgeAddManualPeer(ip, port);
    upsertPeer(peer);

    // Clear input field if present in DOM
    const inputEl = document.getElementById("manualIp");
    if (inputEl) {
      inputEl.value = "";
    }

    showToast(`Device ${peer.device_name || ip} added!`, "success");
  } catch (err) {
    showToast(`Failed to add device: ${err}`, "error");
  }
}
