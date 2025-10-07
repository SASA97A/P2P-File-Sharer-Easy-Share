// Import modules for file handling, peer management, and UI
import { addFiles } from "./files.js";
import { addManualPeer } from "./peers.js";
import { setupTransfer } from "./transfer.js";
import { renderPeers } from "./ui.js";
import { setupSettings } from "./settings.js";

// Initialize the application when DOM is fully loaded
document.addEventListener("DOMContentLoaded", () => {
  const dropArea = document.getElementById("drop-area");
  const fileInput = document.getElementById("fileInput");

  // Initial render of empty peer list
  renderPeers([], () => {});

  // Set up file input and drag & drop functionality
  dropArea.addEventListener("click", () => fileInput.click());
  dropArea.addEventListener("dragover", (e) => {
    e.preventDefault();
    dropArea.style.background = "#f0f0f0"; // Visual feedback
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

  // Set up manual peer addition
  document.getElementById("manualAddBtn").addEventListener("click", () => {
    const host = document.getElementById("manualIp").value.trim();
    addManualPeer(host);
  });

  // Set up file transfer functionality
  setupTransfer();
  
  // Set up settings functionality
  setupSettings();
});
