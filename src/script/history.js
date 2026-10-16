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

let cachedRecords = [];
let currentFilter = "all"; // 'all' | 'sent' | 'received'
let currentSearch = "";

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

function formatTime(timestamp) {
  if (!timestamp) return "";
  const date = new Date(timestamp);
  const now = new Date();
  const diffSec = Math.floor((now - date) / 1000);

  if (diffSec < 60) return "Just now";
  if (diffSec < 3600) return `${Math.floor(diffSec / 60)}m ago`;
  if (diffSec < 86400) return `${Math.floor(diffSec / 3600)}h ago`;
  return date.toLocaleDateString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

export function setActiveFilter(filter) {
  currentFilter = filter;
  const filterBtns = document.querySelectorAll(".history-filter-btn");
  filterBtns.forEach((btn) => {
    if (btn.dataset.filter === filter) {
      btn.classList.add("active");
    } else {
      btn.classList.remove("active");
    }
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

  renderHistoryList(filtered);
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

  const html = records
    .map((record) => {
      const isSent = record.direction === "sent";
      const dirBadgeClass = isSent ? "badge-sent" : "badge-received";
      const dirLabel = isSent ? "Sent" : "Received";
      const dirIcon = isSent ? "↗" : "↙";

      let statusBadgeClass = "badge-status-completed";
      let statusLabel = "Completed";
      if (record.status === "cancelled") {
        statusBadgeClass = "badge-status-cancelled";
        statusLabel = "Cancelled";
      } else if (record.status === "failed") {
        statusBadgeClass = "badge-status-failed";
        statusLabel = "Failed";
      }

      const filesHtml = (record.files || [])
        .map((f) => {
          const hasPath = Boolean(f.path);
          return `
            <div class="history-file-row">
              <span class="history-file-name" title="${escapeHtml(f.name)}">
                📄 ${escapeHtml(f.name)}
              </span>
              <span class="history-file-size">${formatBytes(f.size)}</span>
              ${
                hasPath
                  ? `
                <div class="history-file-actions">
                  <button class="btn-history-file-action" data-action="open-file" data-path="${escapeHtml(f.path)}" title="Open file">
                    Open
                  </button>
                  <button class="btn-history-file-action" data-action="show-folder" data-path="${escapeHtml(f.path)}" title="Show in folder">
                    Folder
                  </button>
                </div>
              `
                  : ""
              }
            </div>
          `;
        })
        .join("");

      return `
        <div class="history-card" data-record-id="${escapeHtml(record.id)}">
          <div class="history-card-header">
            <div class="history-card-meta">
              <span class="history-badge ${dirBadgeClass}">${dirIcon} ${dirLabel}</span>
              <span class="history-peer-name" title="${escapeHtml(record.peer_name)}">${escapeHtml(record.peer_name)}</span>
              <span class="history-badge ${statusBadgeClass}">${statusLabel}</span>
            </div>
            <div class="history-card-right">
              <span class="history-time">${formatTime(record.timestamp)}</span>
              <button class="btn-history-delete" data-action="delete-record" data-id="${escapeHtml(record.id)}" title="Delete record">
                &times;
              </button>
            </div>
          </div>
          <div class="history-card-files">
            ${filesHtml}
          </div>
          <div class="history-card-footer">
            <span class="history-total-bytes">Total: ${formatBytes(record.total_bytes)}</span>
            ${record.error_message ? `<span class="history-error-text">${escapeHtml(record.error_message)}</span>` : ""}
          </div>
        </div>
      `;
    })
    .join("");

  listEl.innerHTML = html;
}

export async function loadAndRenderHistory() {
  try {
    cachedRecords = await getTransferHistory();
    applyFilterAndRender();
  } catch (err) {
    console.error("Failed to load transfer history:", err);
  }
}

export async function openHistoryModal() {
  const modal = document.getElementById("history-modal");
  if (modal) {
    modal.classList.remove("hidden");
    await loadAndRenderHistory();
  }
}

export function closeHistoryModal() {
  const modal = document.getElementById("history-modal");
  if (modal) {
    modal.classList.add("hidden");
  }
}

export function setupHistory() {
  const openBtn = document.getElementById("openHistoryBtn");
  const closeBtn = document.getElementById("closeHistoryBtn");
  const modal = document.getElementById("history-modal");
  const searchInput = document.getElementById("historySearchInput");
  const clearBtn = document.getElementById("clearHistoryBtn");
  const listEl = document.getElementById("historyList");

  if (openBtn) {
    openBtn.addEventListener("click", () => {
      openHistoryModal();
    });
  }

  if (closeBtn) {
    closeBtn.addEventListener("click", () => {
      closeHistoryModal();
    });
  }

  if (modal) {
    modal.addEventListener("click", (e) => {
      if (e.target === modal) {
        closeHistoryModal();
      }
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
    clearBtn.addEventListener("click", async () => {
      if (confirm("Are you sure you want to clear all transfer history?")) {
        try {
          await clearTransferHistory();
          cachedRecords = [];
          applyFilterAndRender();
        } catch (err) {
          console.error("Failed to clear history:", err);
        }
      }
    });
  }

  if (listEl) {
    listEl.addEventListener("click", async (e) => {
      const target = e.target.closest("button");
      if (!target) return;

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
}
