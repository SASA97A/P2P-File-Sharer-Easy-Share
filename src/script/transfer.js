/**
 * Easy Share - Transfer Controller
 * Handles outbound file transfer initiation, incoming consent requests,
 * real-time progress tracking, and completion/error events.
 */

import { getFiles } from "./files.js";
import { getSelectedPeer } from "./peers.js";
import {
  showConsentModal,
  showToast,
  updateReceiveProgress,
  hideReceiveProgress,
  formatSpeed,
  formatEta,
} from "./ui.js";
import {
  startTransfer,
  cancelTransfer,
  pauseTransfer,
  resumeTransfer,
  respondTransferRequest,
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
 * Initializes transfer listeners and the Send button action handler
 */
export function setupTransfer() {
  const sendBtn = document.getElementById("sendBtn") || document.querySelector(".send-btn");
  const cancelTransferBtn = document.getElementById("cancelTransferBtn");
  const pauseTransferBtn = document.getElementById("pauseTransferBtn");
  const sendActionLabel = document.querySelector(".send-action span");
  let isSending = false;
  let isPaused = false;
  let activeBatch = null;

  /**
   * Updates send button text without destroying child DOM elements
   * @param {string} text
   */
  function setSendBtnLabel(text) {
    if (sendActionLabel) {
      sendActionLabel.textContent = text;
    }
  }

  /**
   * Updates pause button text safely
   * @param {string} text
   */
  function setPauseBtnLabel(text) {
    if (pauseTransferBtn) {
      const label = pauseTransferBtn.querySelector(".pause-btn-text") || pauseTransferBtn;
      label.textContent = text;
    }
  }

  /**
   * Finalizes the active outbound transfer batch and resets UI controls
   * @param {number|null} totalFiles
   */
  function finishActiveBatch(totalFiles = null) {
    if (!isSending && !activeBatch) return;
    const count = totalFiles || (activeBatch ? activeBatch.totalFiles : 1);
    if (count > 1) {
      showToast(`All ${count} files transferred successfully!`, "success");
    }
    isSending = false;
    isPaused = false;
    activeBatch = null;
    if (pauseTransferBtn) {
      pauseTransferBtn.classList.add("hidden");
      setPauseBtnLabel("Pause");
    }
    if (cancelTransferBtn) cancelTransferBtn.classList.add("hidden");
    if (sendBtn) sendBtn.disabled = false;
    setSendBtnLabel("Send");
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
        showToast("Please select a target device first.", "error");
        return;
      }
      if (!files || files.length === 0) {
        showToast("Please select at least one file to send.", "error");
        return;
      }

      isSending = true;
      isPaused = false;
      sendBtn.disabled = true;
      if (pauseTransferBtn) {
        pauseTransferBtn.classList.remove("hidden");
        setPauseBtnLabel("Pause");
      }
      if (cancelTransferBtn) cancelTransferBtn.classList.remove("hidden");
      setSendBtnLabel("Connecting…");

      try {
        const sessionToken = await startTransfer(peer, files);
        activeBatch = {
          sessionToken,
          totalFiles: files.length,
          completedFiles: 0,
        };
        setSendBtnLabel("Sending…");
        showToast(`Transfer started to ${peer.device_name || peer.ip}.`, "info");
        console.log(`Transfer active with session token: ${sessionToken}`);
      } catch (err) {
        showToast(`Transfer failed: ${err}`, "error");
        isSending = false;
        isPaused = false;
        activeBatch = null;
        if (pauseTransferBtn) {
          pauseTransferBtn.classList.add("hidden");
          setPauseBtnLabel("Pause");
        }
        if (cancelTransferBtn) cancelTransferBtn.classList.add("hidden");
        if (sendBtn) sendBtn.disabled = false;
        setSendBtnLabel("Send");
      }
    });
  }

  if (pauseTransferBtn) {
    pauseTransferBtn.addEventListener("click", async () => {
      if (!isSending || !activeBatch) return;
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
      const peer = getSelectedPeer();
      const token = activeBatch.sessionToken;
      try {
        await cancelTransfer(token, peer);
      } catch (err) {
        console.error("Error cancelling transfer:", err);
      }
      isSending = false;
      isPaused = false;
      activeBatch = null;
      if (pauseTransferBtn) {
        pauseTransferBtn.classList.add("hidden");
        setPauseBtnLabel("Pause");
      }
      if (cancelTransferBtn) cancelTransferBtn.classList.add("hidden");
      if (sendBtn) sendBtn.disabled = false;
      setSendBtnLabel("Send");
      showToast("Transfer cancelled.", "info");
    });
  }

  // -------------------------------------------------------------
  // Incoming Transfer Consent Event
  // -------------------------------------------------------------
  onTransferRequested(async (request) => {
    try {
      const accepted = await showConsentModal(request);
      await respondTransferRequest(request.request_id, accepted);

      if (accepted) {
        showToast(`Receiving files from ${request.sender_name}…`, "info");
      } else {
        showToast(`Declined transfer from ${request.sender_name}.`, "info");
      }
    } catch (err) {
      console.error("Error processing transfer consent:", err);
    }
  });

  // -------------------------------------------------------------
  // Real-time Outbound Transfer Progress Event
  // -------------------------------------------------------------
  onTransferProgress((progress) => {
    const { file_id, file_name, percent, speed_bps, eta_seconds } = progress;

    if (isSending) {
      if (isPaused) {
        setSendBtnLabel(`Paused (${percent}%)`);
      } else if (speed_bps) {
        setSendBtnLabel(`Sending (${percent}%) • ${formatSpeed(speed_bps)}`);
      } else {
        setSendBtnLabel(`Sending (${percent}%)`);
      }
    }

    // Locate matching file element in the UI staging area by file_id, fallback to name
    let targetItem = file_id ? document.querySelector(`.file-item[data-file-id="${file_id}"]`) : null;
    if (!targetItem) {
      const fileItems = document.querySelectorAll(".file-item");
      for (const item of fileItems) {
        const nameEl = item.querySelector(".file-name");
        if (nameEl && nameEl.textContent.trim() === (file_name || "").trim()) {
          targetItem = item;
          break;
        }
      }
    }

    if (targetItem) {
      targetItem.style.setProperty("--progress", `${percent}%`);
      let metricEl = targetItem.querySelector(".file-metrics");
      if (!metricEl) {
        metricEl = document.createElement("span");
        metricEl.className = "file-metrics";
        const infoEl = targetItem.querySelector(".file-info");
        const removeBtn = targetItem.querySelector(".remove-btn");
        if (infoEl) {
          if (removeBtn) {
            infoEl.insertBefore(metricEl, removeBtn);
          } else {
            infoEl.appendChild(metricEl);
          }
        }
      }
      if (metricEl) {
        if (percent >= 100) {
          metricEl.textContent = "Done";
        } else if (isPaused) {
          metricEl.textContent = "Paused";
        } else if (speed_bps) {
          metricEl.textContent = `${formatSpeed(speed_bps)} • ${formatEta(eta_seconds)}`;
        }
      }
      if (percent >= 100) {
        targetItem.classList.add("completed");
      }
    }
  });

  // -------------------------------------------------------------
  // Outbound Transfer Paused / Resumed Events
  // -------------------------------------------------------------
  onTransferPaused((_payload) => {
    isPaused = true;
    setPauseBtnLabel("Resume");
    setSendBtnLabel("Paused");
    showToast("Transfer paused.", "info");

    const fileItems = document.querySelectorAll(".file-item");
    for (const item of fileItems) {
      if (!item.classList.contains("completed")) {
        const metricEl = item.querySelector(".file-metrics");
        if (metricEl) {
          metricEl.textContent = "Paused";
        }
      }
    }
  });

  onTransferResumed((_payload) => {
    isPaused = false;
    setPauseBtnLabel("Pause");
    setSendBtnLabel("Sending…");
    showToast("Transfer resumed.", "info");
  });

  // -------------------------------------------------------------
  // Individual File Transfer Completed Event
  // -------------------------------------------------------------
  onTransferCompleted((payload) => {
    const { file_id, file_name, saved_path } = payload;
    const pathInfo = saved_path ? ` (Saved to: ${saved_path})` : "";
    showToast(`Transferred: ${file_name}${pathInfo}`, "success");

    // Mark matching UI file item as 100% complete
    let targetItem = file_id ? document.querySelector(`.file-item[data-file-id="${file_id}"]`) : null;
    if (!targetItem) {
      const fileItems = document.querySelectorAll(".file-item");
      for (const item of fileItems) {
        const nameEl = item.querySelector(".file-name");
        if (nameEl && nameEl.textContent.trim() === (file_name || "").trim()) {
          targetItem = item;
          break;
        }
      }
    }

    if (targetItem) {
      targetItem.style.setProperty("--progress", "100%");
      targetItem.classList.add("completed");
      const metricEl = targetItem.querySelector(".file-metrics");
      if (metricEl) {
        metricEl.textContent = "Done";
      }
    }

    if (activeBatch) {
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
    hideReceiveProgress();
    const { file_name, saved_path } = payload;
    const pathInfo = saved_path ? ` (Saved to: ${saved_path})` : "";
    showToast(`Received ${file_name}${pathInfo}`, "success");
  });

  // -------------------------------------------------------------
  // Incoming (Receiver) Session Cancelled Event
  // -------------------------------------------------------------
  onReceiveCancelled((_payload) => {
    hideReceiveProgress();
    showToast("Incoming transfer was cancelled by remote peer.", "info");
  });

  // -------------------------------------------------------------
  // Outbound Transfer Cancelled Event
  // -------------------------------------------------------------
  onTransferCancelled((_payload) => {
    showToast("Transfer was cancelled.", "info");
    isSending = false;
    isPaused = false;
    activeBatch = null;
    if (pauseTransferBtn) {
      pauseTransferBtn.classList.add("hidden");
      setPauseBtnLabel("Pause");
    }
    if (cancelTransferBtn) cancelTransferBtn.classList.add("hidden");
    if (sendBtn) sendBtn.disabled = false;
    setSendBtnLabel("Send");
  });

  // -------------------------------------------------------------
  // Outbound Transfer Error Event
  // -------------------------------------------------------------
  onTransferError((payload) => {
    const { error, file_name } = payload;
    const filePrefix = file_name ? `[${file_name}] ` : "";
    showToast(`Transfer failed: ${filePrefix}${error}`, "error");

    isSending = false;
    isPaused = false;
    activeBatch = null;
    if (pauseTransferBtn) {
      pauseTransferBtn.classList.add("hidden");
      setPauseBtnLabel("Pause");
    }
    if (cancelTransferBtn) cancelTransferBtn.classList.add("hidden");
    if (sendBtn) sendBtn.disabled = false;
    setSendBtnLabel("Send");
  });
}
