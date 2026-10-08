/**
 * Easy Share - Main Application Renderer Entry Point
 * Orchestrates event wiring, drag-and-drop, room controls, and discovery feeds.
 */

import { addFiles } from "./files.js";
import {
  setPeerList,
  upsertPeer,
  removePeer,
  addManualPeerInput,
} from "./peers.js";
import { setupTransfer } from "./transfer.js";
import { updateMyDeviceBadge, showToast } from "./ui.js";
import {
  getMyDeviceInfo,
  getDiscoveredPeers,
  setRoomId,
  onPeerFound,
  onPeerLost,
  onPeerUpdated,
} from "./tauri-bridge.js";

document.addEventListener("DOMContentLoaded", async () => {
  const dropArea = document.getElementById("drop-area");
  const fileInput = document.getElementById("fileInput");
  const manualIpInput = document.getElementById("manualIp");
  const manualAddBtn = document.getElementById("manualAddBtn");
  const roomIdInput = document.getElementById("roomIdInput");
  const setRoomBtn = document.getElementById("setRoomBtn");

  // -------------------------------------------------------------
  // 1. Initial Device & Discovery Bootstrap
  // -------------------------------------------------------------
  try {
    const myDevice = await getMyDeviceInfo();
    updateMyDeviceBadge(myDevice);
    if (myDevice.room_id && roomIdInput) {
      roomIdInput.value = myDevice.room_id;
    }
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
  // 4. Room Configuration Actions
  // -------------------------------------------------------------
  async function applyRoomSetting() {
    if (!roomIdInput) return;
    const roomVal = roomIdInput.value.trim();
    try {
      const updatedInfo = await setRoomId(roomVal || null);
      updateMyDeviceBadge(updatedInfo);
      showToast(
        roomVal
          ? `Room updated to "${roomVal}"`
          : "Switched to Public room mode",
        "success"
      );
    } catch (err) {
      showToast(`Failed to set room: ${err}`, "error");
    }
  }

  if (setRoomBtn) {
    setRoomBtn.addEventListener("click", applyRoomSetting);
  }

  if (roomIdInput) {
    roomIdInput.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        applyRoomSetting();
      }
    });
  }

  // -------------------------------------------------------------
  // 5. Manual Peer Addition Actions
  // -------------------------------------------------------------
  async function triggerManualAdd() {
    if (!manualIpInput) return;
    const ipVal = manualIpInput.value.trim();
    if (!ipVal) {
      showToast("Please enter a device IP address", "error");
      return;
    }
    await addManualPeerInput(ipVal);
  }

  if (manualAddBtn) {
    manualAddBtn.addEventListener("click", triggerManualAdd);
  }

  if (manualIpInput) {
    manualIpInput.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        e.preventDefault();
        triggerManualAdd();
      }
    });
  }

  // -------------------------------------------------------------
  // 6. Transfer Logic & Consent Setup
  // -------------------------------------------------------------
  setupTransfer();
});
