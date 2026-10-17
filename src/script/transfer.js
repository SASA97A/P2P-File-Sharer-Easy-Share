/**
 * Easy Share - Transfer Controller
 * Handles outbound file transfer initiation, incoming consent requests,
 * real-time progress tracking, and completion/error events.
 */

import { getFiles, formatFileSize } from "./files.js";
import { getSelectedPeer, getPeerKey } from "./peers.js";
import {
  showConsentModal,
  showToast,
  updateReceiveProgress,
  hideReceiveProgress,
  setReceiveContext,
  getReceiveContext,
  setReceiveCancelHandler,
  setPeerActivity,
  setNavProgress,
  formatSpeed,
  formatEtaShort,
  formatBytePair,
  escapeHtml,
} from "./ui.js";
import { openHistoryView } from "./history.js";
import {
  startTransfer,
  cancelTransfer,
  pauseTransfer,
  resumeTransfer,
  respondTransferRequest,
  getDownloadDir,
  openDownloadDir,
  onTransferRequested,
  onTransferProgress,
  onTransferPaused,
  onTransferResumed,
  onTransferCompleted,
  onTransferBatchCompleted,
  onTransferCancelled,
  onReceiveProgress,
  onReceiveCompleted,
  onReceiveCancelled,
  onTransferError,
} from "./tauri-bridge.js";

/**
 * Finds the staged file row for a progress event by file_id, falling back to name
 * @param {string} fileId
 * @param {string} fileName
 * @returns {HTMLElement|null}
 */
function findFileRow(fileId, fileName) {
  let row = fileId ? document.querySelector(`.file-item[data-file-id="${CSS.escape(fileId)}"]`) : null;
  if (!row) {
    for (const item of document.querySelectorAll(".file-item")) {
      const nameEl = item.querySelector(".file-name");
      if (nameEl && nameEl.textContent.trim() === (fileName || "").trim()) {
        row = item;
        break;
      }
    }
  }
  return row;
}

/**
 * Initializes transfer listeners and the Send button action handler
 */
