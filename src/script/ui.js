/**
 * Easy Share - UI Rendering and View Components
 * Powered by Lucide SVG Icons
 */

import { formatFileSize } from "./files.js";
import { getPeerKey } from "./peers.js";
import { getDeviceIconSvg, getIconSvg } from "./icons.js";
import { getDownloadDir, pickFolder, setDownloadDir } from "./tauri-bridge.js";

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
  return getDeviceIconSvg(os, deviceType, { size: 17 });
}

const FILE_TYPES = [
  { cls: "type-archive", label: "Archive", exts: ["zip", "rar", "7z", "tar", "gz", "tgz", "bz2", "xz", "iso"] },
  { cls: "type-video", label: "Video", exts: ["mp4", "mov", "mkv", "avi", "webm", "wmv", "m4v"] },
  { cls: "type-image", label: "Image", exts: ["jpg", "jpeg", "png", "gif", "webp", "heic", "bmp", "svg", "tiff"] },
  { cls: "type-audio", label: "Audio", exts: ["mp3", "wav", "flac", "aac", "ogg", "m4a"] },
  { cls: "type-doc", label: "Document", exts: ["pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "txt", "md", "csv", "rtf"] },
];

/**
 * Resolves the badge label, tint class and category for a file name
 * @param {string} name
 * @returns {{ext: string, cls: string, label: string}}
 */
export function getFileType(name = "") {
  const dot = name.lastIndexOf(".");
  const ext = dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
  const match = FILE_TYPES.find((t) => t.exts.includes(ext));
  return {
    ext: ext ? ext.slice(0, 4).toUpperCase() : "FILE",
    cls: match ? match.cls : "",
    label: match ? match.label : "File",
  };
}

/**
 * Renders a file-type badge
 * @param {string} name
 * @param {string} [extraClass]
 * @returns {string}
 */
export function renderTypeBadge(name, extraClass = "") {
  const type = getFileType(name);
  return `<div class="type-badge mono ${type.cls} ${extraClass}">${escapeHtml(type.ext)}</div>`;
}

// Live activity text per peer ("Receiving · 92 MB/s"), keyed by peer key or name
const peerActivity = new Map();

function findActivity(peer) {
  const name = peer.device_name || peer.name || "";
  return peerActivity.get(getPeerKey(peer)) || (name ? peerActivity.get(`name:${name}`) : null) || null;
}

function peerMetaHtml(peer) {
  const activity = findActivity(peer);
  if (activity) {
    return `<span class="device-activity">${escapeHtml(activity)}</span>`;
  }
  const peerIp = peer.ip || peer.host || "0.0.0.0";
  return `
    <span class="device-meta">
      ${peer.os ? `<span>${escapeHtml(formatOs(peer.os))}</span>` : ""}
      <span class="mono">${escapeHtml(peerIp)}</span>
    </span>`;
}

/**
 * Converts raw OS identifiers into display names
 * @param {string} os
 * @returns {string}
 */
export function formatOs(os = "") {
  const lower = String(os).toLowerCase();
  if (lower.includes("windows")) return "Windows";
  if (lower.includes("mac") || lower.includes("darwin")) return "macOS";
  if (lower.includes("android")) return "Android";
  if (lower.includes("ios")) return "iOS";
  if (lower.includes("linux")) return "Linux";
  return os;
}

/**
 * Sets or clears the live activity line shown on a peer card
 * @param {{key?: string, name?: string}} target
 * @param {string|null} text
 */
export function setPeerActivity(target, text) {
  if (!target) return;
  const mapKey = target.key || (target.name ? `name:${target.name}` : null);
  if (!mapKey) return;

  if (text) {
    peerActivity.set(mapKey, text);
  } else {
    peerActivity.delete(mapKey);
  }

  if (typeof document === "undefined" || !document.querySelectorAll) return;
  document.querySelectorAll(".device").forEach((row) => {
    const matches = target.key
      ? row.dataset.peerKey === target.key
      : row.dataset.peerName === target.name;
    if (!matches) return;
    row.classList.toggle("is-busy", Boolean(text));
    const slot = row.querySelector(".device-sub");
    if (slot && row._peer) {
      slot.innerHTML = peerMetaHtml(row._peer);
    }
  });
}

/**
 * Renders the discovered peers list
 * @param {Array<Object>} peers
 * @param {Function} onSelectPeer
 * @param {Object|null} selectedPeer
 */
export function renderPeers(peers, onSelectPeer, selectedPeer = null) {
  const deviceList = document.getElementById("deviceList");
  const countEl = document.getElementById("deviceCount");

  if (countEl) {
    countEl.textContent = String(peers.length);
  }

  updateSendTarget(selectedPeer);

  if (!deviceList) return;
  deviceList.innerHTML = "";

  if (peers.length === 0) {
    const empty = document.createElement("div");
    empty.className = "device-empty";
    empty.textContent = "Searching for nearby devices…";
    deviceList.appendChild(empty);
    return;
  }

  const selectedKey = selectedPeer ? getPeerKey(selectedPeer) : null;

  peers.forEach((peer) => {
    const key = getPeerKey(peer);
    const isSelected = selectedKey === key;
    const peerName = peer.device_name || peer.name || "Unknown Device";

    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = `device${isSelected ? " selected" : ""}${findActivity(peer) ? " is-busy" : ""}`;
    btn.dataset.peerKey = key;
    btn.dataset.peerName = peerName;
    btn.setAttribute("aria-pressed", isSelected ? "true" : "false");
    btn.title = `${peerName} · ${key}`;
    btn._peer = peer;

    btn.innerHTML = `
      <span class="device-icon">${getDeviceIcon(peer.os, peer.device_type)}</span>
      <span class="device-details">
        <span class="device-name">${escapeHtml(peerName)}</span>
        <span class="device-sub">${peerMetaHtml(peer)}</span>
      </span>
      <span class="device-check">${getIconSvg("circleCheck", { size: 18 })}</span>
    `;

    btn.addEventListener("click", () => {
      onSelectPeer(peer);
    });

    deviceList.appendChild(btn);
  });
}

/**
 * Updates the "To" chip in the send bar
 * @param {Object|null} peer
 */
export function updateSendTarget(peer) {
  const chip = document.getElementById("sendTargetChip");
  const nameEl = document.getElementById("sendTargetName");
  if (!chip || !nameEl) return;
  if (peer) {
    nameEl.textContent = peer.device_name || peer.name || peer.ip || "Device";
    chip.classList.remove("is-empty");
  } else {
    nameEl.textContent = "Pick a device";
    chip.classList.add("is-empty");
  }
}

/**
 * Updates the staged file list in the UI
 * @param {Array<Object>} files
 * @param {Function} removeFileCb
 */
export function updateFileList(files, removeFileCb) {
  if (typeof document === "undefined") return;
  const fileList = document.getElementById("fileList");
  if (!fileList) return;

  fileList.innerHTML = "";

  files.forEach((file, index) => {
    const fileId = file.id || `file-${index}`;
    const type = getFileType(file.name);
    const item = document.createElement("div");
    item.className = "file-row file-item";
    item.id = `file-${fileId}`;
    item.dataset.fileId = fileId;
    item.style.setProperty("--progress", "0%");

    item.innerHTML = `
      ${renderTypeBadge(file.name)}
      <div class="file-main">
        <span class="file-name" title="${escapeHtml(file.relativePath || file.fullPath || file.name)}">${escapeHtml(file.name)}</span>
        <span class="file-detail">${escapeHtml(type.label)}${file.relativePath ? ` · ${escapeHtml(file.relativePath)}` : ""}</span>
        <div class="file-bar"><div class="file-bar-fill"></div></div>
      </div>
      <span class="file-size mono">${formatFileSize(file.size)}</span>
      <span class="file-state">Queued</span>
      <button class="btn btn-icon is-delete remove-btn" type="button" data-file-id="${escapeHtml(fileId)}" data-index="${index}" aria-label="Remove ${escapeHtml(file.name)}">
        ${getIconSvg("x", { size: 14 })}
      </button>
    `;

    fileList.appendChild(item);
  });

  // Attach remove handlers
  fileList.querySelectorAll(".remove-btn").forEach((btn) => {
    btn.addEventListener("click", (e) => {
      e.stopPropagation();
      const target = e.currentTarget;
      removeFileCb(target.dataset.fileId || target.dataset.index);
    });
  });

  const sendView = document.getElementById("sendView");
  if (sendView) {
    sendView.classList.toggle("has-files", files.length > 0);
  }
  const label = document.getElementById("stagedLabel");
  if (label && !(sendView && sendView.classList.contains("is-sending"))) {
    label.textContent = `Staged · ${files.length} ${files.length === 1 ? "item" : "items"}`;
  }
}

/**
 * Updates the send bar totals and Send button for the staged files
 * @param {Array<Object>} files
 */
export function updateFilesTotalSize(files) {
  const btn = document.getElementById("sendBtn");
  const labelEl = document.getElementById("sendBtnLabel");
  const totalEl = document.getElementById("sendTotal");
  const count = files ? files.length : 0;
  const totalBytes = count ? files.reduce((sum, f) => sum + (Number(f.size) || 0), 0) : 0;

  if (btn) btn.disabled = count === 0;
  if (labelEl) {
    labelEl.textContent = count ? `Send ${count} ${count === 1 ? "item" : "items"}` : "Send";
  }
  if (totalEl) {
    totalEl.textContent = `${count} ${count === 1 ? "file" : "files"} · ${formatFileSize(totalBytes)}`;
  }
}

/**
 * Updates local device identity card
 * @param {Object} deviceInfo
 */
export function updateMyDeviceBadge(deviceInfo) {
  if (!deviceInfo) return;

  const iconEl = document.getElementById("myDeviceIcon");
  const nameEl = document.getElementById("myDeviceName");

  if (iconEl) {
    iconEl.innerHTML = getDeviceIconSvg(deviceInfo.os, deviceInfo.device_type, { size: 16 });
  }
  if (nameEl) {
    nameEl.textContent = deviceInfo.device_name || "Local Device";
  }
}

/**
 * Shows this device's reachable address on the device card and settings page
 * @param {string} ip
 * @param {number} port
 */
export function updateMyAddress(ip, port) {
  const addr = ip ? `${ip}:${port || 5050}` : "—";
  const cardEl = document.getElementById("myDeviceAddress");
  const settingsEl = document.getElementById("settingsLocalAddress");
  const portEl = document.getElementById("settingsPort");
  if (cardEl) cardEl.textContent = addr;
  if (settingsEl) settingsEl.textContent = ip || "—";
  if (portEl) portEl.textContent = String(port || 5050);
}

/**
 * Displays a non-blocking toast notification
 * @param {string} message
 * @param {"info"|"success"|"error"} type
 * @param {{title?: string, actions?: Array<{label: string, onClick: Function}>, duration?: number}} [options]
 */
export function showToast(message, type = "info", options = {}) {
  const container = document.getElementById("toast-container");
  if (!container) return;

  const toast = document.createElement("div");
  toast.className = `toast ${type}`;
  toast.setAttribute("role", type === "error" ? "alert" : "status");

  let iconName = "info";
  if (type === "success") iconName = "check";
  if (type === "error") iconName = "alertCircle";

  const title = options.title ? `<div class="toast-title">${escapeHtml(options.title)}</div>` : "";
  const actions = options.actions || [];

  toast.innerHTML = `
    <span class="toast-icon">${getIconSvg(iconName, { size: 16 })}</span>
    <div class="toast-body">
      <div class="toast-top">
        ${title ? `<div>${title}<div class="toast-text">${escapeHtml(message)}</div></div>` : `<div class="toast-text">${escapeHtml(message)}</div>`}
        <button class="btn btn-icon toast-close" type="button" aria-label="Dismiss">${getIconSvg("x", { size: 13 })}</button>
      </div>
      ${actions.length ? `<div class="toast-actions">${actions
        .map((a, i) => `<button class="btn btn-ghost btn-sm" type="button" data-action-index="${i}">${escapeHtml(a.label)}</button>`)
        .join("")}</div>` : ""}
    </div>
  `;

  const dismiss = () => toast.remove();

  if (toast.querySelector) {
    const closeBtn = toast.querySelector(".toast-close");
    if (closeBtn) closeBtn.addEventListener("click", dismiss);
    toast.querySelectorAll("[data-action-index]").forEach((btn) => {
      btn.addEventListener("click", () => {
        const action = actions[Number(btn.dataset.actionIndex)];
        if (action && action.onClick) action.onClick();
        dismiss();
      });
    });
  }

  container.appendChild(toast);

  setTimeout(dismiss, options.duration || (actions.length ? 8000 : 3500));
}

// -------------------------------------------------------------
// Incoming transfer progress card
// -------------------------------------------------------------
let receiveContext = null;
let receiveBytesByFile = new Map();
let receiveCancelHandler = null;

/**
 * Stores details of an accepted incoming transfer for the progress card
 * @param {{senderName: string, senderOs?: string, fileCount: number, totalBytes: number, saveDir?: string}|null} ctx
 */
export function setReceiveContext(ctx) {
  receiveContext = ctx;
  receiveBytesByFile = new Map();
}

/**
 * Returns details of the accepted incoming transfer, or null
 * @returns {{senderName: string, senderOs?: string, fileCount: number, totalBytes: number, saveDir?: string}|null}
 */
export function getReceiveContext() {
  return receiveContext;
}

/**
 * Registers the handler called when the user cancels an incoming transfer
 * @param {(sessionToken: string) => void} handler
 */
export function setReceiveCancelHandler(handler) {
  receiveCancelHandler = handler;
}

function buildReceiveCard(card) {
  card.innerHTML = `
    <div class="progress-head">
      <span class="progress-icon">${getIconSvg("download", { size: 18 })}</span>
      <div class="progress-titles">
        <div class="progress-title" data-role="title"></div>
        <div class="progress-subtitle" data-role="subtitle"></div>
      </div>
      <div class="progress-actions">
        <button class="btn btn-ghost btn-danger" type="button" data-role="cancel" id="cancelReceiveBtn">Cancel</button>
      </div>
    </div>
    <div class="progress-metrics">
      <div class="progress-percent mono"><span data-role="percent">0</span><span class="progress-percent-sign">%</span></div>
      <div class="metric-grid">
        <div><div class="eyebrow">Speed</div><div class="metric-value mono" data-role="speed">—</div></div>
        <div><div class="eyebrow">Remaining</div><div class="metric-value mono" data-role="eta">—</div></div>
        <div><div class="eyebrow">Received</div><div class="metric-value mono" data-role="bytes">—</div></div>
      </div>
    </div>
    <div class="progress-bar" role="progressbar" aria-label="Receive progress" aria-valuemin="0" aria-valuemax="100" data-role="bar">
      <div class="progress-fill" data-role="fill"></div>
    </div>
  `;
  const cancelBtn = card.querySelector('[data-role="cancel"]');
  if (cancelBtn) {
    cancelBtn.addEventListener("click", () => {
      if (receiveCancelHandler && card.dataset.sessionToken) {
        receiveCancelHandler(card.dataset.sessionToken);
      }
    });
  }
  card.dataset.built = "true";
}

function setRole(card, role, text) {
  const el = card.querySelector(`[data-role="${role}"]`);
  if (el) el.textContent = text;
  return el;
}

/**
 * Formats a byte pair as "1.25 / 1.84 GB" using the larger value's unit
 * @param {number} done
 * @param {number} total
 * @returns {{done: string, total: string}}
 */
export function formatBytePair(done, total) {
  const totalLabel = formatFileSize(total);
  const unit = totalLabel.split(" ")[1] || "B";
  const div = { B: 1, KB: 1024, MB: 1024 ** 2, GB: 1024 ** 3 }[unit] || 1;
  const value = (Number(done) || 0) / div;
  return { done: unit === "B" ? String(Math.round(value)) : value.toFixed(unit === "GB" ? 2 : 1), total: totalLabel };
}

/**
 * Updates the incoming transfer progress card
 * @param {Object} progress - { session_token, file_id, file_name, percent, received_bytes, total_bytes, speed_bps, eta_seconds }
 */
export function updateReceiveProgress(progress) {
  const card = document.getElementById("receiveCard");
  if (!card) return;

  const { file_id, file_name, percent, received_bytes, total_bytes, speed_bps, eta_seconds, session_token } = progress;

  if (card.dataset.built !== "true") {
    buildReceiveCard(card);
  }
  if (session_token) card.dataset.sessionToken = session_token;

  receiveBytesByFile.set(file_id || file_name || "file", Number(received_bytes) || 0);
  const receivedTotal = Array.from(receiveBytesByFile.values()).reduce((a, b) => a + b, 0);
  const batchTotal = receiveContext && receiveContext.totalBytes ? receiveContext.totalBytes : Number(total_bytes) || 0;
  const overall = batchTotal > 0 ? Math.min(100, Math.floor((receivedTotal / batchTotal) * 100)) : Number(percent) || 0;

  const sender = receiveContext ? receiveContext.senderName : "nearby device";
  const fileCount = receiveContext ? receiveContext.fileCount : 1;
  const saveDir = receiveContext && receiveContext.saveDir ? receiveContext.saveDir : "";
  const speedText = formatSpeed(speed_bps || 0);

  setRole(card, "title", `Receiving from ${sender}`);
  const subtitle = card.querySelector('[data-role="subtitle"]');
  if (subtitle) {
    subtitle.innerHTML = `${fileCount} ${fileCount === 1 ? "file" : "files"} · ${
      saveDir ? `saving to <span class="mono">${escapeHtml(saveDir)}</span>` : escapeHtml(file_name || "")
    }`;
  }
  setRole(card, "percent", String(overall));
  setRole(card, "speed", speedText);
  setRole(card, "eta", formatEtaShort(eta_seconds));
  const bytesEl = card.querySelector('[data-role="bytes"]');
  if (bytesEl) {
    const pair = formatBytePair(receivedTotal, batchTotal);
    bytesEl.innerHTML = `${pair.done} <span class="metric-total">/ ${pair.total}</span>`;
  }
  const bar = card.querySelector('[data-role="bar"]');
  if (bar) bar.setAttribute("aria-valuenow", String(overall));
  const fill = card.querySelector('[data-role="fill"]');
  if (fill) fill.style.width = `${overall}%`;

  card.classList.remove("hidden");
  setNavProgress(overall);

  if (receiveContext) {
    setPeerActivity({ name: receiveContext.senderName }, `Sending to you · ${speedText}`);
  }
}

/**
 * Hides and resets the incoming transfer progress card
 */
export function hideReceiveProgress() {
  const card = document.getElementById("receiveCard");
  if (card) {
    card.classList.add("hidden");
    card.innerHTML = "";
    card.dataset.built = "";
    card.dataset.sessionToken = "";
  }
  if (receiveContext) {
    setPeerActivity({ name: receiveContext.senderName }, null);
  }
  receiveContext = null;
  receiveBytesByFile = new Map();
  setNavProgress(null);
}

/**
 * Shows or hides the progress percentage next to the Send nav item
 * @param {number|null} percent
 */
export function setNavProgress(percent) {
  const badge = document.getElementById("navSendProgress");
  if (!badge) return;
  if (percent === null || percent === undefined) {
    badge.classList.add("hidden");
    badge.textContent = "";
  } else {
    badge.classList.remove("hidden");
    badge.textContent = `${percent}%`;
  }
}

// -------------------------------------------------------------
// Incoming transfer consent dialog
// -------------------------------------------------------------
let activeConsentResolver = null;

/**
 * Shows interactive consent dialog for incoming file transfer requests
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
  const titleEl = document.getElementById("consentModalTitle");
  const senderInfoEl = document.getElementById("consentSenderInfo");
  const countEl = document.getElementById("consentFileCount");
  const sizeEl = document.getElementById("consentTotalSize");
  const fileListEl = document.getElementById("consentFileList");
  const savePathEl = document.getElementById("consentSavePath");
  const changeDirBtn = document.getElementById("consentChangeDirBtn");
  const acceptBtn = document.getElementById("consentAcceptBtn");
  const declineBtn = document.getElementById("consentDeclineBtn");

  const files = request.files || [];
  const senderName = request.sender_name || "Remote device";
  const totalBytes = Number(request.total_bytes) || 0;
  const countLabel = `${files.length} ${files.length === 1 ? "file" : "files"}`;

  if (iconEl) {
    iconEl.innerHTML = getDeviceIconSvg(request.sender_os, "", { size: 20 });
  }
  if (titleEl) {
    titleEl.textContent = `${senderName} wants to send you ${countLabel}`;
  }
  if (senderInfoEl) {
    senderInfoEl.innerHTML = request.sender_os ? `<span>${escapeHtml(formatOs(request.sender_os))}</span>` : "";
  }
  if (countEl) countEl.textContent = countLabel;
  if (sizeEl) sizeEl.textContent = formatFileSize(totalBytes);

  if (fileListEl) {
    fileListEl.innerHTML = "";
    files.forEach((file) => {
      const row = document.createElement("div");
      row.className = "file-row";
      row.innerHTML = `
        ${renderTypeBadge(file.name, "sm")}
        <div class="file-main"><span class="file-name" title="${escapeHtml(file.name)}">${escapeHtml(file.name)}</span></div>
        <span class="file-size mono">${formatFileSize(file.size)}</span>
      `;
      fileListEl.appendChild(row);
    });
  }

  if (savePathEl) {
    getDownloadDir()
      .then((dir) => {
        savePathEl.textContent = dir || "Downloads";
        savePathEl.title = dir || "";
      })
      .catch(() => {});
  }

  const onChangeDir = async () => {
    try {
      const picked = await pickFolder();
      let selectedPath = null;
      if (typeof picked === "string" && picked.trim()) {
        selectedPath = picked.trim();
      } else if (Array.isArray(picked) && picked.length > 0) {
        selectedPath = picked[0].path || picked[0];
      }
      if (selectedPath && savePathEl) {
        const newPath = await setDownloadDir(selectedPath);
        savePathEl.textContent = newPath;
        savePathEl.title = newPath;
      }
    } catch (err) {
      showToast(`Failed to update download folder: ${err}`, "error");
    }
  };

  // Show modal
  modal.classList.remove("hidden");
  modal.setAttribute("aria-hidden", "false");
  if (acceptBtn && acceptBtn.focus) acceptBtn.focus();

  return new Promise((resolve) => {
    activeConsentResolver = resolve;

    const cleanup = () => {
      modal.classList.add("hidden");
      modal.setAttribute("aria-hidden", "true");
      acceptBtn.removeEventListener("click", onAccept);
      declineBtn.removeEventListener("click", onDecline);
      if (changeDirBtn) changeDirBtn.removeEventListener("click", onChangeDir);
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
    if (changeDirBtn) changeDirBtn.addEventListener("click", onChangeDir);
    window.addEventListener("keydown", onKey);
  });
}

/**
 * Formats transfer speed in bytes per second into human-readable units.
 * @param {number} bytesPerSec
 * @returns {string}
 */
export function formatSpeed(bytesPerSec) {
  const b = Number(bytesPerSec) || 0;
  if (b >= 1024 ** 3) {
    return (b / 1024 ** 3).toFixed(1) + " GB/s";
  }
  if (b >= 1024 ** 2) {
    return (b / 1024 ** 2).toFixed(1) + " MB/s";
  }
  if (b >= 1024) {
    return (b / 1024).toFixed(1) + " KB/s";
  }
  return Math.round(b) + " B/s";
}

/**
 * Formats estimated time to completion (ETA in seconds) into human-readable text.
 * @param {number|null|undefined} seconds
 * @returns {string}
 */
export function formatEta(seconds) {
  if (seconds === null || seconds === undefined) {
    return "--";
  }
  const s = Math.round(Number(seconds));
  if (isNaN(s) || s < 0) {
    return "--";
  }
  if (s === 0) {
    return "0s";
  }
  if (s < 60) {
    return `${s}s remaining`;
  }
  const mins = Math.floor(s / 60);
  const remainingSecs = s % 60;
  if (mins < 60) {
    return remainingSecs > 0 ? `${mins}m ${remainingSecs}s remaining` : `${mins}m remaining`;
  }
  const hours = Math.floor(mins / 60);
  const remainingMins = mins % 60;
  return remainingMins > 0 ? `${hours}h ${remainingMins}m remaining` : `${hours}h remaining`;
}

/**
 * Formats an ETA for a metric cell that is already labelled "Remaining".
 * @param {number|null|undefined} seconds
 * @returns {string}
 */
export function formatEtaShort(seconds) {
  const text = formatEta(seconds);
  return text === "--" ? "—" : text.replace(" remaining", "");
}
