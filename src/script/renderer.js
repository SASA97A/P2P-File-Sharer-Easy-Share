/**
 * Easy Share - Main Application Renderer Entry Point
 * Orchestrates event wiring, drag-and-drop, pairing controls, and discovery feeds.
 */

import { addFiles, clearFiles } from "./files.js";
import {
  setPeerList,
  upsertPeer,
  removePeer,
} from "./peers.js";
import { setupTransfer } from "./transfer.js";
import { setupPairing } from "./pairing.js";
import { setupSettings } from "./settings.js";
import { setupHistory } from "./history.js";
import { setupNav } from "./nav.js";
import { updateMyDeviceBadge, updateMyAddress, showToast } from "./ui.js";
import {
  getMyDeviceInfo,
  getDiscoveredPeers,
  getMyPairingInfo,
  onPeerFound,
  onPeerLost,
  onPeerUpdated,
  pickFiles,
  pickFolder,
  inspectPaths,
  onDragDrop,
  onDragHover,
  onDragLeave,
} from "./tauri-bridge.js";

document.addEventListener("DOMContentLoaded", async () => {
  const dropArea = document.getElementById("drop-area");
  const fileInput = document.getElementById("fileInput");

  setupNav();

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
    const pairing = await getMyPairingInfo();
    if (pairing) updateMyAddress(pairing.ip, pairing.port);
  } catch (err) {
    console.error("Failed to fetch local address:", err);
  }

  try {
    const peers = await getDiscoveredPeers();
    setPeerList(peers || []);
  } catch (err) {
    console.error("Failed to fetch initial peers:", err);
    setPeerList([]);
  }

  const refreshBtn = document.getElementById("refreshPeersBtn");
  if (refreshBtn) {
    refreshBtn.addEventListener("click", async () => {
      refreshBtn.classList.add("is-spinning");
      refreshBtn.disabled = true;
      try {
        const peers = await getDiscoveredPeers();
        setPeerList(peers || []);
      } catch (err) {
        console.error("Failed to refresh peers:", err);
      } finally {
        // Keep the spin visible long enough to register
        setTimeout(() => {
          refreshBtn.classList.remove("is-spinning");
          refreshBtn.disabled = false;
        }, 500);
      }
    });
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
  const handleFilePick = async () => {
    try {
      const picked = await pickFiles();
      if (picked && picked.length > 0) {
        addFiles(picked);
        return;
      }
    } catch (err) {
      console.warn("Native file picker unavailable, falling back to input:", err);
      if (fileInput) {
        fileInput.click();
      }
    }
  };

  const handleFolderPick = async () => {
    try {
      const picked = await pickFolder();
      if (picked && picked.length > 0) {
        addFiles(picked);
      }
    } catch (err) {
      console.error("Folder picker failed:", err);
      showToast(`Could not add folder: ${err}`, "error");
    }
  };

  const addFilesBtn = document.getElementById("addFilesBtn");
  const addFolderBtn = document.getElementById("addFolderBtn");
  const clearFilesBtn = document.getElementById("clearFilesBtn");
  if (addFilesBtn) addFilesBtn.addEventListener("click", handleFilePick);
  if (addFolderBtn) addFolderBtn.addEventListener("click", handleFolderPick);
  if (clearFilesBtn) clearFilesBtn.addEventListener("click", () => clearFiles());

  if (dropArea) {
    // Click on drop area triggers native file dialog (or fileInput fallback)
    dropArea.addEventListener("click", () => {
      handleFilePick();
    });

    // Keyboard trigger on drop area (Enter / Space)
    dropArea.addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        handleFilePick();
      }
    });

    // Drag-and-drop events on target drop zone (DOM fallback)
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

  if (fileInput) {
    // DOM file input change fallback
    fileInput.addEventListener("change", () => {
      if (fileInput.files && fileInput.files.length > 0) {
        addFiles(fileInput.files);
        fileInput.value = ""; // Reset to allow re-selection of same file
      }
    });
  }

  // Native Tauri window drag-and-drop integration
  onDragHover(() => {
    if (dropArea) dropArea.classList.add("drag-over");
  });

  onDragLeave(() => {
    if (dropArea) dropArea.classList.remove("drag-over");
  });

  onDragDrop(async (payload) => {
    if (dropArea) dropArea.classList.remove("drag-over");
    const rawPaths =
      payload?.paths ||
      (Array.isArray(payload) ? payload : typeof payload === "string" ? [payload] : []);
    if (rawPaths && rawPaths.length > 0) {
      try {
        const inspected = await inspectPaths(rawPaths);
        if (inspected && inspected.length > 0) {
          addFiles(inspected);
        } else {
          addFiles(rawPaths);
        }
      } catch (err) {
        console.error("Failed to inspect native dropped paths:", err);
        addFiles(rawPaths);
      }
    }
  });

  // Prevent default window-level drag/drop navigation while capturing dropped files
  window.addEventListener("dragover", (e) => e.preventDefault(), false);
  window.addEventListener("drop", (e) => {
    e.preventDefault();
    if (dropArea) dropArea.classList.remove("drag-over");
    const dt = e.dataTransfer;
    if (dt && dt.files && dt.files.length > 0) {
      addFiles(dt.files);
    }
  }, false);

  // -------------------------------------------------------------
  // 4. Transfer Logic & Consent Setup
  // -------------------------------------------------------------
  setupTransfer();

  // -------------------------------------------------------------
  // 5. Device Pairing Setup (PIN, QR Code, Direct IP)
  // -------------------------------------------------------------
  setupPairing();

  // -------------------------------------------------------------
  // 6. Settings Setup (Device Name, Downloads Folder)
  // -------------------------------------------------------------
  setupSettings();

  // -------------------------------------------------------------
  // 7. Transfer History & Activity Log Setup
  // -------------------------------------------------------------
  setupHistory();
});
