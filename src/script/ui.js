/**
 * Easy Share - UI Rendering and View Components
 * Powered by Lucide SVG Icons
 */

import { formatFileSize } from "./files.js";
import { getPeerKey } from "./peers.js";
import { getDeviceIconSvg, getIconSvg } from "./icons.js";

/**
 * Escapes unsafe characters for HTML rendering
 * @param {string} str
 * @returns {string}
 */
export function escapeHtml(str) {
  if (typeof str !== "string") return "";
  return str
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

/**
 * Resolves appropriate Lucide SVG icon for a device
 * @param {string} os
 * @param {string} deviceType
 * @returns {string} SVG HTML string
 */
export function getDeviceIcon(os = "", deviceType = "") {
  return getDeviceIconSvg(os, deviceType, { size: 32 });
}

/**
 * Renders the discovered peers radar grid
 * @param {Array<Object>} peers
 * @param {Function} onSelectPeer
 * @param {Object|null} selectedPeer
 */
export function renderPeers(peers, onSelectPeer, selectedPeer = null) {
  const deviceList = document.getElementById("deviceList");
  const countEl = document.getElementById("deviceCount");

  if (countEl) {
    countEl.textContent = `${peers.length} ${peers.length === 1 ? "device" : "devices"} found`;
  }

  if (!deviceList) return;
  deviceList.innerHTML = "";

  if (peers.length === 0) {
    const loader = document.createElement("div");
    loader.className = "loader";
    loader.textContent = "Searching for nearby devices...";
    deviceList.appendChild(loader);
    return;
  }

  const selectedKey = selectedPeer ? getPeerKey(selectedPeer) : null;

  peers.forEach((peer) => {
    const isSelected = selectedKey === getPeerKey(peer);
    const iconSvg = getDeviceIcon(peer.os, peer.device_type);
    const peerName = peer.device_name || peer.name || "Unknown Device";
    const peerIp = peer.ip || peer.host || "0.0.0.0";
    const peerPort = peer.port || 5050;

    const div = document.createElement("div");
    div.className = `device${isSelected ? " selected" : ""}`;
    div.setAttribute("tabindex", "0");
    div.setAttribute("role", "button");
    div.setAttribute("aria-pressed", isSelected ? "true" : "false");
    div.setAttribute("aria-label", `Select device ${peerName}`);

    div.innerHTML = `
      <div class="device-icon">${iconSvg}</div>
      <div class="device-name" title="${escapeHtml(peerName)}">${escapeHtml(peerName)}</div>
      <div class="device-meta">
        <span class="device-ip">${escapeHtml(peerIp)}:${peerPort}</span>
        ${peer.room_id ? `<span class="room-badge">${escapeHtml(peer.room_id)}</span>` : ""}
        ${peer.os ? `<span class="os-badge">${escapeHtml(peer.os)}</span>` : ""}
      </div>
      <div class="checkmark">${getIconSvg("check", { size: 14 })}</div>
    `;

    // Click & Keyboard Enter/Space selection
    div.addEventListener("click", () => {
      onSelectPeer(peer);
    });

    div.addEventListener("keydown", (e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        onSelectPeer(peer);
      }
    });

    deviceList.appendChild(div);
  });
}

/**
 * Updates the file list in the UI
 * @param {Array<Object>} files
 * @param {Function} removeFileCb
 */
export function updateFileList(files, removeFileCb) {
  const fileList = document.getElementById("fileList");
  if (!fileList) return;

  fileList.innerHTML = "";

  files.forEach((file, index) => {
    const item = document.createElement("div");
    item.className = "file-item";
    item.id = `file-${index}`;
    item.style.setProperty("--progress", "0%");

    item.innerHTML = `
      <div class="file-info">
        <div class="file-icon-box">${getIconSvg("fileText", { size: 18 })}</div>
        <span class="file-name" title="${escapeHtml(file.name)}">${escapeHtml(file.name)}</span>
        <span class="file-size">${formatFileSize(file.size)}</span>
        <button class="remove-btn" data-index="${index}" aria-label="Remove ${escapeHtml(file.name)}">
          ${getIconSvg("x", { size: 14 })}
        </button>
      </div>
    `;

    fileList.appendChild(item);
  });

  // Attach remove handlers
  fileList.querySelectorAll(".remove-btn").forEach((btn) => {
    btn.addEventListener("click", (e) => {
      e.stopPropagation();
      const target = e.currentTarget;
      removeFileCb(target.dataset.index);
    });
  });
}

/**
 * Updates send button based on selected files count and total size
 * @param {Array<Object>} files
 */
export function updateFilesTotalSize(files) {
  const btn = document.querySelector(".send-btn");
  if (!btn) return;

  const countEl = btn.querySelector(".send-count");
  const sizeEl = btn.querySelector(".send-size");

  if (!files || files.length === 0) {
    btn.disabled = true;
    if (countEl) countEl.textContent = "0 files";
    if (sizeEl) sizeEl.textContent = "0 MB";
    return;
  }

  const totalBytes = files.reduce((sum, f) => sum + (Number(f.size) || 0), 0);

  btn.disabled = false;
  if (countEl) {
    countEl.textContent = `${files.length} ${files.length === 1 ? "file" : "files"}`;
  }
  if (sizeEl) {
    sizeEl.textContent = formatFileSize(totalBytes);
  }
}

