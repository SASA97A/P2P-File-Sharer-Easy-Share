import { getFiles } from "./files.js";
import { getSelectedPeer } from "./peers.js";

export function setupTransfer() {
  const sendBtn = document.querySelector(".send-btn");

  sendBtn.addEventListener("click", async () => {
    const peer = getSelectedPeer();
    const files = getFiles();

    if (!peer) {
      showToast("⚠️ Please select a device first!", "error");
      return;
    }
    if (files.length === 0) {
      showToast("⚠️ Please choose a file(s) to send!", "error");
      return;
    }

    try {
      for (let i = 0; i < files.length; i++) {
        const file = files[i];

        // Reset progress
        const bar = document.getElementById(`progress-${i}`);
        if (bar) bar.style.width = "0%";

        // Attach listener for this file’s progress
        window.api.onSendProgress?.((progress) => {
          const percent = Math.round((progress.sent / progress.total) * 100);
          if (bar) bar.style.width = percent + "%";
        });

        await window.api.sendFile(peer, file);

        showToast(`✅ ${file.name} sent successfully!`, "success");
      }
    } catch (err) {
      showToast("❌ Error sending files: " + err, "error");
    }
  });

  window.api.onFileReceived((filePath) => {
    showToast(`📥 File received: ${filePath}`, "success");
  });
}
