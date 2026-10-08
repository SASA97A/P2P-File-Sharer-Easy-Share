/**
 * Easy Share - Main Application Renderer Entry Point
 * Orchestrates event wiring, drag-and-drop, pairing controls, and discovery feeds.
 */

import { addFiles } from "./files.js";
import {
  setPeerList,
  upsertPeer,
  removePeer,
} from "./peers.js";
import { setupTransfer } from "./transfer.js";
import { setupPairing } from "./pairing.js";
import { updateMyDeviceBadge } from "./ui.js";
import {
  getMyDeviceInfo,
  getDiscoveredPeers,
  onPeerFound,
  onPeerLost,
  onPeerUpdated,
} from "./tauri-bridge.js";

document.addEventListener("DOMContentLoaded", async () => {
  const dropArea = document.getElementById("drop-area");
  const fileInput = document.getElementById("fileInput");

  // -------------------------------------------------------------
  // 1. Initial Device & Discovery Bootstrap
  // -------------------------------------------------------------
  try {
    const myDevice = await getMyDeviceInfo();
    updateMyDeviceBadge(myDevice);
  } catch (err) {
    console.error("Failed to fetch local device info:", err);
  }

  try {
    const peers = await getDiscoveredPeers();
    setPeerList(peers || []);
  } catch (err) {
    console.error("Failed to fetch initial peers:", err);
    setPeerList([]);
  }

  // -------------------------------------------------------------
  // 2. Peer Discovery Real-time Subscriptions
  // -------------------------------------------------------------
  onPeerFound((peer) => {
    console.log("Peer found:", peer);
    upsertPeer(peer);
  });

  onPeerLost((peer) => {
    console.log("Peer lost:", peer);
    removePeer(peer);
  });

  onPeerUpdated((peer) => {
    console.log("Peer updated:", peer);
    upsertPeer(peer);
  });

  // -------------------------------------------------------------
  // 3. File Selection & Drag-and-Drop Handlers
  // -------------------------------------------------------------
  if (dropArea && fileInput) {
    // Click on drop area triggers hidden file input
    dropArea.addEventListener("click", () => {
      fileInput.click();
    });

    // Keyboard trigger on drop area (Enter / Space)
    dropArea.addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        fileInput.click();
      }
    });

    // File input change
    fileInput.addEventListener("change", () => {
      if (fileInput.files && fileInput.files.length > 0) {
        addFiles(fileInput.files);
        fileInput.value = ""; // Reset to allow re-selection of same file
      }
    });

    // Drag-and-drop events on target drop zone
    dropArea.addEventListener("dragenter", (e) => {
      e.preventDefault();
      e.stopPropagation();
      dropArea.classList.add("drag-over");
    });

    dropArea.addEventListener("dragover", (e) => {
      e.preventDefault();
      e.stopPropagation();
      dropArea.classList.add("drag-over");
    });

    dropArea.addEventListener("dragleave", (e) => {
      e.preventDefault();
      e.stopPropagation();
      dropArea.classList.remove("drag-over");
    });

    dropArea.addEventListener("drop", (e) => {
      e.preventDefault();
      e.stopPropagation();
      dropArea.classList.remove("drag-over");

      const dt = e.dataTransfer;
      if (dt && dt.files && dt.files.length > 0) {
        addFiles(dt.files);
      }
    });
  }

  // Prevent default window-level drag/drop navigation
  window.addEventListener("dragover", (e) => e.preventDefault(), false);
  window.addEventListener("drop", (e) => e.preventDefault(), false);

  // -------------------------------------------------------------
  // 4. Transfer Logic & Consent Setup
  // -------------------------------------------------------------
  setupTransfer();

  // -------------------------------------------------------------
  // 5. Device Pairing Setup (PIN, QR Code, Direct IP)
  // -------------------------------------------------------------
  setupPairing();
});