/**
 * Updates local device identity badge and room tag
 * @param {Object} deviceInfo
 */
export function updateMyDeviceBadge(deviceInfo) {
  if (!deviceInfo) return;

  const iconEl = document.getElementById("myDeviceIcon");
  const nameEl = document.getElementById("myDeviceName");
  const roomEl = document.getElementById("myDeviceRoom");
  const tagEl = document.getElementById("currentRoomTag");

  const deviceName = deviceInfo.device_name || "Local Device";
  const roomId = deviceInfo.room_id ? deviceInfo.room_id : "Public";

  if (iconEl) {
    iconEl.innerHTML = getDeviceIconSvg(deviceInfo.os, deviceInfo.device_type, { size: 20 });
  }
  if (nameEl) {
    nameEl.textContent = deviceName;
  }
  if (roomEl) {
    roomEl.textContent = `Room: ${roomId}`;
  }
  if (tagEl) {
    tagEl.textContent = deviceInfo.room_id ? deviceInfo.room_id : "Public (Default)";
  }
}

/**
 * Displays a non-blocking toast notification with Lucide icon
 * @param {string} message
 * @param {"info"|"success"|"error"} type
 */
export function showToast(message, type = "info") {
  const container = document.getElementById("toast-container");
  if (!container) return;

  const toast = document.createElement("div");
  toast.className = `toast ${type}`;

  let iconName = "info";
  if (type === "success") iconName = "checkCircle";
  if (type === "error") iconName = "alertCircle";

  toast.innerHTML = `
    <span class="toast-icon">${getIconSvg(iconName, { size: 18 })}</span>
    <span class="toast-text">${escapeHtml(message)}</span>
  `;

  container.appendChild(toast);

  setTimeout(() => {
    toast.remove();
  }, 3500);
}

// Active Consent Modal Resolver reference
let activeConsentResolver = null;

/**
 * Shows interactive Consent Modal for incoming file transfer requests
 * @param {Object} request - TransferRequest payload
 * @returns {Promise<boolean>} Resolves to true if accepted, false if declined
 */
export function showConsentModal(request) {
  const modal = document.getElementById("consent-modal");
  if (!modal) return Promise.resolve(false);

  // If a previous consent prompt is open, decline it
  if (activeConsentResolver) {
    activeConsentResolver(false);
    activeConsentResolver = null;
  }

  const iconEl = document.getElementById("consentSenderIcon");
  const senderInfoEl = document.getElementById("consentSenderInfo");
  const countEl = document.getElementById("consentFileCount");
  const sizeEl = document.getElementById("consentTotalSize");
  const roomBadgeEl = document.getElementById("consentRoomBadge");
  const fileListEl = document.getElementById("consentFileList");
  const acceptBtn = document.getElementById("consentAcceptBtn");
  const declineBtn = document.getElementById("consentDeclineBtn");

  const files = request.files || [];
  const senderName = request.sender_name || "Remote Device";
  const senderOs = request.sender_os ? ` (${request.sender_os})` : "";
  const totalBytes = Number(request.total_bytes) || 0;

  if (iconEl) {
    iconEl.innerHTML = getDeviceIconSvg(request.sender_os, "", { size: 28 });
  }
  if (senderInfoEl) {
    senderInfoEl.textContent = `${senderName}${senderOs}`;
  }
  if (countEl) {
    countEl.textContent = `${files.length} ${files.length === 1 ? "file" : "files"}`;
  }
  if (sizeEl) {
    sizeEl.textContent = formatFileSize(totalBytes);
  }

  if (roomBadgeEl) {
    if (request.room_id) {
      roomBadgeEl.textContent = request.room_id;
      roomBadgeEl.style.display = "inline-block";
    } else {
      roomBadgeEl.style.display = "none";
    }
  }

  if (fileListEl) {
    fileListEl.innerHTML = "";
    files.forEach((file) => {
      const row = document.createElement("div");
      row.className = "modal-file-row";
      row.innerHTML = `
        <div class="modal-file-left">
          <span class="modal-file-icon">${getIconSvg("fileText", { size: 14 })}</span>
          <span class="modal-file-name" title="${escapeHtml(file.name)}">${escapeHtml(file.name)}</span>
        </div>
        <span class="modal-file-size">${formatFileSize(file.size)}</span>
      `;
      fileListEl.appendChild(row);
    });
  }

  // Show modal
  modal.classList.remove("hidden");
  modal.setAttribute("aria-hidden", "false");

  return new Promise((resolve) => {
    activeConsentResolver = resolve;

    const cleanup = () => {
      modal.classList.add("hidden");
      modal.setAttribute("aria-hidden", "true");
      acceptBtn.removeEventListener("click", onAccept);
      declineBtn.removeEventListener("click", onDecline);
      window.removeEventListener("keydown", onKey);
      activeConsentResolver = null;
    };

    const onAccept = () => {
      cleanup();
      resolve(true);
    };

    const onDecline = () => {
      cleanup();
      resolve(false);
    };

    const onKey = (e) => {
      if (e.key === "Escape") {
        onDecline();
      }
    };

    acceptBtn.addEventListener("click", onAccept);
    declineBtn.addEventListener("click", onDecline);
    window.addEventListener("keydown", onKey);
  });
}
