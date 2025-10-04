import { addFiles } from "./files.js";
import { addManualPeer } from "./peers.js";
import { setupTransfer } from "./transfer.js";
import { renderPeers } from "./ui.js";

document.addEventListener("DOMContentLoaded", () => {
  const dropArea = document.getElementById("drop-area");
  const fileInput = document.getElementById("fileInput");

  // Initial render
  renderPeers([], () => {});

  // File input / drag & drop
  dropArea.addEventListener("click", () => fileInput.click());
  dropArea.addEventListener("dragover", (e) => {
    e.preventDefault();
    dropArea.style.background = "#f0f0f0";
  });
  dropArea.addEventListener("dragleave", () => {
    dropArea.style.background = "transparent";
  });
  dropArea.addEventListener("drop", (e) => {
    e.preventDefault();
    addFiles(e.dataTransfer.files);
    dropArea.style.background = "transparent";
  });
  fileInput.addEventListener("change", () => addFiles(fileInput.files));

  // Manual peer add
  document.getElementById("manualAddBtn").addEventListener("click", () => {
    const host = document.getElementById("manualIp").value.trim();
    addManualPeer(host);
  });

  // File transfer setup
  setupTransfer();
});
