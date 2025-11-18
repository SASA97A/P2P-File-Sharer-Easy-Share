// Import modules for file handling, peer management, and UI
import { addFiles } from "./files.js";
import { addManualPeer } from "./peers.js";
import { setupTransfer } from "./transfer.js";
import { renderPeers } from "./ui.js";

// Initialize the application when DOM is fully loaded
document.addEventListener("DOMContentLoaded", () => {
  const dropArea = document.getElementById("drop-area");
  // const fileInput = document.getElementById("fileInput"); // REMOVED

  // Initial render of empty peer list
  renderPeers([], () => {});

  // Set up file input trigger
  dropArea.addEventListener("click", async () => {
    // Invoke main process to open file dialog and get file paths securely
    const files = await window.api.openFileSelectDialog();
    if (files && files.length > 0) {
      addFiles(files);
    }
  });

  // Keep drag/drop visual feedback for UX, but remove the data transfer logic for future use
  dropArea.addEventListener("dragover", (e) => {
    e.preventDefault();
    dropArea.style.background = "#f0f0f0"; // Visual feedback
  });
  dropArea.addEventListener("dragleave", () => {
    dropArea.style.background = "transparent";
  });
  dropArea.addEventListener("drop", (e) => {
    e.preventDefault();
    // No longer process files here; only the click is used for selection.
    dropArea.style.background = "transparent";
  });
  // fileInput.addEventListener("change", () => addFiles(fileInput.files)); // REMOVED

  // Set up manual peer addition
  document.getElementById("manualAddBtn").addEventListener("click", () => {
    const host = document.getElementById("manualIp").value.trim();
    addManualPeer(host);
  });

  // Set up file transfer functionality
  setupTransfer();
});
