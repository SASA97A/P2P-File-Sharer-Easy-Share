/**
 * Easy Share - Transfer Controller
 * Handles outbound file transfer initiation, incoming consent requests,
 * real-time progress tracking, and completion/error events.
 */

import { getFiles } from "./files.js";
import { getSelectedPeer } from "./peers.js";
import { showConsentModal, showToast } from "./ui.js";
import {
  startTransfer,
  respondTransferRequest,
  onTransferRequested,
  onTransferProgress,
  onTransferCompleted,
  onTransferError,
} from "./tauri-bridge.js";

/**
 * Initializes transfer listeners and the Send button action handler
 */
export function setupTransfer() {
  const sendBtn = document.getElementById("sendBtn") || document.querySelector(".send-btn");
  const sendActionLabel = document.querySelector(".send-action span");
  let isSending = false;

  /**
   * Updates send button text without destroying child DOM elements
   * @param {string} text
   */
  function setSendBtnLabel(text) {
    if (sendActionLabel) {
      sendActionLabel.textContent = text;
    }
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
      setSendBtnLabel("Connecting...");

      try {
        const sessionToken = await startTransfer(peer, files);
        setSendBtnLabel("Sending...");
        showToast(`Transfer started to ${peer.device_name || peer.ip}.`, "info");
        console.log(`Transfer active with session token: ${sessionToken}`);
      } catch (err) {
        showToast(`Transfer failed: ${err}`, "error");
        isSending = false;
        sendBtn.disabled = false;
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
        showToast(`Receiving files from ${request.sender_name}...`, "info");
      } else {
        showToast(`Declined transfer from ${request.sender_name}.`, "info");
      }
    } catch (err) {
      console.error("Error processing transfer consent:", err);
    }
  });

  // -------------------------------------------------------------
  // Real-time Transfer Progress Event
  // -------------------------------------------------------------
  onTransferProgress((progress) => {
    const { file_name, percent } = progress;

    if (isSending) {
      setSendBtnLabel(`Sending (${percent}%)`);
    }

    // Locate matching file element in the UI staging area
    const fileItems = document.querySelectorAll(".file-item");
    fileItems.forEach((item) => {
      const nameEl = item.querySelector(".file-name");
      if (nameEl && nameEl.textContent.trim() === (file_name || "").trim()) {
        item.style.setProperty("--progress", `${percent}%`);
        if (percent >= 100) {
          item.classList.add("completed");
        }
      }
    });
  });

  // -------------------------------------------------------------
  // Transfer Completed Event
  // -------------------------------------------------------------
  onTransferCompleted((payload) => {
    const { file_name, saved_path } = payload;
    const pathInfo = saved_path ? ` (Saved to: ${saved_path})` : "";
    showToast(`Transfer completed: ${file_name}${pathInfo}`, "success");

    // Mark matching UI file item as 100% complete
    const fileItems = document.querySelectorAll(".file-item");
    fileItems.forEach((item) => {
      const nameEl = item.querySelector(".file-name");
      if (nameEl && nameEl.textContent.trim() === (file_name || "").trim()) {
        item.style.setProperty("--progress", "100%");
        item.classList.add("completed");
      }
    });

    isSending = false;
    if (sendBtn) sendBtn.disabled = false;
    setSendBtnLabel("Send");
  });

  // -------------------------------------------------------------
  // Transfer Error Event
  // -------------------------------------------------------------
  onTransferError((payload) => {
    const { error, file_name } = payload;
    const filePrefix = file_name ? `[${file_name}] ` : "";
    showToast(`Transfer failed: ${filePrefix}${error}`, "error");

    isSending = false;
    if (sendBtn) sendBtn.disabled = false;
    setSendBtnLabel("Send");
  });
}