export function setupTransfer() {
  const sendBtn = document.getElementById("sendBtn");
  const cancelTransferBtn = document.getElementById("cancelTransferBtn");
  const pauseTransferBtn = document.getElementById("pauseTransferBtn");
  const sendView = document.getElementById("sendView");
  const card = document.getElementById("sendProgressCard");
  const titleEl = document.getElementById("sendProgressTitle");
  const subtitleEl = document.getElementById("sendProgressSubtitle");
  const percentEl = document.getElementById("sendProgressPercent");
  const speedEl = document.getElementById("sendProgressSpeed");
  const etaEl = document.getElementById("sendProgressEta");
  const bytesEl = document.getElementById("sendProgressBytes");
  const barEl = document.getElementById("sendProgressBar");
  const fillEl = document.getElementById("sendProgressFill");
  const pausedNote = document.getElementById("sendPausedNote");
  const stagedLabel = document.getElementById("stagedLabel");

  let isSending = false;
  let isPaused = false;
  let activeBatch = null;
  let lastPercent = 0;
  let lastSpeed = 0;

  function setPauseBtnLabel(text) {
    if (pauseTransferBtn) {
      const label = pauseTransferBtn.querySelector(".pause-btn-text") || pauseTransferBtn;
      label.textContent = text;
    }
  }

  function renderSubtitle() {
    if (!subtitleEl || !activeBatch) return;
    const { totalFiles, peer } = activeBatch;
    const ip = peer.ip || peer.host || "";
    subtitleEl.innerHTML = `${isPaused ? "Paused · " : ""}${totalFiles} ${totalFiles === 1 ? "file" : "files"} · ${formatFileSize(
      activeBatch.totalBytes
    )}${ip ? ` · to <span class="mono">${escapeHtml(ip)}</span>` : ""}`;
  }

  function renderOverall() {
    if (!activeBatch) return;
    const sent = Array.from(activeBatch.sentByFile.values()).reduce((a, b) => a + b, 0);
    const total = activeBatch.totalBytes;
    const percent = total > 0 ? Math.min(100, Math.floor((sent / total) * 100)) : lastPercent;
    lastPercent = percent;

    if (percentEl) percentEl.textContent = String(percent);
    if (fillEl) fillEl.style.width = `${percent}%`;
    if (barEl) barEl.setAttribute("aria-valuenow", String(percent));
    if (bytesEl) {
      const pair = formatBytePair(sent, total);
      bytesEl.innerHTML = `${pair.done} <span class="metric-total">/ ${pair.total}</span>`;
    }
    setNavProgress(percent);
    setPeerActivity(
      { key: activeBatch.peerKey },
      isPaused ? `Paused · ${percent}%` : `Receiving · ${lastSpeed ? formatSpeed(lastSpeed) : "starting…"}`
    );
  }

  function setPausedUi(paused) {
    isPaused = paused;
    if (card) card.classList.toggle("is-paused", paused);
    if (sendView) sendView.classList.toggle("is-paused", paused);
    if (pausedNote) pausedNote.classList.toggle("hidden", !paused);
    setPauseBtnLabel(paused ? "Resume" : "Pause");
    if (paused) {
      if (speedEl) speedEl.textContent = "—";
      if (etaEl) etaEl.textContent = "Paused";
      document.querySelectorAll(".file-item.is-active .file-state").forEach((el) => {
        el.textContent = "Paused";
      });
    }
    renderSubtitle();
    renderOverall();
  }

  function enterSendingUi(peer, files) {
    const peerName = peer.device_name || peer.ip || "device";
    if (sendView) sendView.classList.add("is-sending");
    if (card) card.classList.remove("hidden");
    if (titleEl) titleEl.textContent = `Sending to ${peerName}`;
    if (stagedLabel) stagedLabel.textContent = "Files";
    if (speedEl) speedEl.textContent = "—";
    if (etaEl) etaEl.textContent = "Connecting…";
    if (pauseTransferBtn) pauseTransferBtn.disabled = true;

    document.querySelectorAll(".file-item").forEach((row) => {
      row.classList.remove("completed", "is-active");
      row.style.setProperty("--progress", "0%");
      const state = row.querySelector(".file-state");
      if (state) state.textContent = "Queued";
    });

    lastPercent = 0;
    lastSpeed = 0;
    activeBatch = {
      sessionToken: null,
      peer,
      peerKey: getPeerKey(peer),
      totalFiles: files.length,
      completedFiles: 0,
      totalBytes: files.reduce((sum, f) => sum + (Number(f.size) || 0), 0),
      sentByFile: new Map(),
    };
    setPausedUi(false);
  }

  function exitSendingUi() {
    if (activeBatch) {
      setPeerActivity({ key: activeBatch.peerKey }, null);
    }
    isSending = false;
    isPaused = false;
    activeBatch = null;
    if (sendView) sendView.classList.remove("is-sending", "is-paused");
    if (card) {
      card.classList.add("hidden");
      card.classList.remove("is-paused");
    }
    if (pausedNote) pausedNote.classList.add("hidden");
    setPauseBtnLabel("Pause");
    setNavProgress(null);
    if (sendBtn) sendBtn.disabled = getFiles().length === 0;
    if (stagedLabel) {
      const n = getFiles().length;
      stagedLabel.textContent = `Staged · ${n} ${n === 1 ? "item" : "items"}`;
    }
  }

  /**
   * Finalizes the active outbound transfer batch and resets UI controls
   * @param {number|null} totalFiles
   */
  function finishActiveBatch(totalFiles = null) {
    if (!isSending && !activeBatch) return;
    const count = totalFiles || (activeBatch ? activeBatch.totalFiles : 1);
    const peerName = activeBatch ? activeBatch.peer.device_name || activeBatch.peer.ip : "device";
    const size = activeBatch ? formatFileSize(activeBatch.totalBytes) : "";
    exitSendingUi();
    showToast(`${count} ${count === 1 ? "file" : "files"} to ${peerName}${size ? ` · ${size}` : ""}`, "success", {
      title: "Transfer complete",
      actions: [{ label: "View in history", onClick: () => openHistoryView() }],
    });
  }

  // -------------------------------------------------------------
  // Outbound Transfer Initiation
  // -------------------------------------------------------------
  if (sendBtn) {
    sendBtn.addEventListener("click", async () => {
      if (isSending) return;

      const peer = getSelectedPeer();
      const files = getFiles();

      if (!peer) {
        showToast("Pick a device in Nearby devices first.", "error");
        return;
      }
      if (!files || files.length === 0) {
        showToast("Stage at least one file to send.", "error");
        return;
      }

      isSending = true;
      sendBtn.disabled = true;
      enterSendingUi(peer, files);

      try {
        const sessionToken = await startTransfer(peer, files);
        if (activeBatch) activeBatch.sessionToken = sessionToken;
        if (pauseTransferBtn) pauseTransferBtn.disabled = false;
        if (etaEl && etaEl.textContent === "Connecting…") etaEl.textContent = "—";
        console.log(`Transfer active with session token: ${sessionToken}`);
      } catch (err) {
        exitSendingUi();
        showToast(`Transfer failed: ${err}`, "error");
      }
    });
  }

  if (pauseTransferBtn) {
    pauseTransferBtn.addEventListener("click", async () => {
      if (!isSending || !activeBatch || !activeBatch.sessionToken) return;
      const token = activeBatch.sessionToken;
      try {
        if (!isPaused) {
          await pauseTransfer(token);
        } else {
          await resumeTransfer(token);
        }
      } catch (err) {
        console.error("Error toggling pause state:", err);
      }
    });
  }

  if (cancelTransferBtn) {
    cancelTransferBtn.addEventListener("click", async () => {
      if (!isSending || !activeBatch) return;
      const { peer, sessionToken } = activeBatch;
      try {
        if (sessionToken) await cancelTransfer(sessionToken, peer);
      } catch (err) {
        console.error("Error cancelling transfer:", err);
      }
      exitSendingUi();
      showToast("Transfer cancelled.", "info");
    });
  }

  // -------------------------------------------------------------
  // Incoming Transfer Consent Event
  // -------------------------------------------------------------
  let receiveCompleted = 0;

  onTransferRequested(async (request) => {
    try {
      const accepted = await showConsentModal(request);
      if (accepted) {
        let saveDir = "";
        try {
          saveDir = await getDownloadDir();
        } catch (_err) {
          // Shown without a path
        }
        receiveCompleted = 0;
        setReceiveContext({
          senderName: request.sender_name || "Remote device",
          senderOs: request.sender_os,
          fileCount: (request.files || []).length,
          totalBytes: Number(request.total_bytes) || 0,
          saveDir,
        });
      }
      await respondTransferRequest(request.request_id, accepted);

      if (!accepted) {
        showToast(`Declined transfer from ${request.sender_name}.`, "info");
      }
    } catch (err) {
      console.error("Error processing transfer consent:", err);
    }
  });

  setReceiveCancelHandler(async (sessionToken) => {
    try {
      await cancelTransfer(sessionToken, null);
    } catch (err) {
      console.error("Error cancelling incoming transfer:", err);
    }
    hideReceiveProgress();
    showToast("Incoming transfer cancelled.", "info");
  });

  // -------------------------------------------------------------
  // Real-time Outbound Transfer Progress Event
  // -------------------------------------------------------------
  onTransferProgress((progress) => {
    const { file_id, file_name, percent, sent_bytes, total_bytes, speed_bps, eta_seconds } = progress;
    if (!isSending || !activeBatch) return;

    const row = findFileRow(file_id, file_name);
    const key = (row && row.dataset.fileId) || file_id || file_name;
    const bytes = Number(sent_bytes);
    activeBatch.sentByFile.set(
      key,
      Number.isFinite(bytes) ? bytes : ((Number(total_bytes) || 0) * (Number(percent) || 0)) / 100
    );

    if (speed_bps) lastSpeed = speed_bps;
    if (!isPaused) {
      if (speedEl && speed_bps) speedEl.textContent = formatSpeed(speed_bps);
      if (etaEl && eta_seconds !== undefined && eta_seconds !== null) etaEl.textContent = formatEtaShort(eta_seconds);
    }

    if (row) {
      row.style.setProperty("--progress", `${percent}%`);
      const state = row.querySelector(".file-state");
      if (percent >= 100) {
        row.classList.remove("is-active");
        row.classList.add("completed");
        if (state) state.textContent = "Done";
      } else {
        document.querySelectorAll(".file-item.is-active").forEach((r) => {
          if (r !== row) r.classList.remove("is-active");
        });
        row.classList.add("is-active");
        if (state) state.textContent = isPaused ? "Paused" : `${percent}%`;
      }
    }

    renderOverall();
  });

  // -------------------------------------------------------------
  // Outbound Transfer Paused / Resumed Events
  // -------------------------------------------------------------
  onTransferPaused((_payload) => {
    if (!isSending) return;
    setPausedUi(true);
  });

  onTransferResumed((_payload) => {
    if (!isSending) return;
    setPausedUi(false);
    if (speedEl) speedEl.textContent = lastSpeed ? formatSpeed(lastSpeed) : "—";
    if (etaEl) etaEl.textContent = "—";
  });

  // -------------------------------------------------------------
  // Individual File Transfer Completed Event
  // -------------------------------------------------------------
  onTransferCompleted((payload) => {
    const { file_id, file_name } = payload;
    const row = findFileRow(file_id, file_name);

    if (row) {
      row.style.setProperty("--progress", "100%");
      row.classList.remove("is-active");
      row.classList.add("completed");
      const state = row.querySelector(".file-state");
      if (state) state.textContent = "Done";
      if (activeBatch) {
        const size = getFiles().find((f) => f.id === row.dataset.fileId)?.size;
        if (size) activeBatch.sentByFile.set(row.dataset.fileId, Number(size));
      }
    }

    if (activeBatch) {
      renderOverall();
      activeBatch.completedFiles++;
      if (activeBatch.completedFiles >= activeBatch.totalFiles) {
        finishActiveBatch(activeBatch.totalFiles);
      }
    }
  });

  // -------------------------------------------------------------
  // Full Batch Transfer Completed Event
  // -------------------------------------------------------------
  onTransferBatchCompleted((payload) => {
    finishActiveBatch(payload.total_files);
  });

  // -------------------------------------------------------------
  // Real-time Incoming (Receiver) Progress Event
  // -------------------------------------------------------------
  onReceiveProgress((progress) => {
    updateReceiveProgress(progress);
  });

  // -------------------------------------------------------------
  // Incoming (Receiver) File Completed Event
  // -------------------------------------------------------------
  onReceiveCompleted((payload) => {
    const ctx = getReceiveContext();
    const total = ctx ? ctx.fileCount : 0;
    receiveCompleted++;

    // Keep the card up until every file of the accepted batch has landed
    if (total && receiveCompleted < total) return;

    const sender = ctx ? ctx.senderName : "nearby device";
    const size = ctx && ctx.totalBytes ? ` · ${formatFileSize(ctx.totalBytes)}` : "";
    hideReceiveProgress();
    receiveCompleted = 0;
    showToast(
      total > 1 ? `${total} files from ${sender}${size}` : `${payload.file_name} from ${sender}${size}`,
      "success",
      {
        title: "Transfer complete",
        actions: [
          { label: "Open folder", onClick: () => openDownloadDir().catch(() => {}) },
          { label: "View in history", onClick: () => openHistoryView() },
        ],
      }
    );
  });

  // -------------------------------------------------------------
  // Incoming (Receiver) Session Cancelled Event
  // -------------------------------------------------------------
  onReceiveCancelled((_payload) => {
    hideReceiveProgress();
    receiveCompleted = 0;
    showToast("Incoming transfer was cancelled by the sender.", "info");
  });

  // -------------------------------------------------------------
  // Outbound Transfer Cancelled Event
  // -------------------------------------------------------------
  onTransferCancelled((_payload) => {
    if (!isSending) return;
    exitSendingUi();
    showToast("Transfer was cancelled.", "info");
  });

  // -------------------------------------------------------------
  // Outbound Transfer Error Event
  // -------------------------------------------------------------
  onTransferError((payload) => {
    const { error, file_name } = payload;
    const filePrefix = file_name ? `[${file_name}] ` : "";
    exitSendingUi();
    showToast(`Transfer failed: ${filePrefix}${error}`, "error");
  });
}
