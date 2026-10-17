/**
 * Easy Share - Transfer History & Activity Log UI Controller
 */

import {
  getTransferHistory,
  deleteHistoryRecord,
  clearTransferHistory,
  showInFolder,
  openFile,
  onHistoryUpdated,
} from "./tauri-bridge.js";
import { showView } from "./nav.js";
import { getIconSvg } from "./icons.js";

let cachedRecords = [];
let currentFilter = "all"; // 'all' | 'sent' | 'received'
let currentSearch = "";

const STATUS = {
  completed: { label: "Completed", icon: "circleCheck", cls: "is-completed" },
  cancelled: { label: "Cancelled", icon: "ban", cls: "is-cancelled" },
  failed: { label: "Failed", icon: "alertCircle", cls: "is-failed" },
};

function escapeHtml(str) {
  if (!str) return "";
  return String(str)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;")
    .replace(/'/g, "&#039;");
}

function formatBytes(bytes) {
  if (!bytes || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i] || "B"}`;
}

function formatClock(timestamp) {
  if (!timestamp) return "";
  return new Date(timestamp).toLocaleTimeString(undefined, { hour: "2-digit", minute: "2-digit" });
}

function dayLabel(timestamp) {
  const date = new Date(timestamp || 0);
  const today = new Date();
  const startOf = (d) => new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  const diffDays = Math.round((startOf(today) - startOf(date)) / 86400000);
  if (diffDays === 0) return "Today";
  if (diffDays === 1) return "Yesterday";
  return date.toLocaleDateString(undefined, {
    weekday: diffDays < 7 ? "long" : undefined,
    month: "short",
    day: "numeric",
    year: date.getFullYear() === today.getFullYear() ? undefined : "numeric",
  });
}

function updateCounts() {
  const counts = {
    all: cachedRecords.length,
    sent: cachedRecords.filter((r) => r.direction === "sent").length,
    received: cachedRecords.filter((r) => r.direction === "received").length,
  };
  const ids = { all: "filterAllCount", sent: "filterSentCount", received: "filterReceivedCount" };
  Object.entries(ids).forEach(([key, id]) => {
    const el = document.getElementById(id);
    if (el) el.textContent = String(counts[key]);
  });

  const settingsCount = document.getElementById("settingsHistoryCount");
  if (settingsCount) {
    const n = counts.all;
    settingsCount.textContent = `${n ? `${n} ${n === 1 ? "entry" : "entries"}` : "No entries"}. Clearing removes the log, not the files.`;
  }
}

export function setActiveFilter(filter) {
  currentFilter = filter;
  const filterBtns = document.querySelectorAll(".history-filter-btn");
  filterBtns.forEach((btn) => {
    const isActive = btn.dataset.filter === filter;
    if (isActive) {
      btn.classList.add("active");
    } else {
      btn.classList.remove("active");
    }
    btn.setAttribute("aria-pressed", String(isActive));
  });
  applyFilterAndRender();
}

export function setSearchQuery(query) {
  currentSearch = (query || "").toLowerCase().trim();
  applyFilterAndRender();
}

function applyFilterAndRender() {
  let filtered = [...cachedRecords];

  if (currentFilter === "sent") {
    filtered = filtered.filter((r) => r.direction === "sent");
  } else if (currentFilter === "received") {
    filtered = filtered.filter((r) => r.direction === "received");
  }

  if (currentSearch) {
    filtered = filtered.filter((r) => {
      const peerMatch = (r.peer_name || "").toLowerCase().includes(currentSearch);
      const fileMatch = (r.files || []).some((f) =>
        (f.name || "").toLowerCase().includes(currentSearch)
      );
      return peerMatch || fileMatch;
    });
  }

  updateCounts();
  renderHistoryList(filtered);
}

function renderRow(record) {
  const isSent = record.direction === "sent";
  const status = STATUS[record.status] || STATUS.completed;
  const files = record.files || [];
  const first = files[0];
  const title = first
    ? `${first.name}${files.length > 1 ? ` + ${files.length - 1} more` : ""}`
    : "Transfer";
  const allNames = files.map((f) => f.name).join("\n");
  const path = first && first.path ? first.path : "";
  const canOpen = record.status === "completed" && Boolean(path);
  const disabled = canOpen ? "" : "disabled";

  return `
    <div class="history-row" data-record-id="${escapeHtml(record.id)}">
      <span class="history-dir" role="img" aria-label="${isSent ? "Sent" : "Received"}" title="${isSent ? "Sent" : "Received"}">
        ${getIconSvg(isSent ? "arrowUpRight" : "arrowDownLeft", { size: 16 })}
      </span>
      <div class="history-main">
        <div class="history-title" title="${escapeHtml(allNames)}">${escapeHtml(title)}</div>
        <div class="history-peer">
          <span>${isSent ? "to" : "from"} ${escapeHtml(record.peer_name)}</span>
          ${record.peer_ip ? `<span class="mono">${escapeHtml(record.peer_ip)}</span>` : ""}
          <span class="mono history-time-inline">${formatClock(record.timestamp)}</span>
        </div>
        ${record.error_message ? `<div class="history-error">${escapeHtml(record.error_message)}</div>` : ""}
      </div>
      <span class="history-status ${status.cls}" role="img" aria-label="${status.label}" title="${status.label}">
        ${getIconSvg(status.icon, { size: 18 })}
      </span>
      <span class="history-size mono">${formatBytes(record.total_bytes)}</span>
      <span class="history-time mono">${formatClock(record.timestamp)}</span>
      <div class="history-actions">
        <button class="btn btn-icon" type="button" data-action="open-file" data-path="${escapeHtml(path)}" aria-label="Open file" title="Open file" ${disabled}>
          ${getIconSvg("externalLink", { size: 15 })}
        </button>
        <button class="btn btn-icon" type="button" data-action="show-folder" data-path="${escapeHtml(path)}" aria-label="Show in folder" title="Show in folder" ${disabled}>
          ${getIconSvg("folder", { size: 15 })}
        </button>
        <button class="btn btn-icon is-delete" type="button" data-action="delete-record" data-id="${escapeHtml(record.id)}" aria-label="Delete entry" title="Delete entry">
          ${getIconSvg("trash", { size: 15 })}
        </button>
      </div>
    </div>
  `;
}

export function renderHistoryList(records) {
  const listEl = document.getElementById("historyList");
  const emptyEl = document.getElementById("historyEmptyState");
  if (!listEl) return;

  if (!records || records.length === 0) {
    listEl.innerHTML = "";
    if (emptyEl) emptyEl.classList.remove("hidden");
    return;
  }

  if (emptyEl) emptyEl.classList.add("hidden");

  const sorted = [...records].sort((a, b) => (b.timestamp || 0) - (a.timestamp || 0));
  const groups = [];
  sorted.forEach((record) => {
    const label = dayLabel(record.timestamp);
    const last = groups[groups.length - 1];
    if (last && last.label === label) {
      last.items.push(record);
    } else {
      groups.push({ label, items: [record] });
    }
  });

  listEl.innerHTML = groups
    .map(
      (group) => `
        <section class="history-group" aria-label="${escapeHtml(group.label)}">
          <span class="eyebrow">${escapeHtml(group.label)}</span>
          <div class="list-card">${group.items.map(renderRow).join("")}</div>
        </section>
      `
    )
    .join("");
}

export async function loadAndRenderHistory() {
  try {
    cachedRecords = await getTransferHistory();
    applyFilterAndRender();
  } catch (err) {
    console.error("Failed to load transfer history:", err);
  }
}

export async function openHistoryView() {
  showView("history");
  await loadAndRenderHistory();
}

export function closeHistoryView() {
  showView("send");
}

async function handleClearHistory() {
  if (!cachedRecords.length) return;
  if (confirm("Clear all transfer history? Received files stay on disk.")) {
    try {
      await clearTransferHistory();
      cachedRecords = [];
      applyFilterAndRender();
    } catch (err) {
      console.error("Failed to clear history:", err);
    }
  }
}

export function setupHistory() {
  const openBtn = document.getElementById("openHistoryBtn");
  const searchInput = document.getElementById("historySearchInput");
  const clearBtn = document.getElementById("clearHistoryBtn");
  const settingsClearBtn = document.getElementById("settingsClearHistoryBtn");
  const settingsViewBtn = document.getElementById("settingsViewHistoryBtn");
  const listEl = document.getElementById("historyList");

  if (openBtn) {
    openBtn.addEventListener("click", () => {
      openHistoryView();
    });
  }

  if (settingsViewBtn) {
    settingsViewBtn.addEventListener("click", () => {
      openHistoryView();
    });
  }

  const filterBtns = document.querySelectorAll(".history-filter-btn");
  filterBtns.forEach((btn) => {
    btn.addEventListener("click", () => {
      const filter = btn.dataset.filter || "all";
      setActiveFilter(filter);
    });
  });

  if (searchInput) {
    searchInput.addEventListener("input", (e) => {
      setSearchQuery(e.target.value);
    });
  }

  if (clearBtn) {
    clearBtn.addEventListener("click", handleClearHistory);
  }

  if (settingsClearBtn) {
    settingsClearBtn.addEventListener("click", handleClearHistory);
  }

  if (listEl) {
    listEl.addEventListener("click", async (e) => {
      const target = e.target.closest("button");
      if (!target || target.disabled) return;

      const action = target.dataset.action;
      if (action === "delete-record") {
        const id = target.dataset.id;
        if (id) {
          try {
            await deleteHistoryRecord(id);
            cachedRecords = cachedRecords.filter((r) => r.id !== id);
            applyFilterAndRender();
          } catch (err) {
            console.error("Failed to delete record:", err);
          }
        }
      } else if (action === "show-folder") {
        const path = target.dataset.path;
        if (path) {
          try {
            await showInFolder(path);
          } catch (err) {
            console.error("Failed to show in folder:", err);
          }
        }
      } else if (action === "open-file") {
        const path = target.dataset.path;
        if (path) {
          try {
            await openFile(path);
          } catch (err) {
            console.error("Failed to open file:", err);
          }
        }
      }
    });
  }

  // Subscribe to backend history-updated events
  onHistoryUpdated((updated) => {
    if (Array.isArray(updated)) {
      cachedRecords = updated;
      applyFilterAndRender();
    }
  });

  // Populate counts (settings page, filter pills) at startup
  loadAndRenderHistory();
}
