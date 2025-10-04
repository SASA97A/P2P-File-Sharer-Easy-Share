import { getFiles } from "./files.js";
import { getSelectedPeer } from "./peers.js";

export function setupTransfer() {
  const sendBtn = document.querySelector(".send-btn");
  const progressBar = document.getElementById("progressBar");

  sendBtn.addEventListener("click", async () => {
    const peer = getSelectedPeer();
    const files = getFiles();

    if (!peer) {
      alert("Please select a device first!");
      return;
    }
    if (files.length === 0) {
      alert("Please choose a file(s) to send!");
      return;
    }

    try {
      progressBar.style.width = "0%";

      for (let i = 0; i < files.length; i++) {
        await window.api.sendFile(peer, files[i]);
      }

      window.api.onSendProgress?.((progress) => {
        const percent = Math.round((progress.sent / progress.total) * 100);
        progressBar.style.width = percent + "%";
      });

      alert("File transfer request has been sent!");
      progressBar.style.width = "100%";
    } catch (err) {
      alert("Error sending files: " + err);
    }
  });

  window.api.onFileReceived((filePath) => {
    alert("File received and saved at: " + filePath);
  });
}
