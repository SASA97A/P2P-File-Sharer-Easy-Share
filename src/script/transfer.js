// transfer.js (Revised)

import { getFiles } from "./files.js";
import { getSelectedPeer } from "./peers.js";
import { showToast } from "./ui.js";

/**
 * Sets up the file transfer functionality
 */
export function setupTransfer() {
  const sendBtn = document.querySelector(".send-btn");
  let isSending = false; // State guard to prevent multiple sends

  sendBtn.addEventListener("click", async () => {
    if (isSending) return;
    isSending = true;

    const peer = getSelectedPeer();
    const files = getFiles();

    if (!peer) {
      showToast("⚠️ Please select a device first!", "error");
      isSending = false;
      return;
    }
    if (files.length === 0) {
      showToast("⚠️ Please choose a file(s) to send!", "error");
      isSending = false;
      return;
    }

    try {
      sendBtn.disabled = true;
      sendBtn.textContent = "Sending...";

      for (let i = 0; i < files.length; i++) {
        const file = files[i];

        // Reset progress bar for this file
        const fileItem = document.getElementById(`file-${i}`); //if (bar) bar.style.width = "0%";

        // Attach listener for this file's progress updates
        window.api.onSendProgress?.((progress) => {
          const percent = Math.round((progress.sent / progress.total) * 100);

          if (fileItem) {
            fileItem.style.setProperty("--progress", percent + "%");

            // Optional: mark completed
            if (percent === 100) {
              fileItem.classList.add("completed");
            }
          }
        });

        // **KEY FIX:** Pass a stripped-down object containing the path, not the data
        const fileToSend = {
          name: file.name,
          size: file.size,
          // The fullPath must now be available on the file object
          fullPath: file.fullPath,
        };

        // Send the file and wait for completion
        await window.api.sendFile(peer, file);

        showToast(`✅ ${file.name} sent successfully!`, "success");
      }
    } catch (err) {
      showToast("❌ Error sending files: " + err, "error");
    } finally {
      // Restore button state
      isSending = false;
      sendBtn.disabled = false;
      sendBtn.textContent = "Send";
    }
  });

  // Listen for file received events
  window.api.onFileReceived((filePath) => {
    showToast(`📥 File received: ${filePath}`, "success");
  });
}
