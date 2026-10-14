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
} from "./ui.js";
import {
  startTransfer,
  respondTransferRequest,
  onTransferRequested,
  onTransferProgress,
  onTransferCompleted,
  onTransferBatchCompleted,
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
  const sendActionLabel = document.querySelector(".send-action span");
  let isSending = false;
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
    activeBatch = null;
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
      sendBtn.disabled = true;
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
        activeBatch = null;
        if (sendBtn) sendBtn.disabled = false;
        setSendBtnLabel("Send");
      }
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
    const { file_id, file_name, percent } = progress;

    if (isSending) {
      setSendBtnLabel(`Sending (${percent}%)`);
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
      if (percent >= 100) {
        targetItem.classList.add("completed");
      }
    }
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
  // Outbound Transfer Error Event
  // -------------------------------------------------------------
  onTransferError((payload) => {
    const { error, file_name } = payload;
    const filePrefix = file_name ? `[${file_name}] ` : "";
    showToast(`Transfer failed: ${filePrefix}${error}`, "error");

    isSending = false;
    activeBatch = null;
    if (sendBtn) sendBtn.disabled = false;
    setSendBtnLabel("Send");
  });
}
